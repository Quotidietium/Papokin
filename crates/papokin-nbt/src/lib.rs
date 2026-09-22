//! 读取、写入和操作 Minecraft 的命名二进制标签（NBT）数据。
//!
//! 该 crate 支持标准 Java 版表示、未命名网络
//! NBT 以及 gzip 压缩 NBT。数据会被直接处理
//! 通过 [`Nbt`]、[`NbtCompound`] 与 [`NbtTag`] 进行。

#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use std::{
    io::{self, Write},
    ops::Deref,
};

use bytes::Bytes;
use deserializer::NbtReadHelper;
use serializer::{NbtWriteHelper, NbtWriteHelperJava};
use tag::NbtTag;
use thiserror::Error;

/// 复合标签的存储与构建辅助函数。
pub mod compound;
/// 底层 NBT 反序列化支持。
pub mod deserializer;
/// 读写 gzip 压缩的 NBT。
pub mod nbt_compress;
/// 与 Pumpkin 动态编解码操作的集成。
pub mod nbt_ops;
/// 底层 NBT 序列化支持。
pub mod serializer;
/// 各个独立的 NBT 标签类型。
pub mod tag;

pub use compound::NbtCompound;

// 这个 NBT crate 的灵感来自 CrabNBT

/// 结束标签的数字标识符。
pub const END_ID: u8 = 0x00;
/// 字节标签的数字标识符。
pub const BYTE_ID: u8 = 0x01;
/// 短整型标签的数字标识符。
pub const SHORT_ID: u8 = 0x02;
/// 整型标签的数字标识符。
pub const INT_ID: u8 = 0x03;
/// 长整型标签的数字标识符。
pub const LONG_ID: u8 = 0x04;
/// 单精度浮点标签的数字标识符。
pub const FLOAT_ID: u8 = 0x05;
/// 双精度浮点标签的数字标识符。
pub const DOUBLE_ID: u8 = 0x06;
/// 字节数组标签的数字标识符。
pub const BYTE_ARRAY_ID: u8 = 0x07;
/// 字符串标签的数字标识符。
pub const STRING_ID: u8 = 0x08;
/// 列表标签的数字标识符。
pub const LIST_ID: u8 = 0x09;
/// 复合标签的数字标识符。
pub const COMPOUND_ID: u8 = 0x0A;
/// 整型数组标签的数字标识符。
pub const INT_ARRAY_ID: u8 = 0x0B;
/// 长整型数组标签的数字标识符。
pub const LONG_ARRAY_ID: u8 = 0x0C;

/// 解码列表或数组时接受的最大元素数。
pub const MAX_ARRAY_LENGTH: usize = 512_000;
/// 解码 NBT 复合标签或列表标签时允许的最大嵌套深度。
pub const MAX_NBT_DEPTH: usize = 512;

/// 读取、写入或转换 NBT 数据时产生的错误。
#[derive(Error, Debug)]
pub enum Error {
    /// 根标签不是复合标签（包含所报告的标签 ID）。
    #[error("The root tag of the NBT file is not a compound tag. Received tag id: {0}")]
    NoRootCompound(u8),
    /// 遇到了 NBT 格式未定义的标签 ID。
    #[error("Encountered an unknown NBT tag id: {0}.")]
    UnknownTagId(u8),
    /// 无法解码 Java CESU-8 字符串。
    #[error("Failed to Cesu 8 Decode")]
    Cesu8DecodingError,
    /// 字符串无法解码为 UTF-8。
    #[error("Failed to UTF-8 Decode")]
    Utf8DecodingError,
    /// Serde 报告了无效的值或序列化器状态。
    #[error("Serde error: {0}")]
    SerdeError(String),
    /// 所请求的 Rust 类型没有对应的 NBT 表示。
    #[error("NBT doesn't support this type: {0}")]
    UnsupportedType(String),
    /// 底层读取器或写入器返回了 I/O 错误。
    #[error("NBT reading was cut short: {0}")]
    Incomplete(io::Error),
    /// 列表或数组声明了负数的元素数量。
    #[error("Negative list length: {0}")]
    NegativeLength(i32),
    /// 字符串、列表或数组超出了支持的长度。
    #[error("Length too large: {0}")]
    LargeLength(usize),
    /// NBT 嵌套深度超过了允许的最大限制。
    #[error("NBT depth exceeded maximum allowed limit")]
    MaxDepthExceeded,
    /// 列表标签指定了无效的元素标签类型。
    #[error("Invalid element tag type for list: {0}")]
    InvalidListTag(u8),
}

