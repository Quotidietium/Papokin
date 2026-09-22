use std::{borrow::Cow, fmt::Display};

use papokin_codecs::{DataResult, FlatTryFrom, comap_flat_map_codec_impl};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 默认的 Minecraft 命名空间字符串（`"minecraft"`）。
pub const VANILLA_NAMESPACE: &str = "minecraft";
/// Pumpkin 服务器的命名空间字符串（`"pumpkin"`）。
pub const PUMPKIN_NAMESPACE: &str = "pumpkin";

/// 一个不可变的结构，用于标识特定资源，
/// 它可能是堆分配的。
/// 它们以 `<namespace>:<path>` 的形式表示。
///
/// 命名空间只能包含：
/// - 数字 `[0-9]`
/// - 小写字母 `[a-z]`
/// - 句点 `.`
/// - 下划线 `_`
/// - 连字符 `-`
///
/// 路径允许命名空间所允许的全部字符，但
/// 并额外允许正斜杠 `/`（路径分隔符）。
///
/// 若指定的标识符未带冒号和命名空间，
/// 即仅给出 `<path>`，则命名空间被假定为
/// 为 `minecraft`。
///
/// # Implementation Note
///
/// 命名空间和路径在内部是分开存储的
/// 以 `Cow<'static, str>` 形式。
#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct Identifier {
    namespace: Cow<'static, str>,
    path: Cow<'static, str>,
}

/// 表示一种因下述原因引发的错误：
/// 尝试创建/解析标识符时。
#[derive(Clone, Debug, Error)]
pub enum IdentifierError {
    /// 命名空间包含无效字符。
    #[error("Invalid character in namespace of identifier: {0}")]
    InvalidNamespace(Identifier),

    /// 路径包含无效字符。
    #[error("Invalid character in path of identifier: {0}")]
    InvalidPath(Identifier),
}

/// 表示一次创建 [`Identifier`] 尝试的结果。
pub type IdentifierCreationResult = Result<Identifier, IdentifierError>;

impl Identifier {
    /// 尝试通过同时指定命名空间和路径来创建新的 [`Identifier`]。
    pub fn new(
        namespace: impl Into<Cow<'static, str>>,
        path: impl Into<Cow<'static, str>>,
    ) -> IdentifierCreationResult {
        let namespace = namespace.into();
        let path = path.into();

        let identifier = Self { namespace, path };

        Self::validate_identifier(identifier)
    }

    /// 根据指定的命名空间和路径创建新的 [`Identifier`]
    /// 在编译期。
    ///
    /// 保证返回的标识符 **不会在堆上分配**。
    ///
    /// # Panics
    ///
    /// 如果提供的命名空间或提供的路径无效则 panic。
    #[must_use]
    pub const fn from_static(namespace: &'static str, path: &'static str) -> Self {
        assert!(
            Self::is_valid_namespace(namespace),
            "Invalid namespace provided"
        );
        assert!(Self::is_valid_path(path), "Invalid path provided");

        Self {
            namespace: Cow::Borrowed(namespace),
            path: Cow::Borrowed(path),
        }
    }

    /// 尝试从给定字符串解析出标识符。
    ///
    /// 返回的标识符会在堆上分配。
    pub fn parse(identifier: &str) -> IdentifierCreationResult {
        identifier.bytes().position(|b| b == b':').map_or_else(
            || Self::new(VANILLA_NAMESPACE, identifier.to_string()),
            |colon_i| {
                // 冒号存在。
                let path = identifier[colon_i + 1..].to_string();

                if colon_i == 0 {
                    Self::new(VANILLA_NAMESPACE, path)
                } else {
                    let namespace = identifier[0..colon_i].to_string();
                    Self::new(namespace, path)
                }
            },
        )
    }

