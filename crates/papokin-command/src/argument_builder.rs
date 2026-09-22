use crate::argument_builder::private::Sealed;
use crate::argument_types::argument_type::AnyArgumentType;
use crate::node::detached::{
    ArgumentDetachedNode, CommandDetachedNode, DetachedNode, GlobalNodeId, LiteralDetachedNode,
};
use crate::node::{
    Command, CommandExecutor, RedirectModifier, Redirection, Requirement, Requirements,
};
use crate::source::{CommandSource, DummySource};
use crate::suggestion::provider::SuggestionProvider;
use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::sync::Arc;

/// 表示一个中间结构体，用于
/// 为命令构建参数。
///
/// # Note
///
/// 这是一个实现细节。
struct CommonArgumentBuilder<S: CommandSource = DummySource> {
    pub global_id: GlobalNodeId,
    pub arguments: FxHashMap<String, DetachedNode<S>>,
    pub command: Option<Command<S>>,
    pub requirements: Requirements<S>,
    pub target: Option<Redirection>,
    pub modifier: RedirectModifier<S>,
    pub forks: bool,
}

impl<S: CommandSource> CommonArgumentBuilder<S> {
    fn new() -> Self {
        Self {
            global_id: GlobalNodeId::new(),
            arguments: FxHashMap::default(),
            command: None,
            requirements: Requirements::new(),
            target: None,
            modifier: RedirectModifier::KeepSource,
            forks: false,
        }
    }
}

impl<S: CommandSource> Default for CommonArgumentBuilder<S> {
    fn default() -> Self {
        Self::new()
    }
}

/// 创建新 [`CommandArgumentBuilder`] 的一种简写方式
/// 从字面量与命令描述创建。
///
/// 可以不带前缀直接导入，也可以使用 `argument_builder::` 前缀导入。下面是用法示例：
/// ```
/// use papokin_command::argument_builder::command;
/// use papokin_command::source::DummySource;
///
/// let builder = command::<DummySource>("foo", "A test command");
/// ```
///
/// 返回的构建器最终会构造出 [`CommandDetachedNode`]。
/// 随后可以将此节点注册到调度器中。
pub fn command<S: CommandSource>(
    literal: impl Into<Cow<'static, str>>,
    description: impl Into<Cow<'static, str>>,
) -> CommandArgumentBuilder<S> {
    CommandArgumentBuilder::new(literal, description)
}

/// 创建新 [`LiteralArgumentBuilder`] 的一种简写方式
/// 从字面量创建。
///
/// 可以不带前缀直接导入，也可以使用 `argument_builder::` 前缀导入。下面是用法示例：
/// ```
/// use papokin_command::argument_builder::literal;
/// use papokin_command::source::DummySource;
///
/// let builder = literal::<DummySource>("bar");
/// ```
///
/// 返回的构建器最终会构造出 [`LiteralDetachedNode`]。
pub fn literal<S: CommandSource>(
    literal: impl Into<Cow<'static, str>>,
) -> LiteralArgumentBuilder<S> {
    LiteralArgumentBuilder::new(literal)
}

/// 创建新 [`RequiredArgumentBuilder`] 的一种简写方式
/// 从参数类型与名称创建。
///
/// 可以不带前缀直接导入，也可以使用 `argument_builder::` 前缀导入。下面是用法示例：
/// ```
/// use papokin_command::{
///     argument_builder::argument,
///     argument_types::core::integer::IntegerArgumentType,
///     source::DummySource,
/// };
///
/// let argument_builder = argument::<DummySource>("bar", IntegerArgumentType::new(1, 10));
/// ```
///
/// 返回的构建器最终会构造出 [`ArgumentDetachedNode`]。
pub fn argument<S: CommandSource>(
    name: impl Into<Cow<'static, str>>,
    arg_type: impl AnyArgumentType<S> + 'static,
) -> RequiredArgumentBuilder<S> {
    RequiredArgumentBuilder::new(name, arg_type)
}

/// 一个构建非命令的字面量 [`DetachedNode`] 的构建器。
pub struct LiteralArgumentBuilder<S: CommandSource = DummySource> {
    common: CommonArgumentBuilder<S>,
    literal: Cow<'static, str>,
}

/// 一个构建命令 [`DetachedNode`] 的构建器。
pub struct CommandArgumentBuilder<S: CommandSource = DummySource> {
    common: CommonArgumentBuilder<S>,
    literal: Cow<'static, str>,
    description: Cow<'static, str>,
    source: Option<String>,
}

/// 一个构建参数 [`DetachedNode`] 的构建器。
pub struct RequiredArgumentBuilder<S: CommandSource = DummySource> {
    common: CommonArgumentBuilder<S>,
    name: Cow<'static, str>,
    argument_type: Arc<dyn AnyArgumentType<S>>,
    suggestion_provider: Option<Arc<dyn SuggestionProvider<S>>>,
}

