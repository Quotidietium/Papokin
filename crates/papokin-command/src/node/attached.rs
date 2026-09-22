use crate::context::string_range::StringRange;
use crate::errors::command_syntax_error::CommandSyntaxError;
use crate::errors::error_types::LITERAL_INCORRECT;
use crate::node::detached::GlobalNodeId;
use crate::node::tree::ROOT_NODE_ID;
use crate::node::{
    ArgumentNodeMetadata, Command, CommandNodeMetadata, LiteralNodeMetadata, NodeMetadata,
    OwnedNodeData, RedirectModifier, Redirection, Requirements,
};
use crate::source::{CommandSource, DummySource};
use crate::string_reader::StringReader;
use papokin_util::text::TextComponent;
use rustc_hash::FxHashMap;
use std::num::NonZero;

/// 表示一个唯一的整数编号，对应
/// 任意节点相对于树的位置。
///
/// 此处内部使用了一个 [`NonZero<usize>`]
/// 结构体。这意味着 [`Option<NodeId>`] 承载了
/// 与 [`NodeId`] 相同的大小，但代价是
/// ID `0` 不可分配。
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct NodeId(pub NonZero<usize>);

/// 表示一个唯一的整数编号，对应
/// 根节点相对于树的位置。
///
/// 它是单位大小的，因为它是常量。
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct RootNodeId;

/// 表示一个唯一的整数编号，对应
/// 特定字面量节点相对于树的位置。
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct LiteralNodeId(pub NonZero<usize>);

/// 表示一个唯一的整数编号，对应
/// 特定命令节点相对于树的位置。
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct CommandNodeId(pub NonZero<usize>);

/// 表示一个唯一的整数编号，对应
/// 特定参数节点相对于树的位置。
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct ArgumentNodeId(pub NonZero<usize>);

impl From<RootNodeId> for NodeId {
    fn from(_id: RootNodeId) -> Self {
        ROOT_NODE_ID
    }
}

impl From<LiteralNodeId> for NodeId {
    fn from(id: LiteralNodeId) -> Self {
        Self(id.0)
    }
}

impl From<CommandNodeId> for NodeId {
    fn from(id: CommandNodeId) -> Self {
        Self(id.0)
    }
}

impl From<ArgumentNodeId> for NodeId {
    fn from(id: ArgumentNodeId) -> Self {
        Self(id.0)
    }
}

/// 表示已作为 `Tree` 根节点挂载的节点。
#[derive(Clone)]
pub struct RootAttachedNode<S: CommandSource = DummySource> {
    pub owned: OwnedNodeData<S>,
    pub children: FxHashMap<String, NodeId>,
}

impl<S: CommandSource> Default for RootAttachedNode<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: CommandSource> RootAttachedNode<S> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            owned: OwnedNodeData {
                global_id: GlobalNodeId::new(),
                requirements: Requirements::new(),
                modifier: RedirectModifier::KeepSource,
                forks: false,
                command: None,
            },
            children: FxHashMap::default(),
        }
    }
}

/// 表示一个非命令的字面量节点，已挂载
/// 转换为 `Tree`。
#[derive(Clone)]
pub struct LiteralAttachedNode<S: CommandSource = DummySource> {
    pub owned: OwnedNodeData<S>,
    pub children: FxHashMap<String, NodeId>,
    pub redirect: Option<Redirection>,
    pub meta: LiteralNodeMetadata,
}

/// 表示一个字面量命令节点，已挂载
/// 转换为 `Tree`。
#[derive(Clone)]
pub struct CommandAttachedNode<S: CommandSource = DummySource> {
    pub owned: OwnedNodeData<S>,
    pub children: FxHashMap<String, NodeId>,
    pub redirect: Option<Redirection>,
    pub meta: CommandNodeMetadata,
}

/// 表示一个接受特定类型参数的节点，已挂载
/// 转换为 `Tree`。
#[derive(Clone)]
pub struct ArgumentAttachedNode<S: CommandSource = DummySource> {
    pub owned: OwnedNodeData<S>,
    pub children: FxHashMap<String, NodeId>,
    pub redirect: Option<Redirection>,
    pub meta: ArgumentNodeMetadata<S>,
}

/// 提供一种存储节点类型的方式
/// 而无需真正克隆 [`NodeMetadata`]。
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum NodeClassification {
    Root,
    Literal,
    Command,
    Argument,
}

