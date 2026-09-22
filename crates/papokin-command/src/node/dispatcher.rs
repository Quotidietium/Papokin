use crate::argument_builder::{ArgumentBuilder, CommandArgumentBuilder};
use crate::context::command_context::{CommandContext, CommandContextBuilder, ContextChain};
use crate::errors::command_syntax_error::CommandSyntaxError;
use crate::errors::error_types::{
    DISPATCHER_EXPECTED_ARGUMENT_SEPARATOR, DISPATCHER_UNKNOWN_ARGUMENT,
    DISPATCHER_UNKNOWN_COMMAND, LiteralCommandErrorType,
};
use crate::node::Redirection;
use crate::node::attached::{CommandNodeId, NodeId};
use crate::node::detached::CommandDetachedNode;
use crate::node::tree::{NodeIdClassification, ROOT_NODE_ID, Tree};
use crate::source::{CommandSource, DummySource, ReturnValue};
use crate::string_reader::StringReader;
use crate::suggestion::suggestions::{Suggestions, SuggestionsBuilder};
use papokin_data::translation::java::COMMAND_CONTEXT_HERE;
use papokin_protocol::java::client::play::CommandSuggestion;
use papokin_util::text::TextComponent;
use papokin_util::text::click::ClickEvent;
use papokin_util::text::color::{Color, NamedColor};
use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::BTreeMap;
use std::sync::Arc;

pub const ARG_SEPARATOR: &str = " ";
pub const ARG_SEPARATOR_CHAR: char = ' ';

pub const USAGE_OPTIONAL_OPEN: &str = "[";
pub const USAGE_OPTIONAL_CLOSE: &str = "]";
pub const USAGE_REQUIRED_OPEN: &str = "(";
pub const USAGE_REQUIRED_CLOSE: &str = ")";
pub const USAGE_OR: &str = "|";

/// 在无法解析重定向时抛出。
/// 正常情况下不应发生这种情况，只有在命令配置错误时才会出现。
pub const UNRESOLVED_REDIRECT: LiteralCommandErrorType =
    LiteralCommandErrorType::new("无法解析到节点的重定向");

/// 表示解析的结果。
pub struct ParsingResult<'a, S: CommandSource = DummySource> {
    pub context: CommandContextBuilder<'a, S>,
    pub errors: FxHashMap<NodeId, CommandSyntaxError>,
    pub reader: StringReader<'static>,
}

/// 实现此 trait 的结构体能够在命令补全时执行。
pub trait ResultConsumer<S: CommandSource = DummySource>: Sync + Send {
    fn on_command_completion(&self, context: &CommandContext<S>, result: ReturnValue);
}

/// 一个什么都不做的 [`ResultConsumer`]。
pub struct EmptyResultConsumer;

impl<S: CommandSource> ResultConsumer<S> for EmptyResultConsumer {
    fn on_command_completion(&self, _context: &CommandContext<S>, _result: ReturnValue) {}
}

/// 一个 [`ResultConsumer`]，将给定的结果转交给所提供的来源处理。
pub struct ResultDeferrer;

impl<S: CommandSource> ResultConsumer<S> for ResultDeferrer {
    fn on_command_completion(&self, context: &CommandContext<S>, result: ReturnValue) {
        context.source.call_result(result);
    }
}

/// 核心命令调度器，用于注册、解析和执行命令。
///
/// 该调度器内部存储了一个 [`Tree`]。详见其文档
/// 了解关于节点的更多信息。
#[derive(Clone)]
pub struct CommandDispatcher<S: CommandSource = DummySource> {
    pub tree: Tree<S>,
    pub consumer: Arc<dyn ResultConsumer<S>>,

    /// 已通过服务器配置关闭的命令的主要名称
    /// 配置。被禁用的命令表现得如同不存在：它
    /// 无法执行，也不会出现在命令列表与补全建议中。
    disabled: FxHashSet<String>,

    /// 插件来源当前已卸载的命令。这些命令会被保留
    /// 与配置禁用分开，这样再次加载插件时可以
    /// 使其命令可用，同时不覆盖服务器配置。
    inactive_plugin_commands: FxHashSet<String>,
}

impl<S: CommandSource> Default for CommandDispatcher<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: CommandSource> CommandDispatcher<S> {
    /// 使用新的 [`Tree`] 创建新的 [`CommandDispatcher`]。
    #[must_use]
    pub fn new() -> Self {
        Self::from_existing_tree(Tree::new())
    }

    /// 从一个已存在的树创建该 [`CommandDispatcher`]。
    #[must_use]
    pub fn from_existing_tree(tree: Tree<S>) -> Self {
        Self {
            tree,
            consumer: Arc::new(ResultDeferrer),
            disabled: FxHashSet::default(),
            inactive_plugin_commands: FxHashSet::default(),
        }
    }

    fn normalize_command_name(name: &str) -> String {
        name.to_ascii_lowercase()
    }

    /// 关闭一个命令。被禁用命令的主名称会记录在此处
    /// 使它无法再被执行、列出或建议，无论
    /// 它位于哪个内部分发器上。
    pub fn disable_command(&mut self, name: impl Into<String>) {
        let name = name.into();
        self.disabled.insert(Self::normalize_command_name(&name));
    }

    /// 使插件命令在重新注册前保持不可用。
    fn deactivate_plugin_command(&mut self, name: impl AsRef<str>) {
        self.inactive_plugin_commands
            .insert(Self::normalize_command_name(name.as_ref()));
    }

    /// 使插件命令及其别名在注册前保持不可用。
    pub fn deactivate_plugin_command_and_aliases(&mut self, name: &str) {
        let primary_name = self.primary_command_name(name);
        for alias in self.tree_alias_names(&primary_name) {
            self.deactivate_plugin_command(alias);
        }
        self.deactivate_plugin_command(primary_name);
    }

