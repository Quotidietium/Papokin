#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub async fn handle_plugin_message(&self, plugin_message: SPluginMessage<'_>) {
        debug!("正在处理插件消息");
        if plugin_message.channel.starts_with(BRAND_CHANNEL_PREFIX) {
            debug!("收到客户端品牌");
            match str::from_utf8(plugin_message.data) {
                Ok(brand) => self.brand.store(Arc::new(Some(brand.to_string()))),
                Err(e) => self.kick(TextComponent::text(e.to_string())).await,
            }
        }
    }
}
