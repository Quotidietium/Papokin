use aes::cipher::KeyIvInit;
use async_compression::tokio::bufread::ZlibDecoder;
use bytes::BytesMut;
use tokio::io::{AsyncRead, AsyncReadExt, BufReader};

use crate::{
    Aes128Cfb8Dec, CompressionThreshold, MAX_PACKET_DATA_SIZE, MAX_PACKET_SIZE, PacketDecodeError,
    RawPacket, ReadingError, StreamDecryptor, VarInt,
};

// 解密 -> 解压 -> 原始

pub enum DecompressionReader<R: AsyncRead + Unpin> {
    Decompress(ZlibDecoder<BufReader<R>>),
    None(R),
}

impl<R: AsyncRead + Unpin> AsyncRead for DecompressionReader<R> {
    #[inline]
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            Self::Decompress(reader) => {
                let reader = std::pin::Pin::new(reader);
                reader.poll_read(cx, buf)
            }
            Self::None(reader) => {
                let reader = std::pin::Pin::new(reader);
                reader.poll_read(cx, buf)
            }
        }
    }
}

pub enum DecryptionReader<R: AsyncRead + Unpin> {
    Decrypt(Box<StreamDecryptor<R>>),
    None(R),
}

impl<R: AsyncRead + Unpin> DecryptionReader<R> {
    #[must_use]
    pub fn upgrade(self, cipher: Aes128Cfb8Dec) -> Self {
        match self {
            Self::None(stream) => Self::Decrypt(Box::new(StreamDecryptor::new(cipher, stream))),
            Self::Decrypt(_) => self,
        }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for DecryptionReader<R> {
    #[inline]
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match self.get_mut() {
            Self::Decrypt(reader) => {
                let reader = std::pin::Pin::new(reader);
                reader.poll_read(cx, buf)
            }
            Self::None(reader) => {
                let reader = std::pin::Pin::new(reader);
                reader.poll_read(cx, buf)
            }
        }
    }
}

/// 解码器：客户端 -> 服务器
/// 支持 `ZLib` 解码/解压
/// 支持 Aes128 加密
pub struct TCPNetworkDecoder<R: AsyncRead + Unpin> {
    reader: Option<DecryptionReader<R>>,
    compression: Option<CompressionThreshold>,
    payload_scratch: BytesMut,
}

impl<R: AsyncRead + Unpin> TCPNetworkDecoder<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader: Some(DecryptionReader::None(reader)),
            compression: None,
            payload_scratch: BytesMut::new(),
        }
    }

    pub const fn set_compression(&mut self, threshold: CompressionThreshold) {
        self.compression = Some(threshold);
    }

    /// NOTE: 加密只能设置；Minecraft 流无法退回未加密状态
    pub fn set_encryption(&mut self, key: &[u8; 16]) -> Result<(), PacketDecodeError> {
        if matches!(self.reader, Some(DecryptionReader::Decrypt(_))) {
            return Err(PacketDecodeError::Message(
                "Encryption already enabled".into(),
            ));
        }
        let cipher = Aes128Cfb8Dec::new_from_slices(key, key)
            .map_err(|_| PacketDecodeError::Message("Invalid key".into()))?;

        if let Some(reader) = self.reader.take() {
            self.reader = Some(reader.upgrade(cipher));
        }
        Ok(())
    }

    pub async fn get_raw_packet(&mut self) -> Result<RawPacket, PacketDecodeError> {
        let reader = self
            .reader
            .as_mut()
            .ok_or_else(|| PacketDecodeError::Message("Reader missing".into()))?;

        let packet_len = VarInt::decode_async(reader)
            .await
            .map_err(|err| match err {
                ReadingError::CleanEOF(_) => PacketDecodeError::ConnectionClosed,
                err => PacketDecodeError::MalformedLength(err.to_string()),
            })?;

        let packet_len = packet_len.0 as u64;

        if !(0..=MAX_PACKET_SIZE).contains(&packet_len) {
            Err(PacketDecodeError::OutOfBounds)?;
        }

        let mut bounded_reader = reader.take(packet_len);
        let mut expected_packet_data_len = packet_len as usize;
        let mut expected_uncompressed_packet_data_len = None;

        let mut reader = if let Some(threshold) = self.compression {
            let decompressed_length = VarInt::decode_async(&mut bounded_reader).await?;
            let raw_packet_length = packet_len - decompressed_length.written_size() as u64;
            let decompressed_length = decompressed_length.0 as usize;

            if !(0..=MAX_PACKET_DATA_SIZE).contains(&decompressed_length) {
                Err(PacketDecodeError::TooLong)?;
            }

            if decompressed_length > 0 {
                expected_packet_data_len = decompressed_length;
                expected_uncompressed_packet_data_len = Some(decompressed_length);
                DecompressionReader::Decompress(ZlibDecoder::new(BufReader::new(bounded_reader)))
            } else {
                // 校验数值不低于压缩阈值
                if raw_packet_length > threshold as u64 {
                    Err(PacketDecodeError::NotCompressed)?;
                }

                expected_packet_data_len = raw_packet_length as usize;
                DecompressionReader::None(bounded_reader)
            }
        } else {
            DecompressionReader::None(bounded_reader)
        };

        let packet_id = VarInt::decode_async(&mut reader)
            .await
            .map_err(|_| PacketDecodeError::DecodeID)?
            .0;
        let packet_id_len = VarInt(packet_id).written_size();

        let payload_len_hint = expected_packet_data_len.saturating_sub(packet_id_len);
        self.payload_scratch.clear();
        self.payload_scratch.reserve(payload_len_hint);

        let mut total_read = 0;
        while total_read < payload_len_hint {
            let bytes_read = reader
                .read_buf(&mut self.payload_scratch)
                .await
                .map_err(|err| PacketDecodeError::FailedDecompression(err.to_string()))?;
            if bytes_read == 0 {
                break;
            }
            total_read += bytes_read;
        }

        if let Some(expected_uncompressed_packet_data_len) = expected_uncompressed_packet_data_len {
            let decoded_packet_data_len = packet_id_len + self.payload_scratch.len();
            if decoded_packet_data_len != expected_uncompressed_packet_data_len {
                return Err(PacketDecodeError::FailedDecompression(format!(
                    "Declared decompressed length {expected_uncompressed_packet_data_len} but decoded {decoded_packet_data_len} bytes"
                )));
            }
        }

        let payload = self
            .payload_scratch
            .split_to(self.payload_scratch.len())
            .freeze();

        Ok(RawPacket {
            id: packet_id,
            payload,
        })
    }
}

