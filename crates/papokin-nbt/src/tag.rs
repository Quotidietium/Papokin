//! 单个 NBT 标签的内存表示。

use compound::NbtCompound;
use deserializer::NbtReadHelper;
use serializer::NbtWriteHelper;

use crate::{
    BYTE_ARRAY_ID, BYTE_ID, COMPOUND_ID, DOUBLE_ID, END_ID, Error, FLOAT_ID, INT_ARRAY_ID, INT_ID,
    LIST_ID, LONG_ARRAY_ID, LONG_ID, SHORT_ID, STRING_ID, compound, deserializer, serializer,
};

/// 由 NBT 格式定义的标签类型之一所表示的值。
#[derive(Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum NbtTag {
    /// 标记复合标签的结束。
    End = END_ID,
    /// 一个 8 位有符号整数。
    Byte(i8) = BYTE_ID,
    /// 16 位有符号整数。
    Short(i16) = SHORT_ID,
    /// 32 位有符号整数。
    Int(i32) = INT_ID,
    /// 64 位有符号整数。
    Long(i64) = LONG_ID,
    /// 32 位浮点数。
    Float(f32) = FLOAT_ID,
    /// 64 位浮点数。
    Double(f64) = DOUBLE_ID,
    /// 一个 8 位有符号整数数组。
    ByteArray(Box<[i8]>) = BYTE_ARRAY_ID,
    /// 一个字符串。
    String(Box<str>) = STRING_ID,
    /// 一个标签序列。
    List(Vec<Self>) = LIST_ID,
    /// 命名标签的映射。
    Compound(NbtCompound) = COMPOUND_ID,
    /// 一个 32 位有符号整数数组。
    IntArray(Vec<i32>) = INT_ARRAY_ID,
    /// 一个 64 位有符号整数数组。
    LongArray(Vec<i64>) = LONG_ARRAY_ID,
}

impl NbtTag {
    /// 返回与该数据类型关联的数字 ID。
    #[must_use]
    pub const fn get_type_id(&self) -> u8 {
        // SAFETY: 由于 Self 是 repr(u8)，保证判别值位于第一个字节中
        // 见 https://doc.rust-lang.org/reference/items/enumerations.html#pointer-casting
        unsafe { *std::ptr::from_ref::<Self>(self).cast::<u8>() }
    }

    /// 序列化标签的类型 ID，随后是其负载。
    pub fn serialize<W: NbtWriteHelper>(self, w: &mut W) -> serializer::Result<()> {
        w.write_u8(self.get_type_id())?;
        self.serialize_data(w)?;
        Ok(())
    }

    /// 获取所提供 `Vec` 的 [`NbtTag::List`] 元素类型
    /// 的含义。若发现 `Vec` 中有任何元素属于
    /// 不同类型时，返回 [`COMPOUND_ID`]。
    #[must_use]
    fn get_list_element_type_id(list: &[Self]) -> u8 {
        let mut element_id = END_ID;

        for tag in list {
            let id = tag.get_type_id();
            if element_id == END_ID {
                element_id = id;
            } else if element_id != id {
                return COMPOUND_ID;
            }
        }

        element_id
    }

    /// 尝试解包（展平）被包装的 `NbtTag`。如果存在被包装的标签，则将其返回。
    /// 如果无法解包，则返回给定的标签。
    fn flatten(tag: Self) -> Self {
        if let Self::Compound(mut compound) = tag {
            // 尝试获取由 "" 存储的包装标签。
            if Self::is_wrapper_compound(&compound) {
                compound
                    .child_tags
                    .remove("")
                    .unwrap_or(Self::Compound(compound))
            } else {
                Self::Compound(compound)
            }
        } else {
            tag
        }
    }

    /// 返回某个 [`NbtCompound`] 是否为包装型复合标签。
    ///
    /// *包装复合标签*是恰好只存储一个
    /// 键值对，即空字符串键（`""`）和一个 `NbtTag`。
    fn is_wrapper_compound(compound: &NbtCompound) -> bool {
        compound.child_tags.len() == 1 && compound.child_tags.contains_key("")
    }

    /// 如有需要，用给定元素类型将提供的标签包装为列表
    /// 所包裹标签（如有）将要加入的目标。
    fn wrap_tag_if_needed(element_type: u8, tag: Self) -> Self {
        if element_type == COMPOUND_ID {
            if let Self::Compound(compound) = &tag
                && !Self::is_wrapper_compound(compound)
            {
                tag
            } else {
                Self::wrap_tag(tag)
            }
        } else {
            tag
        }
    }