    /// 使某个插件来源注册的所有根命令不可用。
    pub fn deactivate_commands_from_source(&mut self, source: &str) {
        let primary_names = self
            .tree
            .get_root_children()
            .into_iter()
            .filter_map(|node_id| {
                let metadata = &self.tree[node_id].meta;
                (metadata.source.as_deref() == Some(source)).then(|| metadata.literal.to_string())
            })
            .collect::<Vec<_>>();

        for primary_name in primary_names {
            self.deactivate_plugin_command_and_aliases(&primary_name);
        }
    }

    ///若命令已被禁用或其插件未激活，则返回 `true`。
    #[must_use]
    pub fn is_disabled(&self, name: &str) -> bool {
        Self::contains_command_name(&self.disabled, name)
            || Self::contains_command_name(&self.inactive_plugin_commands, name)
    }

    fn contains_command_name(commands: &FxHashSet<String>, name: &str) -> bool {
        let name = Self::normalize_command_name(name);
        if commands.contains(&name) {
            return true;
        }

        let base_name = name.trim_start_matches('/');
        commands.contains(base_name)
            || commands.contains(&format!("/{base_name}"))
            || commands.contains(&format!("//{base_name}"))
    }

    fn reactivate_plugin_command(&mut self, name: &str) {
        let name = Self::normalize_command_name(name);
        let base_name = name.trim_start_matches('/');
        self.inactive_plugin_commands.remove(&name);
        self.inactive_plugin_commands.remove(base_name);
        self.inactive_plugin_commands
            .remove(&format!("/{base_name}"));
        self.inactive_plugin_commands
            .remove(&format!("//{base_name}"));
    }

    ///若已注册具有给定名称的命令（或别名），则返回 `true`
    /// 基于节点的树上。
    #[must_use]
    pub fn has_command(&self, name: &str) -> bool {
        self.tree.get(name).is_some()
    }

    /// 返回命令 `name` 上记录的来源（所属插件），如果
    /// 该命令存在且带有标签。用于检测跨插件标签
    /// 冲突，因此后续注册会回退到 `plugin:label` 名称。
    #[must_use]
    pub fn get_command_source(&self, name: &str) -> Option<String> {
        let node_id = self.tree.get(name)?;
        self.tree[node_id].meta.source.clone()
    }

    /// 收集每个重定向到该命令的根级别别名的名称
    /// 具有给定的主名称。
    ///
    /// 基于节点的命令将其别名建模为额外的根字面量，这些字面量
    /// 重定向到主节点（见 `register_with_aliases`）。当命令
    /// 被禁用时，我们也需要关闭这些别名，否则玩家
    /// 仍能通过其中之一到达存活的执行器。
    #[must_use]
    pub fn tree_alias_names(&self, primary: &str) -> Vec<String> {
        let primary = Self::normalize_command_name(primary);
        let Some(primary_id) = self.tree.get(&primary) else {
            return Vec::new();
        };
        let primary_node: NodeId = primary_id.into();

        let mut names = Vec::new();
        for child in self.tree.get_root_children() {
            // 按 `NodeId` 索引，以便获得 `AttachedNode` 枚举（它暴露
            // `redirect`/`name`），而不是内部命令节点。
            let node_id: NodeId = child.into();
            let node = &self.tree[node_id];
            if let Some(redirect) = node.redirect()
                && self.tree.resolve(redirect) == Some(primary_node)
            {
                names.push(node.name().to_ascii_lowercase());
            }
        }
        names
    }

    /// 根据别名或主名称解析出命令的主名称。
    #[must_use]
    pub fn primary_command_name(&self, name: &str) -> String {
        let name = Self::normalize_command_name(name);
        if let Some(node_id) = self.tree.get(&name) {
            let node = &self.tree[NodeId::from(node_id)];
            if let Some(redirect) = node.redirect()
                && let Some(resolved) = self.tree.resolve(redirect)
            {
                return self.tree[resolved].name().to_ascii_lowercase();
            }
            return node.name().to_ascii_lowercase();
        }
        name
    }

    /// 提取命令名（第一个以空白分隔的标记）
    /// 原始输入字符串。
    fn command_name(input: &str) -> &str {
        input.split_whitespace().next().unwrap_or("")
    }

    fn unknown_command_error(reader: &StringReader<'_>) -> CommandSyntaxError {
        DISPATCHER_UNKNOWN_COMMAND.create(
            reader,
            TextComponent::text(Self::command_name(reader.string()).to_owned()),
        )
    }

    /// 注册一个随后可被分发的命令。
    /// 返回挂接到树上的节点的本地 ID。
    ///
    /// 注意，至少就当前这套系统而言，还无法
    /// 注销命令。这是因为重定向到了
    /// 可能已注销（释放）的节点。
    pub fn register(&mut self, command_node: impl Into<CommandDetachedNode<S>>) -> CommandNodeId {
        let mut node = command_node.into();
        let name = Self::normalize_command_name(&node.meta.literal);
        node.meta.literal = name.clone().into();
        node.meta.literal_lowercase.clone_from(&name);
        self.reactivate_plugin_command(&name);
        let main_node_id = self.tree.add_child_to_root(node);

        // 对于双斜杠或以斜杠开头的命令（如 //set 或 /set），
        // 自动将另一种斜杠变体注册为别名。
        if let Some(stripped) = name.strip_prefix("//") {
            let single_slash = format!("/{stripped}");
            if self.tree.get(&single_slash).is_none() {
                let main_node = &self.tree[main_node_id];
                let description = main_node.meta.description.clone();
                let mut alias =
                    crate::argument_builder::CommandArgumentBuilder::new(single_slash, description);
                if let Some(executor) = &main_node.owned.command {
                    alias = alias.executes_arc(executor.clone());
                    alias = alias.overwrite_requirements(main_node.owned.requirements.clone());
                }
                alias = alias.redirect(crate::node::Redirection::Local(main_node_id.into()));
                self.tree.add_child_to_root(alias.build());
            }
        } else if let Some(stripped) = name.strip_prefix('/') {
            let double_slash = format!("//{stripped}");
            if self.tree.get(&double_slash).is_none() {
                let main_node = &self.tree[main_node_id];
                let description = main_node.meta.description.clone();
                let mut alias =
                    crate::argument_builder::CommandArgumentBuilder::new(double_slash, description);
                if let Some(executor) = &main_node.owned.command {
                    alias = alias.executes_arc(executor.clone());
                    alias = alias.overwrite_requirements(main_node.owned.requirements.clone());
                }
                alias = alias.redirect(crate::node::Redirection::Local(main_node_id.into()));
                self.tree.add_child_to_root(alias.build());
            }
        }

        main_node_id
    }