#[cfg(test)]
mod tests {

    use std::io::Write;

    use crate::ser::NetworkWriteExt;

    use super::*;
    use aes::Aes128;
    use cfb8::Encryptor as Cfb8Encryptor;
    use flate2::Compression;
    use flate2::write::ZlibEncoder;

    /// 辅助函数：使用 libdeflater 的 Zlib 压缩器压缩数据
    fn compress_zlib(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
        let mut compressed = Vec::new();
        ZlibEncoder::new(&mut compressed, Compression::default()).write_all(data)?;
        Ok(compressed)
    }

    /// 辅助函数：使用 AES-128 CFB-8 模式加密数据
    fn encrypt_aes128(
        data: &mut [u8],
        key: &[u8; 16],
        iv: &[u8; 16],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut encryptor =
            Cfb8Encryptor::<Aes128>::new_from_slices(key, iv).map_err(|_| "Invalid key/iv")?;
        encryptor.encrypt(data);
        Ok(())
    }

    /// 辅助函数：构建数据包，可选压缩与加密
    fn build_packet(
        packet_id: i32,
        payload: &[u8],
        compress: bool,
        key: Option<&[u8; 16]>,
        iv: Option<&[u8; 16]>,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut buffer = Vec::new();

        if compress {
            // 创建包含 `packet_id_varint` 和负载的缓冲区
            let mut data_to_compress = Vec::new();
            let packet_id_varint = VarInt(packet_id);
            data_to_compress.write_var_int(&packet_id_varint)?;
            data_to_compress.write_slice(payload)?;

            // 压缩合并后的数据
            let compressed_payload = compress_zlib(&data_to_compress)?;
            let data_len = data_to_compress.len() as i32; // 1 + payload.len()
            let data_len_varint = VarInt(data_len);
            buffer.write_var_int(&data_len_varint)?;
            buffer.write_slice(&compressed_payload)?;
        } else {
            // 无压缩；`data_len` 为负载长度
            let packet_id_varint = VarInt(packet_id);
            buffer.write_var_int(&packet_id_varint)?;
            buffer.write_slice(payload)?;
        }

        // 计算数据包长度：缓冲区长度
        let packet_len = buffer.len() as i32;
        let packet_len_varint = VarInt(packet_len);
        let mut packet_length_encoded = Vec::new();
        packet_len_varint.encode(&mut packet_length_encoded)?;

        // 为整个数据包创建新缓冲区
        let mut packet = Vec::new();
        packet.extend_from_slice(&packet_length_encoded);
        packet.extend_from_slice(&buffer);

        // 若提供了密钥和 IV，则加密
        if let (Some(k), Some(v)) = (key, iv) {
            encrypt_aes128(&mut packet, k, v)?;
        }
        Ok(packet)
    }