/// 表示尚未挂载到 `Tree` 的节点。
#[derive(Clone)]
pub enum AttachedNode<S: CommandSource = DummySource> {
    Root(RootAttachedNode<S>),
    Literal(LiteralAttachedNode<S>),
    Command(CommandAttachedNode<S>),
    Argument(ArgumentAttachedNode<S>),
}

/// 当字面量无法解析时返回。
pub struct CouldNotParseLiteral;

impl<S: CommandSource> AttachedNode<S> {
    /// 根据属性创建一个 [`AttachedNode`]，允许任意 [`NodeMetadata`]。
    #[must_use]
    pub fn from_parts(
        owned: OwnedNodeData<S>,
        children: FxHashMap<String, NodeId>,
        redirect: Option<Redirection>,
        meta: NodeMetadata<S>,
    ) -> Self {
        match meta {
            NodeMetadata::Root => Self::Root(RootAttachedNode { owned, children }),
            NodeMetadata::Literal(meta) => Self::Literal(LiteralAttachedNode {
                owned,
                children,
                redirect,
                meta,
            }),
            NodeMetadata::Command(meta) => Self::Command(CommandAttachedNode {
                owned,
                children,
                redirect,
                meta,
            }),
            NodeMetadata::Argument(meta) => Self::Argument(ArgumentAttachedNode {
                owned,
                children,
                redirect,
                meta,
            }),
        }
    }

    /// 获取此节点的分类。
    /// 这是一个开销相对较低的操作。
    #[must_use]
    pub const fn classification(&self) -> NodeClassification {
        match self {
            Self::Root(_) => NodeClassification::Root,
            Self::Literal(_) => NodeClassification::Literal,
            Self::Command(_) => NodeClassification::Command,
            Self::Argument(_) => NodeClassification::Argument,
        }
    }

    /// 从此节点获取全局 ID。
    #[must_use]
    pub const fn global_id(&self) -> GlobalNodeId {
        self.owned_node_data_ref().global_id
    }

    /// 获取此节点所拥有数据的引用。
    #[must_use]
    pub const fn owned_node_data_ref(&self) -> &OwnedNodeData<S> {
        match self {
            Self::Root(node) => &node.owned,
            Self::Literal(node) => &node.owned,
            Self::Command(node) => &node.owned,
            Self::Argument(node) => &node.owned,
        }
    }

    /// 获取此节点所拥有数据的可变引用。
    pub const fn owned_node_data_mut_ref(&mut self) -> &mut OwnedNodeData<S> {
        match self {
            Self::Root(node) => &mut node.owned,
            Self::Literal(node) => &mut node.owned,
            Self::Command(node) => &mut node.owned,
            Self::Argument(node) => &mut node.owned,
        }
    }

    /// 获取此节点子节点 ID 的引用。
    #[must_use]
    pub const fn children_ref(&self) -> &FxHashMap<String, NodeId> {
        match self {
            Self::Root(node) => &node.children,
            Self::Literal(node) => &node.children,
            Self::Command(node) => &node.children,
            Self::Argument(node) => &node.children,
        }
    }

    /// 获取此节点子节点 ID 的可变引用。
    pub const fn children_mut_ref(&mut self) -> &mut FxHashMap<String, NodeId> {
        match self {
            Self::Root(node) => &mut node.children,
            Self::Literal(node) => &mut node.children,
            Self::Command(node) => &mut node.children,
            Self::Argument(node) => &mut node.children,
        }
    }

    /// 获取此节点的名称。
    #[must_use]
    pub fn name(&self) -> String {
        match self {
            Self::Root(_) => String::new(),
            Self::Literal(node) => node.meta.literal.to_string(),
            Self::Command(node) => node.meta.literal.to_string(),
            Self::Argument(node) => node.meta.name.to_string(),
        }
    }

    /// 获取此节点的重定向目标。
    #[must_use]
    pub const fn redirect(&self) -> Option<Redirection> {
        match self {
            Self::Root(_) => None,
            Self::Literal(node) => node.redirect,
            Self::Command(node) => node.redirect,
            Self::Argument(node) => node.redirect,
        }
    }

    /// 获取一个 [`Option`]，其中包含指向此节点重定向目标的可变引用。
    pub const fn redirect_mut_ref(&mut self) -> Option<&mut Redirection> {
        match self {
            Self::Root(_) => None,
            Self::Literal(node) => node.redirect.as_mut(),
            Self::Command(node) => node.redirect.as_mut(),
            Self::Argument(node) => node.redirect.as_mut(),
        }
    }

