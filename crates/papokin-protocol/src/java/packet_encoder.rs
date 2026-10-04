use aes::cipher::KeyIvInit;
use bytes::Bytes;
use flate2::{Compress, Compression, FlushCompress, Status};
use papokin_util::version::JavaMinecraftVersion;
use std::cell::RefCell;
use thiserror::Error;
use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::{
    Aes128Cfb8Enc, ClientPacket, CompressionLevel, CompressionThreshold, MAX_PACKET_DATA_SIZE,
    MAX_PACKET_SIZE, PacketEncodeError, StreamEncryptor, VarInt, WritingError,
    ser::NetworkWriteExt,
};

// 原始数据 -> 压缩 -> 加密

/// 收发 scratch 缓冲的容量留存上限。
///
/// 压缩/组帧/负载 scratch 按历史最大数据包扩张；不设上限时，
/// 一次登录突发（配方/标签同步，数 MiB）或单个大包就会让每条
/// 连接的缓冲永久驻留数 MiB（连接数一乘便是数百 MiB 的稳态
/// 占用）。超过本上限且当前包明显更小时收缩回本上限——常规
/// 游玩数据包（含区块包，一般 ≤ 150 KiB）天然低于该值，因此
/// 稳态下收缩条件不成立，不产生收缩/重分配振荡。
pub(crate) const MAX_RETAINED_SCRATCH: usize = 256 * 1024;

/// 每线程池化的 zlib 压缩上下文数量上限（防御性约束；
/// 稳态下每线程在役压缩级别仅一种）。
const MAX_POOLED_COMPRESSORS_PER_THREAD: usize = 4;

thread_local! {
    /// 按线程复用的 zlib 压缩上下文池。
    ///
    /// 压缩上下文（字典窗口 + 哈希/链表 + 待决缓冲，实测单个逾
    /// 百 KiB）原本作为字段随每条连接的编码器常驻，而连接的压缩
    /// 流量是间歇的——大量空闲连接各自白占一份状态。协议线上行为
    /// 逐包无状态（每包 `reset()` 后以 `FlushCompress::Finish`
    /// 完整收尾，等价于一条全新压缩流），上下文在连接间可互换、
    /// 输出字节全等，故按线程检出复用。检出/归还均在
    /// `compress_packet_data` 单次同步调用内完成，不跨 await、
    /// 不跨线程。
    // 已是 const 初始化，clippy 1.98 对 RefCell<Vec<_>> 形式仍误报
    //（与 density_volume 同例）
    #[allow(clippy::missing_const_for_thread_local)]
    static COMPRESSOR_POOL: RefCell<Vec<(CompressionLevel, Compress)>> =
        const { RefCell::new(Vec::new()) };
}

/// 池化压缩上下文守卫：存活期间独占一份上下文，析构时归还
/// 线程池（池满则直接释放）。
struct PooledCompressor(Option<(CompressionLevel, Compress)>);

impl PooledCompressor {
    /// 按压缩级别检出上下文：池中有同级条目则复用，否则新建。
    fn checkout(level: CompressionLevel) -> Self {
        let entry = COMPRESSOR_POOL.with(|pool| {
            let mut pool = pool.borrow_mut();
            pool.iter()
                .position(|(pooled_level, _)| *pooled_level == level)
                .map(|index| pool.swap_remove(index))
        });
        Self(Some(entry.unwrap_or_else(|| {
            (level, Compress::new(Compression::new(level), true))
        })))
    }

    /// 取得内部压缩器的可变引用（上下文仅在 `Drop` 中交出，
    /// 守卫存活期间必为 `Some`）。
    fn compressor_mut(&mut self) -> Option<&mut Compress> {
        self.0.as_mut().map(|(_, compressor)| compressor)
    }
}

impl Drop for PooledCompressor {
    fn drop(&mut self) {
        let Some(entry) = self.0.take() else {
            return;
        };
        COMPRESSOR_POOL.with(|pool| {
            let mut pool = pool.borrow_mut();
            if pool.len() < MAX_POOLED_COMPRESSORS_PER_THREAD {
                pool.push(entry);
            }
        });
    }
}

pub enum EncryptionWriter<W: AsyncWrite + Unpin> {
    Encrypt(Box<StreamEncryptor<W>>),
    None(W),
}

impl<W: AsyncWrite + Unpin> EncryptionWriter<W> {
    #[must_use]
    pub fn upgrade(self, cipher: Aes128Cfb8Enc) -> Self {
        match self {
            Self::None(stream) => Self::Encrypt(Box::new(StreamEncryptor::new(cipher, stream))),
            Self::Encrypt(_) => self,
        }
    }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for EncryptionWriter<W> {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        match self.get_mut() {
            Self::Encrypt(writer) => {
                let writer = std::pin::Pin::new(writer);
                writer.poll_write(cx, buf)
            }
            Self::None(writer) => {
                let writer = std::pin::Pin::new(writer);
                writer.poll_write(cx, buf)
            }
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            Self::Encrypt(writer) => {
                let writer = std::pin::Pin::new(writer);
                writer.poll_flush(cx)
            }
            Self::None(writer) => {
                let writer = std::pin::Pin::new(writer);
                writer.poll_flush(cx)
            }
        }
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        match self.get_mut() {
            Self::Encrypt(writer) => {
                let writer = std::pin::Pin::new(writer);
                writer.poll_shutdown(cx)
            }
            Self::None(writer) => {
                let writer = std::pin::Pin::new(writer);
                writer.poll_shutdown(cx)
            }
        }
    }
}

