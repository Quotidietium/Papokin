use crate::errors::command_syntax_error::CommandSyntaxError;
use crate::errors::error_types::{
    CommandErrorType, READER_INVALID_DOUBLE, READER_INVALID_FLOAT, READER_INVALID_INT,
};
use crate::string_reader::StringReader;
use papokin_data::translation;
use papokin_util::identifier::Identifier;
use papokin_util::math::bounds::{Bounds, DoubleBounds, FloatDegreeBounds, IntBounds};
use papokin_util::text::TextComponent;
use std::str::FromStr;

/// 创建由示例组成的 [`Vec<String>`]，取自
/// 给定的字符串字面量。
macro_rules! examples {
    ( $( $example:literal ),* ) => {
        vec! [
            $( $example.to_string(), )*
        ]
    };
}

// 使用 `StringReader` 进行断言的辅助方法：

/// 断言 `reader` 使用给定参数读取的结果（符合预期），
/// 用于解析的类型 `$argument_type` 的结果等于 `Ok($value)`。
/// 同时将读取器的游标重置回起始位置。
#[cfg(test)]
macro_rules! assert_parse_ok_reset {
    ($reader: expr, $argument_type: expr, $value: expr) => {{
        assert_eq!(
            $crate::argument_types::argument_type::ArgumentType::<crate::source::DummySource>::parse(
                &$argument_type,
                &mut $reader
            ),
            Ok($value)
        );
        $reader.set_cursor(0)
    }};
    ($reader: expr, $argument_type: expr) => {{
        assert!(
            $crate::argument_types::argument_type::ArgumentType::<crate::source::DummySource>::parse(
                &$argument_type,
                &mut $reader
            )
            .is_ok()
        );
        $reader.set_cursor(0)
    }};
}

/// 断言 `reader` 使用给定参数读取的结果（符合预期），
/// 用于解析的类型 `$argument_type` 的结果为一个包含错误类型 `$error_type` 的 `Err`。
/// 同时将读取器的游标重置回起始位置。
#[cfg(test)]
macro_rules! assert_parse_err_reset {
    ($reader: expr, $argument_type: expr, $error_type: expr) => {
        let error_type_dyn: &'static dyn crate::errors::error_types::AnyCommandErrorType =
            $error_type;
        assert_eq!(
            $crate::argument_types::argument_type::ArgumentType::<crate::source::DummySource>::parse(
                &$argument_type,
                &mut $reader
            )
            .map_err(|e| e.error_type),
            Err(error_type_dyn)
        );
        $reader.set_cursor(0)
    };
}

/// 为 `Item` 为 `Copy` 的参数类型实现单个 `get()` 函数的宏。
macro_rules! impl_copy_get {
    ($ty:ty, $item:ty) => {
        impl $ty {
            #[doc = concat!("Returns a `CommandContext`'s parsed `", stringify!($item), "` argument.")]
            pub fn get<S: $crate::source::CommandSource>(context: &$crate::context::command_context::CommandContext<S>, name: &str) -> Result<$item, CommandSyntaxError> {
                Ok(*context.get_argument(name)?)
            }
        }
    };
}

const EMPTY_BOUNDS_ERROR_TYPE: CommandErrorType<0> =
    CommandErrorType::new(translation::java::ARGUMENT_RANGE_EMPTY);
const SWAPPED_BOUNDS_ERROR_TYPE: CommandErrorType<0> =
    CommandErrorType::new(translation::java::ARGUMENT_RANGE_SWAPPED);
const INVALID_IDENTIFIER_ERROR_TYPE: CommandErrorType<0> =
    CommandErrorType::new(translation::java::ARGUMENT_ID_INVALID);

/// 一个用于尝试从 [`StringReader`] 获取值的 trait。
pub trait FromStringReader: Sized {
    /// 尝试从 [`StringReader`] 解析此类型的一个值。
    ///
    /// 如果失败，则返回包含出错详情的 [`CommandSyntaxError`]。
    fn from_reader(reader: &mut StringReader) -> Result<Self, CommandSyntaxError>;
}

fn bounds_from_reader<T: Copy + FromStr + PartialOrd>(
    reader: &mut StringReader,
    error_type: &'static CommandErrorType<1>,
) -> Result<Bounds<T>, CommandSyntaxError> {
    if reader.can_read_char() {
        let i = reader.cursor();
        try_bounds_from_reader(reader, error_type).map_err(|e| {
            // 捕获到任何错误时，将读取器的光标位置设置为范围的起始处。
            if let Some(mut context) = e.context {
                context.cursor = i;
                CommandSyntaxError::create(e.error_type, e.message, &context)
            } else {
                CommandSyntaxError::create_without_context(e.error_type, e.message)
            }
        })
    } else {
        Err(EMPTY_BOUNDS_ERROR_TYPE.create(reader))
    }
}

