#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub async fn handle_client_information_config(
        &self,
        client_information: SClientInformationConfig<'_>,
    ) {
        debug!("正在处理客户端设置");
        if client_information.view_distance <= 0 {
            self.kick(TextComponent::text("视距不能为零或负数！")).await;
            return;
        }

        if let (Ok(main_hand), Ok(chat_mode)) = (
            Hand::try_from(client_information.main_hand.0),
            ChatMode::try_from(client_information.chat_mode.0),
        ) {
            self.config.store(Arc::new(PlayerConfig {
                locale: client_information.locale.to_string(),
                // client_information.view_distance 已在上方检查为 > 0，因此编译器应能将其优化掉。
                view_distance: NonZero::new(client_information.view_distance as u8)
                    .unwrap_or(NonZero::<u8>::MIN),
                chat_mode,
                chat_colors: client_information.chat_colors,
                skin_parts: client_information.skin_parts,
                main_hand,
                text_filtering: client_information.text_filtering,
                server_listing: client_information.server_listing,
            }));
        } else {
            self.kick(TextComponent::text("无效的手或聊天类型")).await;
        }
    }
}
