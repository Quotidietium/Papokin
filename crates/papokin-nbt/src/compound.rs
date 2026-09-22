//! NBT 复合标签的存储与便捷方法。

use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use uuid::Uuid;

use crate::deserializer::NbtReadHelper;
use crate::serializer::NbtWriteHelper;
use crate::tag::NbtTag;
use crate::{END_ID, Error, Nbt};
use std::collections::hash_map::IntoIter;
use std::io::ErrorKind;

#[macro_export]
/// 从键值对创建 [`NbtTag::Compound`](crate::tag::NbtTag::Compound)。
///
/// 该宏还接受空调用以创建空的复合标签。
macro_rules! nbt_compound_tag {
    { $($key:literal : $tag:expr),+ $(,)* } => {
        {
            let mut compound = NbtCompound::new();
            $( compound.put($key, $tag); )+
            NbtTag::Compound(compound)
        }
    };
    // 用于空复合标签
    {} => {
        NbtTag::Compound(NbtCompound::new())
    };
}

/// 表示 NBT 复合标签，实质上是一个哈希映射。
///
/// 内部使用 `HashMap<String, NbtTag>`，它不保留插入顺序，
/// 与 Minecraft: Java Edition 一样，但这确实意味着查找复杂度为 O(1)。
///
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NbtCompound {
    /// 化合物中的标签，按其名称索引。
    pub child_tags: HashMap<Box<str>, NbtTag>,
}

impl NbtCompound {
    /// 创建一个空的复合标签。
    #[must_use]
    pub fn new() -> Self {
        Self {
            child_tags: HashMap::new(),
        }
    }

    /// 使读取器跳过复合标签的有效载荷，而不分配其标签。
    pub fn skip_content<'a, R: NbtReadHelper<'a>>(reader: &mut R) -> Result<(), Error> {
        Self::skip_content_depth(reader, 0)
    }

    /// 使读取器跳过复合标签的有效载荷，并进行深度跟踪。
    pub fn skip_content_depth<'a, R: NbtReadHelper<'a>>(
        reader: &mut R,
        depth: usize,
    ) -> Result<(), Error> {
        if depth > crate::MAX_NBT_DEPTH {
            return Err(Error::MaxDepthExceeded);
        }

        loop {
            let tag_id = match reader.get_u8() {
                Ok(id) => id,
                Err(Error::Incomplete(e)) if e.kind() == ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            };

            if tag_id == END_ID {
                break;
            }

            reader.skip_string()?;

            // 跳过该值
            NbtTag::skip_data_depth(reader, tag_id, depth + 1)?;
        }

        Ok(())
    }

    /// 反序列化复合负载，从复合标签的名称之后开始。
    pub fn deserialize_content<'a, R: NbtReadHelper<'a>>(reader: &mut R) -> Result<Self, Error> {
        Self::deserialize_content_depth(reader, 0)
    }

    /// 反序列化带深度跟踪的复合负载。
    pub fn deserialize_content_depth<'a, R: NbtReadHelper<'a>>(
        reader: &mut R,
        depth: usize,
    ) -> Result<Self, Error> {
        if depth > crate::MAX_NBT_DEPTH {
            return Err(Error::MaxDepthExceeded);
        }

        let mut compound = Self::new();

        loop {
            let tag_id = match reader.get_u8() {
                Ok(id) => id,
                Err(Error::Incomplete(e)) if e.kind() == ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            };

            if tag_id == END_ID {
                break;
            }

            let name = reader.get_string()?;
            let tag = NbtTag::deserialize_data_depth(reader, tag_id, depth + 1)?;

            compound.child_tags.insert(name.into(), tag);
        }

        Ok(compound)
    }

