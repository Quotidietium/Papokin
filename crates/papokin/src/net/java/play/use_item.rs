#[allow(clippy::wildcard_imports)]
use super::*;
use papokin_util::version::JavaMinecraftVersion;

impl JavaClient {
    pub fn handle_use_item(&self, player: &Arc<Player>, use_item: &SUseItem, server: &Arc<Server>) {
        if !player.has_client_loaded() {
            return;
        }
        player.update_last_action_time();

        let inventory = player.inventory();
        let Ok(hand) = Hand::from_packet_id(use_item.hand.0) else {
            self.try_kick(&TextComponent::text("InvalidHand"));
            return;
        };
        if self.version.load() >= JavaMinecraftVersion::V_1_21
            && (!use_item.yaw.is_finite() || !use_item.pitch.is_finite())
        {
            self.try_kick(&TextComponent::text("无效的物品使用旋转"));
            return;
        }
        self.update_sequence(use_item.sequence.0);

        let mut item_in_hand = inventory.get_stack_in_hand(hand);

        let mut consume_event =
            crate::plugin::api::events::player::player_item_consume::PlayerItemConsumeEvent::new(
                player.clone(),
                item_in_hand.item.registry_key.to_string(),
            );
        server
            .plugin_manager
            .fire_blocking(server, &mut consume_event);
        if consume_event.cancelled {
            return;
        }

        let (item_id, _item) = (item_in_hand.item.id, item_in_hand.item);
        player.increment_stat(StatisticCategory::Used, item_id as i32, 1);

        let hit_result = player.world().raycast(
            player.eye_position(),
            player.eye_position().add(
                &(Vector3::rotation_vector(f64::from(use_item.pitch), f64::from(use_item.yaw))
                    * 4.5),
            ),
            |pos, world| {
                let block = world.get_block(pos);
                block != &Block::AIR && block != &Block::WATER && block != &Block::LAVA
            },
        );

        let event = if let Some((hit_pos, _hit_dir)) = hit_result {
            PlayerInteractEvent::new(
                player,
                InteractAction::RightClickBlock,
                player.world().get_block(&hit_pos),
                Some(hit_pos),
            )
        } else {
            PlayerInteractEvent::new(player, InteractAction::RightClickAir, &Block::AIR, None)
        };
        let (item_for_use, stack_for_use) = (item_in_hand.item, item_in_hand.clone());
        let (use_yaw, use_pitch) = if self.version.load() >= JavaMinecraftVersion::V_1_21 {
            (use_item.yaw, use_item.pitch)
        } else {
            player.rotation()
        };
        Self::prepare_hand_item_for_use(player, hand, &mut item_in_hand);

        if !Self::should_continue_use_after_fish_event(server, player, hand, item_for_use) {
            return;
        }

        send_cancellable_blocking! {{
            server;
            event;
            'after: {
                server
                    .item_registry
                    .on_use_with_rotation(&stack_for_use, player, use_yaw, use_pitch);
            }
        }}
    }

    fn prepare_hand_item_for_use(player: &Arc<Player>, hand: Hand, held: &mut ItemStack) {
        let inventory = player.inventory();
        // 进入函数时的槽位快照：装备分支最终写回按数量增量合并
        let before = held.clone();

        if let Some(cooldown) = held.get_use_cooldown() {
            let group = cooldown
                .cooldown_group
                .clone()
                .unwrap_or_else(|| held.item.registry_key.to_string());
            if player.is_on_cooldown(&group) {
                return;
            }
        }

        if held.get_data_component::<ConsumableImpl>().is_some()
            || held.get_data_component::<BlocksAttacksImpl>().is_some()
        {
            // 如果它是食物，我们要确保确实可以食用
            if let Some(food) = held.get_data_component::<FoodImpl>() {
                if player.can_eat(food.can_always_eat) {
                    player.living_entity.set_active_hand(
                        hand,
                        held.clone(),
                        held.get_max_use_time(),
                    );
                }
            } else {
                player
                    .living_entity
                    .set_active_hand(hand, held.clone(), held.get_max_use_time());
            }
        }
        let equipment_slot = held
            .get_data_component::<EquippableImpl>()
            .filter(|equippable| equippable.swappable)
            .map(|equippable| equippable.slot.clone());
        if let Some(slot) = equipment_slot {
            // 整个“读-改-写”在单次持锁内完成：原先分两次加锁，
            // 间隙中并发的装备写入（如发射器装备）会换掉槽位，
            // 造成丢一件或覆盖一件。
            // enqueue_equipment_change 与 set_stack_in_hand 必须在
            // 锁外调用：前者会触发盔甲变更插件事件（阻塞），后者
            // 在副手场景会重入同一映射（自死锁）。
            let equipped_opt = {
                let mut equipment = inventory
                    .entity_equipment
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let current_equipped = equipment.get(&slot);
                if current_equipped.are_items_and_components_equal(held) {
                    None
                } else {
                    let equipped = if current_equipped.is_empty() {
                        let equipped = held.clone();
                        held.decrement_unless_creative(player.gamemode.load(), 1);
                        equipped
                    } else {
                        std::mem::replace(held, current_equipped)
                    };
                    equipment.put(&slot, equipped.clone());
                    Some(equipped)
                }
            };
            if let Some(equipped) = equipped_opt {
                player.enqueue_equipment_change(&slot, &equipped);
                // 条件写回：本地修改按数量增量在写锁内合并进槽位当前
                // 内容，装备事件等待期间并发并入该槽位的物品不会被
                // 陈旧快照覆盖
                inventory.merge_held_delta(hand, &before, held);
            }
        }
    }

    fn should_continue_use_after_fish_event(
        server: &Arc<Server>,
        player: &Arc<Player>,
        hand: Hand,
        item_for_use: &Item,
    ) -> bool {
        if item_for_use.id != Item::FISHING_ROD.id {
            return true;
        }

        // TODO: 收杆时根据捕获类型应用钓鱼竿耐久消耗。
        let mut fish_event = PlayerFishEvent::new(
            player.clone(),
            None,
            uuid::Uuid::nil(),
            String::new(),
            PlayerFishState::Fishing,
            hand,
            0,
        );
        server.plugin_manager.fire_blocking(server, &mut fish_event);
        !fish_event.cancelled
    }
}