    /// 注册一个随后可被分发的命令，连同其
    /// 别名作为第二个参数。返回挂接到树中的节点的本地 ID。
    ///
    /// 在底层，对 `redirect` 和 `executes_arc` 的调用会
    /// 会为每个提供的别名执行。此方法仅为便利而设。
    ///
    /// 注意，至少就当前这套系统而言，还无法
    /// 注销命令。这是因为重定向到了
    /// 可能已注销（释放）的节点。
    pub fn register_with_aliases<Str: AsRef<str>>(
        &mut self,
        command_node: impl Into<CommandDetachedNode<S>>,
        aliases: &[Str],
    ) -> CommandNodeId {
        let main_node_id = self.register(command_node);

        let main_node = &self.tree[main_node_id];
        let description = &main_node.meta.description;

        let mut built_nodes = Vec::with_capacity(aliases.len());

        for alias in aliases {
            let mut alias =
                CommandArgumentBuilder::new(alias.as_ref().to_string(), description.clone());

            // 我们来看一下原始节点持有的数据。
            let reference = &main_node.owned;

            // 如果引用包含执行器，则将其克隆过来。
            // 如果不是，则我们无需检查权限，因为它
            // 将由目标节点完成。
            if let Some(executor) = &reference.command {
                alias = alias.executes_arc(executor.clone());

                // 我们还必须添加相应的需求。
                // 这是因为如果我们只是简单地设置一个执行器，那么
                // 任何玩家都无需任何条件（包括权限）即可执行它！
                //
                // 例如，（假设）为 `/stop` 添加了别名 `/s`，
                // 任何玩家都可用 `/s` 关停服务器！
                alias = alias.overwrite_requirements(reference.requirements.clone());
            }

            // 然后重定向到该节点。
            alias = alias.redirect(Redirection::Local(main_node_id.into()));

            // 构建节点。
            built_nodes.push(alias.build());
        }

        for alias in built_nodes {
            self.register(alias);
        }

        main_node_id
    }

    /// 使用给定的来源执行命令，返回执行结果。
    ///
    /// # Note
    /// 这不会缓存已解析的输入。
    pub fn execute_input(&self, input: &str, source: &S) -> Result<i32, CommandSyntaxError> {
        let mut reader = StringReader::new(input);

        // 被禁用的命令必须表现得如同它在每个视角都不存在
        // 执行路径，而不仅是 `handle_command`。这一兜底机制覆盖
        // 程序化调用方（如 `/execute run <command>`），它们经由
        // 在此经由调度器处理，而不经过 `handle_command`。
        if self.is_disabled(Self::command_name(input)) {
            return Err(Self::unknown_command_error(&reader));
        }

        self.execute_reader(&mut reader, source)
    }

    /// 在 [`StringReader`] 中使用给定的来源执行命令，返回执行结果。
    ///
    /// # Note
    /// 这不会缓存已解析的输入。
    pub fn execute_reader(
        &self,
        reader: &mut StringReader<'_>,
        source: &S,
    ) -> Result<i32, CommandSyntaxError> {
        let parsed = self.parse(reader, source);
        self.execute(parsed)
    }

    /// 执行已从输入解析出的给定结果。
    pub fn execute(&self, parsed: ParsingResult<'_, S>) -> Result<i32, CommandSyntaxError> {
        if parsed.reader.peek().is_some() {
            return if let Some(err) = parsed.errors.values().next() {
                Err(err.clone())
            } else if parsed.context.range.is_empty() {
                Err(Self::unknown_command_error(&parsed.reader))
            } else {
                Err(DISPATCHER_UNKNOWN_ARGUMENT.create(&parsed.reader))
            };
        }

        let command = parsed.reader.string();
        let original_context = parsed.context.build(command);

        match ContextChain::try_flatten(&original_context) {
            None => {
                self.consumer
                    .on_command_completion(&original_context, ReturnValue::Failure);
                Err(Self::unknown_command_error(&parsed.reader))
            }
            Some(flat_context) => {
                flat_context.execute_all(&original_context.source, self.consumer.as_ref())
            }
        }
    }

