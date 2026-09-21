use pumpkin_macros::Event;

/// An event that occurs when a command is registered to the server.
#[derive(Event, Clone)]
pub struct CommandRegisteredEvent {
    /// The label of the registered command.
    pub command_label: String,

    /// The name of the plugin registering the command.
    pub plugin: String,
}

impl CommandRegisteredEvent {
    #[must_use]
    pub const fn new(command_label: String, plugin: String) -> Self {
        Self {
            command_label,
            plugin,
        }
    }
}