    fn wrap_tag(tag: Self) -> Self {
        let mut compound = NbtCompound::new();
        compound.put("", tag);
        Self::Compound(compound)
    }

    /// 序列化标签负载但不写入其类型 ID。
    pub fn serialize_data<W: NbtWriteHelper>(self, w: &mut W) -> serializer::Result<()> {
        match self {
            Self::End => {}
            Self::Byte(byte) => w.write_i8(byte)?,
            Self::Short(short) => w.write_i16(short)?,
            Self::Int(int) => w.write_i32(int)?,
            Self::Long(long) => w.write_i64(long)?,
            Self::Float(float) => w.write_f32(float)?,
            Self::Double(double) => w.write_f64(double)?,
            Self::ByteArray(byte_array) => {
                let len = byte_array.len();
                if len > i32::MAX as usize {
                    return Err(Error::LargeLength(len));
                }

                w.write_i32(len as i32)?;
                // SAFETY: `i8` 与 `u8` 具有相同的布局，且该切片仅被读取。
                let bytes = unsafe {
                    std::slice::from_raw_parts(byte_array.as_ptr().cast::<u8>(), byte_array.len())
                };
                w.write_slice(bytes)?;
            }
            Self::String(string) => {
                w.write_string(&string)?;
            }
            Self::List(list) => {
                let len = list.len();
                if len > i32::MAX as usize {
                    return Err(Error::LargeLength(len));
                }

                let list_element_id = Self::get_list_element_type_id(&list);

                w.write_u8(list_element_id)?;
                w.write_i32(len as i32)?;
                for nbt_tag in list {
                    // 由于同一列表标签中的标签必须具有相同的类型，
                    // 我们需要通过以下方式处理不同标签类型的
                    // 必要时将它们包裹进 `NbtCompound`。
                    Self::wrap_tag_if_needed(list_element_id, nbt_tag).serialize_data(w)?;
                }
            }
            Self::Compound(compound) => {
                compound.serialize_content(w)?;
            }
            Self::IntArray(int_array) => {
                let len = int_array.len();
                if len > i32::MAX as usize {
                    return Err(Error::LargeLength(len));
                }

                w.write_i32(len as i32)?;
                for int in int_array {
                    w.write_i32(int)?;
                }
            }
            Self::LongArray(long_array) => {
                let len = long_array.len();
                if len > i32::MAX as usize {
                    return Err(Error::LargeLength(len));
                }

                w.write_i32(len as i32)?;
                for long in long_array {
                    w.write_i64(long)?;
                }
            }
        }
        Ok(())
    }

    /// 反序列化一个类型 ID 及其后续负载。
    pub fn deserialize<'a, R: NbtReadHelper<'a>>(reader: &mut R) -> Result<Self, Error> {
        let tag_id = reader.get_u8()?;
        Self::deserialize_data(reader, tag_id)
    }

    /// 使读取器跳过属于 `tag_id` 的有效载荷。
    pub fn skip_data<'a, R: NbtReadHelper<'a>>(reader: &mut R, tag_id: u8) -> Result<(), Error> {
        Self::skip_data_depth(reader, tag_id, 0)
    }

