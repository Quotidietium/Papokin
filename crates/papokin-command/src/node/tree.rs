use crate::context::command_context::{CommandContextBuilder, ParsedArgument};
use crate::context::string_range::StringRange;
use crate::errors::command_syntax_error::CommandSyntaxError;
use crate::errors::error_types::LITERAL_INCORRECT;
use crate::node::Redirection;
use crate::node::attached::{
    ArgumentAttachedNode, ArgumentNodeId, AttachedNode, CommandAttachedNode, CommandNodeId,
    LiteralAttachedNode, LiteralNodeId, NodeClassification, NodeId, RootAttachedNode,
};
use crate::node::detached::{CommandDetachedNode, DetachedNode, GlobalNodeId};
use crate::source::{CommandSource, DummySource};
use crate::string_reader::StringReader;
use papokin_util::text::TextComponent;
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};
use std::num::NonZero;
use std::ops::{Index, IndexMut};
use std::sync::Arc;

/// 根节点占用的固定本地 ID。
pub const ROOT_NODE_ID: NodeId = NodeId(NonZero::new(1).expect("1 非零"));

/// 一个处理输入歧义的消费者（当两个或更多节点同时被满足时）
pub trait AmbiguityConsumer<S: CommandSource = DummySource> {
    fn ambiguous(
        &mut self,
        tree: &Tree<S>,
        parent: NodeId,
        child: NodeId,
        sibling: NodeId,
        inputs: Vec<String>,
    );
}

/// 提供一种存储节点类型的方式
/// 连同其 ID。
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum NodeIdClassification {
    Root,
    Literal(LiteralNodeId),
    Command(CommandNodeId),
    Argument(ArgumentNodeId),
}

/// 表示一整棵节点树。
/// 一切都从根节点开始，根节点产生于
/// 到子节点，子节点再延伸到它们各自的子节点，
/// 等等。
///
/// 这允许重定向和分叉
/// 两个节点之间，即使它们来自不同的命令。
///
/// 此树可以像数组一样进行索引，但使用的是 [`NodeId`]。
///
/// # Hierarchy
/// 此树可以拥有四种不同类型的节点，如下所示：
///
/// - **根节点**：
///   没有父节点。此类型的节点有且仅有一个实例
///   存在于每个 [`Tree`] 中。始终可通过 [`ROOT_NODE_ID`] (= 1) 识别。
///   只有命令节点才能作为此节点的子节点。
///
///   **在任何 `Tree` 中**，根节点的 ID 始终为 1。
///
///   **违反此约束属于逻辑错误**，会破坏
///   该结构自身功能与外部功能所作的假设。
///
///   换句话说，非根节点的 ID 绝不能为 1！
///
/// - **命令**：
///   其父节点必须是根节点，并指定一条
///   命令定义。
///
/// - **字面量**：
///   接受特定的常量词。
///
/// - **参数**：
///   解析并接受特定类型的值。这种节点非常动态。
#[derive(Clone)]
pub struct Tree<S: CommandSource = DummySource> {
    /// 此树中存储的所有节点。
    ///
    /// 在此向量中，从 0 开始的索引表示第一个节点（ID = 1），
    /// 1 表示第二个节点（ID = 2），依此类推。
    nodes: Vec<AttachedNode<S>>,

    /// 将 [`GlobalNodeId`] 链接到此树 [`NodeId`] 的键。
    /// 对重定向很有用。
    ids_map: FxHashMap<GlobalNodeId, NodeId>,

    /// 每个命令的缓存映射。
    command_node_mappings: FxHashMap<String, CommandNodeId>,
}

impl<S: CommandSource> Default for Tree<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: CommandSource> Tree<S> {
    /// 构造一棵新树，其中包含一个没有子节点的新根节点。
    #[must_use]
    pub fn new() -> Self {
        let node = RootAttachedNode::new();
        let mut ids_map = FxHashMap::default();
        ids_map.insert(node.owned.global_id, ROOT_NODE_ID);
        Self {
            nodes: vec![AttachedNode::Root(node)],
            ids_map,
            command_node_mappings: FxHashMap::default(),
        }
    }