    /// 仅使用指定的来源解析给定的来源。
    #[must_use]
    pub fn parse_input(&self, command: &str, source: &S) -> ParsingResult<'_, S> {
        let mut reader = StringReader::new(command);
        self.parse(&mut reader, source)
    }

    /// 使用提供的源解析 [`StringReader`] 中的命令。
    pub fn parse(&self, reader: &mut StringReader<'_>, source: &S) -> ParsingResult<'_, S> {
        let context = CommandContextBuilder::new(
            self,
            Arc::new(source.clone()),
            ROOT_NODE_ID,
            reader.cursor(),
        );
        self.parse_nodes(ROOT_NODE_ID, reader, &context)
    }

    fn parse_nodes<'a>(
        &'a self,
        node: NodeId,
        original_reader: &mut StringReader<'_>,
        context_so_far: &CommandContextBuilder<'a, S>,
    ) -> ParsingResult<'a, S> {
        let source = context_so_far.source.clone();
        let mut errors: FxHashMap<NodeId, CommandSyntaxError> = FxHashMap::default();
        let mut potentials: Vec<ParsingResult<'a, S>> = Vec::new();
        let cursor = original_reader.cursor();

        for child in self.tree.get_relevant_nodes(original_reader, node) {
            if !self.tree.can_use(child, &source) {
                continue;
            }
            let mut context = context_so_far.clone();
            let mut reader = original_reader.clone();
            let parse_result = {
                if let Err(error) = self.tree.parse(child, &mut reader, &mut context) {
                    Err(error)
                } else {
                    let peek = reader.peek();
                    if peek.is_some() && peek != Some(ARG_SEPARATOR_CHAR) {
                        Err(DISPATCHER_EXPECTED_ARGUMENT_SEPARATOR.create(&reader))
                    } else {
                        Ok(())
                    }
                }
            };
            if let Err(parse_error) = parse_result {
                errors.insert(child, parse_error);
                reader.set_cursor(cursor);
                continue;
            }

            let child_node = &self.tree[child];
            context.with_command(child_node.command().clone());
            let redirect = self.tree[child].redirect();
            if reader.can_read_chars(if redirect.is_some() { 2 } else { 1 }) {
                reader.skip();
                if let Some(redirect) = redirect {
                    let Some(redirect) = self.tree.resolve(redirect) else {
                        errors.insert(child, UNRESOLVED_REDIRECT.create(&reader));
                        reader.set_cursor(cursor);
                        continue;
                    };
                    let child_context =
                        CommandContextBuilder::new(self, source, redirect, reader.cursor());
                    let parsed = self.parse_nodes(redirect, &mut reader, &child_context);
                    context.with_child(parsed.context);
                    return ParsingResult {
                        context,
                        errors: parsed.errors,
                        reader: parsed.reader,
                    };
                }
                let parsed = self.parse_nodes(child, &mut reader, &context);
                potentials.push(parsed);
            } else {
                potentials.push(ParsingResult {
                    context,
                    errors: FxHashMap::default(),
                    reader: reader.clone_into_owned(),
                });
            }
        }

        if potentials.is_empty() {
            ParsingResult {
                context: context_so_far.clone(),
                errors,
                reader: original_reader.clone_into_owned(),
            }
        } else {
            potentials
                .into_iter()
                .min_by(|a, b| {
                    let a_reader_remaining = a.reader.peek().is_some();
                    let b_reader_remaining = b.reader.peek().is_some();

                    let a_has_errors = !a.errors.is_empty();
                    let b_has_errors = !b.errors.is_empty();

                    (a_reader_remaining, a_has_errors).cmp(&(b_reader_remaining, b_has_errors))
                })
                .expect("potentials 列表非空")
        }
    }

    /// 处理给定来源（发送者）对命令的执行，
    /// 必要时向其返回相应的错误消息。
    ///
    /// 如果输入以一个斜杠（`/`）开头，则将其移除
    /// 在调用自身内部。
    pub fn handle_command<'a>(&'a self, source: &S, mut input: &'a str) {
        // 如果输入以 '/' 开头，但带该前导斜杠的命令并不
        // 注册，而去除斜杠后的命令却已注册（或者它是一条未知命令
        // 以单个斜杠开头（例如来自控制台输入）的情况，去除一个前导斜杠。
        // 对于 WorldEdit 的 `//set` 这类双斜杠命令，客户端会发送 `/set`，
        // 匹配已注册的 `/set` 或 `//set` 命令并保留斜杠。
        if let Some(sliced) = input.strip_prefix('/') {
            let first_token = input.split_whitespace().next().unwrap_or("");
            let sliced_token = sliced.split_whitespace().next().unwrap_or("");
            if !self.has_command(first_token)
                && (self.has_command(sliced_token) || !first_token.starts_with("//"))
            {
                input = sliced;
            }
        }

        // 在配置中被关闭的命令必须表现为
        // 就如同它不存在一样，因此我们报告常规的“未知命令”错误
        // 在任一调度器有机会运行它之前。
        if self.is_disabled(Self::command_name(input)) {
            let reader = StringReader::new(input);
            Self::send_error_to_source(source, Self::unknown_command_error(&reader), input);
            return;
        }

        let output = self.execute_input(input, source);

        if let Err(error) = output {
            Self::send_error_to_source(source, error, input);
        }
    }

    /// 向给定的来源发送命令错误。
    /// 这也会显示上下文信息
    /// 如有必要，还包括通向该错误的链路。
    pub fn send_error_to_source(
        source: &impl CommandSource,
        error: CommandSyntaxError,
        command: &str,
    ) {
        source.send_message(error.message.color(Color::Named(NamedColor::Red)));

        if let Some(context) = error.context {
            let i = context.input.len().min(context.cursor);

            let mut error_text = TextComponent::empty()
                .color(Color::Named(NamedColor::Gray))
                .click_event(ClickEvent::SuggestCommand {
                    command: format!("/{command}").into(),
                });

            if i > 10 {
                error_text = error_text.add_text("...");
            }

            let start = i.saturating_sub(10);

            let command_snippet = &context.input[start..i];
            error_text = error_text.add_text(command_snippet.to_owned());

            if i < context.input.len() {
                let errored_part = &context.input[i..];
                error_text = error_text.add_child(
                    TextComponent::text(errored_part.to_owned())
                        .color(Color::Named(NamedColor::Red))
                        .underlined(),
                );
            }

            error_text = error_text.add_child(
                TextComponent::translate(COMMAND_CONTEXT_HERE, &[])
                    .color(Color::Named(NamedColor::Red))
                    .italic(),
            );

            source.send_error(error_text);
        }
    }

    ///返回一个新的 [`Suggestions`] 结构
    /// 从给定的解析结果创建，该结果是一条已解析的命令，
    /// 假设光标位于末尾。
    ///
    /// 这有助于告知客户端接下来有哪些可用的建议。
    #[must_use]
    pub fn get_completion_suggestions_at_end(
        &self,
        parsing_result: ParsingResult<'_, S>,
    ) -> Suggestions {
        let length = parsing_result.reader.total_length();
        self.get_completion_suggestions(parsing_result, length)
    }

    ///返回一个新的 [`Suggestions`] 结构
    /// 从给定的解析结果创建，该结果是一条已解析的命令。
    ///
    /// 这有助于告知客户端接下来有哪些可用的建议。
    #[must_use]
    pub fn get_completion_suggestions(
        &self,
        parsing_result: ParsingResult<'_, S>,
        cursor: usize,
    ) -> Suggestions {
        let context = parsing_result.context;
        let (parent, start) = {
            let node_before_cursor = context.find_suggestion_context(cursor);
            (
                node_before_cursor.parent,
                node_before_cursor.starting_position.min(cursor),
            )
        };

        let full_input = parsing_result.reader.string();

        let truncated_input = &full_input[0..cursor.min(full_input.len())];

        let children = self.tree.get_children(parent);
        let context = context.build(truncated_input);
        let mut suggestions = Vec::with_capacity(children.len());

        for child in children {
            let builder = SuggestionsBuilder::new(truncated_input, start);

            match self.tree.classify_id(child) {
                NodeIdClassification::Root => {}
                NodeIdClassification::Literal(literal_node_id) => {
                    let node = &self.tree[literal_node_id];
                    if node
                        .meta
                        .literal_lowercase
                        .starts_with(builder.remaining_lowercase())
                    {
                        suggestions.push(builder.suggest(&*node.meta.literal).build());
                    }
                }
                NodeIdClassification::Command(command_node_id) => {
                    let node = &self.tree[command_node_id];
                    if node
                        .meta
                        .literal_lowercase
                        .starts_with(builder.remaining_lowercase())
                    {
                        suggestions.push(builder.suggest(&*node.meta.literal).build());
                    }
                }
                NodeIdClassification::Argument(argument_node_id) => {
                    let node = &self.tree[argument_node_id];
                    if let Some(provider) = &node.meta.suggestion_provider {
                        suggestions.push(provider.suggest(&context, builder));
                    } else {
                        suggestions
                            .push(node.meta.argument_type.list_suggestions(&context, builder));
                    }
                }
            }
        }

        Suggestions::merge(full_input, suggestions)
    }

    /// 以 [`CommandSuggestion`] 的 [`Vec`] 形式获取所有建议。
    ///
    /// # Panics
    ///
    /// 如果提供的来源是哑元（dummy）来源，此函数当前会 panic。
    /// 这在将来可能会发生变化。
    #[must_use]
    pub fn suggest(&self, input: &str, source: &S) -> Vec<CommandSuggestion> {
        self.suggest_with_range(input, source)
            .suggestions
            .into_iter()
            .map(|suggestion| CommandSuggestion {
                suggestion: suggestion.text.cached_text().clone(),
                tooltip: suggestion.tooltip,
            })
            .collect()
    }

    #[must_use]
    pub fn suggest_with_range(&self, input: &str, source: &S) -> Suggestions {
        // 永不对已关闭的命令提供参数建议。
        if self.is_disabled(Self::command_name(input)) {
            return Suggestions::empty();
        }

        let parsed = self.parse_input(input, source);
        self.get_completion_suggestions_at_end(parsed)
    }

    /// 获取此调度器中所有可用的命令，已排序。
    /// 返回的映射以命令名作为键
    /// 而值则作为命令的描述。
    #[must_use]
    pub fn get_all_commands(&self) -> BTreeMap<&str, &str> {
        let mut commands: BTreeMap<&str, &str> = BTreeMap::new();

        for command in self.tree.get_root_children() {
            let meta = &self.tree[command].meta;
            if self.is_disabled(&meta.literal_lowercase) {
                continue;
            }
            commands.insert(&meta.literal_lowercase, &meta.description);
        }

        commands
    }

    /// 获取此调度器中所有可用的命令，这些命令
    /// 给定源所能使用的。
    /// 返回的映射以命令名作为键
    /// 而值则作为命令的描述。
    #[must_use]
    pub fn get_all_permitted_commands(&self, source: &S) -> BTreeMap<&str, &str> {
        let mut commands: BTreeMap<&str, &str> = BTreeMap::new();

        for command in self.tree.get_root_children() {
            if self.tree.can_use(command.into(), source) {
                let meta = &self.tree[command].meta;
                if self.is_disabled(&meta.literal_lowercase) {
                    continue;
                }
                commands.insert(&meta.literal_lowercase, &meta.description);
            }
        }

        commands
    }

    /// 获取给定来源下每条被允许命令的描述与用法。
    ///
    /// 键是命令标识符，
    /// 而值是一个 `(description, usage)` 元组。
    #[must_use]
    pub fn get_all_permitted_commands_usage(&self, source: &S) -> BTreeMap<&str, (&str, Box<str>)> {
        let mut commands: BTreeMap<&str, (&str, Box<str>)> = BTreeMap::new();

        for (command_node_id, usage) in self.get_usage_of_commands(source) {
            let meta = &self.tree[command_node_id].meta;
            let command_name = meta.literal.as_ref();
            if self.is_disabled(&meta.literal_lowercase) {
                continue;
            }
            let command_description = meta.description.as_ref();
            commands.insert(command_name, (command_description, usage.into_boxed_str()));
        }

        commands
    }

    /// 获取来自特定插件的命令的描述与用法。
    /// 仅返回来源有权使用的命令。
    #[must_use]
    pub fn get_all_permitted_commands_usage_by_plugin(
        &self,
        source: &S,
        plugin_name: &str,
    ) -> BTreeMap<&str, (&str, Box<str>)> {
        let mut commands: BTreeMap<&str, (&str, Box<str>)> = BTreeMap::new();

        for (command_node_id, usage) in self.get_usage_of_commands(source) {
            let meta = &self.tree[command_node_id].meta;
            if self.is_disabled(&meta.literal_lowercase) {
                continue;
            }
            if let Some(src) = &meta.source
                && src == plugin_name
            {
                let command_name = meta.literal.as_ref();
                let command_description = meta.description.as_ref();
                commands.insert(command_name, (command_description, usage.into_boxed_str()));
            }
        }

        commands
    }

    /// 获取给定来源下指定命令的描述与用法。
    ///若未找到或命令来源权限不足，则返回 `None`。
    ///
    /// 键是命令标识符，
    /// 而值是一个 `(description, usage)` 元组。
    #[must_use]
    pub fn get_permitted_command_usage(
        &self,
        source: &S,
        command: &str,
    ) -> Option<(&str, Box<str>)> {
        let command_node_id = self.tree.get(command)?;

        // 若权限不足，这会将 `None` 传播到函数结果中。
        let usage = self.get_usage_of_command(command_node_id, source)?;

        let description = self.tree[command_node_id].meta.description.as_ref();

        Some((description, usage.into_boxed_str()))
    }

    /// 返回给定命令节点的用法。
    #[must_use]
    pub fn get_usage_of_command(&self, command_node: CommandNodeId, source: &S) -> Option<String> {
        // 我们知道根节点没有执行器，因此给 `is_optional` 传 false。
        self.get_usage_recursive(command_node.into(), source, false, false, None)
            .map(|mut usage| {
                // 我们添加斜杠作为前缀。
                usage.insert(0, '/');
                usage
            })
    }

    /// 返回给定节点每个子节点的用法（限对给定命令源可用的部分）。
    #[must_use]
    pub fn get_usage_of_children(&self, node: NodeId, source: &S) -> FxHashMap<NodeId, String> {
        let mut map = FxHashMap::default();

        let is_optional = self.tree[node].command().is_some();
        for child in self.tree.get_children(node) {
            if let Some(usage) = self.get_usage_recursive(child, source, is_optional, false, None) {
                map.insert(child, usage);
            }
        }

        map
    }

    /// 返回每个命令的用法（限对给定命令源可用的部分）。
    #[must_use]
    pub fn get_usage_of_commands(&self, source: &S) -> FxHashMap<CommandNodeId, String> {
        self.get_usage_of_children(ROOT_NODE_ID, source)
            .into_iter()
            // 这是安全的，因为根子节点的每个子节点都是命令节点。
            .map(|(k, mut v)| {
                // 我们在开头加一个斜杠以表示命令用法。
                v.insert(0, '/');
                (CommandNodeId(k.0), v)
            })
            .collect()
    }

    /// 递归遍历用法的内部函数。
    fn get_usage_recursive(
        &self,
        node: NodeId,
        source: &S,
        is_optional: bool,
        deep: bool,
        redirector_usage_text: Option<String>,
    ) -> Option<String> {
        if !self.tree.can_use(node, source) {
            return None;
        }

        let usage_text = redirector_usage_text.unwrap_or_else(|| {
            let mut text = self.tree[node].usage_text();
            if is_optional {
                text = format!("{USAGE_OPTIONAL_OPEN}{text}{USAGE_OPTIONAL_CLOSE}");
            }
            text
        });
        let child_optional = self.tree[node].command().is_some();

        if !deep {
            if let Some(redirect) = self.tree[node].redirect() {
                if let Some(target) = self.tree.resolve(redirect) {
                    let target_usage = if target == node {
                        "...".to_string()
                    } else if self.tree.is_command_node(node) && self.tree.is_command_node(target) {
                        // 我们这样做是为了让它能显示例如 /? 的用法：
                        //
                        // /? [<commandOrPage>]
                        //
                        // 而不是
                        //
                        // /? -> 帮助
                        return self.get_usage_recursive(
                            target,
                            source,
                            is_optional,
                            deep,
                            Some(usage_text),
                        );
                    } else {
                        format!("-> {}", self.tree[target].usage_text())
                    };
                    return Some(format!("{usage_text}{ARG_SEPARATOR}{target_usage}"));
                }
            } else {
                let mut children = Vec::new();
                for child in self.tree.get_children(node) {
                    if self.tree.can_use(child, source) {
                        children.push(child);
                    }
                }

                if children.len() == 1 {
                    let child = children[0];
                    if let Some(child_usage_text) =
                        self.get_usage_recursive(child, source, child_optional, true, None)
                    {
                        return Some(format!("{usage_text}{ARG_SEPARATOR}{child_usage_text}"));
                    }
                } else if !children.is_empty() {
                    let mut child_usages = Vec::new();
                    // TODO: 在保持插入顺序的同时优化这个集合算法。
                    for child in children {
                        if let Some(child_usage_text) =
                            self.get_usage_recursive(child, source, child_optional, true, None)
                            && !child_usages.contains(&child_usage_text)
                        {
                            child_usages.push(child_usage_text);
                        }
                    }
                    if child_usages.len() == 1 {
                        let mut child_usage = child_usages.pop().unwrap_or_default();
                        if is_optional {
                            child_usage =
                                format!("{USAGE_OPTIONAL_OPEN}{child_usage}{USAGE_OPTIONAL_CLOSE}");
                        }
                        return Some(format!("{usage_text}{ARG_SEPARATOR}{child_usage}"));
                    } else if !child_usages.is_empty() {
                        let (open, close) = if child_optional {
                            (USAGE_OPTIONAL_OPEN, USAGE_OPTIONAL_CLOSE)
                        } else {
                            (USAGE_REQUIRED_OPEN, USAGE_REQUIRED_CLOSE)
                        };

                        let mut result_usage = usage_text;
                        result_usage += ARG_SEPARATOR;
                        result_usage += open;
                        let mut first = true;
                        for child_usage in child_usages {
                            if !first {
                                result_usage += USAGE_OR;
                            }
                            result_usage += &*child_usage;
                            first = false;
                        }
                        result_usage += close;
                        return Some(result_usage);
                    }
                }
            }
        }

        Some(usage_text)
    }
}

