#[allow(clippy::wildcard_imports)]
use super::*;
use crate::block::entities::beacon::BeaconBlockEntity;
use papokin_inventory::Inventory;
use papokin_inventory::beacon_screen_handler::BeaconScreenHandler;
use papokin_protocol::java::server::play::SSetBeacon;

impl JavaClient {
    pub fn handle_set_beacon(&self, player: &Arc<Player>, packet: &SSetBeacon) {
        let is_valid = {
            let screen_handler_lock = player
                .current_screen_handler
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            let mut screen_handler = screen_handler_lock
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            let Some(beacon_handler) = screen_handler
                .as_any_mut()
                .downcast_mut::<BeaconScreenHandler>()
            else {
                debug!(
                    "玩家 {} 交互了无效菜单（预期为信标）",
                    player.gameprofile.name
                );
                return;
            };

            let Some(beacon_entity) = beacon_handler
                .inventory
                .as_any()
                .downcast_ref::<BeaconBlockEntity>()
            else {
                return;
            };

            // 检查付款槽是否有物品
            if beacon_entity
                .payment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_empty()
            {
                return;
            }

            let levels = beacon_entity.levels.load(Ordering::Relaxed);
            let primary_id = packet.primary_effect.map(|v| v.0);
            let secondary_id = packet.secondary_effect.map(|v| v.0);

            if BeaconBlockEntity::validate_effects(primary_id, secondary_id, levels) {
                beacon_entity
                    .primary_effect
                    .store(primary_id.unwrap_or(-1), Ordering::Relaxed);
                beacon_entity
                    .secondary_effect
                    .store(secondary_id.unwrap_or(-1), Ordering::Relaxed);

                // 从支付槽位移除 1 个物品
                beacon_entity.remove_stack_specific(0, 1);
                beacon_entity.mark_dirty();

                screen_handler.sync_state();

                info!(
                    "玩家 {} 更新了信标效果：主效果 {:?}，副效果 {:?}",
                    player.gameprofile.name, primary_id, secondary_id
                );
                true
            } else {
                false
            }
        };

        if !is_valid {
            let primary_id = packet.primary_effect.map(|v| v.0);
            let secondary_id = packet.secondary_effect.map(|v| v.0);
            warn!(
                "玩家 {} 试图设置无效的信标效果：主效果 {:?}，副效果 {:?}",
                player.gameprofile.name, primary_id, secondary_id
            );
            self.try_kick(&TextComponent::translate(
                "multiplayer.disconnect.generic",
                &[],
            ));
        }
    }
}