    /// 尝试在编译期从给定字符串解析出标识符。
    ///
    /// 保证返回的标识符 **不会在堆上分配**。
    #[must_use]
    pub const fn parse_static(identifier: &'static str) -> Self {
        let bytes = identifier.as_bytes();
        let mut colon_i = 0;

        while colon_i < bytes.len() {
            if bytes[colon_i] == b':' {
                break;
            }
            colon_i += 1;
        }

        if colon_i < bytes.len() {
            // 冒号存在。
            // 我们被迫在 const 中使用 unsafe 代码
            // 因为 Index trait 不是 const 的。

            let path = unsafe {
                // SAFETY: 给定的 start 和 end 是有效的。
                // `colon_i` 位于 ':' 处，它是一个单字节 ASCII 字符 (0x3A)
                // 这意味着 colon_i + 1 是一个有效边界。
                // Rust 保证 `identifier` 是有效的 UTF-8 字符串。
                Self::slice_bytes_to_str_unchecked(bytes, colon_i + 1, bytes.len())
            };

            if colon_i == 0 {
                Self::from_static(VANILLA_NAMESPACE, path)
            } else {
                let namespace = unsafe {
                    // SAFETY: 给定的 start 和 end 是有效的。
                    // `colon_i` 位于 ':' 处，它是一个单字节 ASCII 字符 (0x3A)
                    // 这意味着 colon_i 是一个有效边界。
                    // Rust 保证 `identifier` 是有效的 UTF-8 字符串。
                    Self::slice_bytes_to_str_unchecked(bytes, 0, colon_i)
                };
                Self::from_static(namespace, path)
            }
        } else {
            Self::from_static(VANILLA_NAMESPACE, identifier)
        }
    }

    /// 在有效位置将字节切片为 `&str` 的不安全函数。
    /// 我们这样做是因为 `std::ops::Index` 尚未作为 const trait 稳定下来。
    ///
    /// `start` 和 `end` 以字节表示，充当 `bytes` 的索引。
    ///
    /// # Safety
    /// 若违反以下任一条件，即会发生未定义行为：
    /// - `start <= end <= bytes.len()`
    /// - `start` 和 `end` 都位于 UTF-8 字符边界上。
    /// - 子切片 `bytes[start..end]` 是有效的 UTF-8。
    #[must_use]
    const unsafe fn slice_bytes_to_str_unchecked(bytes: &[u8], start: usize, end: usize) -> &str {
        // SAFETY: 由 `slice_bytes_to_str_unchecked` 的安全前置条件保证：`start..end` 在边界内且位于有效的 UTF-8 边界上。
        unsafe {
            core::str::from_utf8_unchecked(core::slice::from_raw_parts(
                bytes.as_ptr().add(start),
                end - start,
            ))
        }
    }

    /// 尝试创建命名空间为 `minecraft` 的新 [`Identifier`]。
    pub fn vanilla(path: impl Into<Cow<'static, str>>) -> IdentifierCreationResult {
        Self::new(VANILLA_NAMESPACE, path)
    }

    /// 尝试在编译期创建命名空间为 `minecraft` 的新 [`Identifier`]。
    ///
    /// # Panics
    ///
    /// 如果提供的路径无效则 panic。
    #[must_use]
    pub const fn vanilla_static(path: &'static str) -> Self {
        Self::from_static(VANILLA_NAMESPACE, path)
    }

    /// 创建命名空间为 `pumpkin` 的新 [`Identifier`]。
    pub fn pumpkin(path: impl Into<Cow<'static, str>>) -> IdentifierCreationResult {
        Self::new(PUMPKIN_NAMESPACE, path)
    }

    /// 在编译期创建命名空间为 `pumpkin` 的新 [`Identifier`]。
    ///
    /// # Panics
    ///
    /// 如果提供的路径无效则 panic。
    #[must_use]
    pub const fn pumpkin_static(path: &'static str) -> Self {
        Self::from_static(PUMPKIN_NAMESPACE, path)
    }

    /// 消耗此标识符，将其当前路径替换为指定路径。
    pub fn with_path(self, path: impl Into<Cow<'static, str>>) -> IdentifierCreationResult {
        Self::validate_identifier_path(Self {
            namespace: self.namespace,
            path: path.into(),
        })
    }