mod private {
    // 我们想把这个 trait 设为私有，以便
    // 我们只能为我们的
    // 此处定义的参数构建器。
    pub trait Sealed {}
}

pub trait ArgumentBuilder<
    S: CommandSource = DummySource,
    N: Into<DetachedNode<S>> = DetachedNode<S>,
>: Sized + Sealed
{
    /// 在指定本参数之后，紧接着放置下一个待指定的参数。
    ///
    /// # Panics
    ///
    /// 如果此节点被重定向到另一个节点，或该子节点
    /// 所提供的类型为 [`CommandDetachedNode`]。
    #[must_use]
    fn then(self, child: impl Into<DetachedNode<S>>) -> Self;

    /// 获取正在构建的节点要执行的命令。
    #[must_use]
    fn command(&self) -> Option<Command<S>>;

    /// 为正在构建的节点设置要执行的命令。
    #[must_use]
    fn executes(self, command: impl CommandExecutor<S> + 'static) -> Self {
        self.executes_arc(Arc::new(command))
    }

    /// 为正在构建的节点设置要执行的命令。
    #[must_use]
    fn executes_arc(self, command: Arc<dyn CommandExecutor<S> + 'static>) -> Self;

    /// 将正在构建的节点的重定向目标设为另一节点，不带修饰器。
    #[must_use]
    fn redirect(self, redirection: impl Into<Redirection>) -> Self;

    /// 将正在构建的节点的重定向目标设为另一节点，并附带给定修饰器。
    #[must_use]
    fn redirect_with_modifier(
        self,
        redirection: impl Into<Redirection>,
        redirect_modifier: RedirectModifier<S>,
    ) -> Self;

    /// 分叉给定的上下文，供之后多次使用。
    #[must_use]
    fn fork(
        self,
        redirection: impl Into<Redirection>,
        redirect_modifier: RedirectModifier<S>,
    ) -> Self;

    /// 转发给定的上下文，并携带给定的 `fork` 标志。
    #[must_use]
    fn forward(
        self,
        redirection: impl Into<Redirection>,
        redirect_modifier: RedirectModifier<S>,
        fork: bool,
    ) -> Self;

    /// 获取待构建节点参数的引用。
    #[must_use]
    fn arguments(&self) -> &FxHashMap<String, DetachedNode<S>>;

    /// 获取此 [`ArgumentBuilder`] 正在构建的节点所重定向到的节点。
    #[must_use]
    fn target(&self) -> Option<Redirection>;

    /// 将给定谓词添加到正在构建的节点的需求列表中。
    ///
    /// 这意味着可以将多个谓词链接在一起，并且它们必须全部
    /// 得到满足。
    ///
    /// 权限也可以作为 `requirement` 直接插入此方法。
    #[must_use]
    fn requires(self, requirement: impl Into<Requirement<S>>) -> Self;

    /// 将此节点的当前要求覆盖为新值。
    #[must_use]
    fn overwrite_requirements(self, requirements: Requirements<S>) -> Self;

    /// 获取此 [`ArgumentBuilder`] 正在构建的节点的重定向修饰符。
    #[must_use]
    fn redirect_modifier(&self) -> RedirectModifier<S>;

    /// 此构建器是否分叉。
    #[must_use]
    fn forks(&self) -> bool;

    ///返回此 Builder 将产出的节点的“未来 [`GlobalNodeId`]”。
    /// 对重定向非常有用。
    #[must_use]
    fn id(&self) -> GlobalNodeId;

    /// 构建此构建器所表示的节点，过程中消耗自身。
    #[must_use]
    fn build(self) -> N;
}

// 为我们的构建器实现私有 trait！
impl<S: CommandSource> Sealed for LiteralArgumentBuilder<S> {}
impl<S: CommandSource> Sealed for CommandArgumentBuilder<S> {}
impl<S: CommandSource> Sealed for RequiredArgumentBuilder<S> {}

/// 辅助宏：为我们的类型实现 `ArgumentBuilder` 的重复代码。
macro_rules! impl_boilerplate_argument_builder {
    ($S:ident) => {
        fn then(mut self, argument: impl Into<DetachedNode<$S>>) -> Self {
            assert!(
                self.target().is_none(),
                "Cannot add children to a redirected node"
            );
            let node = argument.into();
            assert!(
                !matches!(node, DetachedNode::Command(_)),
                "Cannot add a CommandDetachedNode as a child of a builder"
            );

            self.common.arguments.insert(node.name(), node);
            self
        }

        fn command(&self) -> Option<Command<$S>> {
            self.common.command.clone()
        }

        fn executes_arc(mut self, command: Arc<dyn CommandExecutor<$S> + 'static>) -> Self {
            self.common.command = Some(command);
            self
        }

        fn requires(mut self, requirement: impl Into<Requirement<$S>>) -> Self {
            self.common.requirements.0.push(requirement.into());
            self
        }

        fn overwrite_requirements(mut self, requirements: Requirements<$S>) -> Self {
            self.common.requirements = requirements;
            self
        }

        fn redirect(self, redirection: impl Into<Redirection>) -> Self {
            self.forward(redirection.into(), RedirectModifier::KeepSource, false)
        }

        fn redirect_with_modifier(
            self,
            redirection: impl Into<Redirection>,
            redirect_modifier: RedirectModifier<$S>,
        ) -> Self {
            self.forward(redirection.into(), redirect_modifier, false)
        }

        fn fork(
            self,
            redirection: impl Into<Redirection>,
            redirect_modifier: RedirectModifier<$S>,
        ) -> Self {
            self.forward(redirection.into(), redirect_modifier, true)
        }

        fn forward(
            mut self,
            redirection: impl Into<Redirection>,
            redirect_modifier: RedirectModifier<$S>,
            fork: bool,
        ) -> Self {
            assert!(
                self.common.arguments.is_empty(),
                "Cannot forward a node with children. The node must have no children to redirect somewhere else"
            );
            self.common.target = Some(redirection.into());
            self.common.modifier = redirect_modifier;
            self.common.forks = fork;
            self
        }

        fn arguments(&self) -> &FxHashMap<String, DetachedNode<$S>> {
            &self.common.arguments
        }

        fn target(&self) -> Option<Redirection> {
            self.common.target.clone()
        }

        fn redirect_modifier(&self) -> RedirectModifier<$S> {
            self.common.modifier.clone()
        }

        fn forks(&self) -> bool {
            self.common.forks
        }

        fn id(&self) -> GlobalNodeId {
            self.common.global_id
        }
    };
}

/// 辅助宏：为每个构建器生成 `From` 实现块。
macro_rules! impl_builder_from_impls {
    ($builder: ident => $detached_node: ident) => {
        impl<S: CommandSource> From<$builder<S>> for $detached_node<S> {
            fn from(value: $builder<S>) -> Self {
                value.build()
            }
        }

        impl<S: CommandSource> From<$builder<S>> for DetachedNode<S> {
            fn from(value: $builder<S>) -> Self {
                value.build().into()
            }
        }
    };
}

impl_builder_from_impls!(LiteralArgumentBuilder => LiteralDetachedNode);
impl_builder_from_impls!(CommandArgumentBuilder => CommandDetachedNode);
impl_builder_from_impls!(RequiredArgumentBuilder => ArgumentDetachedNode);

impl<S: CommandSource> LiteralArgumentBuilder<S> {
    /// 根据字面量创建新的 [`LiteralArgumentBuilder`]。
    pub fn new(literal: impl Into<Cow<'static, str>>) -> Self {
        Self {
            common: CommonArgumentBuilder::new(),
            literal: literal.into(),
        }
    }
}

impl<S: CommandSource> CommandArgumentBuilder<S> {
    /// 根据字面量和命令描述创建新的 [`CommandArgumentBuilder`]。
    pub fn new(
        literal: impl Into<Cow<'static, str>>,
        description: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self {
            common: CommonArgumentBuilder::new(),
            literal: literal.into(),
            description: description.into(),
            source: None,
        }
    }

    /// 设置注册此命令的来源（例如插件名称）。
    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }
}

