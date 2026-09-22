use crate::context::string_range::StringRange;
use crate::errors::command_syntax_error::CommandSyntaxError;
use crate::errors::error_types::DISPATCHER_PARSE_EXCEPTION;
use crate::node::attached::NodeId;
use crate::node::dispatcher::{CommandDispatcher, ResultConsumer};
use crate::node::tree::Tree;
use crate::node::{Command, RedirectModifier};
use crate::source::{CommandSource, DummySource, ReturnValue};
use papokin_util::text::TextComponent;
use rustc_hash::FxHashMap;
use std::any::Any;
use std::sync::Arc;

/// 表示该链的当前阶段。
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Stage {
    MODIFY,
    EXECUTE,
}

/// 表示解析后的节点。
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct ParsedNode {
    pub node: NodeId,
    pub range: StringRange,
}

/// 表示涉及某个节点的补全建议上下文。
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct SuggestionContext {
    pub parent: NodeId,
    pub starting_position: usize,
}

/// 表示任意类型的已解析参数。
pub struct ParsedArgument {
    /// 此已解析参数的范围。
    pub range: StringRange,

    /// 此已解析参数的结果。
    pub result: Box<dyn Any + Send + Sync>,
}

impl ParsedArgument {
    /// 根据其范围和结果值创建新的 [`ParsedArgument`]。
    #[must_use]
    pub fn new(range: StringRange, result: Box<dyn Any + Send + Sync>) -> Self {
        Self { range, result }
    }
}

/// 表示运行命令时所用的上下文。
#[derive(Clone)]
pub struct CommandContext<'a, S: CommandSource = DummySource> {
    /// 执行命令的来源。
    pub source: Arc<S>,

    /// 作为命令执行的输入字符串。
    pub input: String,

    /// 已解析并验证过的参数，
    /// 可被获取以执行命令。
    pub arguments: FxHashMap<String, Arc<ParsedArgument>>,

    /// 此上下文所关联的树。
    pub tree: &'a Tree<S>,

    /// 此上下文将使用的根，绑定到该树。
    /// 不过它不一定是树的根节点。
    pub root: NodeId,

    /// 命令的所有已解析节点。
    pub nodes: Vec<ParsedNode>,

    /// 输入的字符串范围。
    pub range: StringRange,

    /// 此上下文的子上下文。
    pub child: Option<Arc<Self>>,

    /// 此上下文的重定向修饰符。
    pub modifier: RedirectModifier<S>,

    /// 此上下文是否分叉。
    pub forks: bool,

    /// 存储在此上下文中的命令，其
    /// 运行以获得命令结果。
    pub command: Option<Command<S>>,
}

impl<S: CommandSource> std::ops::Deref for CommandContext<'_, S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.source
    }
}

