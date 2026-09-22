#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_confirm_teleport(&self, player: &Player, confirm_teleport: &SConfirmTeleport) {
        enum TeleportResult {
            Success,
            WrongId,
            NotTeleporting,
        }

        let result = {
            let mut awaiting_teleport = player
                .awaiting_teleport
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some((id, position)) = awaiting_teleport.as_ref() {
                if id == &confirm_teleport.teleport_id {
                    // 我们现在应把位置设为我们在传送数据包中请求的位置。
                    // 这或许能修复客户端在被传送过程中发送位置所引发的问题。
                    player.get_entity().set_pos(*position);
                    *awaiting_teleport = None;
                    TeleportResult::Success
                } else {
                    TeleportResult::WrongId
                }
            } else {
                TeleportResult::NotTeleporting
            }
        };

        match result {
            TeleportResult::Success => {}
            TeleportResult::WrongId => {
                self.try_kick(&TextComponent::text("错误的传送 ID"));
            }
            TeleportResult::NotTeleporting => {
                self.try_kick(&TextComponent::text("发送了传送确认，但我们并未传送"));
            }
        }
    }
}
