use papokin_data::translation;

// 这些类似于 Minecraft 中可翻译的内建异常。
pub const READER_EXPECTED_START_QUOTE: CommandErrorType<0> =
    CommandErrorType::new(translation::java::PARSING_QUOTE_EXPECTED_START);
pub const READER_EXPECTED_END_QUOTE: CommandErrorType<0> =
    CommandErrorType::new(translation::java::PARSING_QUOTE_EXPECTED_END);
pub const READER_INVALID_ESCAPE: CommandErrorType<1> =
    CommandErrorType::new(translation::java::PARSING_QUOTE_ESCAPE);
pub const READER_INVALID_BOOL: CommandErrorType<1> =
    CommandErrorType::new(translation::java::PARSING_BOOL_INVALID);
pub const READER_EXPECTED_BOOL: CommandErrorType<0> =
    CommandErrorType::new(translation::java::PARSING_BOOL_EXPECTED);
pub const READER_INVALID_INT: CommandErrorType<1> =
    CommandErrorType::new(translation::java::PARSING_INT_INVALID);
pub const READER_EXPECTED_INT: CommandErrorType<0> =
    CommandErrorType::new(translation::java::PARSING_INT_EXPECTED);
pub const READER_INVALID_LONG: CommandErrorType<1> =
    CommandErrorType::new(translation::java::PARSING_LONG_INVALID);
pub const READER_EXPECTED_LONG: CommandErrorType<0> =
    CommandErrorType::new(translation::java::PARSING_LONG_EXPECTED);
pub const READER_INVALID_DOUBLE: CommandErrorType<1> =
    CommandErrorType::new(translation::java::PARSING_DOUBLE_INVALID);
pub const READER_EXPECTED_DOUBLE: CommandErrorType<0> =
    CommandErrorType::new(translation::java::PARSING_DOUBLE_EXPECTED);
pub const READER_INVALID_FLOAT: CommandErrorType<1> =
    CommandErrorType::new(translation::java::PARSING_FLOAT_INVALID);
pub const READER_EXPECTED_FLOAT: CommandErrorType<0> =
    CommandErrorType::new(translation::java::PARSING_FLOAT_EXPECTED);
pub const READER_EXPECTED_SYMBOL: CommandErrorType<1> =
    CommandErrorType::new(translation::java::PARSING_EXPECTED);

pub const LITERAL_INCORRECT: CommandErrorType<1> =
    CommandErrorType::new(translation::java::ARGUMENT_LITERAL_INCORRECT);

pub const DOUBLE_TOO_LOW: CommandErrorType<2> =
    CommandErrorType::new(translation::java::ARGUMENT_DOUBLE_LOW);
pub const DOUBLE_TOO_HIGH: CommandErrorType<2> =
    CommandErrorType::new(translation::java::ARGUMENT_DOUBLE_BIG);
pub const FLOAT_TOO_LOW: CommandErrorType<2> =
    CommandErrorType::new(translation::java::ARGUMENT_FLOAT_LOW);
pub const FLOAT_TOO_HIGH: CommandErrorType<2> =
    CommandErrorType::new(translation::java::ARGUMENT_FLOAT_BIG);
pub const INTEGER_TOO_LOW: CommandErrorType<2> =
    CommandErrorType::new(translation::java::ARGUMENT_INTEGER_LOW);
pub const INTEGER_TOO_HIGH: CommandErrorType<2> =
    CommandErrorType::new(translation::java::ARGUMENT_INTEGER_BIG);
pub const LONG_TOO_LOW: CommandErrorType<2> =
    CommandErrorType::new(translation::java::ARGUMENT_LONG_LOW);
pub const LONG_TOO_HIGH: CommandErrorType<2> =
    CommandErrorType::new(translation::java::ARGUMENT_LONG_BIG);

pub const DISPATCHER_UNKNOWN_COMMAND: CommandErrorType<1> =
    CommandErrorType::new(translation::java::COMMAND_UNKNOWN_COMMAND);
pub const DISPATCHER_UNKNOWN_ARGUMENT: CommandErrorType<0> =
    CommandErrorType::new(translation::java::COMMAND_UNKNOWN_ARGUMENT);
pub const DISPATCHER_EXPECTED_ARGUMENT_SEPARATOR: CommandErrorType<0> =
    CommandErrorType::new(translation::java::COMMAND_EXPECTED_SEPARATOR);
pub const DISPATCHER_PARSE_EXCEPTION: CommandErrorType<1> =
    CommandErrorType::new(translation::java::COMMAND_EXCEPTION);

use crate::errors::{
    command_syntax_error::{CommandSyntaxError, ContextProvider},
    error_types::sealed::Sealed,
};
use papokin_util::text::TextComponent;