impl<S: CommandSource> RequiredArgumentBuilder<S> {
    /// 根据名称和参数类型创建新的 [`RequiredArgumentBuilder`]。
    pub fn new(
        name: impl Into<Cow<'static, str>>,
        arg_type: impl AnyArgumentType<S> + 'static,
    ) -> Self {
        Self {
            common: CommonArgumentBuilder::new(),
            name: name.into(),
            argument_type: Arc::new(arg_type),
            suggestion_provider: None,
        }
    }

    /// 为 `ArgumentDetachedNode` 设置此构建器的 [`SuggestionProvider`]。
    #[must_use]
    pub fn suggests(self, provider: impl SuggestionProvider<S> + 'static) -> Self {
        self.suggests_arc(Arc::new(provider))
    }

    /// 为 `ArgumentDetachedNode` 设置此构建器的 [`SuggestionProvider`]。
    #[must_use]
    pub fn suggests_arc(mut self, provider: Arc<dyn SuggestionProvider<S>>) -> Self {
        self.suggestion_provider = Some(provider);
        self
    }
}

impl<S: CommandSource> ArgumentBuilder<S, LiteralDetachedNode<S>> for LiteralArgumentBuilder<S> {
    impl_boilerplate_argument_builder!(S);

    fn build(self) -> LiteralDetachedNode<S> {
        let mut node = LiteralDetachedNode::new(
            self.common.global_id,
            self.literal,
            self.common.command,
            self.common.requirements,
            self.common.target,
            self.common.modifier,
            self.common.forks,
        );
        node.children = self.common.arguments;
        node
    }
}