impl<S: CommandSource> CommandContext<'_, S> {
    /// 使用提供的来源复制此上下文。
    #[must_use]
    pub fn with_source(&self, source: Arc<S>) -> Self {
        Self {
            source,
            input: self.input.clone(),
            arguments: self.arguments.clone(),
            nodes: self.nodes.clone(),
            range: self.range,
            child: self.child.clone(),
            modifier: self.modifier.clone(),
            forks: self.forks,
            command: self.command.clone(),
            tree: self.tree,
            root: self.root,
        }
    }

    ///返回紧邻此节点下方的子节点。
    #[must_use]
    pub const fn get_child(&self) -> Option<&Arc<Self>> {
        self.child.as_ref()
    }

    ///返回不包含源自此节点的子节点的那个子节点。
    /// 这可能返回其自身。
    #[must_use]
    pub fn get_last_child(&self) -> &Self {
        let mut current_child = self;
        while let Some(child) = &current_child.child {
            current_child = child;
        }
        current_child
    }

    ///返回对类型为 `T` 的特定参数的引用。
    /// 若失败，则返回带有相应消息的错误。
    ///
    /// 理想情况下应与 `?` 运算符配合使用。
    ///
    /// # Example
    /// 一个简单示例，接收从节点指定的两个参数
    /// 并将它们的和作为 `Executor` 的状态输出返回：
    /// ```
    /// use papokin_command::context::command_context::CommandContext;
    /// use papokin_command::node::{CommandExecutor, CommandExecutorResult};
    ///
    /// struct Executor;
    /// impl CommandExecutor for Executor {
    ///     fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
    ///         // The `get_argument` method returns a `Result<&i32, CommandSyntaxError>`.
    ///         // We apply the `?` operator first, propagating the `CommandSyntaxError` if contained.
    ///         // Finally, we dereference the `&i32`, as `i32` implements Copy.
    ///         let operand1: i32 = *context.get_argument("operand1")?;
    ///         let operand2: i32 = *context.get_argument("operand2")?;
    ///         Ok(operand1 + operand2)
    ///     }
    /// }
    /// ```
    pub fn get_argument<T: 'static>(&self, name: &str) -> Result<&T, CommandSyntaxError> {
        // 以下错误绝不应由用户输入引发。
        // 如果返回此注释下方的错误，则意味着命令
        // 定义不当。
        //
        // 不过，我们提供有用的错误，而不是 panic。

        let arg = self.arguments.get(name).ok_or_else(|| {
            DISPATCHER_PARSE_EXCEPTION.create_without_context(TextComponent::text(format!(
                "找不到名称为 '{name}' 的参数"
            )))
        })?;
        let dyn_ref = &*arg.result;
        dyn_ref.downcast_ref::<T>().ok_or_else(|| {
            DISPATCHER_PARSE_EXCEPTION.create_without_context(TextComponent::text(format!(
                "无法将参数 '{name}' 向下转型"
            )))
        })
    }
}

/// 表示由 [`CommandContext`] 组成的链表，前一个将后一个作为子节点链接。
#[derive(Clone)]
pub struct ContextChain<'a, S: CommandSource = DummySource> {
    /// 此上下文链的修饰符。
    modifiers: Vec<Arc<CommandContext<'a, S>>>,

    /// 要执行的特定 [`CommandContext`]。
    execute: Arc<CommandContext<'a, S>>,
}

impl<'a, S: CommandSource> ContextChain<'a, S> {
    /// 从一个上下文向量以及其中待执行的一个创建新的上下文链。
    ///
    /// # Panics
    ///
    /// 如果给定的 `execute` 不可执行则 panic。
    #[must_use]
    pub fn new(
        modifiers: Vec<Arc<CommandContext<'a, S>>>,
        execute: Arc<CommandContext<'a, S>>,
    ) -> Self {
        assert!(
            execute.command.is_some(),
            "Expected last command in chain to be executable"
        );
        Self { modifiers, execute }
    }

    /// 尝试展平一个 [`CommandContext`]。如果没有命令
    /// 在链的末端可用，则返回 [`None`]。
    #[must_use]
    pub fn try_flatten(root: &CommandContext<'a, S>) -> Option<Self> {
        let mut modifiers = Vec::new();
        let mut current = Arc::new(root.clone());
        loop {
            if let Some(child) = current.get_child() {
                let temp_child = child.clone();
                modifiers.push(current);
                current = temp_child;
            } else {
                return current
                    .command
                    .is_some()
                    .then(|| Self::new(modifiers, current));
            }
        }
    }

    /// 使用提供的详情运行给定的修改器。
    pub fn run_modifier(
        modifier: &CommandContext<'a, S>,
        source: &Arc<S>,
        result_consumer: &dyn ResultConsumer<S>,
        forked_mode: bool,
    ) -> Result<Vec<Arc<S>>, CommandSyntaxError> {
        let source_modifier = &modifier.modifier;

        if matches!(source_modifier, RedirectModifier::KeepSource) {
            return Ok(vec![source.clone()]);
        }

        let context_to_use = modifier.with_source(source.clone());
        let mut result = source_modifier.sources(&context_to_use);

        if result.is_err() {
            result_consumer.on_command_completion(&context_to_use, ReturnValue::Failure);
            if forked_mode {
                result = Ok(vec![]);
            }
        }

        result
    }

