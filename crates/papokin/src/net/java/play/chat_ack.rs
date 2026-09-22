#[allow(clippy::wildcard_imports)]
use super::*;
use papokin_protocol::java::server::play::SChatAck;

impl JavaClient {
    pub fn handle_chat_ack(&self, player: &Arc<Player>, packet: &SChatAck) {
        let offset = packet.offset.0;
        if offset < 0 {
            warn!(
                "校验来自 {} 的消息确认偏移失败：偏移为负值 {}",
                player.gameprofile.name, offset
            );
            self.try_kick(&TextComponent::translate(
                translation::java::MULTIPLAYER_DISCONNECT_CHAT_VALIDATION_FAILED,
                [],
            ));
            return;
        }

        let validation_err = {
            let mut cache = player
                .signature_cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            cache
                .last_seen_validator
                .apply_offset(offset as usize)
                .err()
        };

        if let Some(err) = validation_err {
            warn!(
                "校验来自 {} 的消息确认偏移失败：{}",
                player.gameprofile.name, err
            );
            self.try_kick(&TextComponent::translate(
                translation::java::MULTIPLAYER_DISCONNECT_CHAT_VALIDATION_FAILED,
                [],
            ));
            return;
        }

        trace!(
            "玩家 {} 确认了 {} 条聊天消息",
            player.gameprofile.name, offset
        );
    }
}
