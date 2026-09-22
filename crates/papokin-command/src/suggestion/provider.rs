use crate::context::command_context::CommandContext;
use crate::source::{CommandSource, DummySource};
use crate::suggestion::suggestions::{Suggestions, SuggestionsBuilder};

/// 由 [`SuggestionProvider`] 给出的 [`Suggestions`]。
pub type SuggestionProviderResult = Suggestions;

/// 一个允许对象使用以下内容提供补全建议的 trait
/// [`CommandContext`] 和 [`SuggestionsBuilder`]。
pub trait SuggestionProvider<S: CommandSource = DummySource>: Send + Sync {
    /// 使用 [`CommandContext`] 与 [`SuggestionsBuilder`] 来提供建议。
    ///
    /// # Arguments
    /// - `context`：用于构建建议的上下文。
    /// - `builder`：用于生成建议并将被消费的构建器。
    ///
    /// # Returns
    /// 表示建议项的 [`Suggestions`]。
    fn suggest(
        &self,
        context: &CommandContext<S>,
        builder: SuggestionsBuilder,
    ) -> SuggestionProviderResult;
}
