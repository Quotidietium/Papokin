#[allow(clippy::wildcard_imports)]
use super::*;
use crate::item::registry::should_try_block_placement;

impl JavaClient {
    #[allow(clippy::too_many_lines)]
    pub fn handle_use_item_on(
        &self,
        player: &Arc<Player>,
        use_item_on: &SUseItemOn,
        server: &Arc<Server>,
    ) -> Result<(), BlockPlacingError> {
        if !player.has_client_loaded() {
            return Ok(());
        }
        player.update_last_action_time();
        self.update_sequence(use_item_on.sequence.0);

        let position = use_item_on.position;
        let cursor_pos = use_item_on.cursor_pos;

        let mut should_try_decrement = false;

        if !player.can_interact_with_block_at(&position, 1.0) {
            // TODO: 也许该记录日志？
            return Err(BlockPlacingError::BlockOutOfReach);
        }

        let Ok(face) = BlockDirection::try_from(use_item_on.face.0) else {
            return Err(BlockPlacingError::InvalidBlockFace);
        };

        let Ok(hand) = Hand::from_packet_id(use_item_on.hand.0) else {
            return Err(BlockPlacingError::InvalidHand);
        };

        if player.gamemode.load() == GameMode::Spectator {
            let entity = &player.get_entity();
            let world = entity.world.load_full();
            let block = world.get_block(&position);

            let event = PlayerInteractEvent::new(
                player,
                InteractAction::RightClickBlock,
                block,
                Some(position),
            );

            send_cancellable_blocking! {{
                server;
                event;
                'cancelled: {
                    let state_id = world.get_block_state_id(&position);
                    player.try_send_client_packet(&CBlockUpdate::new(
                        position,
                        VarInt(i32::from(state_id.as_u16())),
                    ));
                    return Ok(());
                }
            }}

            if let Some(factory) = server
                .block_registry
                .get_screen_handler_factory(block, player, &position, server, &world)
            {
                player.open_handled_screen(factory.as_ref(), Some(position));
            }
            return Ok(());
        }

        let inventory = player.inventory();
        let held_item = inventory.held_item();
        let off_hand_item = inventory.off_hand_item();
        let held_item_empty = held_item.is_empty();
        let off_hand_item_empty = off_hand_item.is_empty();

        let mut item = inventory.get_stack_in_hand(hand);
        let item_id = item.item.id;
        player.increment_stat(StatisticCategory::Used, item_id as i32, 1);

        let entity = &player.get_entity();
        let world = entity.world.load_full();
        let block = world.get_block(&position);

        let event = PlayerInteractEvent::new(
            player,
            InteractAction::RightClickBlock,
            block,
            Some(position),
        );

