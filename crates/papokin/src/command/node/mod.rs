use crate::command::context::command_context::CommandContext;
use crate::command::context::command_source::CommandSource;

pub use papokin_command::node::*;

pub mod attached {
    pub use papokin_command::node::attached::*;
    pub type AttachedNode =
        papokin_command::node::attached::AttachedNode<crate::command::CommandSource>;
    pub type RootAttachedNode =
        papokin_command::node::attached::RootAttachedNode<crate::command::CommandSource>;
    pub type LiteralAttachedNode =
        papokin_command::node::attached::LiteralAttachedNode<crate::command::CommandSource>;
    pub type CommandAttachedNode =
        papokin_command::node::attached::CommandAttachedNode<crate::command::CommandSource>;
    pub type ArgumentAttachedNode =
        papokin_command::node::attached::ArgumentAttachedNode<crate::command::CommandSource>;
}

pub mod detached {
    pub use papokin_command::node::detached::*;
    pub type DetachedNode =
        papokin_command::node::detached::DetachedNode<crate::command::CommandSource>;
    pub type LiteralDetachedNode =
        papokin_command::node::detached::LiteralDetachedNode<crate::command::CommandSource>;
    pub type CommandDetachedNode =
        papokin_command::node::detached::CommandDetachedNode<crate::command::CommandSource>;
    pub type ArgumentDetachedNode =
        papokin_command::node::detached::ArgumentDetachedNode<crate::command::CommandSource>;
}

pub mod tree {
    pub use papokin_command::node::tree::*;
    pub type Tree = papokin_command::node::tree::Tree<crate::command::CommandSource>;
}

pub mod dispatcher {
    pub use papokin_command::dispatcher::*;
    pub type CommandDispatcher =
        papokin_command::dispatcher::CommandDispatcher<crate::command::CommandSource>;
}

pub type Command = papokin_command::node::Command<CommandSource>;
pub type Requirement = papokin_command::node::Requirement<CommandSource>;
pub type Requirements = papokin_command::node::Requirements<CommandSource>;
pub type RedirectModifier = papokin_command::node::RedirectModifier<CommandSource>;
pub type RedirectModifierResult = papokin_command::node::RedirectModifierResult<CommandSource>;
pub type RedirectModifierExecutor = papokin_command::node::RedirectModifierExecutor<CommandSource>;

/// 实现此 trait 的结构体能够在给定的上下文中运行。
pub trait CommandExecutor: Sync + Send {
    /// 针对一条命令执行此执行器。
    fn execute(&self, context: &CommandContext) -> papokin_command::node::CommandExecutorResult;
}

pub struct CommandExecutorAdapter<T>(pub T);

impl<T: CommandExecutor> papokin_command::node::CommandExecutor<CommandSource>
    for CommandExecutorAdapter<T>
{
    fn execute(
        &self,
        context: &papokin_command::context::command_context::CommandContext<'_, CommandSource>,
    ) -> papokin_command::node::CommandExecutorResult {
        self.0.execute(context)
    }
}

pub struct ArcCommandExecutorAdapter(pub std::sync::Arc<dyn CommandExecutor>);

impl papokin_command::node::CommandExecutor<CommandSource> for ArcCommandExecutorAdapter {
    fn execute(
        &self,
        context: &papokin_command::context::command_context::CommandContext<'_, CommandSource>,
    ) -> papokin_command::node::CommandExecutorResult {
        self.0.execute(context)
    }
}
