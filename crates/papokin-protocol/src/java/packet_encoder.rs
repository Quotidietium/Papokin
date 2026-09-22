use aes::cipher::KeyIvInit;
use bytes::Bytes;
use flate2::{Compress, Compression, FlushCompress, Status};
use papokin_util::version::JavaMinecraftVersion;
use thiserror::Error;
use tokio::io::{AsyncWrite, AsyncWriteExt};

use crate::{
    Aes128Cfb8Enc, ClientPacket, CompressionLevel, CompressionThreshold, MAX_PACKET_DATA_SIZE,
    MAX_PACKET_SIZE, PacketEncodeError, StreamEncryptor, VarInt, WritingError,
    ser::NetworkWriteExt,
};

// 原始数据 -> 压缩 -> 加密

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
    // 复用压缩器，避免为每个数据包构建 zlib 状态。
    compressor: Option<(CompressionLevel, Compress)>,
    // 复用压缩缓冲区，避免为每个数据包分配新的 Vec。
    compression_scratch: Vec<u8>,
    frame_scratch: Vec<u8>,
}

impl<W: AsyncWrite + Unpin> TCPNetworkEncoder<W> {
    pub const fn new(writer: W) -> Self {
        Self {
            writer: Some(EncryptionWriter::None(writer)),
            compression: None,
            compressor: None,
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
        if reserve_hint > current_capacity {
            self.compression_scratch
                .reserve(reserve_hint.saturating_sub(current_capacity));
        }

        let needs_new_compressor = match self.compressor.as_ref() {
            Some((level, _)) => *level != compression_level,
            None => true,
        };
        if needs_new_compressor {
            self.compressor = Some((
                compression_level,
                Compress::new(Compression::new(compression_level), true),
            ));
        }

        let (_, compressor) = self.compressor.as_mut().ok_or_else(|| {
            PacketEncodeError::Message("compressor must be present after initialization".into())
        })?;
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
        let mut frame = std::mem::take(&mut self.frame_scratch);
        frame.clear();
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
        // 创建小负载的 CStatusResponse 数据包
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
}
