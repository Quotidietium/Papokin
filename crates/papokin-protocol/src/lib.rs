#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use std::{
    io::{Error, Write},
    pin::Pin,
    task::{Context, Poll},
};

use aes::cipher::BlockSizeUser;
use bytes::Bytes;
use codec::var_int::VarInt;
use hybrid_array::{Array, sizes::U1};
use papokin_util::{
    resource_location::ResourceLocation,
    text::{TextComponent, style::Style},
    version::JavaMinecraftVersion,
};
use ser::{ReadingError, WritingError};

use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub use crate::packet::{MultiVersionJavaPacket, Packet};

pub mod codec;
pub mod java;
pub mod packet;
#[cfg(feature = "query")]
pub mod query;
pub mod rcon;
pub mod ser;
pub mod serial;
pub mod tag_overlay;

pub const MAX_PACKET_SIZE: u64 = 2_097_152;
pub const MAX_PACKET_DATA_SIZE: usize = 8_388_608;

pub type FixedBitSet = Box<[u8]>;

/// 表示压缩阈值。
///
/// 该阈值决定了应进行压缩的数据最小大小。
/// 小于阈值的数据不会被压缩。
pub type CompressionThreshold = usize;

/// 表示压缩级别。
///
/// 该级别控制对数据施加的压缩程度。
/// 更高的级别通常带来更高的压缩比，但同时
/// 增加 CPU 占用。
pub type CompressionLevel = u32;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ConnectionState {
    HandShake,
    Status,
    Login,
    Transfer,
    Config,
    Play,
}
pub struct InvalidConnectionState;

impl TryFrom<VarInt> for ConnectionState {
    type Error = InvalidConnectionState;

    fn try_from(value: VarInt) -> Result<Self, Self::Error> {
        let value = value.0;
        match value {
            1 => Ok(Self::Status),
            2 => Ok(Self::Login),
            3 => Ok(Self::Transfer),
            _ => Err(InvalidConnectionState),
        }
    }
}

#[derive(PartialEq, Eq, Clone)]
pub enum IdOr<T> {
    Id(u16),
    Value(T),
}

impl<T> IdOr<T> {
    pub fn read<R: ser::NetworkReadExt>(
        read: &mut R,
        read_value: impl FnOnce(&mut R) -> Result<T, ser::ReadingError>,
    ) -> Result<Self, ser::ReadingError> {
        let id = read.get_var_int()?.0;
        if id == 0 {
            Ok(Self::Value(read_value(read)?))
        } else {
            Ok(Self::Id((id - 1) as u16))
        }
    }