    /// 使读取器跳过属于 `tag_id` 的有效载荷，并进行深度跟踪。
    pub fn skip_data_depth<'a, R: NbtReadHelper<'a>>(
        reader: &mut R,
        tag_id: u8,
        depth: usize,
    ) -> Result<(), Error> {
        if depth > crate::MAX_NBT_DEPTH {
            return Err(Error::MaxDepthExceeded);
        }

        match tag_id {
            END_ID => Ok(()),
            BYTE_ID => reader.skip_i8(),
            SHORT_ID => reader.skip_i16(),
            INT_ID => reader.skip_i32(),
            LONG_ID => reader.skip_i64(),
            FLOAT_ID => reader.skip_f32(),
            DOUBLE_ID => reader.skip_f64(),
            BYTE_ARRAY_ID => {
                let len = reader.get_i32()?;
                if len < 0 {
                    return Err(Error::NegativeLength(len));
                }
                let len = len as usize;
                if len > crate::MAX_ARRAY_LENGTH {
                    return Err(Error::LargeLength(len));
                }
                reader.skip_bytes(len as i64)
            }
            STRING_ID => reader.skip_string(),
            LIST_ID => {
                let tag_type_id = reader.get_u8()?;
                let len = reader.get_i32()?;
                if len < 0 {
                    return Err(Error::NegativeLength(len));
                }
                if tag_type_id == END_ID && len > 0 {
                    return Err(Error::InvalidListTag(tag_type_id));
                }

                let len = len as usize;
                if len > crate::MAX_ARRAY_LENGTH {
                    return Err(Error::LargeLength(len));
                }

                for _ in 0..len {
                    Self::skip_data_depth(reader, tag_type_id, depth + 1)?;
                }

                Ok(())
            }
            COMPOUND_ID => NbtCompound::skip_content_depth(reader, depth + 1),
            INT_ARRAY_ID => {
                let len = reader.get_i32()?;
                if len < 0 {
                    return Err(Error::NegativeLength(len));
                }

                let len = len as usize;
                if len > crate::MAX_ARRAY_LENGTH {
                    return Err(Error::LargeLength(len));
                }

                for _ in 0..len {
                    reader.skip_i32()?;
                }

                Ok(())
            }
            LONG_ARRAY_ID => {
                let len = reader.get_i32()?;
                if len < 0 {
                    return Err(Error::NegativeLength(len));
                }

                let len = len as usize;
                if len > crate::MAX_ARRAY_LENGTH {
                    return Err(Error::LargeLength(len));
                }

                for _ in 0..len {
                    reader.skip_i64()?;
                }

                Ok(())
            }
            _ => Err(Error::UnknownTagId(tag_id)),
        }
    }

    /// 反序列化一个负载，其类型由 `tag_id` 标识。
    pub fn deserialize_data<'a, R: NbtReadHelper<'a>>(
        reader: &mut R,
        tag_id: u8,
    ) -> Result<Self, Error> {
        Self::deserialize_data_depth(reader, tag_id, 0)
    }

    /// 反序列化类型由 `tag_id` 标识的负载，并带深度跟踪。
    #[allow(clippy::too_many_lines)]
    pub fn deserialize_data_depth<'a, R: NbtReadHelper<'a>>(
        reader: &mut R,
        tag_id: u8,
        depth: usize,
    ) -> Result<Self, Error> {
        if depth > crate::MAX_NBT_DEPTH {
            return Err(Error::MaxDepthExceeded);
        }

        match tag_id {
            END_ID => Ok(Self::End),
            BYTE_ID => {
                let byte = reader.get_i8()?;
                Ok(Self::Byte(byte))
            }
            SHORT_ID => {
                let short = reader.get_i16()?;
                Ok(Self::Short(short))
            }
            INT_ID => {
                let int = reader.get_i32()?;
                Ok(Self::Int(int))
            }
            LONG_ID => {
                let long = reader.get_i64()?;
                Ok(Self::Long(long))
            }
            FLOAT_ID => {
                let float = reader.get_f32()?;
                Ok(Self::Float(float))
            }
            DOUBLE_ID => {
                let double = reader.get_f64()?;
                Ok(Self::Double(double))
            }
            BYTE_ARRAY_ID => {
                let len = reader.get_i32()?;
                if len < 0 {
                    return Err(Error::NegativeLength(len));
                }

                let len = len as usize;
                if len > crate::MAX_ARRAY_LENGTH {
                    return Err(Error::LargeLength(len));
                }
                Ok(Self::ByteArray(reader.get_byte_array(len)?.into()))
            }
            STRING_ID => Ok(Self::String(reader.get_string()?.into_owned().into())),
            LIST_ID => {
                let tag_type_id = reader.get_u8()?;
                let len = reader.get_i32()?;
                if len < 0 {
                    return Err(Error::NegativeLength(len));
                }
                if tag_type_id == END_ID && len > 0 {
                    return Err(Error::InvalidListTag(tag_type_id));
                }

                let len = len as usize;
                if len > crate::MAX_ARRAY_LENGTH {
                    return Err(Error::LargeLength(len));
                }

                let mut list = Vec::with_capacity(len.min(4096));
                for _ in 0..len {
                    let tag = Self::deserialize_data_depth(reader, tag_type_id, depth + 1)?;
                    if tag.get_type_id() != tag_type_id {
                        return Err(Error::InvalidListTag(tag.get_type_id()));
                    }
                    // 尝试解包标签。
                    list.push(Self::flatten(tag));
                }
                Ok(Self::List(list))
            }
            COMPOUND_ID => Ok(Self::Compound(NbtCompound::deserialize_content_depth(
                reader,
                depth + 1,
            )?)),
            INT_ARRAY_ID => {
                let len = reader.get_i32()?;
                if len < 0 {
                    return Err(Error::NegativeLength(len));
                }

                let len = len as usize;
                if len > crate::MAX_ARRAY_LENGTH {
                    return Err(Error::LargeLength(len));
                }

                Ok(Self::IntArray(reader.get_i32_array(len)?))
            }
            LONG_ARRAY_ID => {
                let len = reader.get_i32()?;
                if len < 0 {
                    return Err(Error::NegativeLength(len));
                }

                let len = len as usize;
                if len > crate::MAX_ARRAY_LENGTH {
                    return Err(Error::LargeLength(len));
                }

                Ok(Self::LongArray(reader.get_i64_array(len)?))
            }
            _ => Err(Error::UnknownTagId(tag_id)),
        }
    }

