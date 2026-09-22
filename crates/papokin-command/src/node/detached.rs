use crate::argument_types::argument_type::AnyArgumentType;
use crate::node::{
    ArgumentNodeMetadata, Command, CommandNodeMetadata, LiteralNodeMetadata, NodeMetadata,
    OwnedNodeData, RedirectModifier, Redirection, Requirements,
};
use crate::source::{CommandSource, DummySource};
use crate::suggestion::provider::SuggestionProvider;
use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::num::NonZero;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DETACHED_NODE_ID: AtomicU64 = AtomicU64::new(1);

/// 表示一个**全局**的整数编号，对应
/// 任何在运行时唯一的节点类型。
///
/// 这对于未绑定到具体对象的节点很重要
/// 树。
#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug)]
pub struct GlobalNodeId(pub NonZero<u64>);

impl GlobalNodeId {
    /// 生成一个保证唯一的 ID
    /// 通过使用原子操作在运行时保证唯一。
    pub fn new() -> Self {
        Self(
            NonZero::new(NEXT_DETACHED_NODE_ID.fetch_add(1, Ordering::Relaxed))
                .expect("预期为非零 ID"),
        )
    }
}

impl Default for GlobalNodeId {
    fn default() -> Self {
        Self::new()
    }
}

/// 表示一个非命令的字面量节点，尚未挂载
/// 转换为一棵树。
///
/// 如果想以该节点开始命令，请改用 [`CommandDetachedNode`]。
///
/// 必须在之后附加到某棵树上才能发挥作用。
#[derive(Clone)]
pub struct LiteralDetachedNode<S: CommandSource = DummySource> {
    pub owned: OwnedNodeData<S>,
    pub children: FxHashMap<String, DetachedNode<S>>,
    pub redirect: Option<Redirection>,
    pub meta: LiteralNodeMetadata,
}

impl<S: CommandSource> LiteralDetachedNode<S> {
    /// 根据属性创建独立的字面量节点，
    /// 没有任何子节点。
    ///
    /// # Note
    /// 建议使用 [`literal`](crate::argument_builder::literal) 函数而不是此函数。
    pub fn new(
        global_id: GlobalNodeId,
        literal: impl Into<Cow<'static, str>>,
        command: Option<Command<S>>,
        requirements: Requirements<S>,
        redirect: Option<Redirection>,
        modifier: RedirectModifier<S>,
        forks: bool,
    ) -> Self {
        Self {
            owned: OwnedNodeData {
                global_id,
                requirements,
                modifier,
                forks,
                command,
            },
            children: FxHashMap::default(),
            redirect,
            meta: LiteralNodeMetadata::new(literal),
        }
    }
}

/// 表示一个字面量命令节点，尚未挂载
/// 转换为一棵树。
///
/// 如果不想以该节点开始命令，请改用 [`LiteralDetachedNode`]。
///
/// 必须在之后附加到某棵树上才能发挥作用。
#[derive(Clone)]
pub struct CommandDetachedNode<S: CommandSource = DummySource> {
    pub owned: OwnedNodeData<S>,
    pub children: FxHashMap<String, DetachedNode<S>>,
    pub redirect: Option<Redirection>,
    pub meta: CommandNodeMetadata,
}

impl<S: CommandSource> CommandDetachedNode<S> {
    /// 根据属性创建独立的字面量节点，
    /// 没有任何子节点。
    ///
    /// # Note
    /// 建议使用 [`command`](crate::argument_builder::command) 函数而不是此函数。
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        global_id: GlobalNodeId,
        literal: impl Into<Cow<'static, str>>,
        description: impl Into<Cow<'static, str>>,
        command: Option<Command<S>>,
        requirements: Requirements<S>,
        redirect: Option<Redirection>,
        modifier: RedirectModifier<S>,
        forks: bool,
    ) -> Self {
        Self {
            owned: OwnedNodeData {
                global_id,
                requirements,
                modifier,
                forks,
                command,
            },
            children: FxHashMap::default(),
            redirect,
            meta: CommandNodeMetadata::new(literal, description),
        }
    }
}