/// 表示可用作模板的文本，该模板在
/// 编译期确定，运行时无法更改。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TemplateText {
    /// 先翻译此内容，然后代入参数（参数不是常量）。
    /// 当它需要被显示时。
    TranslationKey(&'static str),

    /// 直接展示给用户，不做任何翻译。
    Literal(&'static str),
}

/// 一种需要**恰好** `N` 个翻译参数的命令错误。
/// 此方法接受翻译键。若需要不可翻译的版本，
/// 请使用 [`LiteralCommandErrorType`]。
///
/// **与 Brigadier 的比较**：
/// - [`CommandErrorType<0>`] = `SimpleCommandExceptionType`
/// - [`CommandErrorType<1>`] = `DynamicCommandExceptionType`
/// - [`CommandErrorType<2>`] = `Dynamic2CommandExceptionType`
/// - [`CommandErrorType<3>`] = `Dynamic3CommandExceptionType`
/// - [`CommandErrorType<4>`] = `Dynamic4CommandExceptionType`
/// - `CommandErrorType<N>` = `DynamicNCommandExceptionType`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommandErrorType<const N: usize> {
    pub java_translation_key: &'static str,
}

impl<const N: usize> CommandErrorType<N> {
    /// 从给定的翻译字符串创建错误类型。
    #[must_use]
    pub const fn new(java_translation_key: &'static str) -> Self {
        Self {
            java_translation_key,
        }
    }

    /// 基于自身和 `N` 个参数的切片创建不带上下文的错误。
    #[must_use]
    pub fn create_without_context_args_slice(
        &'static self,
        args: &[TextComponent; N],
    ) -> CommandSyntaxError {
        CommandSyntaxError::create_without_context(
            self,
            TextComponent::translate(self.java_translation_key, args.to_vec()),
        )
    }

    /// 基于自身和 `N` 个参数的切片创建带上下文的错误。
    pub fn create_args_slice<C>(
        &'static self,
        context_provider: &C,
        args: &[TextComponent; N],
    ) -> CommandSyntaxError
    where
        C: ContextProvider,
    {
        CommandSyntaxError::create(
            self,
            TextComponent::translate(self.java_translation_key, args.to_vec()),
            context_provider,
        )
    }
}

/// 一种未经翻译且不能接受任何参数的命令错误。
/// 此方法接受常量字符串字面量。若需要可翻译的版本，
/// 请使用 [`CommandErrorType`]。
///
/// 只要可能，就应优先使用 [`CommandErrorType`] 而非本方式。
///
/// 用于自定义错误消息，它们不含任何
/// 原版中的翻译。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LiteralCommandErrorType {
    pub literal: &'static str,
}

impl LiteralCommandErrorType {
    /// 从给定的字面量字符串创建错误类型。
    ///
    /// 在命令解析期间使用（即在 `ArgumentType` 中）。
    #[must_use]
    pub const fn new(literal: &'static str) -> Self {
        Self { literal }
    }

    /// 基于自身创建不带上下文的错误。
    ///
    /// 在命令执行期间使用。
    #[must_use]
    pub fn create_without_context(&'static self) -> CommandSyntaxError {
        CommandSyntaxError::create_without_context(self, TextComponent::text(self.literal))
    }

    /// 基于自身创建带上下文的错误。
    pub fn create<C>(&'static self, context_provider: &C) -> CommandSyntaxError
    where
        C: ContextProvider,
    {
        CommandSyntaxError::create(self, TextComponent::text(self.literal), context_provider)
    }
}

// 防止其他 crate 使用此 trait
// 因此，我们可以有效地“封闭”这个 trait，其本意
// 仅用于 `CommandErrorType<N>`。
mod sealed {
    /// 私有 trait，确保只有 `CommandErrorType<N>` 能实现 `AnyCommandErrorType`。
    pub trait Sealed {}
}

/// 一个仅由 [`CommandErrorType<N>`] 类型实现的 trait。
///
/// 它暴露了此类类型的公共属性，同时使
/// 它可以更动态地访问其属性，例如翻译
/// 键以及参数数量（在运行时）。
pub trait AnyCommandErrorType: Sealed + std::fmt::Debug + Send + Sync {
    /// 返回此具体错误类型所对应的底层翻译键。
    fn text(&self) -> TemplateText;