    /// 消耗此标识符，为当前路径添加一个前缀。
    pub fn prefix_path(self, prefix: &str) -> IdentifierCreationResult {
        Self::validate_identifier_path(Self {
            namespace: self.namespace,
            path: format!("{prefix}{}", self.path).into(),
        })
    }

    /// 消耗此标识符，为当前路径添加一个后缀。
    pub fn suffix_path(self, suffix: &str) -> IdentifierCreationResult {
        Self::validate_identifier_path(Self {
            namespace: self.namespace,
            path: format!("{}{suffix}", self.path).into(),
        })
    }

    /// 消耗此标识符，通过给定函数映射其路径来创建一个新标识符。
    pub fn map_path<F>(self, f: F) -> IdentifierCreationResult
    where
        F: FnOnce(&str) -> String,
    {
        Self::validate_identifier_path(Self {
            namespace: self.namespace,
            path: f(&self.path).into(),
        })
    }

    /// 获取此 [`Identifier`] 的命名空间。
    #[must_use]
    #[inline]
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// 获取此 [`Identifier`] 的路径。
    #[must_use]
    #[inline]
    pub fn path(&self) -> &str {
        &self.path
    }

    fn validate_identifier(identifier: Self) -> IdentifierCreationResult {
        if !Self::is_valid_namespace(&identifier.namespace) {
            return Err(IdentifierError::InvalidNamespace(identifier));
        }
        if !Self::is_valid_path(&identifier.path) {
            return Err(IdentifierError::InvalidPath(identifier));
        }
        Ok(identifier)
    }

    fn validate_identifier_path(identifier: Self) -> IdentifierCreationResult {
        if !Self::is_valid_path(&identifier.path) {
            return Err(IdentifierError::InvalidPath(identifier));
        }
        Ok(identifier)
    }

    /// 返回给定命名空间在以下情况下是否有效：
    /// 用于标识符中。
    #[must_use]
    pub const fn is_valid_namespace(namespace: &str) -> bool {
        // 我们必须使用手动循环，以便该函数
        // 可标记为 `const` 函数。
        let bytes = namespace.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if !matches!(bytes[i], b'0'..=b'9' | b'a'..=b'z' | b'-' | b'_' | b'.') {
                return false;
            }
            i += 1;
        }
        true
    }

    /// 返回给定路径在以下情况下是否有效：
    /// 用于标识符中。
    #[must_use]
    pub const fn is_valid_path(path: &str) -> bool {
        // 我们必须使用手动循环，以便该函数
        // 可标记为 `const` 函数。
        let bytes = path.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if !matches!(bytes[i], b'0'..=b'9' | b'a'..=b'z' | b'-' | b'_' | b'.' | b'/') {
                return false;
            }
            i += 1;
        }
        true
    }

    /// 返回给定字符在标识符中是否可能有效。
    #[must_use]
    pub const fn is_valid_char(c: char) -> bool {
        matches!(c, '0'..='9' | 'a'..='z' | '-' | '_' | '.' | '/' | ':')
    }

    /// 获取……内部字符串的引用元组
    /// 命名空间和路径。
    #[must_use]
    pub fn view(&self) -> (&str, &str) {
        (&self.namespace, &self.path)
    }

    /// 返回此标识符是否为带 `minecraft:` 前缀的标识符。
    #[must_use]
    pub fn is_vanilla(&self) -> bool {
        self.namespace() == VANILLA_NAMESPACE
    }

    /// 返回此标识符是否为带 `pumpkin:` 前缀的标识符。
    #[must_use]
    pub fn is_pumpkin(&self) -> bool {
        self.namespace() == PUMPKIN_NAMESPACE
    }

    /// 如果此标识符带有 `minecraft:` 前缀，则返回
    /// 一个包含此标识符路径的 [`Some`]。否则，
    /// 则返回 [`None`]。
    #[must_use]
    pub fn is_vanilla_then(&self) -> Option<&str> {
        self.is_vanilla().then_some(&self.path)
    }

    /// 如果此标识符带有 `pumpkin:` 前缀，则返回
    /// 一个包含此标识符路径的 [`Some`]。否则，
    /// 则返回 [`None`]。
    #[must_use]
    pub fn is_pumpkin_then(&self) -> Option<&str> {
        self.is_pumpkin().then_some(&self.path)
    }
}