    /// 返回此树中的节点数量。
    #[must_use]
    pub const fn len(&self) -> usize {
        self.nodes.len()
    }

    ///若该树没有节点，则返回 `true`。
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// 通过创建新的唯一 ID 来分配一个新的 [`NodeId`]。
    const fn alloc(&self) -> NodeId {
        NodeId(NonZero::new(self.nodes.len() + 1).expect("预期为非零 ID"))
    }

    /// 辅助函数：附加给定的 [`AttachedNode`]，并返回
    /// 其 [`NodeId`]。
    fn add(&mut self, node: AttachedNode<S>) -> NodeId {
        let global_id = node.global_id();
        let local_id = self.alloc();

        // 更新状态变量。
        self.nodes.push(node);
        self.ids_map.insert(global_id, local_id);

        local_id
    }

    /// 辅助函数：以不可逆方式附加 [`DetachedNode`]
    /// 附加到此 [`Tree`]，并返回刚附加节点的 ID
    /// 节点。
    fn attach(&mut self, node: DetachedNode<S>) -> NodeId {
        // 首先，我们分解该节点
        let node = node.decompose();

        // 将其子节点加入此树。
        let mut children = FxHashMap::with_capacity_and_hasher(node.children.len(), FxBuildHasher);
        for (child_name, child) in node.children {
            let child_id = self.attach(child);
            children.insert(child_name, child_id);
        }

        // 现在创建要被“附着”的节点。
        let node = AttachedNode::from_parts(node.owned, children, node.redirect, node.meta);

        self.add(node)
    }

    /// 获取此 [`Tree`] 的大小，即此树包含的节点数。
    #[must_use]
    pub const fn size(&self) -> usize {
        self.nodes.len()
    }

    /// 获取此 [`Tree`] 的大小，即此树包含的节点数。
    #[must_use]
    pub fn size_nonzero(&self) -> NonZero<usize> {
        self.nodes
            .len()
            .try_into()
            .expect("预期大小非零，但 Tree 的大小不知为何为零")
    }

    /// 向此树的根节点添加一个 [`CommandDetachedNode`]。
    pub fn add_child_to_root(&mut self, node: impl Into<CommandDetachedNode<S>>) -> CommandNodeId {
        // 首先，将该节点附加到此树
        let node = node.into();
        let name = node.meta.literal.to_string();
        let node = self.attach(node.into());
        let node = self.add_attached_child(ROOT_NODE_ID, node);

        // 这是安全的，因为节点 ID 现在指向一个 `CommandAttachedNode`。
        let node = CommandNodeId(node.0);

        self.command_node_mappings.insert(name, node);
        node
    }

    /// 向给定节点添加一个子节点。
    ///
    /// # Panics
    ///
    /// 如果要添加到非根节点的节点是 [`CommandDetachedNode`] 则 panic。
    ///
    /// 从本质上说，这意味着 [`CommandDetachedNode`] 必须拥有根节点
    /// 树*作为其父节点*，以便挂载到树上。
    pub fn add_child(&mut self, parent: NodeId, node: impl Into<DetachedNode<S>>) -> NodeId {
        let node = node.into();
        assert!(
            parent == ROOT_NODE_ID || !matches!(node, DetachedNode::Command(_)),
            "Cannot add a CommandDetachedNode as a child of a non-root node"
        );

        // 首先，将该节点附加到此树
        let node = self.attach(node);
        self.add_attached_child(parent, node)
    }

    /// 向给定节点添加一个已附加的子节点。
    ///
    /// # Panics
    ///
    /// 如果要添加到非根节点的节点是 [`CommandAttachedNode`] 则 panic，
    /// 或要添加到某节点的节点是 [`RootAttachedNode`]，
    ///
    /// 从本质上说，这意味着 [`CommandAttachedNode`] 必须拥有根节点
    /// 树*作为其父节点*，而 [`RootAttachedNode`] 不能拥有父节点。
    fn add_attached_child(&mut self, parent: NodeId, node: NodeId) -> NodeId {
        assert!(
            parent == ROOT_NODE_ID || self[node].classification() != NodeClassification::Command,
            "Cannot add a CommandAttachedNode as a child of a non-root node"
        );

        let node_name = self[node].name();

        let child = self[parent].children_ref().get(&node_name);
        if let Some(child) = child {
            let node_command = self[node].command().clone();
            let node_children: Vec<NodeId> = self[node].children_ref().values().copied().collect();

            let child = *child;
            // 合并到子节点上。
            if let Some(command) = node_command {
                self[child].set_command(Some(command));
            }
            for grandchild in node_children {
                self.add_attached_child(child, grandchild);
            }
            child
        } else {
            self[parent].children_mut_ref().insert(node_name, node);
            node
        }
    }