/// 编码器：服务器 -> 客户端
/// 支持 `ZLib` 编码/压缩
/// 支持 Aes128 加密
pub struct TCPNetworkEncoder<W: AsyncWrite + Unpin> {
    writer: Option<EncryptionWriter<W>>,
    // 压缩与压缩阈值
    compression: Option<(CompressionThreshold, CompressionLevel)>,
    // 复用压缩缓冲区，避免为每个数据包分配新的 Vec。
    compression_scratch: Vec<u8>,
    frame_scratch: Vec<u8>,
}

impl<W: AsyncWrite + Unpin> TCPNetworkEncoder<W> {
    pub const fn new(writer: W) -> Self {
        Self {
            writer: Some(EncryptionWriter::None(writer)),
            compression: None,
            compression_scratch: Vec::new(),
            frame_scratch: Vec::new(),
        }
    }

    pub const fn set_compression(
        &mut self,
        compression_info: (CompressionThreshold, CompressionLevel),
    ) {
        self.compression = Some(compression_info);
    }

    /// NOTE: 加密只能设置；Minecraft 流无法退回未加密状态
    pub fn set_encryption(&mut self, key: &[u8; 16]) -> Result<(), PacketEncodeError> {
        if matches!(self.writer, Some(EncryptionWriter::Encrypt(_))) {
            return Err(PacketEncodeError::Message(
                "Encryption already enabled".into(),
            ));
        }
        let cipher = Aes128Cfb8Enc::new_from_slices(key, key)
            .map_err(|_| PacketEncodeError::Message("Invalid key".into()))?;

        if let Some(writer) = self.writer.take() {
            self.writer = Some(writer.upgrade(cipher));
        }
        Ok(())
    }

    fn compress_packet_data(
        &mut self,
        packet_data: &[u8],
        compression_level: CompressionLevel,
    ) -> Result<(), PacketEncodeError> {
        self.compression_scratch.clear();
        let reserve_hint = packet_data
            .len()
            .saturating_add(packet_data.len() / 16)
            .saturating_add(64);
        let current_capacity = self.compression_scratch.capacity();
        // 容量治理：大包把缓冲撑大后，若当前包明显更小则收缩回
        // 留存上限（此刻 len 为 0，收缩零拷贝）；半量判定保证
        // 区块流量等常规尺寸不会触发收缩/重分配振荡。
        if current_capacity > MAX_RETAINED_SCRATCH && reserve_hint < current_capacity / 2 {
            self.compression_scratch
                .shrink_to(MAX_RETAINED_SCRATCH.max(reserve_hint));
        }
        let current_capacity = self.compression_scratch.capacity();
        if reserve_hint > current_capacity {
            self.compression_scratch
                .reserve(reserve_hint.saturating_sub(current_capacity));
        }

        // 压缩上下文自线程池检出：逐包 reset + Finish 的线上行为
        // 与每连接独占上下文字节全等（见 `COMPRESSOR_POOL` 文档），
        // 守卫析构时归还池中。
        let mut pooled = PooledCompressor::checkout(compression_level);
        let compressor = pooled
            .compressor_mut()
            .ok_or_else(|| PacketEncodeError::Message("池化压缩上下文缺失（不变式破坏）".into()))?;
        compressor.reset();
        let status = compressor
            .compress_vec(
                packet_data,
                &mut self.compression_scratch,
                FlushCompress::Finish,
            )
            .map_err(|err| PacketEncodeError::CompressionFailed(err.to_string()))?;

        if !matches!(status, Status::StreamEnd) {
            return Err(PacketEncodeError::CompressionFailed(format!(
                "Unexpected compressor status: {status:?}"
            )));
        }
        Ok(())
    }