    /// 序列化复合标签的各个条目，随后写入结束标签。
    pub fn serialize_content<W: NbtWriteHelper>(self, w: &mut W) -> Result<(), Error> {
        for (name, tag) in self.child_tags {
            w.write_u8(tag.get_type_id())?;
            w.write_string(&name)?;
            tag.serialize_data(w)?;
        }
        w.write_u8(END_ID)?;
        Ok(())
    }

    ///当复合标签不包含任何子标签时，返回 `true`。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.child_tags.is_empty()
    }

    /// 为 `name` 插入或替换标签。
    pub fn put(&mut self, name: &str, value: impl Into<NbtTag>) {
        self.child_tags.insert(name.into(), value.into());
    }

    /// 当 `name` 尚不存在时，插入字符串标签。
    pub fn put_string(&mut self, name: &str, value: String) {
        self.put(name, NbtTag::String(value.into()));
    }

    /// 当 `name` 尚不存在时，插入列表标签。
    pub fn put_list(&mut self, name: &str, value: Vec<NbtTag>) {
        self.put(name, NbtTag::List(value));
    }

    /// 当 `name` 尚不存在时，插入字节标签。
    pub fn put_byte(&mut self, name: &str, value: i8) {
        self.put(name, NbtTag::Byte(value));
    }

    /// 当 `name` 尚不存在时，插入以字节标签编码的布尔值。
    pub fn put_bool(&mut self, name: &str, value: bool) {
        self.put(name, NbtTag::Byte(i8::from(value)));
    }

    /// 当 `name` 尚不存在时，插入短整型标签。
    pub fn put_short(&mut self, name: &str, value: i16) {
        self.put(name, NbtTag::Short(value));
    }

    /// 当 `name` 尚不存在时，插入整型标签。
    pub fn put_int(&mut self, name: &str, value: i32) {
        self.put(name, NbtTag::Int(value));
    }
    /// 当 `name` 尚不存在时，插入长整型标签。
    pub fn put_long(&mut self, name: &str, value: i64) {
        self.put(name, NbtTag::Long(value));
    }

    /// 当 `name` 尚不存在时，插入单精度浮点标签。
    pub fn put_float(&mut self, name: &str, value: f32) {
        self.put(name, NbtTag::Float(value));
    }

    /// 当 `name` 尚不存在时，插入双精度浮点标签。
    pub fn put_double(&mut self, name: &str, value: f64) {
        self.put(name, NbtTag::Double(value));
    }

    /// 当 `name` 尚不存在时，插入复合标签。
    pub fn put_compound(&mut self, name: &str, value: Self) {
        self.put(name, NbtTag::Compound(value));
    }

    /// 将 UUID 存储为 4 元素整数数组，最高有效位在前
    /// (即原版 `UUID` 布局)。
    pub fn put_uuid(&mut self, name: &str, value: Uuid) {
        let value = value.as_u128();
        self.put(
            name,
            NbtTag::IntArray(vec![
                (value >> 96) as i32,
                ((value >> 64) & 0xFFFF_FFFF) as i32,
                ((value >> 32) & 0xFFFF_FFFF) as i32,
                (value & 0xFFFF_FFFF) as i32,
            ]),
        );
    }

    /// 返回指定名称的字节值；若标签不存在或类型不符，则返回 `None`。
    #[must_use]
    pub fn get_byte(&self, name: &str) -> Option<i8> {
        self.get(name).and_then(super::tag::NbtTag::extract_byte)
    }

    /// 返回指定名称的标签。
    #[inline]
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&NbtTag> {
        self.child_tags.get(name)
    }

    /// 返回复合标签是否包含 `name`。
    #[inline]
    #[must_use]
    pub fn has(&self, name: &str) -> bool {
        self.child_tags.contains_key(name)
    }

    /// 返回指定名称的短整数值；若标签不存在或类型不符，则返回 `None`。
    #[must_use]
    pub fn get_short(&self, name: &str) -> Option<i16> {
        self.get(name).and_then(super::tag::NbtTag::extract_short)
    }

    /// 返回指定名称的整数值；若标签不存在或类型不符，则返回 `None`。
    #[must_use]
    pub fn get_int(&self, name: &str) -> Option<i32> {
        self.get(name).and_then(super::tag::NbtTag::extract_int)
    }

    /// 返回指定名称的长整数值；若标签不存在或类型不符，则返回 `None`。
    #[must_use]
    pub fn get_long(&self, name: &str) -> Option<i64> {
        self.get(name).and_then(super::tag::NbtTag::extract_long)
    }

    /// 返回指定名称的单精度浮点值；若标签不存在或类型不符，则返回 `None`。
    #[must_use]
    pub fn get_float(&self, name: &str) -> Option<f32> {
        self.get(name).and_then(super::tag::NbtTag::extract_float)
    }

    /// 返回指定名称的双精度浮点值；若标签不存在或类型不符，则返回 `None`。
    #[must_use]
    pub fn get_double(&self, name: &str) -> Option<f64> {
        self.get(name).and_then(super::tag::NbtTag::extract_double)
    }

    /// 返回指定名称的字节并作为布尔值处理，零表示 `false`。
    #[must_use]
    pub fn get_bool(&self, name: &str) -> Option<bool> {
        self.get(name).and_then(super::tag::NbtTag::extract_bool)
    }

    /// 返回指定名称的字符串；若标签不存在或类型不符，则返回 `None`。
    #[must_use]
    pub fn get_string(&self, name: &str) -> Option<&str> {
        self.get(name).and_then(|tag| tag.extract_string())
    }

    /// 返回指定名称的列表；若标签不存在或类型不符，则返回 `None`。
    #[must_use]
    pub fn get_list(&self, name: &str) -> Option<&[NbtTag]> {
        self.get(name).and_then(|tag| tag.extract_list())
    }

    /// 返回指定名称的复合标签；若标签不存在或类型不符，则返回 `None`。
    #[must_use]
    pub fn get_compound(&self, name: &str) -> Option<&Self> {
        self.get(name).and_then(|tag| tag.extract_compound())
    }

    /// 返回指定名称的字节数组；若标签不存在或类型不符，则返回 `None`。
    #[must_use]
    pub fn get_byte_array(&self, name: &str) -> Option<&[i8]> {
        self.get(name).and_then(|tag| tag.extract_byte_array())
    }

    /// 返回指定名称的整数数组；若标签不存在或类型不符，则返回 `None`。
    #[must_use]
    pub fn get_int_array(&self, name: &str) -> Option<&[i32]> {
        self.get(name).and_then(|tag| tag.extract_int_array())
    }

    /// 返回指定名称的长整数数组；若标签不存在或类型不符，则返回 `None`。
    #[must_use]
    pub fn get_long_array(&self, name: &str) -> Option<&[i64]> {
        self.get(name).and_then(|tag| tag.extract_long_array())
    }

    /// 读取以 4 元素整型数组存储的 UUID，最高有效位
    /// 首先读取（由 [`Self::put_uuid`] 写入的原版 `UUID` 布局）。
    #[must_use]
    pub fn get_uuid(&self, name: &str) -> Option<Uuid> {
        let &[a, b, c, d] = self.get_int_array(name)? else {
            return None;
        };
        Some(Uuid::from_u128(
            ((a as u32 as u128) << 96)
                | ((b as u32 as u128) << 64)
                | ((c as u32 as u128) << 32)
                | (d as u32 as u128),
        ))
    }
}

