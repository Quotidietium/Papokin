pub mod attached;
pub mod detached;
pub mod dispatcher;
pub mod tree;

use crate::argument_types::argument_type::AnyArgumentType;
use crate::context::command_context::CommandContext;
use crate::errors::command_syntax_error::CommandSyntaxError;
use crate::node::attached::NodeId;
use crate::node::detached::GlobalNodeId;
use crate::source::{CommandSource, DummySource};
use crate::suggestion::provider::SuggestionProvider;
use std::borrow::Cow;
use std::sync::Arc;

/// 表示 [`CommandExecutor`] 的执行结果。
///
/// 如果命令**成功执行**，则返回包含 [`i32`] 的 [`Ok`]。
/// 这表示命令的“输出值”，与之 *同源* 的是
/// 原版命令执行器**成功时**返回的 `int`。
///
/// **在以下情况下应将成功结果选为 `1`**：
/// - 你不知道你的操作在成功时该用什么值
///   自有命令，或
/// - 你不理解这个值的含义，或者
/// - 你完全不关心这个值
///
/// 如果命令**失败**，则返回包含 [`CommandSyntaxError`] 的 [`Err`]
/// 导致此结果的原因。
pub type CommandExecutorResult = Result<i32, CommandSyntaxError>;

/// 实现此 trait 的结构体能够在给定的上下文中运行。
pub trait CommandExecutor<S: CommandSource = DummySource>: Sync + Send {
    /// 针对一条命令执行此执行器。
    fn execute(&self, context: &CommandContext<S>) -> CommandExecutorResult;
}

impl<F, S: CommandSource> CommandExecutor<S> for F
where
    F: Fn(&CommandContext<S>) -> CommandExecutorResult + Send + Sync,
{
    fn execute(&self, context: &CommandContext<S>) -> CommandExecutorResult {
        self(context)
    }
}

/// 一个接收上下文并返回命令结果的函数。
pub type Command<S = DummySource> = Arc<dyn CommandExecutor<S>>;

/// 表示来自 [`CommandContext`] 的 [`Arc<S>`] 的结果。
pub type RedirectModifierResult<S = DummySource> = Result<Vec<Arc<S>>, CommandSyntaxError>;

/// 一个执行所需修改的函数。
pub type RedirectModifierExecutor<S = DummySource> =
    dyn Fn(&CommandContext<S>) -> RedirectModifierResult<S> + Send + Sync;

/// 一个根据给定上下文返回新来源集合的函数。
#[derive(Clone)]
pub enum RedirectModifier<S: CommandSource = DummySource> {
    /// 始终仅返回给定上下文中的源。
    KeepSource,

    ///从一个上下文中通过给定方式返回多个 [`CommandSource`]
    /// 自定义行为。
    Custom(Arc<RedirectModifierExecutor<S>>),
}

impl<S: CommandSource> RedirectModifier<S> {
    /// 尝试从一个……提供 [`Vec`] 形式的 [`Arc<S>`]
    /// 给定的 [`CommandContext`]。
    pub fn sources(&self, command_context: &CommandContext<S>) -> RedirectModifierResult<S> {
        match self {
            Self::KeepSource => Ok(vec![command_context.source.clone()]),
            Self::Custom(function) => function(command_context),
        }
    }
}

/// 表示节点要求的结果。
pub type RequirementResult = bool;

/// 一个谓词，返回所提供的来源是否满足其条件。
#[derive(Clone)]
pub struct Requirement<S: CommandSource = DummySource>(
    pub Arc<dyn Fn(&S) -> RequirementResult + Send + Sync>,
);

impl<S: CommandSource> Requirement<S> {
    /// 评估给定的条件，返回条件是否成立
    /// 给定的 [`CommandSource`] 满足此要求。
    #[must_use]
    pub fn evaluate(&self, command_source: &S) -> RequirementResult {
        self.0(command_source)
    }
}

impl<F, S: CommandSource> From<F> for Requirement<S>
where
    F: Fn(&S) -> RequirementResult + Send + Sync + 'static,
{
    fn from(value: F) -> Self {
        Self(Arc::new(value))
    }
}

// 权限
impl<S: CommandSource> From<String> for Requirement<S> {
    fn from(value: String) -> Self {
        Self(Arc::new({
            let permission = Arc::new(value);

            move |source: &S| source.has_permission(&permission)
        }))
    }
}