    /// 获取树中给定节点的子节点。
    #[must_use]
    pub fn get_children(&self, node: NodeId) -> Vec<NodeId> {
        self[node].children_ref().values().copied().collect()
    }

    /// 获取树中根节点的子节点。
    #[must_use]
    pub fn get_root_children(&self) -> Vec<CommandNodeId> {
        self[ROOT_NODE_ID]
            .children_ref()
            .values()
            .copied()
            // 这应该没问题，因为其所有子节点都
            // 根节点均为命令节点。
            .map(|id| CommandNodeId(id.0))
            .collect()
    }

    /// 返回给定节点能否被给定命令源使用。
    #[must_use]
    pub fn can_use(&self, node: NodeId, source: &S) -> bool {
        self[node].requirements().evaluate(source)
    }

    /// 查找输入中的歧义并将其交给 [`AmbiguityConsumer`]。
    pub fn find_ambiguities(&self, node: NodeId, consumer: &mut impl AmbiguityConsumer<S>) {
        let mut matches: FxHashSet<String> = FxHashSet::default();

        for child in self.get_children(node) {
            for sibling in self.get_children(node) {
                if child == sibling {
                    continue;
                }
                for input in self[child].examples() {
                    if self[sibling].is_valid_input(&input) {
                        matches.insert(input.clone());
                    }
                }

                if !matches.is_empty() {
                    consumer.ambiguous(self, node, child, sibling, matches.drain().collect());
                }
            }

            self.find_ambiguities(child, consumer);
        }
    }

    /// 将给定节点归类为带类型的 ID。
    #[must_use]
    pub fn classify_id(&self, node: NodeId) -> NodeIdClassification {
        match self[node].classification() {
            NodeClassification::Root => NodeIdClassification::Root,
            NodeClassification::Literal => NodeIdClassification::Literal(LiteralNodeId(node.0)),
            NodeClassification::Command => NodeIdClassification::Command(CommandNodeId(node.0)),
            NodeClassification::Argument => NodeIdClassification::Argument(ArgumentNodeId(node.0)),
        }
    }

    /// 返回给定 ID 是否指向命令节点。
    #[must_use]
    pub fn is_command_node(&self, node: NodeId) -> bool {
        matches!(self[node].classification(), NodeClassification::Command)
    }

    #[must_use]
    pub fn get_relevant_nodes(&self, reader: &mut StringReader, node: NodeId) -> Vec<NodeId> {
        let children = self.get_children(node);
        let mut literals = Vec::new();
        let mut commands = Vec::new();
        let mut arguments = Vec::new();
        for child in children {
            let id = self.classify_id(child);
            match id {
                NodeIdClassification::Root => {}
                NodeIdClassification::Literal(literal) => literals.push(literal),
                NodeIdClassification::Command(command) => commands.push(command),
                NodeIdClassification::Argument(arg) => arguments.push(arg),
            }
        }

        // 优先顺序：
        // 1. 命令 > 字面量
        // 2. 参数

        if !literals.is_empty() || !commands.is_empty() {
            let cursor = reader.cursor();
            while !matches!(reader.peek(), None | Some(' ')) {
                reader.skip();
            }
            let new_cursor = reader.cursor();
            reader.set_cursor(cursor);
            let text = &reader.string()[cursor..new_cursor];

            for command in commands {
                if self[command].meta.literal == text {
                    return vec![command.into()];
                }
            }
            for literal in literals {
                if self[literal].meta.literal == text {
                    return vec![literal.into()];
                }
            }
        }

        arguments.into_iter().map(ArgumentNodeId::into).collect()
    }

