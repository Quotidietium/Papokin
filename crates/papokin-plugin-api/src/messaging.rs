//! 插件消息通道（等同于 Bukkit 的 `Messenger`）。
//!
//! 注册你的插件要处理的通道；玩家在这些通道上发送的消息
//! 会送达 [`Plugin::on_plugin_message`](crate::Plugin::on_plugin_message)。
//! 用 [`Context::send_plugin_message`] 向玩家发送消息。
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
    /// 注册一个由本插件处理的传入通道。
    ///
    /// # Errors
    ///当通道为保留通道（`minecraft:*`）或已被注册时，返回错误
    /// 无效。
    pub fn register_incoming_channel(&self, channel: &str) -> crate::Result<()> {
        crate::wit::papokin::plugin::messaging::register_incoming_channel(channel)
    }

    /// 注销先前注册的入站通道。
    pub fn unregister_incoming_channel(&self, channel: &str) {
        crate::wit::papokin::plugin::messaging::unregister_incoming_channel(channel);
    }

    /// 返回此插件注册的传入通道。
    #[must_use]
    pub fn get_incoming_channels(&self) -> Vec<String> {
        crate::wit::papokin::plugin::messaging::get_incoming_channels()
    }

    /// 在给定频道上向玩家发送插件消息。
    ///
    /// # Errors
    ///当玩家不在线或通道未注册时，返回错误
    /// 无效。
    pub fn send_plugin_message(
        &self,
        player_uuid: &str,
        channel: &str,
        data: &[u8],
    ) -> crate::Result<()> {
        crate::wit::papokin::plugin::messaging::send_plugin_message(player_uuid, channel, data)
    }
}
