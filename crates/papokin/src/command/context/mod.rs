pub mod command_context {
    pub use papokin_command::context::command_context::*;
    pub type CommandContext<'a> =
        papokin_command::context::CommandContext<'a, crate::command::CommandSource>;
}
pub mod command_source;
pub mod string_range {
    pub use papokin_command::context::string_range::*;
}
pub use command_context::CommandContext;
pub use papokin_command::context::*;