impl<S: CommandSource> ArgumentBuilder<S, CommandDetachedNode<S>> for CommandArgumentBuilder<S> {
    impl_boilerplate_argument_builder!(S);

    fn build(self) -> CommandDetachedNode<S> {
        let mut node = CommandDetachedNode::new(
            self.common.global_id,
            self.literal,
            self.description,
            self.common.command,
            self.common.requirements,
            self.common.target,
            self.common.modifier,
            self.common.forks,
        );
        node.meta.source = self.source;
        node.children = self.common.arguments;
        node
    }
}

impl<S: CommandSource> ArgumentBuilder<S, ArgumentDetachedNode<S>> for RequiredArgumentBuilder<S> {
    impl_boilerplate_argument_builder!(S);

    fn build(self) -> ArgumentDetachedNode<S> {
        let mut node = ArgumentDetachedNode::new(
            self.common.global_id,
            self.name,
            self.argument_type,
            self.common.command,
            self.common.requirements,
            self.common.target,
            self.common.modifier,
            self.common.forks,
            self.suggestion_provider,
        );
        node.children = self.common.arguments;
        node
    }
}

#[cfg(test)]
mod test {
    use crate::argument_builder::{ArgumentBuilder, argument, command, literal};
    use crate::argument_types::core::double::DoubleArgumentType;
    use crate::argument_types::core::integer::IntegerArgumentType;
    use crate::argument_types::core::string::StringArgumentType;
    use crate::errors::error_types;
    use crate::node::Redirection;
    use crate::node::attached::AttachedNode;
    use crate::node::tree::Tree;
    use crate::source::DummySource;
    use crate::string_reader::StringReader;

    #[test]
    fn literal_one() {
        let builder = literal::<DummySource>("test");
        let node = builder.build();

        assert_eq!(node.meta.literal, "test");
    }

    #[test]
    fn required_one() {
        let builder = argument::<DummySource>("test", IntegerArgumentType::new(1, 10));
        let node = builder.build();

        assert_eq!(node.meta.name, "test");

        let mut reader1 = StringReader::new("5");
        let mut reader2 = StringReader::new("11");

        let boxed_result = node
            .meta
            .argument_type
            .parse(&mut reader1)
            .expect("解析本不应出错");
        let result = boxed_result.downcast::<i32>().expect("向下转型本不应失败");
        assert_eq!(result, Box::new(5));

        let error = node
            .meta
            .argument_type
            .parse(&mut reader2)
            .expect_err("解析本应出错，因为 11 超出了范围");
        assert!(error.is(&error_types::INTEGER_TOO_HIGH));
    }

    #[test]
    fn literal_multiple() {
        let mut builder = command::<DummySource>("letter", "A test command");
        for letter in 'a'..='z' {
            // 为参数的每个字母添加一个节点。
            let letter_string = letter.to_string();
            builder = builder.then(literal(letter_string));
        }

        let node = builder.build();
        assert_eq!(node.children.len(), 26);
    }

    #[test]
    fn required_multiple() {
        let builder = command::<DummySource>("test", "A test command")
            .then(argument("number", DoubleArgumentType::any()))
            .then(argument("word", StringArgumentType::SingleWord));

        let node = builder.build();
        assert_eq!(node.children.len(), 2);
    }

    #[test]
    fn redirect() {
        let builder = command::<DummySource>("test", "A test command").redirect(Redirection::Root);

        let mut tree = Tree::<DummySource>::new();
        let node_id = tree.add_child_to_root(builder);

        let node = &tree[node_id];
        let redirect = node.redirect.expect("重定向应当存在，因为它之前已添加");

        let target_id = tree.resolve(redirect).expect("目标应当已正确解析");
        let target = &tree[target_id];

        assert!(matches!(target, AttachedNode::Root(_)));
    }

    #[test]
    #[should_panic = "Cannot forward a node with children. The node must have no children to redirect somewhere else"]
    fn redirect_after_child() {
        let _ = command::<DummySource>("test", "A test command")
            .then(literal("child"))
            .redirect(Redirection::Root);
    }

    #[test]
    #[should_panic = "Cannot add children to a redirected node"]
    fn redirect_before_child() {
        let _ = command::<DummySource>("test", "A test command")
            .redirect(Redirection::Root)
            .then(literal("child"));
    }

    #[test]
    #[should_panic = "Cannot add a CommandDetachedNode as a child of a builder"]
    fn add_command_as_child() {
        let _ = command::<DummySource>("foo", "A test command")
            .then(command("bar", "Another test command"));
    }
}