        send_cancellable_blocking! {{
            server;
            event;
            'cancelled: {
                let state_id = world.get_block_state_id(&position);
                player.try_send_client_packet(&CBlockUpdate::new(
                    position,
                    VarInt(i32::from(state_id.as_u16())),
                ));
                return Ok(());
            }
        }}

        let equipment_slot = if matches!(hand, Hand::Right) {
            EquipmentSlot::MAIN_HAND
        } else {
            EquipmentSlot::OFF_HAND
        };

        let sneaking = player.get_entity().is_sneaking();

        // 代码基于 Java 类 ServerPlayerInteractionManager
        if !(sneaking && (!held_item_empty || !off_hand_item_empty)) {
            let result = Self::call_use_item_on(
                player,
                &position,
                &cursor_pos,
                face,
                &mut item,
                &equipment_slot,
                &world,
                block,
                server,
            );
            if result.consumes_action() {
                // TODO: 触发 ANY_BLOCK_USE 判据

                if matches!(result, BlockActionResult::SuccessServer) {
                    player.swing_hand(hand, true);
                }
                return Ok(());
            }
        }

        let slot_index = if matches!(hand, Hand::Right) {
            inventory.get_selected_slot() as usize
        } else {
            PlayerInventory::OFF_HAND_SLOT
        };

        if item.is_empty() {
            // TODO 物品冷却
            // 如果手为空，我们在此停止
            return Ok(());
        }

        let before = item.clone();

        let item_result = server
            .item_registry
            .use_on_block(&mut item, player, position, face, cursor_pos, block, server);

        if should_try_block_placement(&item_result) {
            // 检查物品是否为方块，因为并非每个物品都能放置 :D
            let item_id = item.item.id;
            if let Some(block) = Block::from_item_id(item_id) {
                should_try_decrement =
                    Self::run_is_block_place(player, block, server, use_item_on, position, face)?;
            }
        }

        if should_try_decrement {
            // TODO: 配置
            // 减少方块计数
            if player.gamemode.load() != GameMode::Creative {
                item.decrement(1);
            }
        }

        let after = item.clone();

        if matches!(item_result, BlockActionResult::SuccessServer) {
            player.swing_hand(hand, true);
        }

        // 在槽位同步之前广播破坏实体状态；客户端
        // 需要槽位中的旧物品贴图来生成破坏粒子。
        if !before.is_empty() && after.is_empty() {
            let slot = if slot_index == player.inventory.get_selected_slot() as usize {
                &EquipmentSlot::MAIN_HAND
            } else {
                &EquipmentSlot::OFF_HAND
            };
            if before.is_damageable() {
                player.increment_stat(StatisticCategory::Broken, before.item.id as i32, 1);
            }
            player
                .world()
                .send_entity_status(player.get_entity(), equipment_break_status(slot));
        }

        if !after.are_equal(&before) {
            player.sync_hand_slot(slot_index, after.clone());
            inventory.set_stack_in_hand(hand, after);
        }

        Ok(())
    }

    #[expect(clippy::too_many_arguments)]
    fn call_use_item_on(
        player: &Arc<Player>,
        position: &BlockPos,
        cursor_pos: &Vector3<f32>,
        face: BlockDirection,
        held_item: &mut ItemStack,
        equipment_slot: &EquipmentSlot,
        world: &Arc<World>,
        block: &Block,
        server: &Arc<Server>,
    ) -> BlockActionResult {
        let result = server.block_registry.use_with_item(
            block,
            player,
            position,
            &BlockHitResult {
                face: &face,
                cursor_pos,
            },
            held_item,
            equipment_slot,
            server,
            world,
        );

        if result.consumes_action() {
            // TODO: 触发 ITEM_USED_ON_BLOCK 判据
            return result;
        }

        if matches!(result, BlockActionResult::PassToDefaultBlockAction) {
            let result = server.block_registry.on_use(
                block,
                player,
                position,
                &BlockHitResult {
                    face: &face,
                    cursor_pos,
                },
                server,
                world,
            );

            if result.consumes_action() {
                // TODO: 触发 DEFAULT_BLOCK_USE 判据
                return result;
            }
        }

        BlockActionResult::Pass
    }

    fn run_is_block_place(
        player: &Arc<Player>,
        block: &'static Block,
        server: &Arc<Server>,
        use_item_on: &SUseItemOn,
        location: BlockPos,
        face: BlockDirection,
    ) -> Result<bool, BlockPlacingError> {
        match server
            .block_registry
            .place_block(player, block, server, use_item_on, location, face)
        {
            Ok(Some((final_block_pos, new_state))) => {
                player.try_send_client_packet(&CBlockUpdate::new(
                    final_block_pos,
                    VarInt(i32::from(new_state.as_u16())),
                ));
                Ok(true)
            }
            Ok(None) => Ok(false),
            Err(crate::block::registry::BlockPlacingError::InvalidGamemode) => {
                Err(BlockPlacingError::InvalidGamemode)
            }
            Err(crate::block::registry::BlockPlacingError::BlockOutOfWorld) => {
                Err(BlockPlacingError::BlockOutOfWorld)
            }
        }
    }
}