    /// 运行给定的可执行文件，成功时返回 [`i32`]。
    ///
    /// # Panics
    ///
    /// 如果提供的 `executable` 无法执行则 panic。
    pub fn run_executable(
        executable: &CommandContext<'a, S>,
        source: &Arc<S>,
        result_consumer: &dyn ResultConsumer<S>,
        forked_mode: bool,
    ) -> Result<i32, CommandSyntaxError> {
        let context_to_use = executable.with_source(source.clone());

        let mut result = executable.command.as_ref().map_or_else(
            || panic!("`executable` 应当是可执行的"),
            |command| command.execute(&context_to_use),
        );

        if let Ok(result) = result {
            result_consumer.on_command_completion(&context_to_use, ReturnValue::Success(result));
            Ok(if forked_mode { 1 } else { result })
        } else {
            result_consumer.on_command_completion(&context_to_use, ReturnValue::Failure);
            if forked_mode {
                result = Ok(0);
            }
            result
        }
    }

    /// 执行链中的所有上下文，返回最终结果。
    pub fn execute_all(
        &self,
        source: &Arc<S>,
        result_consumer: &dyn ResultConsumer<S>,
    ) -> Result<i32, CommandSyntaxError> {
        if self.modifiers.is_empty() {
            return Self::run_executable(&self.execute, source, result_consumer, false);
        }

        let mut forked_mode = false;
        let mut current_sources: Vec<Arc<S>> = vec![source.clone()];

        for modifier in &self.modifiers {
            forked_mode |= modifier.forks;

            let mut next_sources = Vec::new();
            for source in current_sources {
                let mut to_add =
                    Self::run_modifier(modifier, &source, result_consumer, forked_mode)?;
                next_sources.append(&mut to_add);
            }
            if next_sources.is_empty() {
                return Ok(0);
            }
            current_sources = next_sources;
        }

        let mut result = 0;
        for execution_source in current_sources {
            result += Self::run_executable(
                &self.execute,
                &execution_source,
                result_consumer,
                forked_mode,
            )?;
        }

        Ok(result)
    }

    /// 获取此上下文的当前阶段。
    #[must_use]
    pub const fn get_stage(&self) -> Stage {
        if self.modifiers.is_empty() {
            Stage::EXECUTE
        } else {
            Stage::MODIFY
        }
    }

    /// 获取此链顶层上下文的引用。
    #[must_use]
    pub fn get_top_context(&'_ self) -> &'_ Arc<CommandContext<'a, S>> {
        if self.modifiers.is_empty() {
            &self.execute
        } else {
            &self.modifiers[0]
        }
    }

    /// 获取此链顶层上下文的可变引用。
    pub fn get_top_context_mut(&'_ mut self) -> &mut Arc<CommandContext<'a, S>> {
        if self.modifiers.is_empty() {
            &mut self.execute
        } else {
            &mut self.modifiers[0]
        }
    }

    /// 获取此链的下一阶段。
    #[must_use]
    pub fn next_stage(&self) -> Option<Self> {
        if self.modifiers.is_empty() {
            None
        } else {
            Some(Self::new(
                self.modifiers[1..].to_vec(),
                self.execute.clone(),
            ))
        }
    }
}

/// 一个帮助创建 [`CommandContext`] 的构建器。
///
/// 此构建器的生命周期绑定到提供给它的调度器。
#[derive(Clone)]
pub struct CommandContextBuilder<'a, S: CommandSource = DummySource> {
    /// 此构建器所关联的调度器。
    pub dispatcher: &'a CommandDispatcher<S>,

    /// 执行命令的来源。
    pub source: Arc<S>,

