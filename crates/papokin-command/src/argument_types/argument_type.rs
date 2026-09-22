use papokin_protocol::java::client::play::SuggestionProviders;

use crate::context::command_context::CommandContext;
use crate::source::{CommandSource, DummySource};
use crate::suggestion::suggestions::Suggestions;
use crate::suggestion::suggestions::SuggestionsBuilder;
use crate::{errors::command_syntax_error::CommandSyntaxError, string_reader::StringReader};
use std::any::Any;

pub type JavaClientArgumentType = papokin_protocol::java::client::play::ArgumentType;
pub type ParseWithSourceAnyResult = Result<Box<dyn Any + Send + Sync>, CommandSyntaxError>;

/// 表示解析特定类型 `Item` 的参数类型。
pub trait ArgumentType<S: CommandSource = DummySource>: Send + Sync {
    /// 此参数类型解析出的数据类型。
    type Item: Send + Sync;

    /// 使用 [`StringReader`] 解析一个 `T`。仅在没有源时才调用此方法。
    ///
    /// 应使用 `?` 运算符传播错误，它会
    /// 复刻 Brigadier 的异常处理行为。
    fn parse(&self, reader: &mut StringReader) -> Result<Self::Item, CommandSyntaxError>;

    /// 使用 [`StringReader`] 解析一个 `T`，
    /// 以及一个 `S` 类型的特定来源。
    ///
    /// 应使用 `?` 运算符传播错误，它会
    /// 复刻 Brigadier 的异常处理行为。
    fn parse_with_source(
        &self,
        reader: &mut StringReader,
        _source: &S,
    ) -> Result<Self::Item, CommandSyntaxError> {
        self.parse(reader)
    }

    /// 提供来自此参数类型的建议列表。
    #[must_use]
    fn list_suggestions(
        &self,
        _context: &CommandContext<S>,
        _builder: SuggestionsBuilder,
    ) -> Suggestions {
        Suggestions::empty()
    }

    ///返回此参数类型所使用的 Java 客户端侧解析器。
    #[must_use]
    fn client_side_parser(&self) -> JavaClientArgumentType;

    /// 如果存在包含建议提供器的 [`Some`]，则覆盖此参数提供的建议提供器
    /// 则返回。
    #[must_use]
    fn override_suggestion_providers(&self) -> Option<SuggestionProviders> {
        None
    }

    /// 获取被选中的示例列表，这些示例被认为
    /// 解析为类型 `T` 时有效。
    ///
    /// 用于冲突。
    #[must_use]
    fn examples(&self) -> Vec<String> {
        Vec::new()
    }
}

// 防止其他 crate 使用此 trait
// 因此，我们可以有效地“封闭”这个 trait，其本意
// 仅用于 `AnyArgumentType`。
mod sealed {
    /// 私有 trait，确保只有实现了 `ArgumentType` 的类型才能实现 `AnyArgumentType`。
    pub trait Sealed<S> {}
}

/// 表示具有任意可解析类型的参数类型。
pub trait AnyArgumentType<S: CommandSource = DummySource>: sealed::Sealed<S> + Send + Sync {
    /// 使用 [`StringReader`] 解析一个值。仅在没有源时才调用此方法。
    ///
    /// 应使用 `?` 运算符传播错误，它会
    /// 复刻 Brigadier 的异常处理行为。
    fn parse(
        &self,
        reader: &mut StringReader,
    ) -> Result<Box<dyn Any + Send + Sync>, CommandSyntaxError>;

    /// 使用 [`StringReader`] 并结合给定源解析一个值。
    ///
    /// 应使用 `?` 运算符传播错误，它会
    /// 复刻 Brigadier 的异常处理行为。
    fn parse_with_source(&self, reader: &mut StringReader, source: &S) -> ParseWithSourceAnyResult;

    /// 提供来自此参数类型的建议列表。
    #[must_use]
    fn list_suggestions(
        &self,
        context: &CommandContext<S>,
        builder: SuggestionsBuilder,
    ) -> Suggestions;

    ///返回此参数类型所使用的 Java 客户端侧解析器。
    #[must_use]
    fn client_side_parser(&self) -> JavaClientArgumentType;

    /// 如果存在包含建议提供器的 [`Some`]，则覆盖此参数提供的建议提供器
    /// 则返回。
    #[must_use]
    fn override_suggestion_providers(&self) -> Option<SuggestionProviders> {
        None
    }

    /// 获取被选中的示例列表，这些示例被认为
    /// 解析为类型 `T` 时有效。
    ///
    /// 用于冲突。
    #[must_use]
    fn examples(&self) -> Vec<String> {
        Vec::new()
    }

    fn as_any(&self) -> &dyn Any;
}

// 为所有参数类型实现我们的私有 trait。
impl<U: ArgumentType<S>, S: CommandSource> sealed::Sealed<S> for U {}

impl<U: ArgumentType<S> + 'static, S: CommandSource + 'static> AnyArgumentType<S> for U
where
    U::Item: 'static,
{
    fn parse(
        &self,
        reader: &mut StringReader,
    ) -> Result<Box<dyn Any + Send + Sync>, CommandSyntaxError> {
        match self.parse(reader) {
            Ok(value) => Ok(Box::new(value)),
            Err(error) => Err(error),
        }
    }

    fn parse_with_source(&self, reader: &mut StringReader, source: &S) -> ParseWithSourceAnyResult {
        match self.parse_with_source(reader, source) {
            Ok(value) => {
                let value: Box<dyn Any + Send + Sync> = Box::new(value);
                Ok(value)
            }
            Err(error) => Err(error),
        }
    }

    fn list_suggestions(
        &self,
        context: &CommandContext<S>,
        builder: SuggestionsBuilder,
    ) -> Suggestions {
        self.list_suggestions(context, builder)
    }

    fn client_side_parser(&self) -> JavaClientArgumentType {
        self.client_side_parser()
    }

    fn override_suggestion_providers(&self) -> Option<SuggestionProviders> {
        self.override_suggestion_providers()
    }

    fn examples(&self) -> Vec<String> {
        self.examples()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