impl From<Nbt> for NbtCompound {
    fn from(value: Nbt) -> Self {
        value.root_tag
    }
}

impl FromIterator<(String, NbtTag)> for NbtCompound {
    fn from_iter<T: IntoIterator<Item = (String, NbtTag)>>(iter: T) -> Self {
        let mut compound = Self::new();
        for (key, value) in iter {
            compound.put(&key, value);
        }
        compound
    }
}

impl FromIterator<(Box<str>, NbtTag)> for NbtCompound {
    fn from_iter<T: IntoIterator<Item = (Box<str>, NbtTag)>>(iter: T) -> Self {
        let mut compound = Self::new();
        for (key, value) in iter {
            compound.child_tags.insert(key, value);
        }
        compound
    }
}

impl IntoIterator for NbtCompound {
    type Item = (Box<str>, NbtTag);
    type IntoIter = IntoIter<Box<str>, NbtTag>;

    fn into_iter(self) -> Self::IntoIter {
        self.child_tags.into_iter()
    }
}

impl Extend<(String, NbtTag)> for NbtCompound {
    fn extend<T: IntoIterator<Item = (String, NbtTag)>>(&mut self, iter: T) {
        for (key, value) in iter {
            self.put(&key, value);
        }
    }
}

impl Extend<(Box<str>, NbtTag)> for NbtCompound {
    fn extend<T: IntoIterator<Item = (Box<str>, NbtTag)>>(&mut self, iter: T) {
        self.child_tags.extend(iter);
    }
}

