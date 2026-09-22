use crate::{
    errors::error_types::CommandErrorType,
    snbt::rules::{EXPECTED_BINARY_NUMERAL, EXPECTED_DECIMAL_NUMERAL, EXPECTED_HEX_NUMERAL},
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Sign {
    Plus = 0,
    Minus = 1,
}

impl Sign {
    /// 返回表达此符号以供解析所需的最少字符数。
    ///
    /// 例如，在 `5.0` 和 `+5.0` 之间，前者无需为 `+` 符号占用空间，
    /// 而对于 `-5.0`，必须有一个 `-` 符号，因此那里的最小长度是 `1` 而非 `0`。
    #[must_use]
    #[inline]
    pub const fn minimum_size_parsable(self) -> usize {
        self as usize
    }

    /// 追加包含将此符号解析为 String 所需最少字符的切片
    /// 由给定可变引用所引用的值。
    ///
    /// 对于 [`Sign::Plus`] 这是空操作；而对于 [`Sign::Minus`]，则会追加一个 `-`。
    #[inline]
    pub fn append_minimum_str_parsable(self, buffer: &mut String) {
        if self == Self::Minus {
            buffer.push('-');
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SignedPrefix {
    None,
    Unsigned,
    Signed,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TypeSuffix {
    None,
    Byte,
    Short,
    Int,
    Long,
    Float,
    Double,
}

impl TypeSuffix {
    #[must_use]
    ///若此后缀是 [`TypeSuffix::None`]，则返回 `default` 后缀，否则返回自身
    /// 它会返回自身。
    pub fn or(self, default: Self) -> Self {
        if self == Self::None { default } else { self }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct IntegerSuffix(pub SignedPrefix, pub TypeSuffix);

impl IntegerSuffix {
    pub const EMPTY: Self = Self(SignedPrefix::None, TypeSuffix::None);
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Base {
    Binary,
    Decimal,
    Hexadecimal,
}

impl Base {
    #[must_use]
    pub const fn should_allow(self, c: char) -> bool {
        matches!(
            (self, c),
            (_, '_') |
            (Self::Binary, '0' | '1') |
            (Self::Decimal, '0'..='9') |
            (Self::Hexadecimal, '0'..='9' | 'A'..='F' | 'a'..='f')
        )
    }

    #[must_use]
    pub const fn no_value_error_type(self) -> &'static CommandErrorType<0> {
        match self {
            Self::Binary => &EXPECTED_BINARY_NUMERAL,
            Self::Decimal => &EXPECTED_DECIMAL_NUMERAL,
            Self::Hexadecimal => &EXPECTED_HEX_NUMERAL,
        }
    }

    #[must_use]
    pub const fn radix(self) -> u32 {
        match self {
            Self::Binary => 2,
            Self::Decimal => 10,
            Self::Hexadecimal => 16,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IntegerLiteral {
    pub sign: Sign,
    pub base: Base,
    pub digits: String,
    pub suffix: IntegerSuffix,
}

impl IntegerLiteral {
    pub const fn get_signed_prefix_or_default(&self) -> SignedPrefix {
        match (self.suffix.0, self.base) {
            (SignedPrefix::None, Base::Binary | Base::Hexadecimal) => SignedPrefix::Unsigned,
            (SignedPrefix::None, Base::Decimal) => SignedPrefix::Signed,
            (prefix, _) => prefix,
        }
    }
}

pub struct Signed<T> {
    pub sign: Sign,
    pub value: T,
}

/// 表示数组的显式前缀集合。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ArrayPrefix {
    Byte,
    Long,
    Int,
}