    /// 已解析并验证过的参数，
    /// 可被获取以执行命令。
    pub arguments: FxHashMap<String, Arc<ParsedArgument>>,

    /// 此上下文将使用的根，绑定到该树
    /// 不过它不一定是树的根节点。
    pub root: NodeId,

    /// 命令的所有已解析节点。
    pub nodes: Vec<ParsedNode>,

    /// 输入的字符串范围。
    pub range: StringRange,

    /// 此上下文的子上下文。
    pub child: Option<Box<Self>>,

    /// 此上下文的重定向修饰符。
    pub modifier: RedirectModifier<S>,

    /// 此上下文是否分叉。
    pub forks: bool,

    /// 存储在此上下文中的命令，其
    /// 运行以获得命令结果。
    pub command: Option<Command<S>>,
}

impl<'a, S: CommandSource> CommandContextBuilder<'a, S> {
    /// 根据初始化所需的属性创建新的 [`CommandContextBuilder`]。
    ///
    /// 注意，构建器的生命周期绑定到提供给它的调度器。
    #[must_use]
    pub fn new(
        dispatcher: &'a CommandDispatcher<S>,
        source: Arc<S>,
        root: NodeId,
        start: usize,
    ) -> Self {
        CommandContextBuilder {
            dispatcher,
            source,
            arguments: FxHashMap::default(),
            root,
            nodes: Vec::new(),
            range: StringRange::at(start),
            child: None,
            modifier: RedirectModifier::KeepSource,
            forks: false,
            command: None,
        }
    }