#[cfg(test)]
mod test {
    use crate::argument_builder::{
        ArgumentBuilder, CommandArgumentBuilder, LiteralArgumentBuilder, RequiredArgumentBuilder,
    };
    use crate::argument_types::core::integer::IntegerArgumentType;
    use crate::context::command_context::CommandContext;
    use crate::errors::error_types::DISPATCHER_UNKNOWN_COMMAND;
    use crate::node::dispatcher::CommandDispatcher;
    use crate::node::{CommandExecutor, CommandExecutorResult};
    use crate::source::DummySource;

    #[test]
    fn unknown_command() {
        let mut dispatcher = CommandDispatcher::new();
        dispatcher.register(
            CommandArgumentBuilder::new("unknown", "A command without an executor").build(),
        );
        let source = DummySource::dummy();
        let result = dispatcher.execute_input("unknown", &source);
        assert!(result.is_err_and(|error| error.error_type == &DISPATCHER_UNKNOWN_COMMAND));
    }

    #[test]
    fn unknown_command_message_includes_command_name() {
        for (input, registered, disabled) in [
            ("x", false, false),
            ("x extra arguments", false, false),
            ("x", true, false),
            ("x extra arguments", true, true),
        ] {
            let mut dispatcher = CommandDispatcher::new();
            if registered {
                dispatcher.register(CommandArgumentBuilder::new("x", "test command").build());
            }
            if disabled {
                dispatcher.disable_command("x");
            }
            let error = dispatcher
                .execute_input(input, &DummySource::dummy())
                .expect_err("unknown or unavailable command");
            let expected = "Unknown or incomplete command. See below for error";
            assert_eq!(error.message.clone().to_pretty_console(), expected);
            let java = serde_json::to_value(&error.message).expect("Java 文本组件");
            assert_eq!(java["translate"], "command.unknown.command");
            assert_eq!(java["with"][0]["text"], "x");
            assert_eq!(error.context.expect("错误上下文").input, input);
        }
    }