/// 一个完整的 NBT 文档，包含一个有名称的根复合标签。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Nbt {
    /// 与根复合标签一同存储的名称。
    pub name: String,
    /// 包含文档标签的根复合标签。
    pub root_tag: NbtCompound,
}

impl Nbt {
    /// 根据根名称和复合标签创建文档。
    #[must_use]
    pub const fn new(name: String, tag: NbtCompound) -> Self {
        Self {
            name,
            root_tag: tag,
        }
    }

    /// 从特定格式的读取器中读取带名称的 NBT 文档。
    ///
    /// 当第一个标签不是复合标签时，返回 [`Error::NoRootCompound`]。
    pub fn read<'a, R: NbtReadHelper<'a>>(reader: &mut R) -> Result<Self, Error> {
        let tag_type_id = reader.get_u8()?;

        if tag_type_id != COMPOUND_ID {
            return Err(Error::NoRootCompound(tag_type_id));
        }

        Ok(Self {
            name: reader.get_string()?.into_owned(),
            root_tag: NbtCompound::deserialize_content(reader)?,
        })
    }

    /// 读取省略了根复合标签名称的 NBT 文档。
    ///
    /// 返回的文档具有空的 [`Self::name`]。
    pub fn read_unnamed<'a, R: NbtReadHelper<'a>>(reader: &mut R) -> Result<Self, Error> {
        let tag_type_id = reader.get_u8()?;

        if tag_type_id != COMPOUND_ID {
            return Err(Error::NoRootCompound(tag_type_id));
        }

        Ok(Self {
            name: String::new(),
            root_tag: NbtCompound::deserialize_content(reader)?,
        })
    }

    /// 使用 Java 版的 NBT 表示序列化此文档。
    #[must_use]
    pub fn write(self) -> Bytes {
        let mut bytes = Vec::new();
        let mut writer = NbtWriteHelperJava::new(&mut bytes);
        if writer.write_u8(COMPOUND_ID).is_ok()
            && NbtTag::String(self.name.into())
                .serialize_data(&mut writer)
                .is_ok()
        {
            let _ = self.root_tag.serialize_content(&mut writer);
        }

        bytes.into()
    }

    /// 以 Java 版的表示形式写入此文档。
    pub fn write_to_writer<W: Write>(self, mut writer: W) -> Result<(), io::Error> {
        writer.write_all(&self.write())?;
        Ok(())
    }

    /// 序列化此文档但不包含根复合标签的名称。
    #[must_use]
    pub fn write_unnamed(self) -> Bytes {
        let mut bytes = Vec::new();
        let mut writer = NbtWriteHelperJava::new(&mut bytes);

        if writer.write_u8(COMPOUND_ID).is_ok() {
            let _ = self.root_tag.serialize_content(&mut writer);
        }

        bytes.into()
    }

    /// 写入此文档，但不包含根复合标签的名称。
    pub fn write_unnamed_to_writer<W: Write>(self, mut writer: W) -> Result<(), io::Error> {
        writer.write_all(&self.write_unnamed())?;
        Ok(())
    }
}

impl Deref for Nbt {
    type Target = NbtCompound;

    fn deref(&self) -> &Self::Target {
        &self.root_tag
    }
}

impl From<NbtCompound> for Nbt {
    fn from(value: NbtCompound) -> Self {
        Self::new(String::new(), value)
    }
}

impl<T> AsRef<T> for Nbt
where
    T: ?Sized,
    <Self as Deref>::Target: AsRef<T>,
{
    fn as_ref(&self) -> &T {
        self.deref().as_ref()
    }
}

impl AsMut<NbtCompound> for Nbt {
    fn as_mut(&mut self) -> &mut NbtCompound {
        &mut self.root_tag
    }
}