    /// 构建所需的 [`CommandContext`]，过程中消耗自身。
    #[must_use]
    pub fn build(self, input: &str) -> CommandContext<'a, S> {
        CommandContext {
            source: self.source,
            input: input.to_string(),
            arguments: self.arguments,
            tree: &self.dispatcher.tree,
            root: self.root,
            nodes: self.nodes,
            range: self.range,
            child: self.child.map(|child| Arc::new(child.build(input))),
            modifier: self.modifier,
            forks: self.forks,
            command: self.command,
        }
    }

    /// 修改自身以设置为新的来源集合。
    pub fn with_source(&mut self, source: Arc<S>) {
        self.source = source;
    }

    /// 修改自身并添加一个新参数。
    pub fn with_argument(&mut self, name: String, argument: Arc<ParsedArgument>) {
        self.arguments.insert(name, argument);
    }

    /// 修改自身以设置为新的命令集合。
    pub fn with_command(&mut self, command: Option<Command<S>>) {
        self.command = command;
    }

    /// 修改自身并向此构建器添加一个新节点。
    pub fn with_node(&mut self, node: NodeId, range: StringRange) {
        self.nodes.push(ParsedNode { node, range });
        self.range = StringRange::encompass(self.range, range);
        self.modifier = self.dispatcher.tree[node].modifier().clone();
        self.forks = self.dispatcher.tree[node].forks();
    }

    /// 修改自身以设置为新的子节点集合。
    pub fn with_child(&mut self, child: Self) {
        self.child = Some(Box::new(child));
    }

    /// 修改此构建器的最后一个子节点。
    #[must_use]
    pub fn last_child(&self) -> &Self {
        let mut result = self;
        while let Some(child) = &result.child {
            result = child;
        }
        result
    }

    /// 根据提供的光标位置创建 [`SuggestionContext`]。
    ///
    /// # Panics
    ///
    /// 如果在光标之前找不到节点则 panic。
    #[must_use]
    pub fn find_suggestion_context(&self, cursor: usize) -> SuggestionContext {
        assert!(
            self.range.start <= cursor,
            "Could not find node before cursor"
        );
        if self.range.end < cursor {
            self.child.as_ref().map_or_else(
                || {
                    self.nodes.last().as_ref().map_or_else(
                        || SuggestionContext {
                            parent: self.root,
                            starting_position: self.range.start,
                        },
                        |last_node| SuggestionContext {
                            parent: last_node.node,
                            starting_position: last_node.range.end + 1,
                        },
                    )
                },
                |child| child.find_suggestion_context(cursor),
            )
        } else {
            let mut previous = self.root;
            for node in &self.nodes {
                let node_range = node.range;
                if (node_range.start..=node_range.end).contains(&cursor) {
                    return SuggestionContext {
                        parent: previous,
                        starting_position: node_range.start,
                    };
                }
                previous = node.node;
            }
            SuggestionContext {
                parent: previous,
                starting_position: self.range.start,
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::argument_builder::{ArgumentBuilder, CommandArgumentBuilder};
    use crate::context::command_context::{
        CommandContext, CommandContextBuilder, ContextChain, ParsedArgument, Stage,
    };
    use crate::context::string_range::StringRange;
    use crate::errors::command_syntax_error::CommandSyntaxError;
    use crate::node::dispatcher::{CommandDispatcher, EmptyResultConsumer};
    use crate::node::tree::ROOT_NODE_ID;
    use crate::node::{CommandExecutor, CommandExecutorResult, RedirectModifier, Redirection};
    use crate::source::DummySource;
    use papokin_util::math::vector3::Vector3;
    use std::sync::Arc;

    struct TenExecutor;
    impl CommandExecutor for TenExecutor {
        fn execute(&self, _context: &CommandContext) -> CommandExecutorResult {
            Ok(10)
        }
    }

    // 用于测试目的
    fn builder(dispatcher: &'_ CommandDispatcher) -> CommandContextBuilder<'_> {
        let mut builder =
            CommandContextBuilder::new(dispatcher, Arc::new(DummySource::dummy()), ROOT_NODE_ID, 0);

        let parsed_argument = ParsedArgument::new(StringRange::between(0, 1), Box::new(6789i32));

        builder.with_argument("foo".to_string(), Arc::new(parsed_argument));

        builder
    }

    #[test]
    fn get_argument() -> Result<(), CommandSyntaxError> {
        let dispatcher = CommandDispatcher::new();
        let builder = builder(&dispatcher);

        let context = builder.build("6789");
        assert_eq!(context.get_argument::<i32>("foo")?, &6789);

        Ok(())
    }

    #[test]
    fn get_nonexistent_argument() {
        let dispatcher = CommandDispatcher::new();
        let builder = builder(&dispatcher);

        let context = builder.build("6789");
        assert!(context.get_argument::<i32>("bar").is_err());
    }

    #[test]
    fn get_different_type_argument() {
        let dispatcher = CommandDispatcher::new();
        let builder = builder(&dispatcher);

        let context = builder.build("6789");
        assert!(context.get_argument::<f32>("foo").is_err());
    }

    #[test]
    fn execute_single_command_chain() {
        let mut dispatcher = CommandDispatcher::new();
        dispatcher
            .register(CommandArgumentBuilder::new("foo", "A test command").executes(TenExecutor));

        let source = Arc::new(DummySource::dummy());
        let result = dispatcher.parse_input("foo", &source);
        let top_context = result.context.build("foo");
        let chain = ContextChain::try_flatten(&top_context)
            .expect("上下文应当已正确扁平化，因为它有要执行的命令");

        assert_eq!(chain.execute_all(&source, &EmptyResultConsumer), Ok(10));
    }

    #[test]
    fn execute_redirected_command_chain() {
        let mut dispatcher = CommandDispatcher::new();
        dispatcher
            .register(CommandArgumentBuilder::new("foo", "A test command").executes(TenExecutor));
        dispatcher.register(
            CommandArgumentBuilder::new("bar", "Another test command").redirect(Redirection::Root),
        );

        let source = Arc::new(DummySource::dummy());
        let result = dispatcher.parse_input("bar foo", &source);
        let top_context = result.context.build("bar foo");
        let chain = ContextChain::try_flatten(&top_context)
            .expect("上下文应当已正确扁平化，因为它有要执行的命令");

        assert_eq!(chain.execute_all(&source, &EmptyResultConsumer), Ok(10));
    }

    #[test]
    fn single_stage_execution() {
        let mut dispatcher = CommandDispatcher::new();
        dispatcher
            .register(CommandArgumentBuilder::new("foo", "A test command").executes(TenExecutor));

        let source = Arc::new(DummySource::dummy());
        let result = dispatcher.parse_input("foo", &source);
        let top_context = result.context.build("foo");
        let chain = ContextChain::try_flatten(&top_context)
            .expect("上下文应当已正确扁平化，因为它有要执行的命令");

        assert_eq!(chain.get_stage(), Stage::EXECUTE);
        assert!(chain.next_stage().is_none());
    }

    #[test]
    fn multi_stage_execution() {
        let mut dispatcher = CommandDispatcher::new();
        dispatcher
            .register(CommandArgumentBuilder::new("foo", "A test command").executes(TenExecutor));
        dispatcher.register(
            CommandArgumentBuilder::new("bar", "Another test command").redirect(Redirection::Root),
        );
        dispatcher.register(
            CommandArgumentBuilder::new("qux", "Yet another test command")
                .redirect(Redirection::Root),
        );

        let source = Arc::new(DummySource::dummy());
        let result = dispatcher.parse_input("bar qux foo", &source);
        let top_context = result.context.build("bar qux foo");
        let chain = ContextChain::try_flatten(&top_context)
            .expect("上下文应当已正确扁平化，因为它有要执行的命令");
        assert_eq!(chain.get_stage(), Stage::MODIFY);

        let chain2 = chain.next_stage().expect("应当存在下一个阶段");
        assert_eq!(chain2.get_stage(), Stage::MODIFY);

        let chain3 = chain2.next_stage().expect("应当存在下一个阶段");
        assert_eq!(chain3.get_stage(), Stage::EXECUTE);
        assert!(chain3.next_stage().is_none());
    }

    #[test]
    fn missing_command() {
        let mut dispatcher = CommandDispatcher::new();
        dispatcher.register(CommandArgumentBuilder::new("foo", "A test command"));

        let source = DummySource::dummy();
        let result = dispatcher.parse_input("foo", &source);
        let top_context = result.context.build("foo");

        assert!(ContextChain::try_flatten(&top_context).is_none());
    }

    struct CustomExecutor;
    impl CommandExecutor for CustomExecutor {
        fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
            let source = &context.source;
            assert_eq!(source.position, Vector3::new(0f64, 10f64, 0f64));
            Ok(1)
        }
    }

    #[test]
    fn multi_stage_modifier_execution() {
        let mut dispatcher = CommandDispatcher::new();
        dispatcher.register(
            CommandArgumentBuilder::new("foo", "A test command").executes(CustomExecutor),
        );
        dispatcher.register(
            CommandArgumentBuilder::new("bar", "Another test command").redirect_with_modifier(
                Redirection::Root,
                RedirectModifier::Custom(Arc::new(|context: &CommandContext<DummySource>| {
                    let mut new_source = context.source.as_ref().clone();
                    new_source.position = Vector3::new(0f64, 10f64, 0f64);
                    Ok(vec![Arc::new(new_source)])
                })),
            ),
        );
        let source = Arc::new(DummySource::dummy());
        let result = dispatcher.parse_input("bar foo", source.as_ref());
        let top_context = result.context.build("bar foo");
        let chain = ContextChain::try_flatten(&top_context)
            .expect("上下文应当已正确扁平化，因为它有要执行的命令");
        assert_eq!(chain.get_top_context().source.position, Vector3::default());
        let chain2 = chain.next_stage().expect("应当存在下一个阶段");
        assert!(chain2.next_stage().is_none());
        let res = chain.execute_all(&source, dispatcher.consumer.as_ref());
        assert!(res.is_ok_and(|val| val == 1));
    }
}