    pub fn write<W: ser::NetworkWriteExt>(
        &self,
        write: &mut W,
        write_value: impl FnOnce(&mut W, &T) -> Result<(), ser::WritingError>,
    ) -> Result<(), ser::WritingError> {
        match self {
            Self::Id(id) => write.write_var_int(&((*id as i32) + 1).into()),
            Self::Value(value) => {
                write.write_var_int(&0.into())?;
                write_value(write, value)
            }
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct SoundEvent {
    pub sound_name: ResourceLocation,
    pub range: Option<f32>,
}

type Aes128Cfb8Dec = cfb8::Decryptor<aes::Aes128>;

pub struct StreamDecryptor<R: AsyncRead + Unpin> {
    cipher: Aes128Cfb8Dec,
    read: R,
}

impl<R: AsyncRead + Unpin> StreamDecryptor<R> {
    pub const fn new(cipher: Aes128Cfb8Dec, stream: R) -> Self {
        Self {
            cipher,
            read: stream,
        }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for StreamDecryptor<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let ref_self = self.get_mut();
        let read = Pin::new(&mut ref_self.read);
        let cipher = &mut ref_self.cipher;

        // 获取起始位置
        let original_fill = buf.filled().len();
        // 读取原始数据
        let internal_poll = read.poll_read(cx, buf);

        if matches!(internal_poll, Poll::Ready(Ok(()))) {
            // 原地解密原始数据；注意我们的块大小为 1 字节，因此总是安全的
            for block in buf.filled_mut()[original_fill..].chunks_mut(Aes128Cfb8Dec::block_size()) {
                cipher.decrypt(block);
            }
        }

        internal_poll
    }
}

type Aes128Cfb8Enc = cfb8::Encryptor<aes::Aes128>;

///NOTE: 这会产生大量小写入；请确保下游某处存在缓冲区
pub struct StreamEncryptor<W: AsyncWrite + Unpin> {
    cipher: Aes128Cfb8Enc,
    write: W,
    last_unwritten_encrypted_byte: Option<u8>,
}

impl<W: AsyncWrite + Unpin> StreamEncryptor<W> {
    pub fn new(cipher: Aes128Cfb8Enc, stream: W) -> Self {
        debug_assert_eq!(Aes128Cfb8Enc::block_size(), 1);
        Self {
            cipher,
            write: stream,
            last_unwritten_encrypted_byte: None,
        }
    }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for StreamEncryptor<W> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, Error>> {
        let ref_self = self.get_mut();
        let cipher = &mut ref_self.cipher;

        let mut total_written = 0;
        // 解密原始数据；注意我们的块大小为 1 字节，因此总是安全的
        for block in buf.chunks(Aes128Cfb8Enc::block_size()) {
            let mut out = [0u8];

            if let Some(out_to_use) = ref_self.last_unwritten_encrypted_byte {
                // 这假设此 `poll_write` 是在同一个字节流上调用的，而我
                // 认为是合理的假设，因为这对 TCP 流而言本来就是不变式。

                // 这里绝不应发生 panic
                out[0] = out_to_use;
            } else {
                let out_block: &mut Array<u8, U1> = (&mut out[..])
                    .try_into()
                    .map_err(|_| Error::other("Output slice size does not match block size"))?;
                cipher
                    .encrypt_b2b(block, out_block)
                    .map_err(|_| Error::other("Encryption failed"))?;
            }

            let write = Pin::new(&mut ref_self.write);
            match write.poll_write(cx, &out) {
                Poll::Pending => {
                    ref_self.last_unwritten_encrypted_byte = Some(out[0]);
                    if total_written == 0 {
                        //如果我们没有写入任何内容，则返回 pending
                        return Poll::Pending;
                    }
                    // 否则，说明我们确实写入了内容
                    return Poll::Ready(Ok(total_written));
                }
                Poll::Ready(result) => {
                    ref_self.last_unwritten_encrypted_byte = None;
                    match result {
                        Ok(written) => total_written += written,
                        Err(err) => return Poll::Ready(Err(err)),
                    }
                }
            }
        }

        Poll::Ready(Ok(total_written))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        let ref_self = self.get_mut();
        let write = Pin::new(&mut ref_self.write);
        write.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        let ref_self = self.get_mut();
        let write = Pin::new(&mut ref_self.write);
        write.poll_shutdown(cx)
    }
}

pub struct RawPacket {
    pub id: i32,
    pub payload: Bytes,
}

pub trait ClientPacket: MultiVersionJavaPacket {
    fn write_packet_data(
        &self,
        write: impl Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError>;

    fn write_packet(
        &self,
        version: &JavaMinecraftVersion,
        write: impl Write,
    ) -> Result<(), WritingError> {
        crate::java::packet_encoder::write_packet(self, version, write)
    }

    fn serialize_packet(&self, version: &JavaMinecraftVersion) -> Result<Bytes, WritingError> {
        crate::java::packet_encoder::serialize_packet(self, version)
    }
}

pub trait ServerPacket<'a>: MultiVersionJavaPacket + Sized {
    fn read(read: &mut &'a [u8], version: &JavaMinecraftVersion) -> Result<Self, ReadingError>;
}

/// 数据包编码期间可能发生的错误。
#[derive(Error, Debug)]
pub enum PacketEncodeError {
    #[error("Packet exceeds maximum length: {0}")]
    TooLong(usize),
    #[error("Compression failed {0}")]
    CompressionFailed(String),
    #[error("Writing packet failed: {0}")]
    Message(String),
}

#[derive(Error, Debug)]
pub enum PacketDecodeError {
    #[error("failed to decode packet ID")]
    DecodeID,
    #[error("packet exceeds maximum length")]
    TooLong,
    #[error("packet length is out of bounds")]
    OutOfBounds,
    #[error("malformed packet length VarInt: {0}")]
    MalformedLength(String),
    #[error("failed to decompress packet: {0}")]
    FailedDecompression(String), // 已更新以包含错误详情
    #[error("packet is uncompressed but greater than the threshold")]
    NotCompressed,
    #[error("the connection has closed")]
    ConnectionClosed,
    #[error("{0}")]
    Message(String),
}

impl From<ReadingError> for PacketDecodeError {
    fn from(value: ReadingError) -> Self {
        Self::FailedDecompression(value.to_string())
    }
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    /// 服务器正在运行的版本。（可选）
    pub version: Option<Version>,
    /// 当前已连接玩家的信息。（可选）
    pub players: Option<Players>,
    /// 显示的描述，也称为 MOTD（每日消息）。（可选）
    pub description: TextComponent,
    /// 显示的图标。（可选）
    pub favicon: Option<String>,
    /// 是否强制玩家使用安全聊天。
    pub enforce_secure_chat: bool,
}
#[derive(Clone, serde::Serialize)]
pub struct Version {
    /// 版本的名称（例如 1.21.4）
    pub name: String,
    /// 协议版本（例如 767）
    pub protocol: u32,
}

#[derive(Clone, serde::Serialize)]
pub struct Players {
    /// 服务器允许的最大玩家数。
    pub max: u32,
    /// 当前在线玩家数量。
    pub online: u32,
    /// 当前已连接玩家的信息。
    /// 注意：玩家可在此禁用列表显示。
    pub sample: Vec<Sample>,
}

#[derive(Clone, serde::Serialize)]
pub struct Sample {
    /// 玩家的名称。
    pub name: String,
    /// 玩家的 UUID。
    pub id: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Property {
    pub name: Box<str>,
    pub value: Box<str>,
    pub signature: Option<Box<str>>,
}

impl Property {
    pub fn read(read: &mut impl ser::NetworkReadExt) -> Result<Self, ser::ReadingError> {
        Ok(Self {
            name: read.get_str()?,
            value: read.get_str()?,
            signature: read.get_option(ser::NetworkReadExt::get_str)?,
        })
    }

    pub fn write(&self, write: &mut impl ser::NetworkWriteExt) -> Result<(), ser::WritingError> {
        write.write_string(&self.name)?;
        write.write_string(&self.value)?;
        write.write_option(&self.signature, |w, v| w.write_string(v))?;
        Ok(())
    }
}

pub struct KnownPack<'a> {
    pub namespace: &'a str,
    pub id: &'a str,
    pub version: &'a str,
}

impl KnownPack<'_> {
    pub fn write(&self, write: &mut impl ser::NetworkWriteExt) -> Result<(), ser::WritingError> {
        write.write_string(self.namespace)?;
        write.write_string(self.id)?;
        write.write_string(self.version)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NumberFormat {
    /// 不显示任何内容。
    Blank,
    /// 格式化分数数字时所使用的样式。
    Styled(Style),
    /// 用作占位符的文本。
    Fixed(TextComponent),
}

impl NumberFormat {
    pub fn write(&self, write: &mut impl ser::NetworkWriteExt) -> Result<(), ser::WritingError> {
        match self {
            Self::Blank => write.write_var_int(&0.into()),
            Self::Styled(_style) => {
                write.write_var_int(&1.into())?;
                // TODO: 样式写入
                Ok(())
            }
            Self::Fixed(text) => {
                write.write_var_int(&2.into())?;
                write.write_slice(&text.encode())?;
                Ok(())
            }
        }
    }
}

/// 对于前 8 个值，置位表示相对值，未置位表示绝对值
#[derive(Debug, PartialEq, Eq, Hash)]
pub enum PositionFlag {
    X,
    Y,
    Z,
    YRot,
    XRot,
    DeltaX,
    DeltaY,
    DeltaZ,
    RotateDelta,
}

impl PositionFlag {
    const fn get_mask(&self) -> i32 {
        match self {
            Self::X => 1 << 0,
            Self::Y => 1 << 1,
            Self::Z => 1 << 2,
            Self::YRot => 1 << 3,
            Self::XRot => 1 << 4,
            Self::DeltaX => 1 << 5,
            Self::DeltaY => 1 << 6,
            Self::DeltaZ => 1 << 7,
            Self::RotateDelta => 1 << 8,
        }
    }

    #[must_use]
    pub fn get_bitfield(flags: &[Self]) -> i32 {
        flags.iter().fold(0, |acc, flag| acc | flag.get_mask())
    }

    #[must_use]
    pub fn from_bitfield(bits: i32) -> Vec<Self> {
        let all = [
            Self::X,
            Self::Y,
            Self::Z,
            Self::YRot,
            Self::XRot,
            Self::DeltaX,
            Self::DeltaY,
            Self::DeltaZ,
            Self::RotateDelta,
        ];
        all.into_iter()
            .filter(|flag| (bits & flag.get_mask()) != 0)
            .collect()
    }
}

#[derive(Clone, Debug)]
pub enum Label {
    BuiltIn(LinkType),
    TextComponent(Box<TextComponent>),
}

pub struct Link<'a> {
    pub is_built_in: bool,
    pub label: Label,
    pub url: &'a String,
}

impl<'a> Link<'a> {
    #[must_use]
    pub const fn new(label: Label, url: &'a String) -> Self {
        Self {
            is_built_in: match label {
                Label::BuiltIn(_) => true,
                Label::TextComponent(_) => false,
            },
            label,
            url,
        }
    }

    pub fn write(&self, write: &mut impl ser::NetworkWriteExt) -> Result<(), ser::WritingError> {
        match &self.label {
            Label::BuiltIn(link_type) => {
                write.write_bool(true)?;
                write.write_var_int(&(*link_type as i32).into())?;
            }
            Label::TextComponent(text_component) => {
                write.write_bool(false)?;
                write.write_slice(&text_component.encode())?;
            }
        }
        write.write_string(self.url)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(i32)]
pub enum LinkType {
    BugReport = 0,
    CommunityGuidelines = 1,
    Support = 2,
    Status = 3,
    Feedback = 4,
    Community = 5,
    Website = 6,
    Forums = 7,
    News = 8,
    Announcements = 9,
}