    /// 测试不带压缩与加密的解码
    #[tokio::test]
    async fn decode_without_compression_and_encryption() -> Result<(), Box<dyn std::error::Error>> {
        // 示例数据包数据：packet_id = 1，payload = "Hello"
        let packet_id = 1;
        let payload = b"Hello";

        // 构建未启用压缩与加密的数据包
        let packet = build_packet(packet_id, payload, false, None, None)?;

        // 在不启用压缩和加密的情况下初始化解码器
        let mut decoder = TCPNetworkDecoder::new(packet.as_slice());

        // 尝试解码
        let raw_packet = decoder.get_raw_packet().await.map_err(|e| e.to_string())?;

        assert_eq!(raw_packet.id, packet_id);
        assert_eq!(raw_packet.payload.as_ref(), payload);
        Ok(())
    }

    /// 测试带压缩的解码
    #[tokio::test]
    async fn decode_with_compression() -> Result<(), Box<dyn std::error::Error>> {
        // 示例数据包数据：packet_id = 2，payload = "Hello, compressed world!"
        let packet_id = 2;
        let payload = b"Hello, compressed world!";

        // 构建启用压缩的数据包
        let packet = build_packet(packet_id, payload, true, None, None)?;

        // 启用压缩来初始化解码器
        let mut decoder = TCPNetworkDecoder::new(packet.as_slice());
        // 大于负载
        decoder.set_compression(1000);

        // 尝试解码
        let raw_packet = decoder.get_raw_packet().await.map_err(|e| e.to_string())?;

        assert_eq!(raw_packet.id, packet_id);
        assert_eq!(raw_packet.payload.as_ref(), payload);
        Ok(())
    }

    /// 测试带加密的解码
    #[tokio::test]
    async fn decode_with_encryption() -> Result<(), Box<dyn std::error::Error>> {
        // 示例数据包数据：packet_id = 3，payload = "Hello, encrypted world!"
        let packet_id = 3;
        let payload = b"Hello, encrypted world!";

        // 定义加密密钥和 IV
        let key = [0x00u8; 16]; // 示例密钥

        // 构建启用加密（无压缩）的数据包
        let packet = build_packet(packet_id, payload, false, Some(&key), Some(&key))?;

        // 启用加密来初始化解码器
        let mut decoder = TCPNetworkDecoder::new(packet.as_slice());
        decoder.set_encryption(&key).map_err(|e| e.to_string())?;

        // 尝试解码
        let raw_packet = decoder.get_raw_packet().await.map_err(|e| e.to_string())?;

        assert_eq!(raw_packet.id, packet_id);
        assert_eq!(raw_packet.payload.as_ref(), payload);
        Ok(())
    }

    /// 测试同时带压缩与加密的解码
    #[tokio::test]
    async fn decode_with_compression_and_encryption() -> Result<(), Box<dyn std::error::Error>> {
        // 示例数据包数据：packet_id = 4，payload = "Hello, compressed and encrypted world!"
        let packet_id = 4;
        let payload = b"Hello, compressed and encrypted world!";

        // 定义加密密钥和 IV
        let key = [0x01u8; 16]; // 示例密钥
        let iv = [0x01u8; 16]; // 示例 IV

        // 构建同时启用压缩与加密的数据包
        let packet = build_packet(packet_id, payload, true, Some(&key), Some(&iv))?;

        // 同时启用压缩和加密来初始化解码器
        let mut decoder = TCPNetworkDecoder::new(packet.as_slice());
        decoder.set_compression(1000);
        decoder.set_encryption(&key).map_err(|e| e.to_string())?;

        // 尝试解码
        let raw_packet = decoder.get_raw_packet().await.map_err(|e| e.to_string())?;

        assert_eq!(raw_packet.id, packet_id);
        assert_eq!(raw_packet.payload.as_ref(), payload);
        Ok(())
    }