    /// 解析给定节点，失败时返回错误。
    pub fn parse<'a>(
        &self,
        node_id: NodeId,
        reader: &'a mut StringReader<'_>,
        command_context_builder: &'a mut CommandContextBuilder<'_, S>,
    ) -> Result<(), CommandSyntaxError> {
        match &self[node_id] {
            AttachedNode::Root(_) => {}
            AttachedNode::Literal(node) => {
                let start = reader.cursor();
                let Ok(end) = AttachedNode::<S>::parse_literal(reader, &node.meta.literal) else {
                    return Err(LITERAL_INCORRECT
                        .create(reader, TextComponent::text(node.meta.literal.to_string())));
                };
                command_context_builder.with_node(node_id, StringRange::between(start, end));
            }
            AttachedNode::Command(node) => {
                let start = reader.cursor();
                let Ok(end) = AttachedNode::<S>::parse_literal(reader, &node.meta.literal) else {
                    return Err(LITERAL_INCORRECT
                        .create(reader, TextComponent::text(node.meta.literal.to_string())));
                };
                command_context_builder.with_node(node_id, StringRange::between(start, end));
            }
            AttachedNode::Argument(node) => {
                let start = reader.cursor();
                let result = node
                    .meta
                    .argument_type
                    .parse_with_source(reader, &command_context_builder.source)?;
                let range = StringRange::between(start, reader.cursor());
                let parsed = ParsedArgument::new(range, result);
                command_context_builder.with_argument(node.meta.name.to_string(), Arc::new(parsed));
                command_context_builder.with_node(node_id, range);
            }
        }
        Ok(())
    }

    /// 基于该树解析给定的重定向，该树即是从
    /// 发生重定向之处。
    ///
    /// 若能找到所需节点，则返回 [`Some`]，
    /// 否则返回 [`None`]。
    #[must_use]
    pub fn resolve(&self, redirect: Redirection) -> Option<NodeId> {
        match redirect {
            Redirection::Root => Some(ROOT_NODE_ID),
            Redirection::Global(id) => self.ids_map.get(&id).copied(),
            Redirection::Local(id) => (id.0 < self.size_nonzero()).then_some(id),
        }
    }

    /// 根据给定的字面量按 ID 获取命令节点。
    #[must_use]
    pub fn get(&self, name: &str) -> Option<CommandNodeId> {
        self.command_node_mappings.get(name).copied()
    }

    ///返回遍历此树所有节点的迭代器。
    pub fn iter(&self) -> std::slice::Iter<'_, AttachedNode<S>> {
        self.nodes.iter()
    }
}

impl<S: CommandSource> Index<NodeId> for Tree<S> {
    type Output = AttachedNode<S>;

    fn index(&self, index: NodeId) -> &Self::Output {
        &self.nodes[index.0.get() - 1]
    }
}

impl<S: CommandSource> IndexMut<NodeId> for Tree<S> {
    fn index_mut(&mut self, index: NodeId) -> &mut Self::Output {
        &mut self.nodes[index.0.get() - 1]
    }
}

/// 宏辅助工具：为使用类型化 ID 的 [`Tree`] 创建 [`Index`] 与 [`IndexMut`]。
macro_rules! impl_index_index_mut {
    ($node_id: ident -> AttachedNode::$attached_node_enum: ident($attached_node: ident)) => {
        impl<S: CommandSource> Index<$node_id> for Tree<S> {
            type Output = $attached_node<S>;

            fn index(&self, index: $node_id) -> &Self::Output {
                if let AttachedNode::$attached_node_enum(node) = &self.nodes[index.0.get() - 1] {
                    node
                } else {
                    unreachable!(
                        "节点应当是 AttachedNode::{}",
                        stringify!($attached_node_enum)
                    )
                }
            }
        }

        impl<S: CommandSource> IndexMut<$node_id> for Tree<S> {
            fn index_mut(&mut self, index: $node_id) -> &mut Self::Output {
                if let AttachedNode::$attached_node_enum(node) = &mut self.nodes[index.0.get() - 1]
                {
                    node
                } else {
                    unreachable!(
                        "节点应当是 AttachedNode::{}",
                        stringify!($attached_node_enum)
                    )
                }
            }
        }
    };
}

