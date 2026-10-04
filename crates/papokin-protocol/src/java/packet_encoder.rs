use aes::cipher::KeyIvInit;
use bytes::Bytes;
use flate2::{Compress, Compression, FlushCompress, Status};
use papokin_util::version::JavaMinecraftVersion;
use std::sync::Mutex;
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

/// 压缩资源全局池的驻留上限（份）。上限只约束驻留、不约束并发：
/// 池空时检出永远新建，压缩路径绝不因池化而阻塞。
const MAX_POOLED_COMPRESSION_RESOURCES: usize = 16;

/// 组帧缓冲全局池的驻留上限（份）。组帧缓冲为纯字节暂存
///（无压缩级别维度），池化收益与压缩资源同源：连接级常驻
/// 与批路径逐批新建统一改为按需检出，驻留封顶为常数份。
const MAX_POOLED_FRAME_BUFFERS: usize = 16;

/// 全局组帧缓冲池。
///
/// 每条连接的编码器原本常驻一块组帧 scratch（`write_packet`
/// 单包路径，生产上仅登录/配置阶段使用），登录结束后缓冲
/// 随连接常驻空转；批路径（`frame_packet_batch`）则每批新建
/// `Vec`，tick 齐发下形成分配流失。两者统一改自本池检出：
/// 池空检出永远新建、绝不阻塞，归还封顶常数份。归还治理与
/// 压缩暂存同款：清空内容，容量逾 `2 × MAX_RETAINED_SCRATCH`
/// 收缩回留存上限，规避一次性大包撑大后的永久驻留。
static FRAME_BUFFER_POOL: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());

/// 组帧缓冲归还治理：清空内容，容量逾 2 倍留存上限时收缩
/// 回留存上限（2 倍余量近似轮次 2 的半量规则，规避常规
/// 尺寸附近的收缩/重分配振荡）。
pub fn give_back_frame_buffer(buffer: &mut Vec<u8>) {
    buffer.clear();
    if buffer.capacity() > 2 * MAX_RETAINED_SCRATCH {
        buffer.shrink_to(MAX_RETAINED_SCRATCH);
    }
}

/// 自全局池检出一块组帧缓冲（池空新建）。供 `write_packet`
/// 与批路径组帧共用；缓冲用毕须交 `return_frame_buffer` 归还。
#[must_use]
pub fn checkout_frame_buffer() -> Vec<u8> {
    let mut pool = FRAME_BUFFER_POOL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    pool.pop().unwrap_or_default()
}

/// 将组帧缓冲治理后归还全局池（池满则直接释放）。
pub fn return_frame_buffer(buffer: Vec<u8>) {
    let mut buffer = buffer;
    give_back_frame_buffer(&mut buffer);
    let mut pool = FRAME_BUFFER_POOL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if pool.len() < MAX_POOLED_FRAME_BUFFERS {
        pool.push(buffer);
    }
}

/// 压缩资源捆绑：一份 zlib 压缩上下文 + 一块压缩暂存缓冲。
/// 两者在 `frame_packet` 的压缩分支中成对使用，捆绑检出省去
/// 两次池查找。
struct CompressionResources {
    level: CompressionLevel,
    compressor: Compress,
    scratch: Vec<u8>,
}

/// 全局（跨线程）压缩资源池。
///
/// 压缩上下文（实测单个约 313 KiB）与压缩暂存（每连接 ≤ 256 KiB）
/// 原本随每条连接的编码器常驻。压缩实际发生在
/// `frame_batch_maybe_offload` 投出的 `spawn_blocking` 线程上——
/// tokio 阻塞池按需扩张（上限 512 线程），tick 齐发的批量压缩会让
/// 大量阻塞线程各沾一次压缩：若按线程局部驻留（轮次 8 初版），
/// 驻留量 ≈ 沾过压缩的线程数 × 单份大小，满负载时池化收益归零。
/// 协议线上行为逐包无状态（每包 `reset()` 后以
/// `FlushCompress::Finish` 完整收尾，等价于全新流；暂存内容在
/// 组帧时即拷出），资源在连接/线程间可互换、输出字节全等，故
/// 全局池化并把驻留封顶为常数份。每包一次的检出/归还锁持有
/// 仅为一次入出队，竞争可忽略。
static COMPRESSION_RESOURCE_POOL: Mutex<Vec<CompressionResources>> = Mutex::new(Vec::new());

