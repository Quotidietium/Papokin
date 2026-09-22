use papokin_util::text::TextComponent;

use crate::errors::error_types::AnyCommandErrorType;

/// 一个详细描述语法错误上下文（包括发生位置）的结构体。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSyntaxErrorContext {
    pub input: String,
    pub cursor: usize,
}

/// 表示一个能从自身提供命令语法错误上下文的对象。
pub trait ContextProvider {
    fn context(&self) -> CommandSyntaxErrorContext;
}

impl ContextProvider for CommandSyntaxErrorContext {
    fn context(&self) -> CommandSyntaxErrorContext {
        self.clone()
    }
}

/// 一个详细描述语法错误的结构体。
///
/// 尽管名称如此，此错误仍可能
/// 也可能在命令执行期间引起。
///
/// 不过，大多数 `CommandSyntaxError` 都是在
/// 解析，它们额外携带了关于
/// 它们在（命令中）出现的位置相对于
/// 命令执行期间抛出的 `CommandSyntaxError`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSyntaxError {
    pub error_type: &'static dyn AnyCommandErrorType,
    pub message: TextComponent,
    pub context: Option<CommandSyntaxErrorContext>,
}

impl CommandSyntaxError {
    /// 构造一个新的 [`CommandSyntaxError`]，不带任何错误上下文，
    /// 且仅含错误消息本身。
    ///
    /// 这意味着此错误不会向客户端打印上下文，
    /// 包含该字符串以及引发错误的位置。
    #[must_use]
    pub const fn create_without_context(
        error_type: &'static dyn AnyCommandErrorType,
        message: TextComponent,
    ) -> Self {
        Self {
            error_type,
            message,
            context: None,
        }
    }

    /// 构造一个新的 [`CommandSyntaxError`]，带有给定的错误上下文，
    /// 其中包含字符串以及导致错误的位置，
    /// 以及错误消息本身。
    #[must_use]
    pub fn create<C>(
        error_type: &'static dyn AnyCommandErrorType,
        message: TextComponent,
        context_provider: &C,
    ) -> Self
    where
        C: ContextProvider,
    {
        Self {
            error_type,
            message,
            context: Some(context_provider.context()),
        }
    }

    /// 返回此错误的类型是否与提供的类型相似。
    pub fn is(&self, error_type: &'static dyn AnyCommandErrorType) -> bool {
        self.error_type == error_type
    }
}