impl<S: CommandSource> From<&'static str> for Requirement<S> {
    fn from(value: &'static str) -> Self {
        Self(Arc::new(move |source: &S| source.has_permission(value)))
    }
}

/// 一种结构，返回来源是否有足够的资格运行命令。
#[derive(Clone)]
pub struct Requirements<S: CommandSource = DummySource>(pub Vec<Requirement<S>>);

impl<S: CommandSource> Requirements<S> {
    /// 创建一个不含任何要求的新 `Requirements`。
    /// 如果被使用，求值时将始终返回 `true`。
    #[must_use]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// 评估给定的条件，返回条件是否成立
    /// 给定的 [`CommandSource`] 满足所有包含的要求。
    #[must_use]
    pub fn evaluate(&self, command_source: &S) -> RequirementResult {
        for predicate in &self.0 {
            if !predicate.evaluate(command_source) {
                return false;
            }
        }

        true
    }
}

impl<S: CommandSource> Default for Requirements<S> {
    fn default() -> Self {
        Self::new()
    }
}

/// 存储节点的公共自有数据。
#[derive(Clone)]
pub struct OwnedNodeData<S: CommandSource = DummySource> {
    pub global_id: GlobalNodeId,
    pub requirements: Requirements<S>,
    pub modifier: RedirectModifier<S>,
    pub forks: bool,
    pub command: Option<Command<S>>,
}

/// 表示存储字面量的节点的附加元数据。
#[derive(Clone)]
pub struct LiteralNodeMetadata {
    pub literal: Cow<'static, str>,
    pub literal_lowercase: String,
}

impl LiteralNodeMetadata {
    pub fn new(literal: impl Into<Cow<'static, str>>) -> Self {
        let literal = literal.into();
        Self {
            literal: literal.clone(),
            literal_lowercase: literal.to_lowercase(),
        }
    }
}

/// [`LiteralNodeMetadata`] 的一种特殊类型，包含
/// 以及该命令的描述。
#[derive(Clone)]
pub struct CommandNodeMetadata {
    pub literal: Cow<'static, str>,
    pub literal_lowercase: String,
    pub description: Cow<'static, str>,
    pub source: Option<String>,
}

impl CommandNodeMetadata {
    pub fn new(
        literal: impl Into<Cow<'static, str>>,
        description: impl Into<Cow<'static, str>>,
    ) -> Self {
        let literal = literal.into();
        Self {
            literal: literal.clone(),
            literal_lowercase: literal.to_lowercase(),
            description: description.into(),
            source: None,
        }
    }
}

/// 表示任意类型参数的附加元数据。
#[derive(Clone)]
pub struct ArgumentNodeMetadata<S: CommandSource = DummySource> {
    pub name: Cow<'static, str>,
    pub argument_type: Arc<dyn AnyArgumentType<S>>,
    pub suggestion_provider: Option<Arc<dyn SuggestionProvider<S>>>,
}

impl<S: CommandSource> ArgumentNodeMetadata<S> {
    pub fn new(
        name: impl Into<Cow<'static, str>>,
        argument_type: Arc<dyn AnyArgumentType<S>>,
        suggestion_provider: Option<Arc<dyn SuggestionProvider<S>>>,
    ) -> Self {
        Self {
            name: name.into(),
            argument_type,
            suggestion_provider,
        }
    }
}

/// 表示不同类型节点的附加元数据。可以是根节点、字面量节点、命令节点或参数节点。
pub enum NodeMetadata<S: CommandSource = DummySource> {
    /// 根节点的元数据。
    Root,

    /// 不作为命令开头的字面量节点的元数据。
    Literal(LiteralNodeMetadata),

    /// 作为命令开头的字面量节点的元数据。
    Command(CommandNodeMetadata),

    /// 参数节点的元数据。
    Argument(ArgumentNodeMetadata<S>),
}

/// 存储此重定向将指向的位置。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Redirection {
    /// 通向树的根节点。
    Root,

    /// 从其树内局部 ID 通向树中的一个节点。
    Local(NodeId),

    /// 从其全局 ID 通向树中的一个节点。
    Global(GlobalNodeId),
}

impl<T: Into<NodeId>> From<T> for Redirection {
    fn from(value: T) -> Self {
        Self::Local(value.into())
    }
}

impl From<GlobalNodeId> for Redirection {
    fn from(value: GlobalNodeId) -> Self {
        Self::Global(value)
    }
}