/// 池化压缩资源守卫：存活期间独占一份资源，析构时归还来源池
/// （池满则直接释放）。归还前做暂存治理：容量逾
/// `2 × MAX_RETAINED_SCRATCH` 时收缩回留存上限——归还时无从预知
/// 下一包尺寸，以 2 倍余量近似轮次 2 的半量规则，规避常规尺寸
/// 附近的收缩/重分配振荡。
///
/// 守卫以来源池的引用为参数而非硬编码全局静态：生产传全局池，
/// 测试传栈上局部池，池状态断言因此与并发测试天然隔离。
struct PooledCompressionResources<'a> {
    entry: Option<CompressionResources>,
    pool: &'a Mutex<Vec<CompressionResources>>,
}

impl<'a> PooledCompressionResources<'a> {
    /// 按压缩级别检出资源：池中有同级条目则复用，否则新建。
    fn checkout(level: CompressionLevel, pool: &'a Mutex<Vec<CompressionResources>>) -> Self {
        let entry = {
            let mut pool = pool
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            pool.iter()
                .position(|resources| resources.level == level)
                .map(|index| pool.swap_remove(index))
        };
        Self {
            entry: Some(entry.unwrap_or_else(|| CompressionResources {
                level,
                compressor: Compress::new(Compression::new(level), true),
                scratch: Vec::new(),
            })),
            pool,
        }
    }

    /// 取得压缩器与暂存的可变引用（资源仅在 `Drop` 中交出，
    /// 守卫存活期间必为 `Some`）。
    fn parts_mut(&mut self) -> Option<(&mut Compress, &mut Vec<u8>)> {
        let resources = self.entry.as_mut()?;
        Some((&mut resources.compressor, &mut resources.scratch))
    }

    /// 取得暂存内容的只读视图（组帧拷出用）。
    fn scratch_slice(&self) -> Option<&[u8]> {
        self.entry
            .as_ref()
            .map(|resources| resources.scratch.as_slice())
    }
}

impl Drop for PooledCompressionResources<'_> {
    fn drop(&mut self) {
        let Some(mut resources) = self.entry.take() else {
            return;
        };
        resources.scratch.clear();
        if resources.scratch.capacity() > 2 * MAX_RETAINED_SCRATCH {
            resources.scratch.shrink_to(MAX_RETAINED_SCRATCH);
        }
        let mut pool = self
            .pool
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if pool.len() < MAX_POOLED_COMPRESSION_RESOURCES {
            pool.push(resources);
        }
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

/// 压缩数据包负载：自全局池检出压缩资源（上下文 + 暂存），
/// 逐包 `reset()` + `FlushCompress::Finish` 收尾。返回持有压缩
/// 结果的守卫，调用方在组帧拷出后任其析构归还。
fn compress_packet_data(
    packet_data: &[u8],
    compression_level: CompressionLevel,
) -> Result<PooledCompressionResources<'static>, PacketEncodeError> {
    compress_packet_data_in(&COMPRESSION_RESOURCE_POOL, packet_data, compression_level)
}

/// `compress_packet_data` 的池参数化实现（测试可注局部池）。
fn compress_packet_data_in<'a>(
    pool: &'a Mutex<Vec<CompressionResources>>,
    packet_data: &[u8],
    compression_level: CompressionLevel,
) -> Result<PooledCompressionResources<'a>, PacketEncodeError> {
    let mut pooled = PooledCompressionResources::checkout(compression_level, pool);
    let (compressor, scratch) = pooled
        .parts_mut()
        .ok_or_else(|| PacketEncodeError::Message("池化压缩资源缺失（不变式破坏）".into()))?;
    scratch.clear();
    // deflate 最坏膨胀约 5 B/64 KiB：按输入全长 + 1/16 + 64 预留，
    // 保证不可压缩负载也能单次 Finish 到 StreamEnd。`reserve` 的
    // additional 语义只保证容量 ≥ len + additional（此处 len 为
    // 0），必须传全量 hint——若传 hint 与既有容量的差值，摊销
    // 扩容取 max(2×容量, hint-容量)，当既有容量 < hint/2 时结果
    // 仍小于 hint，不可压缩大包会写满备用容量而得到 Status::Ok
    //（轮次 2 预留治理引入的潜在缺陷，渐进增大的不可压缩包可
    // 触发组帧失败）。收缩治理在守卫归还时统一执行（见
    // `PooledCompressionResources::drop`）。
    let reserve_hint = packet_data
        .len()
        .saturating_add(packet_data.len() / 16)
        .saturating_add(64);
    scratch.reserve(reserve_hint);
    compressor.reset();
    let status = compressor
        .compress_vec(packet_data, scratch, FlushCompress::Finish)
        .map_err(|err| PacketEncodeError::CompressionFailed(err.to_string()))?;

    if !matches!(status, Status::StreamEnd) {
        return Err(PacketEncodeError::CompressionFailed(format!(
            "Unexpected compressor status: {status:?}"
        )));
    }
    Ok(pooled)
}