    #[test]
    fn simple_command() {
        let mut dispatcher = CommandDispatcher::new();
        let executor: fn(&CommandContext) -> CommandExecutorResult = |_| Ok(1);
        dispatcher
            .register(CommandArgumentBuilder::new("simple", "A simple command").executes(executor));
        let source = DummySource::dummy();
        let result = dispatcher.execute_input("simple", &source);
        assert_eq!(result, Ok(1));
    }

    #[test]
    fn disabled_command_cannot_be_executed_directly() {
        // 防护 `/execute run <command>` 的绕过：被禁用的命令必须
        // 被拒绝，即使经由 `execute_input` 而非
        // `handle_command`.
        let mut dispatcher = CommandDispatcher::new();
        let executor: fn(&CommandContext) -> CommandExecutorResult = |_| Ok(1);
        dispatcher
            .register(CommandArgumentBuilder::new("simple", "A simple command").executes(executor));
        dispatcher.disable_command("simple");

        let source = DummySource::dummy();
        let result = dispatcher.execute_input("simple", &source);
        assert!(result.is_err_and(|error| error.error_type == &DISPATCHER_UNKNOWN_COMMAND));
    }

    #[test]
    fn plugin_commands_follow_unload_and_reload_lifecycle() {
        let mut dispatcher = CommandDispatcher::new();
        let executor: fn(&CommandContext) -> CommandExecutorResult = |_| Ok(1);
        let command = || {
            CommandArgumentBuilder::new("Plugin-Command", "A plugin command")
                .with_source("test-plugin")
                .executes(executor)
        };

        dispatcher.register_with_aliases(command(), &["Plugin-Alias"]);
        let source = DummySource::dummy();
        assert_eq!(dispatcher.execute_input("plugin-command", &source), Ok(1));
        assert_eq!(dispatcher.execute_input("plugin-alias", &source), Ok(1));

        dispatcher.deactivate_commands_from_source("test-plugin");
        assert!(
            dispatcher
                .execute_input("plugin-command", &source)
                .is_err_and(|error| error.error_type == &DISPATCHER_UNKNOWN_COMMAND)
        );
        assert!(
            dispatcher
                .execute_input("plugin-alias", &source)
                .is_err_and(|error| error.error_type == &DISPATCHER_UNKNOWN_COMMAND)
        );

        dispatcher.register_with_aliases(command(), &["Plugin-Alias"]);
        assert_eq!(dispatcher.execute_input("plugin-command", &source), Ok(1));
        assert_eq!(dispatcher.execute_input("plugin-alias", &source), Ok(1));
        assert_eq!(
            dispatcher.tree_alias_names("PLUGIN-COMMAND"),
            vec!["plugin-alias"]
        );

        dispatcher.deactivate_plugin_command_and_aliases("PLUGIN-ALIAS");
        assert!(
            dispatcher
                .execute_input("plugin-command", &source)
                .is_err_and(|error| error.error_type == &DISPATCHER_UNKNOWN_COMMAND)
        );
        assert!(
            dispatcher
                .execute_input("plugin-alias", &source)
                .is_err_and(|error| error.error_type == &DISPATCHER_UNKNOWN_COMMAND)
        );

        dispatcher.register_with_aliases(command(), &["Plugin-Alias"]);
        dispatcher.disable_command("PLUGIN-COMMAND");
        dispatcher.deactivate_commands_from_source("test-plugin");
        dispatcher.register_with_aliases(command(), &["Plugin-Alias"]);
        assert!(
            dispatcher
                .execute_input("plugin-command", &source)
                .is_err_and(|error| error.error_type == &DISPATCHER_UNKNOWN_COMMAND)
        );
    }