    /// 返回此错误类型支持的参数数量。
    fn argument_count(&self) -> usize;
}

impl Eq for dyn AnyCommandErrorType {}

impl PartialEq for dyn AnyCommandErrorType {
    fn eq(&self, other: &Self) -> bool {
        self.text() == other.text() && self.argument_count() == other.argument_count()
    }
}

impl<T: AnyCommandErrorType> PartialEq<T> for dyn AnyCommandErrorType {
    fn eq(&self, other: &T) -> bool {
        self.text() == other.text() && self.argument_count() == other.argument_count()
    }
}

// 为我们的类型实现私有 trait。
impl<const N: usize> Sealed for CommandErrorType<N> {}
impl<const N: usize> AnyCommandErrorType for CommandErrorType<N> {
    fn text(&self) -> TemplateText {
        TemplateText::TranslationKey(self.java_translation_key)
    }

    fn argument_count(&self) -> usize {
        N
    }
}

impl Sealed for LiteralCommandErrorType {}
impl AnyCommandErrorType for LiteralCommandErrorType {
    fn text(&self) -> TemplateText {
        TemplateText::Literal(self.literal)
    }

    fn argument_count(&self) -> usize {
        0
    }
}

// 易用性实现：

/// 为 `CommandErrorType<N>` 生成特定实现，带有两个用于创建的方法
/// 一个错误而不使用切片。它们改为接受特定数量的
/// 参数。每个 `impl` 都被赋予一个特定的 `N` 值和一些参数名。
macro_rules! error_type_no_arg_slice_impl {
    (N = $N: literal => $($arg:ident),* | $doc: expr) => {
        impl CommandErrorType<$N> {
            /// 基于此错误类型创建不带上下文的错误
            #[doc = $doc]
            #[must_use]
            pub fn create_without_context(
                &'static self,
                $($arg: TextComponent),*
            ) -> CommandSyntaxError {
                self.create_without_context_args_slice(&[
                    $($arg),*
                ])
            }

            /// 基于此错误类型创建带上下文的错误
            #[doc = $doc]
            #[must_use]
            pub fn create<C>(
                &'static self,
                context_provider: &C,
                $($arg: TextComponent),*
            ) -> CommandSyntaxError
            where
                C: ContextProvider,
            {
                self.create_args_slice(context_provider, &[
                    $($arg),*
                ])
            }
        }
    };
}

// 实现定义如下：

error_type_no_arg_slice_impl!(N = 0 =>                        | "without taking any translation arguments.");
error_type_no_arg_slice_impl!(N = 1 => arg1                   | "by taking 1 translation argument.");
error_type_no_arg_slice_impl!(N = 2 => arg1, arg2             | "by taking 2 translation arguments.");
error_type_no_arg_slice_impl!(N = 3 => arg1, arg2, arg3       | "by taking 3 translation arguments.");
error_type_no_arg_slice_impl!(N = 4 => arg1, arg2, arg3, arg4 | "by taking 4 translation arguments.");

#[cfg(test)]
mod test {
    use crate::errors::error_types::{CommandErrorType, LiteralCommandErrorType};
    use crate::string_reader::StringReader;
    use papokin_util::text::TextComponent;

    const TEST_LITERAL_ERROR_TYPE: LiteralCommandErrorType =
        LiteralCommandErrorType::new("Test error");
    const TEST_TRANSLATABLE_ERROR_TYPE: CommandErrorType<1> =
        CommandErrorType::new("this.key.is.arbitrary");

    #[test]
    fn create_literal_error() {
        let mut reader = StringReader::new("foo bar");
        reader.set_cursor(4);

        let error = TEST_LITERAL_ERROR_TYPE.create(&reader);

        assert_eq!(error.error_type, &TEST_LITERAL_ERROR_TYPE);
        assert_eq!(error.message, TextComponent::text("Test error"));

        match &error.context {
            Some(context) => {
                assert_eq!(context.cursor, 4);
                assert_eq!(context.input, "foo bar");
            }
            None => panic!("错误应当带有上下文"),
        }
    }

    #[test]
    fn create_literal_error_without_context() {
        let error = TEST_LITERAL_ERROR_TYPE.create_without_context();

        assert_eq!(error.error_type, &TEST_LITERAL_ERROR_TYPE);
        assert_eq!(error.message, TextComponent::text("Test error"));
        assert_eq!(error.context, None);
    }

    #[test]
    fn create_translatable_error() {
        let mut reader = StringReader::new("foo bar");
        reader.set_cursor(4);

        let error = TEST_TRANSLATABLE_ERROR_TYPE.create(&reader, TextComponent::text("某个参数"));

        assert_eq!(error.error_type, &TEST_TRANSLATABLE_ERROR_TYPE);
        assert_eq!(
            error.message,
            TextComponent::translate("this.key.is.arbitrary", [TextComponent::text("某个参数")])
        );

        match &error.context {
            Some(context) => {
                assert_eq!(context.cursor, 4);
                assert_eq!(context.input, "foo bar");
            }
            None => panic!("错误应当带有上下文"),
        }
    }
}