    /// 将客户端方向的 `ClientPacket` 追加到内部缓冲区，并在需要时应用压缩。
    ///
    /// 若已启用压缩且数据包大小超过阈值，则会对数据包进行压缩。
    /// 数据包以自身长度作为前缀，若已压缩则还包含未压缩数据长度。
    /// 数据包格式如下：
    ///
    /// **未压缩：**
    /// |-----------------------|
    /// | 数据包长度（`VarInt`）|
    /// |-----------------------|
    /// | 数据包 ID（`VarInt`）    |
    /// |-----------------------|
    /// | 数据（字节数组）     |
    /// |-----------------------|
    ///
    /// **压缩：**
    /// |------------------------|
    /// | 数据包长度（`VarInt`） |
    /// |------------------------|
    /// | 数据长度（`VarInt`）   |
    /// |------------------------|
    /// | 数据包 ID（`VarInt`）     |
    /// |------------------------|
    /// | 数据（字节数组）      |
    /// |------------------------|
    ///
    /// -   `Packet Length`: 数据包的总长度，*不包括* `Packet Length` 字段本身。
    /// -   `Data Length`: （仅存在于压缩数据包中）未压缩的 `Packet ID` 与 `Data` 的长度。
    /// -   `Packet ID`: 数据包的 ID。
    /// -   `Data`: 数据包的数据。
    ///
    /// NOTE: 此方法不会刷新。请调用 [`Self::flush`] 来刷新缓冲数据。
    pub async fn write_packet(&mut self, packet_data: Bytes) -> Result<(), PacketEncodeError> {
        // 未压缩包不会触碰压缩缓冲，若只在此刻收缩组帧缓冲，
        // 低于阈值的稳态小包会让压缩缓冲按历史最大压缩包永久
        // 驻留。缓冲内容在上一包组帧后即失效，先清空再同策收缩
        //（len 为 0，收缩零拷贝）。
        if !self.is_compressing_packet(&packet_data) {
            self.compression_scratch.clear();
            let reserve_hint = packet_data
                .len()
                .saturating_add(packet_data.len() / 16)
                .saturating_add(64);
            let capacity = self.compression_scratch.capacity();
            if capacity > MAX_RETAINED_SCRATCH && reserve_hint < capacity / 2 {
                self.compression_scratch
                    .shrink_to(MAX_RETAINED_SCRATCH.max(reserve_hint));
            }
        }
        let mut frame = std::mem::take(&mut self.frame_scratch);
        frame.clear();
        // 与压缩缓冲同策：大包之后收缩回留存上限，防每条连接
        // 按历史最大包永久驻留
        let frame_hint = packet_data.len().saturating_add(10);
        let frame_capacity = frame.capacity();
        if frame_capacity > MAX_RETAINED_SCRATCH && frame_hint < frame_capacity / 2 {
            frame.shrink_to(MAX_RETAINED_SCRATCH.max(frame_hint));
        }
        let framed = self.frame_packet(&packet_data, &mut frame);
        let result = match framed {
            Ok(()) => self.write_frame(&frame).await,
            Err(err) => Err(err),
        };
        self.frame_scratch = frame;
        result
    }

    pub async fn write_frame(&mut self, frame: &[u8]) -> Result<(), PacketEncodeError> {
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| PacketEncodeError::Message("Writer missing".into()))?;
        writer
            .write_all(frame)
            .await
            .map_err(|err| PacketEncodeError::Message(err.to_string()))
    }

    #[must_use]
    pub fn is_compressing_packet(&self, packet_data: &Bytes) -> bool {
        self.compression
            .is_some_and(|(threshold, _)| packet_data.len() >= threshold)
    }

    #[allow(clippy::too_many_lines)]
    pub fn frame_packet(
        &mut self,
        packet_data: &Bytes,
        out: &mut Vec<u8>,
    ) -> Result<(), PacketEncodeError> {
        let data_len = packet_data.len();
        if data_len > MAX_PACKET_DATA_SIZE {
            return Err(PacketEncodeError::TooLong(data_len));
        }

        let data_len_var_int: VarInt = data_len.try_into().map_err(|_| {
            PacketEncodeError::Message(format!(
                "Packet data length is too large to fit in VarInt! ({data_len})"
            ))
        })?;

        let mut header_buf = [0u8; 10];
        let mut header_cursor = std::io::Cursor::new(&mut header_buf[..]);

        let payload_to_write: &[u8] = if let Some((compression_threshold, compression_level)) =
            self.compression
        {
            if data_len >= compression_threshold {
                self.compress_packet_data(packet_data.as_ref(), compression_level)?;
                debug_assert!(!self.compression_scratch.is_empty());

                let full_packet_len_var_int: VarInt = (data_len_var_int.written_size()
                    + self.compression_scratch.len())
                .try_into()
                .map_err(|_| {
                    PacketEncodeError::Message(format!(
                        "Full packet length is too large to fit in VarInt! ({data_len})"
                    ))
                })?;

                let complete_serialization_length =
                    full_packet_len_var_int.written_size() + full_packet_len_var_int.0 as usize;
                if complete_serialization_length > MAX_PACKET_SIZE as usize {
                    return Err(PacketEncodeError::TooLong(complete_serialization_length));
                }

                full_packet_len_var_int
                    .encode(&mut header_cursor)
                    .map_err(|err| PacketEncodeError::Message(err.to_string()))?;
                data_len_var_int
                    .encode(&mut header_cursor)
                    .map_err(|err| PacketEncodeError::Message(err.to_string()))?;

                self.compression_scratch.as_slice()
            } else {
                let data_len_var_int: VarInt = 0.into();
                let full_packet_len_var_int: VarInt = (data_len_var_int.written_size() + data_len)
                    .try_into()
                    .map_err(|_| {
                        PacketEncodeError::Message(format!(
                            "Full packet length is too large to fit in VarInt! ({data_len})"
                        ))
                    })?;

                let complete_serialization_length =
                    full_packet_len_var_int.written_size() + full_packet_len_var_int.0 as usize;
                if complete_serialization_length > MAX_PACKET_SIZE as usize {
                    return Err(PacketEncodeError::TooLong(complete_serialization_length));
                }

                full_packet_len_var_int
                    .encode(&mut header_cursor)
                    .map_err(|err| PacketEncodeError::Message(err.to_string()))?;
                data_len_var_int
                    .encode(&mut header_cursor)
                    .map_err(|err| PacketEncodeError::Message(err.to_string()))?;

                packet_data.as_ref()
            }
        } else {
            let full_packet_len_var_int: VarInt = data_len_var_int;

            let complete_serialization_length =
                full_packet_len_var_int.written_size() + full_packet_len_var_int.0 as usize;
            if complete_serialization_length > MAX_PACKET_SIZE as usize {
                return Err(PacketEncodeError::TooLong(complete_serialization_length));
            }

            full_packet_len_var_int
                .encode(&mut header_cursor)
                .map_err(|err| PacketEncodeError::Message(err.to_string()))?;

            packet_data.as_ref()
        };

        let header_len = header_cursor.position() as usize;
        let header_bytes = &header_buf[..header_len];

        out.reserve(header_len + payload_to_write.len());
        out.extend_from_slice(header_bytes);
        out.extend_from_slice(payload_to_write);

        Ok(())
    }

    pub async fn flush(&mut self) -> Result<(), PacketEncodeError> {
        self.writer
            .as_mut()
            .ok_or_else(|| PacketEncodeError::Message("Writer missing".into()))?
            .flush()
            .await
            .map_err(|err| PacketEncodeError::Message(err.to_string()))
    }

    /// 当前压缩/组帧 scratch 的容量（诊断与基准观测用）。
    #[must_use]
    pub const fn scratch_capacity(&self) -> (usize, usize) {
        (
            self.compression_scratch.capacity(),
            self.frame_scratch.capacity(),
        )
    }
}