fn try_bounds_from_reader<T: Copy + FromStr + PartialOrd>(
    reader: &mut StringReader,
    error_type: &'static CommandErrorType<1>,
) -> Result<Bounds<T>, CommandSyntaxError> {
    let min = read_number_from_reader(reader, error_type)?;
    let max = if reader.peek() == Some('.') && reader.peek_with_offset(1) == Some('.') {
        reader.skip();
        reader.skip();
        read_number_from_reader(reader, error_type)?
    } else {
        min
    };
    if min.is_none() && max.is_none() {
        Err(EMPTY_BOUNDS_ERROR_TYPE.create(reader))
    } else {
        Ok(Bounds::<T>::new(min, max))
    }
}

/// 尝试从读取器读取指定类型的一个数字。
fn read_number_from_reader<T: FromStr>(
    reader: &mut StringReader,
    error_type: &'static CommandErrorType<1>,
) -> Result<Option<T>, CommandSyntaxError> {
    let i = reader.cursor();
    while has_allowed_peaked_character(reader) {
        reader.skip();
    }
    let string = &reader.string()[i..reader.cursor()].to_string();
    if string.is_empty() {
        Ok(None)
    } else {
        string.parse::<T>().map_or_else(
            |_| Err(error_type.create(reader, TextComponent::text(string.clone()))),
            |t| Ok(Some(t)),
        )
    }
}

fn has_allowed_peaked_character(reader: &StringReader) -> bool {
    let c = reader.peek();
    if matches!(c, Some('0'..='9' | '-')) {
        true
    } else {
        c.is_some_and(|c| {
            c == '.' && (!reader.can_read_chars(2) || reader.peek_with_offset(1) != Some('.'))
        })
    }
}

impl FromStringReader for IntBounds {
    fn from_reader(reader: &mut StringReader) -> Result<Self, CommandSyntaxError> {
        let i = reader.cursor();
        let bounds = bounds_from_reader(reader, &READER_INVALID_INT)?;
        if bounds.are_swapped() {
            reader.set_cursor(i);
            Err(SWAPPED_BOUNDS_ERROR_TYPE.create(reader))
        } else {
            Ok(Self::from_bounds(bounds))
        }
    }
}

impl FromStringReader for DoubleBounds {
    fn from_reader(reader: &mut StringReader) -> Result<Self, CommandSyntaxError> {
        let i = reader.cursor();
        let bounds = bounds_from_reader(reader, &READER_INVALID_DOUBLE)?;
        if bounds.are_swapped() {
            reader.set_cursor(i);
            Err(SWAPPED_BOUNDS_ERROR_TYPE.create(reader))
        } else {
            Ok(Self::from_bounds(bounds))
        }
    }
}

impl FromStringReader for FloatDegreeBounds {
    fn from_reader(reader: &mut StringReader) -> Result<Self, CommandSyntaxError> {
        let i = reader.cursor();
        let bounds = bounds_from_reader(reader, &READER_INVALID_FLOAT)?;
        if bounds.are_swapped() {
            reader.set_cursor(i);
            Err(SWAPPED_BOUNDS_ERROR_TYPE.create(reader))
        } else {
            Ok(Self::from_bounds(bounds))
        }
    }
}

impl FromStringReader for Identifier {
    fn from_reader(reader: &mut StringReader) -> Result<Self, CommandSyntaxError> {
        let start = reader.cursor();
        while let Some(c) = reader.peek()
            && Self::is_valid_char(c)
        {
            reader.skip();
        }

        let raw_identifier = &reader.string()[start..reader.cursor()];
        let identifier_result = Self::parse(raw_identifier);

        identifier_result.map_or_else(
            |_| {
                reader.set_cursor(start);
                Err(INVALID_IDENTIFIER_ERROR_TYPE.create(reader))
            },
            Ok,
        )
    }
}

pub mod argument_type;
pub mod attribute;
pub mod block;
pub mod block_predicate;
pub mod component;
pub mod coordinates;
pub mod core;
pub mod dialog;
pub mod entity_anchor;
pub mod gamemode;
pub mod hex_color;
pub mod identifier;
pub mod item;
pub mod item_predicate;
pub mod nbt;
pub mod nbt_path;
pub mod particle;
pub mod placed_feature;
pub mod range;
pub mod resource;
pub mod resource_or_tag;
pub mod slot;
pub mod sound_category;
pub mod structure;
pub mod team_color;
pub mod time;
pub mod uuid;
