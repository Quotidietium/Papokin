//! Plugin messaging channels (Bukkit `Messenger` equivalent).
//!
//! Register the channels your plugin handles; messages players send on those
//! channels arrive in [`Plugin::on_plugin_message`](crate::Plugin::on_plugin_message).
//! Send messages to players with [`Context::send_plugin_message`].
//!
//! # Example
//!
//! ```rust,ignore
//! // In on_enable:
//! context.register_incoming_channel("my-plugin:stats")?;
//!
//! // Elsewhere:
//! context.send_plugin_message(&player_uuid, "my-plugin:stats", b"ping".to_vec())?;
//! ```

use crate::Context;

impl Context {
    /// Registers an incoming channel handled by this plugin.
    ///
    /// # Errors
    /// Returns an error when the channel is reserved (`minecraft:*`) or
    /// invalid.
    pub fn register_incoming_channel(&self, channel: &str) -> crate::Result<()> {
        crate::wit::pumpkin::plugin::messaging::register_incoming_channel(channel)
    }

    /// Unregisters a previously registered incoming channel.
    pub fn unregister_incoming_channel(&self, channel: &str) {
        crate::wit::pumpkin::plugin::messaging::unregister_incoming_channel(channel);
    }

    /// Returns the incoming channels registered by this plugin.
    #[must_use]
    pub fn get_incoming_channels(&self) -> Vec<String> {
        crate::wit::pumpkin::plugin::messaging::get_incoming_channels()
    }

    /// Sends a plugin message to a player on the given channel.
    ///
    /// # Errors
    /// Returns an error when the player is not online or the channel is
    /// invalid.
    pub fn send_plugin_message(
        &self,
        player_uuid: &str,
        channel: &str,
        data: &[u8],
    ) -> crate::Result<()> {
        crate::wit::pumpkin::plugin::messaging::send_plugin_message(player_uuid, channel, data)
    }
}