    /// 测试用无效压缩数据解码
    #[tokio::test]
    async fn decode_with_invalid_compressed_data() -> Result<(), Box<dyn std::error::Error>> {
        // 示例数据包数据：packet_id = 5，payload_len = 10，但压缩数据无效
        let data_len = 10; // 预期的解压后大小
        let invalid_compressed_data = vec![0xFF, 0xFF, 0xFF]; // 无效的 Zlib 数据

        // 构建启用压缩但压缩数据无效的数据包
        let mut buffer = Vec::new();
        let data_len_varint = VarInt(data_len);
        buffer.write_var_int(&data_len_varint)?;
        buffer.write_slice(&invalid_compressed_data)?;

        // 计算数据包长度：VarInt(data_len) + 无效的压缩数据
        let packet_len = buffer.len() as i32;
        let packet_len_varint = VarInt(packet_len);

        // 为整个数据包创建新缓冲区
        let mut packet_buffer = Vec::new();
        packet_buffer.write_var_int(&packet_len_varint)?;
        packet_buffer.write_slice(&buffer)?;

        let packet_bytes = packet_buffer;

        // 启用压缩来初始化解码器
        let mut decoder = TCPNetworkDecoder::new(&packet_bytes[..]);
        decoder.set_compression(1000);

        // 尝试解码并预期出现解压错误
        let result = decoder.get_raw_packet().await;

        assert!(result.is_err(), "This should have errored!");
        Ok(())
    }

    /// 测试解码零长度数据包
    #[tokio::test]
    async fn decode_with_zero_length_packet() -> Result<(), Box<dyn std::error::Error>> {
        // 示例数据包数据：packet_id = 7，payload = ""（空）
        let packet_id = 7;
        let payload = b"";

        // 构建未启用压缩与加密的数据包
        let packet = build_packet(packet_id, payload, false, None, None)?;

        // 在不启用压缩和加密的情况下初始化解码器
        let mut decoder = TCPNetworkDecoder::new(packet.as_slice());

        // 尝试解码并预期出现读取错误
        let raw_packet = decoder.get_raw_packet().await.map_err(|e| e.to_string())?;
        assert_eq!(raw_packet.id, packet_id);
        assert_eq!(raw_packet.payload.as_ref(), payload);
        Ok(())
    }

    /// 测试解码最大长度数据包
    #[tokio::test]
    #[expect(clippy::print_stdout)]
    async fn decode_with_maximum_length_packet() -> Result<(), Box<dyn std::error::Error>> {
        // 示例数据包数据：packet_id = 8，payload = "A" 重复 MAX_PACKET_SIZE 次
        // 示例数据包数据：packet_id = 8，payload = "A" 重复 (MAX_PACKET_SIZE - 1) 次
        let packet_id = 8;
        let payload = vec![0x41u8; MAX_PACKET_SIZE as usize - 1]; // 重复的 "A"

        // 构建启用压缩的数据包
        let packet = build_packet(packet_id, &payload, true, None, None)?;
        println!("已构建数据包（带压缩，最大长度）：{packet:?}");

        // 启用压缩来初始化解码器
        let mut decoder = TCPNetworkDecoder::new(packet.as_slice());
        decoder.set_compression(MAX_PACKET_SIZE as usize + 1);

        // 尝试解码
        let result = decoder.get_raw_packet().await;

        let raw_packet = result.map_err(|e| e.to_string())?;
        assert_eq!(raw_packet.id, packet_id);
        assert_eq!(raw_packet.payload.as_ref(), payload);
        Ok(())
    }

    /// 测试顺序解码多个数据包，验证容量得以保留以实现零分配
    #[tokio::test]
    async fn decode_multiple_packets_zero_allocation() -> Result<(), Box<dyn std::error::Error>> {
        let packet1 = build_packet(1, b"Hello", false, None, None)?;
        let packet2 = build_packet(2, b"World", false, None, None)?;

        let mut stream = Vec::new();
        stream.extend_from_slice(&packet1);
        stream.extend_from_slice(&packet2);

        let mut decoder = TCPNetworkDecoder::new(stream.as_slice());

        let p1 = decoder.get_raw_packet().await?;
        assert_eq!(p1.id, 1);
        assert_eq!(p1.payload.as_ref(), b"Hello");
        drop(p1);

        let cap_after_p1 = decoder.payload_scratch.capacity();
        assert!(
            cap_after_p1 > 0,
            "Capacity should be allocated after first read"
        );

        let p2 = decoder.get_raw_packet().await?;
        assert_eq!(p2.id, 2);
        assert_eq!(p2.payload.as_ref(), b"World");
        drop(p2);

        let cap_after_p2 = decoder.payload_scratch.capacity();
        assert_eq!(
            cap_after_p2, cap_after_p1,
            "Buffer capacity should be retained and reused without new heap allocations"
        );
        Ok(())
    }
}