impl TryFrom<&str> for Identifier {
    type Error = IdentifierError;

    fn try_from(value: &str) -> IdentifierCreationResult {
        Self::parse(value)
    }
}

impl TryFrom<&String> for Identifier {
    type Error = IdentifierError;

    fn try_from(value: &String) -> IdentifierCreationResult {
        Self::parse(value)
    }
}

impl Display for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.namespace, self.path)
    }
}

impl Serialize for Identifier {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Identifier {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let identifier_string = String::deserialize(deserializer)?;
        Self::parse(&identifier_string).map_err(|error| serde::de::Error::custom(error.to_string()))
    }
}

comap_flat_map_codec_impl!(String => Identifier, Identifier::flat_try_from, ToString::to_string);

impl FlatTryFrom<String> for Identifier {
    fn flat_try_from(value: String) -> DataResult<Self> {
        Self::parse(&value).map_or_else(
            |_| DataResult::new_error(format!("Not a valid resource location: {value}")),
            DataResult::new_success,
        )
    }
}

#[cfg(test)]
mod test {
    use crate::identifier::{Identifier, IdentifierError};
    use papokin_codecs::json_ops::JsonOps;
    use papokin_codecs::{assert_decode, assert_encode_success};
    use serde_json::json;

    #[test]
    fn new() -> Result<(), IdentifierError> {
        assert_eq!(Identifier::new("abc", "def")?.to_string(), "abc:def");
        assert_eq!(Identifier::from_static("abc", "def").to_string(), "abc:def");

        Ok(())
    }

    #[test]
    fn parse() -> Result<(), IdentifierError> {
        assert_eq!(Identifier::parse("abc")?.to_string(), "minecraft:abc");
        assert_eq!(Identifier::parse("abc:def")?.to_string(), "abc:def");
        assert_eq!(Identifier::parse("abc:")?.to_string(), "abc:");
        assert_eq!(Identifier::parse(":def")?.to_string(), "minecraft:def");
        assert_eq!(Identifier::parse(":")?.to_string(), "minecraft:");

        assert_eq!(Identifier::parse_static("abc").to_string(), "minecraft:abc");
        assert_eq!(Identifier::parse_static("abc:def").to_string(), "abc:def");

        let _ = Identifier::parse("")?;
        let _ = Identifier::parse("abc:/4/5")?;
        let _ = Identifier::parse("a._b-c:/4_-/5.9")?;

        assert!(Identifier::parse("::").is_err());
        assert!(Identifier::parse("a:b:c").is_err());
        assert!(Identifier::parse("he/llo:bye").is_err());
        assert!(Identifier::parse("1234+567:89").is_err());

        Ok(())
    }

    #[test]
    fn codec() {
        assert_encode_success!(
            Identifier::from_static("abc", "def"),
            JsonOps,
            json!("abc:def")
        );
        assert_encode_success!(
            Identifier::from_static("", "no_namespace"),
            JsonOps,
            json!(":no_namespace")
        );
        assert_encode_success!(
            Identifier::vanilla_static("example"),
            JsonOps,
            json!("minecraft:example")
        );

        assert_decode!(Identifier, json!("abc:def"), JsonOps, is_success);
        assert_decode!(Identifier, json!("vanilla"), JsonOps, is_success);
        assert_decode!(Identifier, json!("2 + 3"), JsonOps, is_error);
        assert_decode!(Identifier, json!("a._b-c:/4_-/5.9"), JsonOps, is_success);
    }
}