impl_index_index_mut!(LiteralNodeId -> AttachedNode::Literal(LiteralAttachedNode));
impl_index_index_mut!(CommandNodeId -> AttachedNode::Command(CommandAttachedNode));
impl_index_index_mut!(ArgumentNodeId -> AttachedNode::Argument(ArgumentAttachedNode));

impl<'a, S: CommandSource> IntoIterator for &'a Tree<S> {
    type Item = &'a AttachedNode<S>;
    type IntoIter = std::slice::Iter<'a, AttachedNode<S>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod test {
    use crate::argument_builder::{
        ArgumentBuilder, CommandArgumentBuilder, LiteralArgumentBuilder, RequiredArgumentBuilder,
    };
    use crate::argument_types::core::string::StringArgumentType;
    use crate::node::attached::NodeId;
    use crate::node::tree::{AmbiguityConsumer, Tree};
    use crate::source::DummySource;

    #[test]
    fn adding_nodes() {
        // 新树（只包含一个根节点）
        let mut tree = Tree::<DummySource>::new();
        assert_eq!(tree.size(), 1);

        // 添加一个节点。
        tree.add_child_to_root(CommandArgumentBuilder::new("foo", "A test command"));
        assert_eq!(tree.size(), 2);

        // 添加一个带子节点的节点。
        tree.add_child_to_root(
            // 每个子命令都是一个子节点
            CommandArgumentBuilder::new("bar", "Another test command")
                .then(LiteralArgumentBuilder::new("baz"))
                .then(LiteralArgumentBuilder::new("qux")),
        );
        assert_eq!(tree.size(), 5);
    }

    #[test]
    fn adding_children_to_attached_node() {
        let mut tree = Tree::<DummySource>::new();

        let parent: NodeId = tree
            .add_child_to_root(CommandArgumentBuilder::new("foo", "A test command"))
            .into();

        tree.add_child(parent, LiteralArgumentBuilder::new("baz"));
        tree.add_child(parent, LiteralArgumentBuilder::new("qux"));

        assert_eq!(tree.size(), 4);
        assert_eq!(tree.get_children(parent).len(), 2);
    }

    #[test]
    #[should_panic = "Cannot add a CommandDetachedNode as a child of a non-root node"]
    fn adding_command_node_to_non_root_node() {
        let mut tree = Tree::<DummySource>::new();

        let parent: NodeId = tree
            .add_child_to_root(CommandArgumentBuilder::new("foo", "A test command"))
            .into();

        tree.add_child(
            parent,
            CommandArgumentBuilder::new("bar", "Another test command"),
        );
    }

    #[test]
    fn finding_ambiguities() {
        struct Consumer {
            inputs_received: usize,
            expected_parent: NodeId,
            expected_sibling: NodeId,
        }

        impl AmbiguityConsumer for Consumer {
            fn ambiguous(
                &mut self,
                _tree: &Tree,
                parent: NodeId,
                _child: NodeId,
                sibling: NodeId,
                inputs: Vec<String>,
            ) {
                self.inputs_received += inputs.len();

                assert_eq!(self.expected_parent, parent);
                assert_eq!(self.expected_sibling, sibling);
            }
        }

        let mut tree = Tree::new();

        let parent: NodeId = tree
            .add_child_to_root(CommandArgumentBuilder::new("foo", "A test command"))
            .into();

        tree.add_child(parent, LiteralArgumentBuilder::new("hello"));
        tree.add_child(parent, LiteralArgumentBuilder::new("bye"));
        let sibling = tree.add_child(
            parent,
            RequiredArgumentBuilder::new("string", StringArgumentType::SingleWord),
        );

        let mut consumer = Consumer {
            inputs_received: 0,
            expected_parent: parent,
            expected_sibling: sibling,
        };
        tree.find_ambiguities(parent, &mut consumer);

        assert_eq!(consumer.inputs_received, 2);
    }
}
