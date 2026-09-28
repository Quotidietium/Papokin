#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_sign_update(&self, player: &Player, sign_data: &SUpdateSign<'_>) {
        let world = player.get_entity().world.load_full();
        let Some(block_entity) = world.get_block_entity(&sign_data.location) else {
            return;
        };
        let Some(sign_entity) =
            crate::block::entities::sign::SignEntityRef::from_block_entity(&*block_entity)
        else {
            return;
        };
        if sign_entity.is_waxed() {
            return;
        }

        let currently_editing = *sign_entity
            .currently_editing_player()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // 编辑会话必须由服务端建立（放置或右键打开编辑器时写入）：
        // 无会话（如磁盘加载的既有告示牌）或会话属于他人均拒绝，
        // 防止客户端伪造更新包改写任意位置的告示牌文本
        if currently_editing != Some(player.gameprofile.id) {
            return;
        }

        // 会话有效仍需在场：玩家开完编辑器后可能走远或被传送，
        // 远程提交同样会改写告示牌（原版以 blockInteractionRange
        // 约 4.5+1 格校验）。取 8 格平方距离，覆盖伸手可及范围。
        let player_pos = player.get_entity().pos.load();
        let sign_pos = sign_data.location.0;
        let dx = player_pos.x - f64::from(sign_pos.x) - 0.5;
        let dy = player_pos.y - f64::from(sign_pos.y) - 0.5;
        let dz = player_pos.z - f64::from(sign_pos.z) - 0.5;
        if dx.mul_add(dx, dy * dy + dz * dz) > 64.0 {
            return;
        }

        let lines = vec![
            sign_data.line_1.to_string(),
            sign_data.line_2.to_string(),
            sign_data.line_3.to_string(),
            sign_data.line_4.to_string(),
        ];

        if let Some(player_arc) = world.get_player_by_uuid(player.gameprofile.id) {
            let mut event = crate::plugin::api::events::block::sign_change::SignChangeEvent::new(
                player_arc,
                sign_data.location,
                lines,
            );
            if let Some(server) = world.server.upgrade() {
                server.plugin_manager.fire_blocking(&server, &mut event);
            }
            if event.cancelled {
                return;
            }
        }

        let text = sign_entity.get_text(sign_data.is_front_text);

        let new_messages = [
            Box::<str>::from(sign_data.line_1),
            Box::<str>::from(sign_data.line_2),
            Box::<str>::from(sign_data.line_3),
            Box::<str>::from(sign_data.line_4),
        ];

        text.messages
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone_from(&new_messages);
        // 我们不做任何过滤，因此过滤后的副本必须跟随原始版本。让它
        // 为空会使其产生差异，从而在网络上放入空的 `filtered_messages` 并将
        // 告示牌，供启用了文本过滤的客户端渲染。
        *text
            .filtered_messages
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = new_messages;
        *sign_entity
            .currently_editing_player()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        world.update_block_entity(&block_entity);
    }
}