pub fn write_packet<P: ClientPacket + ?Sized>(
    packet: &P,
    version: &JavaMinecraftVersion,
    mut write: impl std::io::Write,
) -> Result<(), WritingError> {
    let version_number = P::to_id(*version);
    if version_number == -1 {
        return Err(WritingError::UnsupportedVersion(*version));
    }
    write.write_var_int(&VarInt(version_number))?;
    packet.write_packet_data(write, version)
}

pub fn serialize_packet<P: ClientPacket + ?Sized>(
    packet: &P,
    version: &JavaMinecraftVersion,
) -> Result<Bytes, WritingError> {
    let mut packet_buf = Vec::new();
    write_packet(packet, version, &mut packet_buf)?;
    Ok(packet_buf.into())
}

#[derive(Error, Debug)]
#[error("Invalid compression Level")]
pub struct CompressionLevelError;

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;
    use crate::java::client::status::CStatusResponse;
    use crate::packet::MultiVersionJavaPacket;
    use crate::ser::{NetworkReadExt, NetworkWriteExt};
    use crate::{ClientPacket, ReadingError};
    use aes::Aes128;
    use cfb8::Decryptor as Cfb8Decryptor;
    use flate2::read::ZlibDecoder;
    use papokin_data::packet::clientbound::status::STATUS_RESPONSE;
    use papokin_macros::java_packet;
    use papokin_util::version::JavaMinecraftVersion;

    /// 定义用于测试最大数据包大小的自定义数据包
    #[java_packet(STATUS_RESPONSE)]
    pub struct MaxSizePacket {
        data: Vec<u8>,
    }

    impl MaxSizePacket {
        pub fn new(size: usize) -> Self {
            Self {
                data: vec![0xAB; size], // 填充任意数据
            }
        }
    }

    impl ClientPacket for MaxSizePacket {
        fn write_packet_data(
            &self,
            mut write: impl std::io::Write,
            _version: &JavaMinecraftVersion,
        ) -> Result<(), crate::WritingError> {
            write
                .write_all(&self.data)
                .map_err(crate::WritingError::IoError)?;
            Ok(())
        }
    }

    /// 辅助函数：从字节中解码 `VarInt`
    fn decode_varint(buffer: &mut &[u8]) -> Result<i32, ReadingError> {
        Ok(buffer.get_var_int()?.0)
    }

    /// 辅助函数：使用 libdeflater 的 Zlib 解压器解压数据
    fn decompress_zlib(data: &[u8], expected_size: usize) -> Result<Vec<u8>, std::io::Error> {
        assert!(!data.is_empty());
        let mut decompressed = vec![0u8; expected_size];
        ZlibDecoder::new(data).read_exact(&mut decompressed)?;
        Ok(decompressed)
    }

    /// 辅助函数：使用 AES-128 CFB-8 模式解密数据
    fn decrypt_aes128(
        encrypted_data: &mut [u8],
        key: &[u8; 16],
        iv: &[u8; 16],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut decryptor =
            Cfb8Decryptor::<Aes128>::new_from_slices(key, iv).map_err(|_| "Invalid key/iv")?;
        decryptor.decrypt(encrypted_data);
        Ok(())
    }

    /// 辅助函数：构建数据包，可选压缩与加密
    async fn build_packet_with_encoder<T: ClientPacket>(
        packet: &T,
        compression_info: Option<(CompressionThreshold, CompressionLevel)>,
        key: Option<&[u8; 16]>,
    ) -> Result<Box<[u8]>, Box<dyn std::error::Error>> {
        let mut buf = Vec::new();
        let mut encoder = TCPNetworkEncoder::new(&mut buf);
        if let Some(compression_info) = compression_info {
            encoder.set_compression(compression_info);
        }

        if let Some(key) = key {
            encoder.set_encryption(key).map_err(|e| e.to_string())?;
        }

        let mut packet_buf = Vec::new();
        let writer = &mut packet_buf;
        writer.write_var_int(&VarInt(T::to_id(JavaMinecraftVersion::V_1_21_11)))?;
        packet.write_packet_data(writer, &JavaMinecraftVersion::V_1_21_11)?;

        encoder
            .write_packet(packet_buf.into())
            .await
            .map_err(|e| e.to_string())?;

        Ok(buf.into_boxed_slice())
    }

    /// 测试不带压缩与加密的编码
    #[tokio::test]
    async fn encode_without_compression_and_encryption() -> Result<(), Box<dyn std::error::Error>> {
        // 创建 CStatusResponse 数据包
        let packet =
            CStatusResponse::new(String::from("{\"description\": \"A Minecraft Server\"}"));

        // 构建未启用压缩与加密的数据包
        let packet_bytes = build_packet_with_encoder(&packet, None, None).await?;

        // 手动解码数据包以验证正确性
        let mut buffer = &packet_bytes[..];

        // 读取数据包长度 VarInt
        let packet_length = decode_varint(&mut buffer).map_err(|e| e.to_string())?;
        assert_eq!(
            packet_length as usize,
            buffer.len(),
            "Packet length mismatch"
        );

        // 读取数据包 ID VarInt
        let decoded_packet_id = decode_varint(&mut buffer).map_err(|e| e.to_string())?;
        assert_eq!(
            decoded_packet_id,
            CStatusResponse::to_id(JavaMinecraftVersion::V_1_21_11)
        );

        // 剩余缓冲区为有效载荷
        // 我们需要获得预期的负载
        let mut expected_payload = Vec::new();
        packet.write_packet_data(&mut expected_payload, &JavaMinecraftVersion::V_1_21_11)?;

        assert_eq!(buffer, expected_payload);
        Ok(())
    }

    /// 测试带压缩的编码
    #[tokio::test]
    async fn encode_with_compression() -> Result<(), Box<dyn std::error::Error>> {
        // 创建 CStatusResponse 数据包
        let packet = CStatusResponse::new("{\"description\": \"A Minecraft Server\"}".to_string());

        // 构建启用压缩的数据包
        let packet_bytes = build_packet_with_encoder(&packet, Some((0, 6)), None).await?;

        // 手动解码数据包以验证正确性
        let mut buffer = &packet_bytes[..];

        // 读取数据包长度 VarInt
        let packet_length = decode_varint(&mut buffer).map_err(|e| e.to_string())?;
        assert_eq!(
            packet_length as usize,
            buffer.len(),
            "Packet length mismatch"
        );

        // 读取数据长度 VarInt（未压缩数据长度）
        let data_length = decode_varint(&mut buffer).map_err(|e| e.to_string())?;
        let mut expected_payload = Vec::new();
        packet.write_packet_data(&mut expected_payload, &JavaMinecraftVersion::V_1_21_11)?;
        let uncompressed_data_length =
            VarInt(CStatusResponse::to_id(JavaMinecraftVersion::V_1_21_11)).written_size()
                + expected_payload.len();
        assert_eq!(data_length as usize, uncompressed_data_length);

        // 剩余缓冲区为压缩数据
        let compressed_data = buffer;

        // 解压数据
        let decompressed_data = decompress_zlib(compressed_data, data_length as usize)?;

        // 验证数据包 ID 与负载
        let mut decompressed_buffer = &decompressed_data[..];

        // 读取数据包 ID VarInt
        let decoded_packet_id =
            decode_varint(&mut decompressed_buffer).map_err(|e| e.to_string())?;
        assert_eq!(
            decoded_packet_id,
            CStatusResponse::to_id(JavaMinecraftVersion::V_1_21_11)
        );

        // 剩余缓冲区为有效载荷
        assert_eq!(decompressed_buffer, expected_payload);
        Ok(())
    }

    /// 池化复用：守卫析构归还线程池，同级别下次检出命中
    ///（池长归 0），再次析构后回到池中。
    #[test]
    fn pooled_compressor_reuse_round_trip() {
        let level: CompressionLevel = 6;
        drop(super::PooledCompressor::checkout(level));
        super::COMPRESSOR_POOL.with(|pool| assert_eq!(pool.borrow().len(), 1));
        let guard = super::PooledCompressor::checkout(level);
        super::COMPRESSOR_POOL.with(|pool| assert!(pool.borrow().is_empty()));
        drop(guard);
        super::COMPRESSOR_POOL.with(|pool| assert_eq!(pool.borrow().len(), 1));
    }

    /// 池容量上限：超出上限的归还直接释放，池长不超上限。
    #[test]
    fn pooled_compressor_respects_per_thread_cap() {
        let guards: Vec<_> = (0..6).map(super::PooledCompressor::checkout).collect();
        drop(guards);
        super::COMPRESSOR_POOL.with(|pool| {
            assert_eq!(
                pool.borrow().len(),
                super::MAX_POOLED_COMPRESSORS_PER_THREAD
            );
        });
    }

    /// 字节全等门：同一数据包经两个先后创建的编码器（后者自
    /// 池中检出前者归还的上下文）压缩输出必须逐字节一致。
    #[tokio::test]
    async fn compressed_output_identical_across_encoder_instances()
    -> Result<(), Box<dyn std::error::Error>> {
        let packet = MaxSizePacket::new(4096);
        let first = build_packet_with_encoder(&packet, Some((0, 6)), None).await?;
        let second = build_packet_with_encoder(&packet, Some((0, 6)), None).await?;
        assert_eq!(first, second, "池化复用不得改变线上字节");
        Ok(())
    }

    /// 测试带加密的编码
    #[tokio::test]
    async fn encode_with_encryption() -> Result<(), Box<dyn std::error::Error>> {
        // 创建 CStatusResponse 数据包
        let packet = CStatusResponse::new("{\"description\": \"A Minecraft Server\"}".to_string());

        // 加密密钥和 IV（在本例中 IV 与密钥相同）
        let key = [0x00u8; 16]; // 示例密钥

        // 构建启用加密（无压缩）的数据包
        let mut packet_bytes = build_packet_with_encoder(&packet, None, Some(&key)).await?;

        // 解密数据包
        decrypt_aes128(&mut packet_bytes, &key, &key)?;

        // 手动解码数据包以验证正确性
        let mut buffer = &packet_bytes[..];

        // 读取数据包长度 VarInt
        let packet_length = decode_varint(&mut buffer).map_err(|e| e.to_string())?;
        assert_eq!(
            packet_length as usize,
            buffer.len(),
            "Packet length mismatch"
        );

        // 读取数据包 ID VarInt
        let decoded_packet_id = decode_varint(&mut buffer).map_err(|e| e.to_string())?;
        assert_eq!(
            decoded_packet_id,
            CStatusResponse::to_id(JavaMinecraftVersion::V_1_21_11)
        );

        // 剩余缓冲区为有效载荷
        let mut expected_payload = Vec::new();
        packet.write_packet_data(&mut expected_payload, &JavaMinecraftVersion::V_1_21_11)?;
        assert_eq!(buffer, expected_payload);
        Ok(())
    }

    /// 测试同时带压缩与加密的编码
    #[tokio::test]
    async fn encode_with_compression_and_encryption() -> Result<(), Box<dyn std::error::Error>> {
        // 创建 CStatusResponse 数据包
        let packet = CStatusResponse::new("{\"description\": \"A Minecraft Server\"}".to_string());

        // 加密密钥和 IV（在本例中 IV 与密钥相同）
        let key = [0x01u8; 16]; // 示例密钥

        // 构建同时启用压缩与加密的数据包
        // 压缩阈值设为 0 以强制压缩
        let mut packet_bytes = build_packet_with_encoder(&packet, Some((0, 6)), Some(&key)).await?;

        // 解密数据包
        decrypt_aes128(&mut packet_bytes, &key, &key)?;

        // 手动解码数据包以验证正确性
        let mut buffer = &packet_bytes[..];

        // 读取数据包长度 VarInt
        let packet_length = decode_varint(&mut buffer).map_err(|e| e.to_string())?;
        assert_eq!(
            packet_length as usize,
            buffer.len(),
            "Packet length mismatch"
        );

        // 读取数据长度 VarInt（未压缩数据长度）
        let data_length = decode_varint(&mut buffer).map_err(|e| e.to_string())?;
        let mut expected_payload = Vec::new();
        packet.write_packet_data(&mut expected_payload, &JavaMinecraftVersion::V_1_21_11)?;
        let uncompressed_data_length =
            VarInt(CStatusResponse::to_id(JavaMinecraftVersion::V_1_21_11)).written_size()
                + expected_payload.len();
        assert_eq!(data_length as usize, uncompressed_data_length);

        // 剩余缓冲区为压缩数据
        let compressed_data = buffer;

        // 解压数据
        let decompressed_data = decompress_zlib(compressed_data, data_length as usize)?;

        // 验证数据包 ID 与负载
        let mut decompressed_buffer = &decompressed_data[..];

        // 读取数据包 ID VarInt
        let decoded_packet_id =
            decode_varint(&mut decompressed_buffer).map_err(|e| e.to_string())?;
        assert_eq!(
            decoded_packet_id,
            CStatusResponse::to_id(JavaMinecraftVersion::V_1_21_11)
        );

        // 剩余缓冲区为有效载荷
        assert_eq!(decompressed_buffer, expected_payload);
        Ok(())
    }

    /// 测试编码零长度负载
    #[tokio::test]
    async fn encode_with_zero_length_payload() -> Result<(), Box<dyn std::error::Error>> {
        // 创建空负载的 CStatusResponse 数据包
        let packet = CStatusResponse::new(String::new());

        // 构建未启用压缩与加密的数据包
        let packet_bytes = build_packet_with_encoder(&packet, None, None).await?;

        // 手动解码数据包以验证正确性
        let mut buffer = &packet_bytes[..];

        // 读取数据包长度 VarInt
        let packet_length = decode_varint(&mut buffer).map_err(|e| e.to_string())?;
        assert_eq!(
            packet_length as usize,
            buffer.len(),
            "Packet length mismatch"
        );

        // 读取数据包 ID VarInt
        let decoded_packet_id = decode_varint(&mut buffer).map_err(|e| e.to_string())?;
        assert_eq!(
            decoded_packet_id,
            CStatusResponse::to_id(JavaMinecraftVersion::V_1_21_11)
        );

        // 剩余缓冲区为有效载荷（空）
        let mut expected_payload = Vec::new();
        packet.write_packet_data(&mut expected_payload, &JavaMinecraftVersion::V_1_21_11)?;

        assert_eq!(
            buffer.len(),
            expected_payload.len(),
            "Payload length mismatch"
        );
        assert_eq!(buffer, expected_payload);
        Ok(())
    }

    /// 测试编码最大长度负载
    #[tokio::test]
    async fn encode_with_maximum_string_length() -> Result<(), Box<dyn std::error::Error>> {
        // 允许的最大字符串长度为 32767 字节
        let max_string_length = 32767;
        let payload_str = "A".repeat(max_string_length);
        let packet = CStatusResponse::new(payload_str);

        // 构建未启用压缩与加密的数据包
        let packet_bytes = build_packet_with_encoder(&packet, None, None).await?;

        // 验证数据包大小不超过 MAX_PACKET_SIZE as usize
        assert!(
            packet_bytes.len() <= MAX_PACKET_SIZE as usize,
            "Packet size exceeds maximum allowed size"
        );

        // 手动解码数据包以验证正确性
        let mut buffer = &packet_bytes[..];

        // 读取数据包长度 VarInt
        let packet_length = decode_varint(&mut buffer).map_err(|e| e.to_string())?;
        assert_eq!(
            packet_length as usize,
            buffer.len(),
            "Packet length mismatch"
        );

        // 读取数据包 ID VarInt
        let decoded_packet_id = decode_varint(&mut buffer).map_err(|e| e.to_string())?;
        // 假设 CStatusResponse 的数据包 ID 为 0
        assert_eq!(
            decoded_packet_id,
            CStatusResponse::to_id(JavaMinecraftVersion::V_1_21_11)
        );

        // 剩余缓冲区为有效载荷
        let mut expected_payload = Vec::new();
        packet.write_packet_data(&mut expected_payload, &JavaMinecraftVersion::V_1_21_11)?;

        assert_eq!(buffer, expected_payload);
        Ok(())
    }

    /// 测试编码超过 `MAX_PACKET_SIZE`（以 usize 计）的数据包
    #[tokio::test]
    async fn encode_packet_exceeding_maximum_size() -> Result<(), Box<dyn std::error::Error>> {
        // 创建数据超过 MAX_PACKET_SIZE（usize）的自定义数据包
        let data_size = MAX_PACKET_SIZE as usize + 1; // 超出 1 字节
        let packet = MaxSizePacket::new(data_size);

        // 构建未启用压缩与加密的数据包
        // 这里应返回 PacketEncodeError::TooLong
        let result = build_packet_with_encoder(&packet, None, None).await;
        assert!(result.is_err());
        // 由于 TooLong 被装箱，我们难以直接检查，但已验证它会返回错误
        Ok(())
    }

    /// 测试编码不应被压缩的小负载
    #[tokio::test]
    async fn encode_small_payload_no_compression() -> Result<(), Box<dyn std::error::Error>> {
        // 创建数据负载的 CStatusResponse 数据包
        let packet = CStatusResponse::new(String::from("Hi"));

        // 构建启用压缩的数据包
        // 压缩阈值设为高于负载长度的值
        let packet_bytes = build_packet_with_encoder(&packet, Some((10, 6)), None).await?;

        // 手动解码数据包以验证其未被压缩
        let mut buffer = &packet_bytes[..];

        // 读取数据包长度 VarInt
        let packet_length = decode_varint(&mut buffer).map_err(|e| e.to_string())?;
        assert_eq!(
            packet_length as usize,
            buffer.len(),
            "Packet length mismatch"
        );

        // 读取数据长度 VarInt（应为 0，表示无压缩）
        let data_length = decode_varint(&mut buffer).map_err(|e| e.to_string())?;
        assert_eq!(
            data_length, 0,
            "Data length should be 0 indicating no compression"
        );

        // 读取数据包 ID VarInt
        let decoded_packet_id = decode_varint(&mut buffer).map_err(|e| e.to_string())?;
        assert_eq!(
            decoded_packet_id,
            CStatusResponse::to_id(JavaMinecraftVersion::V_1_21_11)
        );

        // 剩余缓冲区为有效载荷
        let mut expected_payload = Vec::new();
        packet.write_packet_data(&mut expected_payload, &JavaMinecraftVersion::V_1_21_11)?;

        assert_eq!(buffer, expected_payload);
        Ok(())
    }

    /// xorshift 伪随机字节（不可压缩，确保 scratch 真实扩张）
    fn pseudo_random_bytes(len: usize, seed: u64) -> Vec<u8> {
        let mut state = seed | 1;
        (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state >> 32) as u8
            })
            .collect()
    }

    /// 大包之后 scratch 容量必须收缩回留存上限；
    /// 常规尺寸（≤ 留存上限）流量不得触发收缩。
    #[tokio::test]
    async fn scratch_capacity_shrinks_after_large_packet() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut buf = Vec::new();
        let mut encoder = TCPNetworkEncoder::new(&mut buf);
        encoder.set_compression((0, 6));

        // 1 MiB 不可压缩大包：两个 scratch 都被撑大
        let big = pseudo_random_bytes(1024 * 1024, 42);
        let big_len = big.len();
        encoder.write_packet(big.into()).await?;
        let (big_comp_cap, big_frame_cap) = encoder.scratch_capacity();
        assert!(
            big_comp_cap >= big_len || big_frame_cap >= big_len,
            "大包后容量应显著扩张: comp={big_comp_cap} frame={big_frame_cap}"
        );

        // 小包：容量收缩回留存上限（分配器可能按大小档向上取整，
        // 断言留一个页档余量）
        let small = pseudo_random_bytes(64, 43);
        encoder.write_packet(small.into()).await?;
        let (comp_cap, frame_cap) = encoder.scratch_capacity();
        assert!(
            comp_cap <= MAX_RETAINED_SCRATCH + 4096,
            "压缩 scratch 应收缩回留存上限附近: {comp_cap}"
        );
        assert!(
            frame_cap <= MAX_RETAINED_SCRATCH + 4096,
            "组帧 scratch 应收缩回留存上限附近: {frame_cap}"
        );

        // 中包（128 KiB < 留存上限）：容量不得再涨落振荡
        let medium = pseudo_random_bytes(128 * 1024, 44);
        encoder.write_packet(medium.into()).await?;
        let (comp_cap2, frame_cap2) = encoder.scratch_capacity();
        assert_eq!(comp_cap, comp_cap2, "中包不应触发压缩缓冲再分配");
        assert_eq!(frame_cap, frame_cap2, "中包不应触发组帧缓冲再分配");

        // 线上字节正确性：解析缓冲末尾一帧并校验其内部结构
        let mut last_frame = last_frame_bytes(&buf);
        let packet_length = decode_varint(&mut last_frame).map_err(|e| e.to_string())?;
        assert_eq!(packet_length as usize, last_frame.len());
        let data_length = decode_varint(&mut last_frame).map_err(|e| e.to_string())?;
        assert_eq!(
            data_length as usize,
            128 * 1024,
            "未压缩长度应为中包原始长度"
        );
        Ok(())
    }

    /// 低于压缩阈值的小包不触碰压缩缓冲；写路径须同步
    /// 收缩压缩 scratch，否则稳态小包流下它仍按历史最大
    /// 压缩包驻留。
    #[tokio::test]
    async fn compression_scratch_shrinks_via_uncompressed_writes()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut buf = Vec::new();
        let mut encoder = TCPNetworkEncoder::new(&mut buf);
        // 阈值 256：64 字节小包不压缩，1 MiB 大包压缩
        encoder.set_compression((256, 6));

        let big = pseudo_random_bytes(1024 * 1024, 45);
        encoder.write_packet(big.into()).await?;
        let (big_comp_cap, _) = encoder.scratch_capacity();
        assert!(
            big_comp_cap >= 1024 * 1024,
            "压缩大包应撑大压缩 scratch: {big_comp_cap}"
        );

        // 未压缩小包：压缩 scratch 同样收缩回留存上限
        let small = pseudo_random_bytes(64, 46);
        encoder.write_packet(small.into()).await?;
        let (comp_cap, frame_cap) = encoder.scratch_capacity();
        assert!(
            comp_cap <= MAX_RETAINED_SCRATCH + 4096,
            "未压缩小包写后压缩 scratch 应收缩回留存上限附近: {comp_cap}"
        );
        assert!(
            frame_cap <= MAX_RETAINED_SCRATCH + 4096,
            "组帧 scratch 应收缩回留存上限附近: {frame_cap}"
        );

        // 线上字节正确性：末帧 data_length 必须为 0（未压缩标记）
        let mut last_frame = last_frame_bytes(&buf);
        let packet_length = decode_varint(&mut last_frame).map_err(|e| e.to_string())?;
        assert_eq!(packet_length as usize, last_frame.len());
        let data_length = decode_varint(&mut last_frame).map_err(|e| e.to_string())?;
        assert_eq!(data_length, 0, "低于阈值的包应以未压缩形式组帧");
        Ok(())
    }

    /// 解析缓冲末尾一帧的字节切片（帧 = `VarInt` 长度前缀 + 负载）。
    fn last_frame_bytes(buf: &[u8]) -> &[u8] {
        // 逐帧跳过至最后一帧
        let mut rest = buf;
        let mut last = rest;
        while !rest.is_empty() {
            let mut cursor = rest;
            let Ok(len) = decode_varint(&mut cursor) else {
                break;
            };
            let len = len as usize;
            let header = rest.len() - cursor.len();
            if cursor.len() < len {
                break;
            }
            last = &rest[..header + len];
            rest = &cursor[len..];
        }
        last
    }
}