    ///若这是字节标签，则返回其包含的字节。
    #[must_use]
    pub const fn extract_byte(&self) -> Option<i8> {
        match self {
            Self::Byte(byte) => Some(*byte),
            _ => None,
        }
    }

    ///若这是短整数标签，则返回其包含的短整数。
    #[must_use]
    pub const fn extract_short(&self) -> Option<i16> {
        match self {
            Self::Short(short) => Some(*short),
            _ => None,
        }
    }

    ///若这是整数标签，则返回其包含的整数。
    #[must_use]
    pub const fn extract_int(&self) -> Option<i32> {
        match self {
            Self::Int(int) => Some(*int),
            _ => None,
        }
    }

    ///若这是长整数标签，则返回其包含的长整数。
    #[must_use]
    pub const fn extract_long(&self) -> Option<i64> {
        match self {
            Self::Long(long) => Some(*long),
            _ => None,
        }
    }

    ///若这是浮点标签，则返回其包含的浮点数。
    #[must_use]
    pub const fn extract_float(&self) -> Option<f32> {
        match self {
            Self::Float(float) => Some(*float),
            _ => None,
        }
    }

    ///若这是双精度浮点标签，则返回其包含的双精度浮点数。
    #[must_use]
    pub const fn extract_double(&self) -> Option<f64> {
        match self {
            Self::Double(double) => Some(*double),
            _ => None,
        }
    }

    ///以布尔值形式返回其包含的字节，其中 0 表示 `false`。
    #[must_use]
    pub fn extract_bool(&self) -> Option<bool> {
        match self {
            Self::Byte(byte) => Some(byte != &0),
            _ => None,
        }
    }

    ///若这是字节数组标签，则返回其包含的字节数组。
    #[must_use]
    pub fn extract_byte_array(&self) -> Option<&[i8]> {
        match self {
            Self::ByteArray(byte_array) => Some(byte_array),
            _ => None,
        }
    }

    ///若这是字符串标签，则返回其包含的字符串。
    #[must_use]
    pub fn extract_string(&self) -> Option<&str> {
        match self {
            Self::String(string) => Some(string),
            _ => None,
        }
    }

    ///若这是列表标签，则返回其包含的列表。
    #[must_use]
    pub fn extract_list(&self) -> Option<&[Self]> {
        match self {
            Self::List(list) => Some(list),
            _ => None,
        }
    }

    ///若这是复合标签，则返回其包含的复合数据。
    #[must_use]
    pub const fn extract_compound(&self) -> Option<&NbtCompound> {
        match self {
            Self::Compound(compound) => Some(compound),
            _ => None,
        }
    }

    ///若这是整数数组标签，则返回其包含的整数数组。
    #[must_use]
    pub fn extract_int_array(&self) -> Option<&[i32]> {
        match self {
            Self::IntArray(int_array) => Some(int_array),
            _ => None,
        }
    }

    ///若这是长整数数组标签，则返回其包含的长整数数组。
    #[must_use]
    pub fn extract_long_array(&self) -> Option<&[i64]> {
        match self {
            Self::LongArray(long_array) => Some(long_array),
            _ => None,
        }
    }
}

impl From<&str> for NbtTag {
    fn from(value: &str) -> Self {
        Self::String(value.into())
    }
}

impl From<&[i8]> for NbtTag {
    fn from(value: &[i8]) -> Self {
        Self::ByteArray(value.into())
    }
}

impl From<f32> for NbtTag {
    fn from(value: f32) -> Self {
        Self::Float(value)
    }
}

impl From<f64> for NbtTag {
    fn from(value: f64) -> Self {
        Self::Double(value)
    }
}

impl From<bool> for NbtTag {
    fn from(value: bool) -> Self {
        Self::Byte(i8::from(value))
    }
}
