#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    #[expect(clippy::too_many_lines)]
    pub fn handle_player_action(
        &self,
        player: &Arc<Player>,
        player_action: &SPlayerAction,
        server: &Server,
    ) {
        if !player.has_client_loaded() {
            return;
        }
        player.update_last_action_time();
        match Status::try_from(player_action.status.0) {
            Ok(status) => match status {
                Status::StartedDigging => {
                    if !player.can_interact_with_block_at(&player_action.position, 1.0) {
                        warn!(
                            "玩家 {0} 试图交互 {1} 处无法触及的方块",
                            player.gameprofile.name, player_action.position
                        );
                        self.update_sequence(player_action.sequence.0);
                        return;
                    }
                    let position = player_action.position;
                    let entity = &player.get_entity();
                    let world = entity.world.load_full();
                    let (block, state) = world.get_block_and_state(&position);

                    if let Some(server_arc) = world.server.upgrade() {
                        let mut event =
                            crate::plugin::api::events::block::block_damage::BlockDamageEvent::new(
                                player.clone(),
                                block,
                                position,
                                false,
                            );
                        server_arc
                            .plugin_manager
                            .fire_blocking(&server_arc, &mut event);
                        if event.cancelled {
                            self.update_sequence(player_action.sequence.0);
                            return;
                        }
                    }

                    if block == &papokin_data::Block::NOTE_BLOCK {
                        let props =
                            papokin_data::block_properties::NoteBlockLikeProperties::from_state_id(
                                state.id,
                            );
                        crate::block::blocks::note::NoteBlock::play_note(&props, &world, &position);
                        player.increment_stat(
                            StatisticCategory::Custom,
                            CustomStatistic::PlayNoteblock as i32,
                            1,
                        );
                    }

                    let inventory = player.inventory();
                    let held = inventory.held_item();
                    if !server.item_registry.can_mine(held.item, player) {
                        player.try_send_client_packet(&CBlockUpdate::new(
                            position,
                            VarInt(i32::from(state.id.as_u16())),
                        ));
                        self.update_sequence(player_action.sequence.0);
                        return;
                    }

                    // TODO: 进行校验
                    // TODO: 配置
                    if player.gamemode.load() == GameMode::Creative {
                        // 破坏方块并播放音效
                        let new_state = world.break_block(
                            &position,
                            Some(player),
                            BlockFlags::NOTIFY_ALL | BlockFlags::SKIP_DROPS,
                        );
                        if new_state.is_some() {
                            server
                                .block_registry
                                .broken(&world, block, player, &position, server, state);
                        }
                        self.sync_block_state_to_client(&world, position);
                        self.update_sequence(player_action.sequence.0);
                        return;
                    }
                    player.start_mining_time.store(
                        player.tick_counter.load(Ordering::Relaxed),
                        Ordering::Relaxed,
                    );
                    if !state.is_air() {
                        let speed = block::calc_block_breaking(player, state, block);
                        // 即时破坏
                        if speed >= 1.0 {
                            let broken_state = world.get_block_state(&position);
                            let can_harvest = player.can_harvest(broken_state, block);
                            let flags = if can_harvest {
                                BlockFlags::NOTIFY_ALL
                            } else {
                                BlockFlags::SKIP_DROPS | BlockFlags::NOTIFY_ALL
                            };
                            let new_state = world.break_block(&position, Some(player), flags);
                            if new_state.is_some() {
                                server.block_registry.broken(
                                    &world,
                                    block,
                                    player,
                                    &position,
                                    server,
                                    broken_state,
                                );
                                player.apply_tool_damage_for_block_break(broken_state);
                                if can_harvest {
                                    player.add_exhaustion(MINE_BLOCK_EXHAUSTION);
                                }
                                let item_id = player.inventory().held_item().item.id;
                                player.increment_stat(StatisticCategory::Used, item_id as i32, 1);
                                player.increment_stat(
                                    StatisticCategory::Mined,
                                    block.id.as_u16() as i32,
                                    1,
                                );
                            }
                            self.sync_block_state_to_client(&world, position);
                        } else {
                            player.mining.store(true, Ordering::Relaxed);
                            *player
                                .mining_pos
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner) = position;
                            let progress = (speed * 10.0) as i32;
                            player
                                .current_block_breaking_speed
                                .store(speed.to_bits(), Ordering::Relaxed);
                            world.set_block_breaking(
                                entity,
                                position,
                                BlockBreakingProgress::Start { stage: progress },
                            );
                            player
                                .current_block_destroy_stage
                                .store(progress, Ordering::Relaxed);
                        }
                    }
                    self.update_sequence(player_action.sequence.0);
                }
                Status::CancelledDigging => {
                    if !player.can_interact_with_block_at(&player_action.position, 1.0) {
                        warn!(
                            "玩家 {0} 试图交互 {1} 处无法触及的方块",
                            player.gameprofile.name, player_action.position
                        );
                        self.update_sequence(player_action.sequence.0);
                        return;
                    }
                    let entity = &player.get_entity();
                    let world = entity.world.load_full();
                    if let Some(server_arc) = world.server.upgrade() {
                        let mut abort_event = crate::plugin::api::events::block::block_damage_abort::BlockDamageAbortEvent::new(
                            player.clone(),
                            player_action.position,
                            world.clone(),
                            player.inventory().held_item(),
                        );
                        server_arc
                            .plugin_manager
                            .fire_blocking(&server_arc, &mut abort_event);
                    }

                    player.mining.store(false, Ordering::Relaxed);
                    world.set_block_breaking(
                        entity,
                        player_action.position,
                        BlockBreakingProgress::Stop,
                    );
                    self.update_sequence(player_action.sequence.0);
                }
                Status::FinishedDigging => {
                    let location = player_action.position;
                    if !player.can_interact_with_block_at(&location, 1.0) {
                        warn!(
                            "玩家 {0} 试图交互 {1} 处无法触及的方块",
                            player.gameprofile.name, player_action.position
                        );
                        self.update_sequence(player_action.sequence.0);
                        return;
                    }

                    // 原版行为：服务端只承认进度达标的挖掘完成。改过的
                    // 客户端可以不等待挖掘时间直接连发 Started/Finished
                    // 瞬时破坏任意方块（黑曜石甚至基岩），因此此处必须
                    // 校验该方块确实被持续挖掘至进度 >= 1.0。创造模式除外
                    //（Started 即破坏，与原版一致）。
                    if player.gamemode.load() != GameMode::Creative {
                        let (mining, mining_pos) = {
                            let pos = player
                                .mining_pos
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                            (player.mining.load(Ordering::Relaxed), *pos)
                        };
                        let elapsed = player
                            .tick_counter
                            .load(Ordering::Relaxed)
                            .saturating_sub(player.start_mining_time.load(Ordering::Relaxed));
                        // 与 continue_mining 同式（time + 1），容忍客户端
                        // 相对服务端 tick 先行一步的网络时序
                        let speed = f32::from_bits(
                            player.current_block_breaking_speed.load(Ordering::Relaxed),
                        );
                        let progress = speed * (elapsed + 1) as f32;
                        if !mining || mining_pos != location || progress < 1.0 {
                            warn!(
                                "玩家 {} 声称完成挖掘 {}，但进度未达标（挖掘中：{}，进度 {:.2}），已驳回",
                                player.gameprofile.name,
                                location,
                                mining && mining_pos == location,
                                progress
                            );
                            // 拒绝破坏，并把真实方块状态同步回客户端以恢复显示
                            let entity = &player.get_entity();
                            let world = entity.world.load_full();
                            self.sync_block_state_to_client(&world, location);
                            self.update_sequence(player_action.sequence.0);
                            return;
                        }
                    }

                    // 破坏方块并播放音效
                    let entity = &player.get_entity();
                    let world = entity.world.load_full();

                    player.mining.store(false, Ordering::Relaxed);
                    world.set_block_breaking(entity, location, BlockBreakingProgress::Stop);

                    let (block, state) = world.get_block_and_state(&location);
                    let block_drop = player.gamemode.load() != GameMode::Creative
                        && player.can_harvest(state, block);

                    let new_state = world.break_block(
                        &location,
                        Some(player),
                        if block_drop {
                            BlockFlags::NOTIFY_ALL
                        } else {
                            BlockFlags::SKIP_DROPS | BlockFlags::NOTIFY_ALL
                        },
                    );
                    if new_state.is_some() {
                        server
                            .block_registry
                            .broken(&world, block, player, &location, server, state);

                        player.apply_tool_damage_for_block_break(state);
                        if block_drop {
                            player.add_exhaustion(MINE_BLOCK_EXHAUSTION);
                        }
                        let item_id = player.inventory().held_item().item.id;
                        player.increment_stat(StatisticCategory::Used, item_id as i32, 1);
                        player.increment_stat(
                            StatisticCategory::Mined,
                            block.id.as_u16() as i32,
                            1,
                        );
                    }

                    self.sync_block_state_to_client(&world, location);

                    self.update_sequence(player_action.sequence.0);
                }
                Status::DropItem => {
                    player.drop_held_item(false);
                }
                Status::DropItemStack => {
                    player.drop_held_item(true);
                }
                Status::ReleaseItemInUse => {
                    let item_in_use = player
                        .living_entity
                        .item_in_use
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .clone();
                    if let Some(stack) = item_in_use {
                        // 停止使用物品的通知（例如放开弓）。
                        let world = player.get_entity().world.load_full();
                        if let Some(server_arc) = world.server.upgrade() {
                            let mut stop_event = crate::plugin::api::events::player::player_stop_using_item::PlayerStopUsingItemEvent::new(
                                player.clone(),
                                stack.clone(),
                            );
                            server_arc
                                .plugin_manager
                                .fire_blocking(&server_arc, &mut stop_event);
                        }
                        server.item_registry.on_stopped_using(&stack, player);
                    }

                    player.living_entity.clear_active_hand();
                }
                Status::SwapItem => {
                    player.swap_item();
                }
                Status::SpearJab => {
                    if player.gamemode.load() == GameMode::Spectator {
                        return;
                    }

                    let stack = player.inventory().held_item();
                    server.item_registry.on_spear_jab(&stack, player);
                }
                // 被挖掘的方块没有变化，因此无需更新任何内容
                Status::ChangeDestroyDirection => {}
            },
            Err(_) => self.try_kick(&TextComponent::text("无效的状态")),
        }
    }

    pub fn update_sequence(&self, sequence: i32) {
        if sequence < 0 {
            error!("数据包序列号应 >= 0");
        }
        self.packet_sequence.store(
            self.packet_sequence.load(Ordering::Relaxed).max(sequence),
            Ordering::Relaxed,
        );
    }

    fn sync_block_state_to_client(&self, world: &World, position: BlockPos) {
        let synced_state_id = world.get_block_state_id(&position);
        self.try_send_packet(&CBlockUpdate::new(
            position,
            VarInt(i32::from(synced_state_id.as_u16())),
        ));
    }
}