/// 表示接受特定类型参数的节点。
///
/// 必须在之后附加到某棵树上才能发挥作用。
#[derive(Clone)]
pub struct ArgumentDetachedNode<S: CommandSource = DummySource> {
    pub owned: OwnedNodeData<S>,
    pub children: FxHashMap<String, DetachedNode<S>>,
    pub redirect: Option<Redirection>,
    pub meta: ArgumentNodeMetadata<S>,
}

impl<S: CommandSource> ArgumentDetachedNode<S> {
    /// 根据属性创建独立的参数节点，
    /// 没有任何子节点。
    ///
    /// # Note
    /// 建议使用 [`argument`](crate::argument_builder::argument) 函数而不是此函数。
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        global_id: GlobalNodeId,
        name: impl Into<Cow<'static, str>>,
        argument_type: Arc<dyn AnyArgumentType<S>>,
        command: Option<Command<S>>,
        requirements: Requirements<S>,
        redirect: Option<Redirection>,
        modifier: RedirectModifier<S>,
        forks: bool,
        suggestion_provider: Option<Arc<dyn SuggestionProvider<S>>>,
    ) -> Self {
        Self {
            owned: OwnedNodeData {
                global_id,
                requirements,
                modifier,
                forks,
                command,
            },
            children: FxHashMap::default(),
            redirect,
            meta: ArgumentNodeMetadata::new(name, argument_type, suggestion_provider),
        }
    }
}

/// 表示尚未挂载到 `Tree` 的节点。
#[derive(Clone)]
pub enum DetachedNode<S: CommandSource = DummySource> {
    Literal(LiteralDetachedNode<S>),
    Command(CommandDetachedNode<S>),
    Argument(ArgumentDetachedNode<S>),
}

/// 表示一个已被不可逆地
/// 分解成各个元素，以便能重新转换
/// 转换为新的 `AttachedNode`。
pub struct DecomposedNode<S: CommandSource = DummySource> {
    pub owned: OwnedNodeData<S>,
    pub children: FxHashMap<String, DetachedNode<S>>,
    pub redirect: Option<Redirection>,
    pub meta: NodeMetadata<S>,
}

impl<S: CommandSource> From<LiteralDetachedNode<S>> for DetachedNode<S> {
    fn from(node: LiteralDetachedNode<S>) -> Self {
        Self::Literal(node)
    }
}

impl<S: CommandSource> From<CommandDetachedNode<S>> for DetachedNode<S> {
    fn from(node: CommandDetachedNode<S>) -> Self {
        Self::Command(node)
    }
}

impl<S: CommandSource> From<ArgumentDetachedNode<S>> for DetachedNode<S> {
    fn from(node: ArgumentDetachedNode<S>) -> Self {
        Self::Argument(node)
    }
}

impl<S: CommandSource> DetachedNode<S> {
    /// 不可逆地将此 [`DetachedNode`] 分解为其组成元素。
    /// 这样它随后便可以重新转换为新的 `AttachedNode`。
    #[must_use]
    pub fn decompose(self) -> DecomposedNode<S> {
        match self {
            Self::Literal(node) => DecomposedNode {
                owned: node.owned,
                children: node.children,
                redirect: node.redirect,
                meta: NodeMetadata::Literal(node.meta),
            },
            Self::Command(node) => DecomposedNode {
                owned: node.owned,
                children: node.children,
                redirect: node.redirect,
                meta: NodeMetadata::Command(node.meta),
            },
            Self::Argument(node) => DecomposedNode {
                owned: node.owned,
                children: node.children,
                redirect: node.redirect,
                meta: NodeMetadata::Argument(node.meta),
            },
        }
    }

    #[must_use]
    pub fn name(&self) -> String {
        match self {
            Self::Literal(node) => node.meta.literal.to_string(),
            Self::Command(node) => node.meta.literal.to_string(),
            Self::Argument(node) => node.meta.name.to_string(),
        }
    }
}