    #[test]
    fn arithmetic_command() {
        enum Operation {
            Add,
            Subtract,
            Multiply,
            Divide,
        }

        struct Executor(Operation);
        impl CommandExecutor for Executor {
            fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
                let operand1: i32 = *context.get_argument("operand1")?;
                let operand2: i32 = *context.get_argument("operand2")?;
                Ok(match self.0 {
                    Operation::Add => operand1 + operand2,
                    Operation::Subtract => operand1 - operand2,
                    Operation::Multiply => operand1 * operand2,
                    Operation::Divide => operand1 / operand2,
                })
            }
        }

        let mut dispatcher = CommandDispatcher::new();
        dispatcher.register(
            CommandArgumentBuilder::new(
                "arithmetic",
                "A command which adds two integers, returning the result",
            )
            .then(
                RequiredArgumentBuilder::new("operand1", IntegerArgumentType::any())
                    .then(
                        LiteralArgumentBuilder::new("+").then(
                            RequiredArgumentBuilder::new("operand2", IntegerArgumentType::any())
                                .executes(Executor(Operation::Add)),
                        ),
                    )
                    .then(
                        LiteralArgumentBuilder::new("-").then(
                            RequiredArgumentBuilder::new("operand2", IntegerArgumentType::any())
                                .executes(Executor(Operation::Subtract)),
                        ),
                    )
                    .then(
                        LiteralArgumentBuilder::new("*").then(
                            RequiredArgumentBuilder::new("operand2", IntegerArgumentType::any())
                                .executes(Executor(Operation::Multiply)),
                        ),
                    )
                    .then(
                        LiteralArgumentBuilder::new("/").then(
                            RequiredArgumentBuilder::new("operand2", IntegerArgumentType::any())
                                .executes(Executor(Operation::Divide)),
                        ),
                    ),
            ),
        );
        let source = DummySource::dummy();
        assert_eq!(
            dispatcher.execute_input("arithmetic 3 + -7", &source),
            Ok(-4)
        );
        assert_eq!(
            dispatcher.execute_input("arithmetic 4 - -8", &source),
            Ok(12)
        );
        assert_eq!(
            dispatcher.execute_input("arithmetic 2 * 9", &source),
            Ok(18)
        );
        assert_eq!(dispatcher.execute_input("arithmetic 9 / 2", &source), Ok(4));
    }

    #[test]
    fn alias_simple() {
        let mut dispatcher = CommandDispatcher::new();
        let executor: fn(&CommandContext) -> CommandExecutorResult = |_| Ok(1);
        dispatcher.register(CommandArgumentBuilder::new("a", "A command").executes(executor));
        // 注意这里我们不能使用 redirect，因为节点本身需要执行该命令，
        // 而非其“子项”。
        dispatcher.register(CommandArgumentBuilder::new("b", "An alias for /a").executes(executor));
        let source = DummySource::dummy();
        assert_eq!(dispatcher.execute_input("a", &source), Ok(1));
        assert_eq!(dispatcher.execute_input("b", &source), Ok(1));
    }

    #[test]
    fn alias_complex() {
        struct Executor;
        impl CommandExecutor for Executor {
            fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
                Ok(*context.get_argument("result")?)
            }
        }

        let mut dispatcher = CommandDispatcher::new();

        let a = dispatcher.register(CommandArgumentBuilder::new("a", "A command").then(
            RequiredArgumentBuilder::new("result", IntegerArgumentType::any()).executes(Executor),
        ));
        // 注意这次我们应当使用 redirect — 它指向另一个拥有 `command` 的节点。
        dispatcher.register(CommandArgumentBuilder::new("b", "An alias for /a").redirect(a));
        let source = DummySource::dummy();
        assert_eq!(dispatcher.execute_input("a 5", &source), Ok(5));
        assert_eq!(dispatcher.execute_input("b 7", &source), Ok(7));
    }

    #[test]
    fn recurse() {
        struct Executor;
        impl CommandExecutor for Executor {
            fn execute(&self, _context: &CommandContext) -> CommandExecutorResult {
                Ok(1)
            }
        }

        let mut dispatcher = CommandDispatcher::new();

        let mut builder = CommandArgumentBuilder::new(
            "recurse",
            "Recurses itself, doing nothing with the numbers provided",
        )
        .executes(Executor);

        let id = builder.id();
        builder = builder.then(
            RequiredArgumentBuilder::new("value", IntegerArgumentType::any())
                .executes(Executor)
                .redirect(id),
        );

        dispatcher.register(builder);

        let source = DummySource::dummy();
        assert_eq!(dispatcher.execute_input("recurse", &source), Ok(1));
        assert_eq!(dispatcher.execute_input("recurse 4", &source), Ok(1));
        assert_eq!(dispatcher.execute_input("recurse 9 -1", &source), Ok(1));
        assert_eq!(
            dispatcher.execute_input("recurse 9 7 -6 5 -4", &source),
            Ok(1)
        );
        assert_eq!(
            dispatcher.execute_input("recurse 1 2 4 8 16 32 64 128 256 512", &source),
            Ok(1)
        );
    }

    #[test]
    fn double_slash_command_execution() {
        let mut dispatcher = CommandDispatcher::new();
        let executor: fn(&CommandContext) -> CommandExecutorResult = |_| Ok(42);

        dispatcher.register(
            CommandArgumentBuilder::new("//set", "WorldEdit set command").executes(executor),
        );

        let source = DummySource::dummy();
        // 使用 //set 直接执行
        assert_eq!(dispatcher.execute_input("//set", &source), Ok(42));
        // 通过 /set 别名执行（Java 客户端为 //set 发送的形式）
        assert_eq!(dispatcher.execute_input("/set", &source), Ok(42));
    }
}