// Rust 的 AsRef 目前不具备自反性，因此我们需要手动实现它
impl AsRef<Self> for NbtCompound {
    fn as_ref(&self) -> &Self {
        self
    }
}

impl From<NbtCompound> for NbtTag {
    fn from(value: NbtCompound) -> Self {
        Self::Compound(value)
    }
}

/// `NbtCompound` 的 SNBT 展示实现
impl Display for NbtCompound {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("{")?;
        for (i, (key, value)) in self.child_tags.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{key}: {value}")?;
        }
        f.write_str("}")
    }
}

impl Display for NbtTag {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::End => Ok(()),
            Self::Byte(v) => write!(f, "{v}b"),
            Self::Short(v) => write!(f, "{v}s"),
            Self::Int(v) => write!(f, "{v}"),
            Self::Long(v) => write!(f, "{v}L"),
            Self::Float(v) => write!(f, "{v}f"),
            Self::Double(v) => write!(f, "{v}d"),
            Self::String(v) => write!(f, "\"{v}\""), // TODO: 需要正确的转义才能保证 SNBT 的健壮性
            Self::Compound(v) => write!(f, "{v}"),
            Self::ByteArray(v) => {
                f.write_str("[B;")?;
                for (i, byte) in v.iter().enumerate() {
                    if i > 0 {
                        f.write_str(",")?;
                    }
                    write!(f, " {byte}b")?;
                }
                f.write_str("]")
            }
            Self::List(v) => {
                f.write_str("[")?;
                for (i, tag) in v.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{tag}")?;
                }
                f.write_str("]")
            }
            Self::IntArray(v) => {
                f.write_str("[I;")?;
                for (i, int) in v.iter().enumerate() {
                    if i > 0 {
                        f.write_str(",")?;
                    }
                    write!(f, " {int}")?;
                }
                f.write_str("]")
            }
            Self::LongArray(v) => {
                f.write_str("[L;")?;
                for (i, long) in v.iter().enumerate() {
                    if i > 0 {
                        f.write_str(",")?;
                    }
                    write!(f, " {long}L")?;
                }
                f.write_str("]")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NbtCompound;
    use uuid::Uuid;

    #[test]
    fn uuid_int_array_round_trip() {
        let original = Uuid::from_u128(0x0123_4567_89ab_cdef_fedc_ba98_7654_3210);
        let mut nbt = NbtCompound::new();
        nbt.put_uuid("UUID", original);
        assert_eq!(nbt.get_uuid("UUID"), Some(original));

        // 缺失或格式错误的条目回退为 None。
        assert_eq!(nbt.get_uuid("missing"), None);
        let mut short = NbtCompound::new();
        short.put("UUID", crate::tag::NbtTag::IntArray(vec![1, 2, 3]));
        assert_eq!(short.get_uuid("UUID"), None);
    }
}
