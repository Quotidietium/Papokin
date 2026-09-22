use serde::de::{Error, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt::Formatter;

/// 表示单个物品或标签引用。
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum TagType {
    /// 由名称标识的单个物品。
    Item(String),
    /// 一个标签引用，通常以 `#` 为前缀。
    Tag(String),
}

impl TagType {
    /// 将标签类型序列化为字符串表示。
    ///
    /// # Returns
    /// - 对于 `Item`，返回物品名称。
    /// - 对于 `Tag`，返回带有 `#` 前缀的标签。
    #[must_use]
    pub fn serialize(&self) -> String {
        match self {
            Self::Item(name) => name.clone(),
            Self::Tag(tag) => format!("#{tag}"),
        }
    }
}

/// 用于从字符串反序列化 `TagType` 的访问者。
pub struct TagVisitor;

impl Visitor<'_> for TagVisitor {
    type Value = TagType;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "valid tag")
    }

    fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
        v.strip_prefix('#').map_or_else(
            || Ok(TagType::Item(v.to_string())),
            |v| Ok(TagType::Tag(v.to_string())),
        )
    }
}

impl<'de> Deserialize<'de> for TagType {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(TagVisitor)
    }
}

/// 表示单个标签类型或多个标签类型的列表。
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum RegistryEntryList {
    /// 单个标签类型。
    Single(TagType),
    /// 多种标签类型。
    Many(Vec<TagType>),
}

impl RegistryEntryList {
    /// 将注册表条目列表转换为扁平的 `TagType` 向量。
    ///
    /// # Returns
    /// 一个包含所有内含 `TagType` 值的向量。
    #[must_use]
    pub fn into_vec(self) -> Vec<TagType> {
        match self {
            Self::Single(s) => vec![s],
            Self::Many(s) => s,
        }
    }
}

impl PartialEq<TagType> for RegistryEntryList {
    fn eq(&self, other: &TagType) -> bool {
        match self {
            Self::Single(ingredient) => other == ingredient,
            Self::Many(ingredients) => ingredients.contains(other),
        }
    }
}

impl<'de> Deserialize<'de> for RegistryEntryList {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SlotTypeVisitor;
        impl<'de> Visitor<'de> for SlotTypeVisitor {
            type Value = RegistryEntryList;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                write!(formatter, "valid ingredient slot")
            }

            fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(RegistryEntryList::Single(TagVisitor.visit_str(v)?))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut ingredients: Vec<TagType> = vec![];
                while let Some(element) = seq.next_element()? {
                    ingredients.push(element);
                }
                if ingredients.len() == 1 {
                    Ok(RegistryEntryList::Single(ingredients[0].clone()))
                } else {
                    Ok(RegistryEntryList::Many(ingredients))
                }
            }
        }
        deserializer.deserialize_any(SlotTypeVisitor)
    }
}