    /// 设置此节点的重定向。
    pub const fn set_redirect(&mut self, redirect: Option<Redirection>) {
        match self {
            Self::Root(_) => {}
            Self::Literal(node) => node.redirect = redirect,
            Self::Command(node) => node.redirect = redirect,
            Self::Argument(node) => node.redirect = redirect,
        }
    }

    /// 获取运行此节点所需的全部要求。
    #[must_use]
    pub const fn requirements(&self) -> &Requirements<S> {
        &self.owned_node_data_ref().requirements
    }

    /// 将此节点运行所需的全部条件覆盖为某个值。
    pub fn set_requirement(&mut self, requirements: Requirements<S>) {
        self.owned_node_data_mut_ref().requirements = requirements;
    }

    /// 获取运行此节点所用的修饰符。
    #[must_use]
    pub const fn modifier(&self) -> &RedirectModifier<S> {
        &self.owned_node_data_ref().modifier
    }

    /// 将此节点的修饰器设置为给定值。
    pub fn set_modifier(&mut self, modifier: RedirectModifier<S>) {
        self.owned_node_data_mut_ref().modifier = modifier;
    }

    /// 此节点是否分叉 `CommandSources`。
    #[must_use]
    pub const fn forks(&self) -> bool {
        self.owned_node_data_ref().forks
    }

    /// 设置此节点是否对 `CommandSources` 进行分叉。
    pub const fn set_forks(&mut self, forks: bool) {
        self.owned_node_data_mut_ref().forks = forks;
    }

    /// 获取此节点可执行的命令。
    #[must_use]
    pub fn command(&self) -> &Option<Command<S>> {
        &self.owned_node_data_ref().command
    }

    /// 设置此节点要执行的命令。
    pub fn set_command(&mut self, command: Option<Command<S>>) {
        self.owned_node_data_mut_ref().command = command;
    }

    /// 获取此节点的用法文本。
    #[must_use]
    pub fn usage_text(&self) -> String {
        match self {
            Self::Root(_) => String::new(),
            Self::Literal(node) => node.meta.literal.to_string(),
            Self::Command(node) => node.meta.literal.to_string(),
            Self::Argument(node) => format!("<{}>", node.meta.name),
        }
    }

    /// 检查给定输入对此节点是否有效。
    #[must_use]
    pub fn is_valid_input(&self, input: &str) -> bool {
        match self {
            Self::Root(_) => false,
            Self::Literal(node) => {
                let mut reader = StringReader::new(input);
                Self::parse_literal(&mut reader, &node.meta.literal).is_ok()
            }
            Self::Command(node) => {
                let mut reader = StringReader::new(input);
                Self::parse_literal(&mut reader, &node.meta.literal).is_ok()
            }
            Self::Argument(node) => {
                let mut reader = StringReader::new(input);
                let parsed = node.meta.argument_type.parse(&mut reader);
                if parsed.is_ok() {
                    matches!(reader.peek(), Some(' ') | None)
                } else {
                    false
                }
            }
        }
    }

    /// 为此节点解析给定输入。
    /// 建议使用 `CommandDispatcher` 而不是直接调用此函数。
    pub fn parse(
        &self,
        reader: &mut StringReader,
        literal: &str,
    ) -> Result<StringRange, CommandSyntaxError> {
        let start = reader.cursor();
        Self::parse_literal(reader, literal).map_or_else(
            |_| Err(LITERAL_INCORRECT.create(reader, TextComponent::text(literal.to_string()))),
            |end| Ok(StringRange::between(start, end)),
        )
    }

    /// 解析字面量的内部函数。由 `Tree` 使用。
    pub fn parse_literal(
        reader: &mut StringReader,
        literal: &str,
    ) -> Result<usize, CouldNotParseLiteral> {
        let start = reader.cursor();
        let len = literal.len();
        if reader.can_read_bytes(len) {
            let end = start + len;
            if &reader.string()[start..end] == literal {
                reader.set_cursor(end);
                if matches!(reader.peek(), Some(' ') | None) {
                    return Ok(end);
                }
                reader.set_cursor(start);
            }
        }
        Err(CouldNotParseLiteral)
    }

    /// 获取此节点接受的示例。
    #[must_use]
    pub fn examples(&self) -> Vec<String> {
        match self {
            Self::Root(_) => Vec::new(),
            Self::Literal(node) => vec![node.meta.literal.to_string()],
            Self::Command(node) => vec![node.meta.literal.to_string()],
            Self::Argument(node) => node.meta.argument_type.examples(),
        }
    }
}