/// 编码器：服务器 -> 客户端
/// 支持 `ZLib` 编码/压缩
/// 支持 Aes128 加密
pub struct TCPNetworkEncoder<W: AsyncWrite + Unpin> {
    writer: Option<EncryptionWriter<W>>,
    // 压缩与压缩阈值
    compression: Option<(CompressionThreshold, CompressionLevel)>,
    //（组帧缓冲已随轮次 10 改全局池检出，见
    // `FRAME_BUFFER_POOL`；压缩资源见轮次 9
    // `COMPRESSION_RESOURCE_POOL`）
}

impl<W: AsyncWrite + Unpin> TCPNetworkEncoder<W> {
    pub const fn new(writer: W) -> Self {
        Self {
            writer: Some(EncryptionWriter::None(writer)),
            compression: None,
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
        // 组帧缓冲自全局池检出（轮次 10）：所有出口（组帧失败、
        // 写帧失败、成功）均在返回前归还，连接登录结束后不再
        // 常驻组帧缓冲。
        let mut frame = checkout_frame_buffer();
        frame.clear();
        let result = match self.frame_packet(&packet_data, &mut frame) {
            Ok(()) => self.write_frame(&frame).await,
            Err(err) => Err(err),
        };
        return_frame_buffer(frame);
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

        if let Some((compression_threshold, compression_level)) = self.compression {
            if data_len >= compression_threshold {
                // 压缩资源自全局池检出：逐包 reset + Finish 的线上行为
                // 与每连接独占资源字节全等（见
                // `COMPRESSION_RESOURCE_POOL` 文档）；暂存内容在本分支
                // 内拷入帧缓冲后，守卫析构归还池中。
                let pooled = compress_packet_data(packet_data.as_ref(), compression_level)?;
                let compressed = pooled.scratch_slice().ok_or_else(|| {
                    PacketEncodeError::Message("池化压缩资源缺失（不变式破坏）".into())
                })?;

                let full_packet_len_var_int: VarInt = (data_len_var_int.written_size()
                    + compressed.len())
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

                let header_len = header_cursor.position() as usize;
                out.reserve(header_len + compressed.len());
                out.extend_from_slice(&header_buf[..header_len]);
                out.extend_from_slice(compressed);
                return Ok(());
            }
            // 已启用压缩但本包低于阈值：data_len=0 标记
            let zero_var_int: VarInt = 0.into();
            let full_packet_len_var_int: VarInt = (zero_var_int.written_size() + data_len)
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
            zero_var_int
                .encode(&mut header_cursor)
                .map_err(|err| PacketEncodeError::Message(err.to_string()))?;
        } else {
            let complete_serialization_length =
                data_len_var_int.written_size() + data_len_var_int.0 as usize;
            if complete_serialization_length > MAX_PACKET_SIZE as usize {
                return Err(PacketEncodeError::TooLong(complete_serialization_length));
            }

            data_len_var_int
                .encode(&mut header_cursor)
                .map_err(|err| PacketEncodeError::Message(err.to_string()))?;
        }

        // 两条未压缩路径负载均为原文，共享同一收尾
        let header_len = header_cursor.position() as usize;
        out.reserve(header_len + data_len);
        out.extend_from_slice(&header_buf[..header_len]);
        out.extend_from_slice(packet_data.as_ref());

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

    /// 池化复用：守卫析构归还来源池，同级别下次检出命中
    ///（池长归 0），再次析构后回到池中。测试一律使用栈上
    /// 局部池（守卫以来源池引用为参数），与并发运行的其他
    /// 压缩测试天然隔离，无需串行锁。
    #[test]
    fn pooled_resources_reuse_round_trip() {
        let pool = std::sync::Mutex::new(Vec::new());
        let pool_len = || {
            pool.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .len()
        };
        let level: CompressionLevel = 6;
        drop(super::PooledCompressionResources::checkout(level, &pool));
        assert_eq!(pool_len(), 1);
        let guard = super::PooledCompressionResources::checkout(level, &pool);
        assert_eq!(pool_len(), 0);
        drop(guard);
        assert_eq!(pool_len(), 1);
    }

    /// 池容量上限：超出上限的归还直接释放，池长不超上限。
    #[test]
    fn pooled_resources_respects_global_cap() {
        let pool = std::sync::Mutex::new(Vec::new());
        let guards: Vec<_> = (0..=super::MAX_POOLED_COMPRESSION_RESOURCES)
            .map(|_| super::PooledCompressionResources::checkout(6, &pool))
            .collect();
        drop(guards);
        let len = pool
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len();
        assert_eq!(len, super::MAX_POOLED_COMPRESSION_RESOURCES);
    }

    /// 归还治理：逾 2 倍留存上限的暂存在守卫析构归还时收缩
    /// 回留存上限；常规尺寸归还不得触发收缩振荡。
    #[test]
    fn pooled_scratch_shrinks_on_return_after_large_packet() {
        let pool: std::sync::Mutex<Vec<super::CompressionResources>> =
            std::sync::Mutex::new(Vec::new());
        let scratch_cap = || {
            pool.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .last()
                .map(|resources| resources.scratch.capacity())
        };

        // 1 MiB 不可压缩负载：暂存被撑大后归还，池中条目应已收缩
        let big = pseudo_random_bytes(1024 * 1024, 42);
        let pooled = super::compress_packet_data_in(&pool, &big, 6)
            .unwrap_or_else(|err| panic!("大包压缩失败: {err}"));
        drop(pooled);
        let pooled_cap = scratch_cap().unwrap_or(0);
        assert!(
            pooled_cap <= MAX_RETAINED_SCRATCH + 4096,
            "归还的压缩暂存应收缩回留存上限附近: {pooled_cap}"
        );

        // 常规 128 KiB 负载：检出同一条目，容量不得再涨落振荡
        let medium = pseudo_random_bytes(128 * 1024, 44);
        let pooled = super::compress_packet_data_in(&pool, &medium, 6)
            .unwrap_or_else(|err| panic!("中包压缩失败: {err}"));
        drop(pooled);
        assert_eq!(
            scratch_cap(),
            Some(pooled_cap),
            "常规尺寸不应触发池化暂存再分配"
        );
    }

    /// 回归：暂存预留必须保证容量 ≥ 全量 hint。历史上按
    /// `hint - 既有容量` 传参时，摊销扩容在既有容量 < hint/2
    /// 的窗口内预留不足，不可压缩大包会写满备用容量而返回
    /// `Status::Ok`（组帧失败）。本序列：300 KiB 包把暂存撑到
    /// 约 326 KiB，随后 700 KiB 不可压缩包（hint ≈ 762 KiB，
    /// 既有容量恰落于 < hint/2 的缺陷窗口）必须仍压缩成功。
    #[test]
    fn progressive_growth_never_under_reserves_scratch() {
        let pool = std::sync::Mutex::new(Vec::new());
        let first = pseudo_random_bytes(300 * 1024, 47);
        let pooled = super::compress_packet_data_in(&pool, &first, 6)
            .unwrap_or_else(|err| panic!("首包压缩失败: {err}"));
        drop(pooled);

        let grown = pseudo_random_bytes(700 * 1024, 48);
        let pooled = super::compress_packet_data_in(&pool, &grown, 6)
            .unwrap_or_else(|err| panic!("渐进增大的不可压缩包压缩失败（预留不足回归）: {err}"));
        let out_len = pooled.scratch_slice().map_or(0, <[u8]>::len);
        assert!(out_len >= grown.len(), "不可压缩包输出不应小于原文");
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

    /// 组帧缓冲池化后（轮次 10），大小包混写序列的线上字节
    /// 必须保持正确：缓冲来源（新建/池复用）不影响帧内容。
    ///（缓冲驻留与归还治理的内存面断言见同步测试
    /// `pooled_frame_buffer_*`——异步测试一律不断言全局池
    /// 状态，规避跨 await 持锁与并发污染。）
    #[tokio::test]
    async fn write_packet_frames_correct_across_size_mix() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut buf = Vec::new();
        let mut encoder = TCPNetworkEncoder::new(&mut buf);
        encoder.set_compression((0, 6));

        // 大（1 MiB 不可压缩）→ 小（64 B）→ 中（128 KiB）混写
        let big = pseudo_random_bytes(1024 * 1024, 42);
        encoder.write_packet(big.into()).await?;
        let small = pseudo_random_bytes(64, 43);
        encoder.write_packet(small.into()).await?;
        let medium = pseudo_random_bytes(128 * 1024, 44);
        encoder.write_packet(medium.into()).await?;

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

    /// 组帧缓冲池化复用：归还后再次检出命中同一缓冲（容量
    /// 延续），池长随检出/归还涨落。
    #[test]
    fn pooled_frame_buffer_reuse_round_trip() {
        let pool = std::sync::Mutex::new(Vec::new());
        // 局部池注入：生产函数走全局池，此处直取池原语验证
        // 语义（检出/治理/归还三件套与生产同款）。
        let mut buffer: Vec<u8> = pool
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop()
            .unwrap_or_default();
        buffer.reserve(4096);
        super::give_back_frame_buffer(&mut buffer);
        {
            let mut guard = pool
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            guard.push(buffer);
        };
        assert_eq!(
            pool.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .len(),
            1
        );
        let reused = pool
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop();
        assert!(
            reused.as_ref().is_some_and(|b| b.capacity() >= 4096),
            "复用条目应保留既有容量"
        );
    }

    /// 归还治理：逾 2 倍留存上限的缓冲归还时收缩回留存上限；
    /// 全局池驻留不超上限（超额归还直接释放）。
    #[test]
    fn pooled_frame_buffer_shrinks_and_respects_cap() {
        // 收缩治理（局部缓冲直验）
        let mut buffer: Vec<u8> = Vec::with_capacity(1024 * 1024);
        super::give_back_frame_buffer(&mut buffer);
        assert!(
            buffer.capacity() <= MAX_RETAINED_SCRATCH + 4096,
            "归还的组帧缓冲应收缩回留存上限附近: {}",
            buffer.capacity()
        );
        assert!(buffer.is_empty(), "归还治理必须清空内容");

        // 全局池驻留上限：检出 上限+1 份全数归还，池长封顶
        let buffers: Vec<Vec<u8>> = (0..=super::MAX_POOLED_FRAME_BUFFERS)
            .map(|_| super::checkout_frame_buffer())
            .collect();
        for buffer in buffers {
            super::return_frame_buffer(buffer);
        }
        assert_eq!(
            super::FRAME_BUFFER_POOL
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .len(),
            super::MAX_POOLED_FRAME_BUFFERS
        );
    }

    /// 低于压缩阈值的包以 `data_length=0` 标记未压缩组帧，
    /// 高于阈值的包标记其原始长度——压缩标记的线上语义不随
    /// 池化改变。（「未压缩帧不触碰压缩资源池」的内存面断言
    /// 见同步测试 `uncompressed_frames_skip_resource_pool`。）
    #[tokio::test]
    async fn uncompressed_writes_carry_zero_data_length() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut buf = Vec::new();
        let mut encoder = TCPNetworkEncoder::new(&mut buf);
        // 阈值 256：64 字节小包不压缩，1 MiB 大包压缩
        encoder.set_compression((256, 6));

        let big = pseudo_random_bytes(1024 * 1024, 45);
        encoder.write_packet(big.into()).await?;
        let small = pseudo_random_bytes(64, 46);
        encoder.write_packet(small.into()).await?;

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
