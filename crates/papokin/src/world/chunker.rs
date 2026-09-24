use papokin_util::math::vector2::Vector2;
use std::{num::NonZero, sync::Arc};

use papokin_protocol::java::client::play::CCenterChunk;
use papokin_world::cylindrical_chunk_iterator::Cylindrical;

use crate::entity::{EntityBase, player::Player};

pub fn get_view_distance(player: &Player) -> NonZero<u8> {
    let fallback = NonZero::new(2).unwrap_or(NonZero::<u8>::MIN);
    let Some(server) = player.world().server.upgrade() else {
        return fallback;
    };
    let max_view_distance = server.advanced_config.networking.java.view_distance;
    player
        .config
        .load()
        .view_distance
        .clamp(fallback, max_view_distance)
}

// 检查目标区块与中心区块的切比雪夫距离（L_∞）是否在范围内。
#[must_use]
#[inline]
pub fn is_within_chebyshev_distance(
    center: Vector2<i32>,
    target: Vector2<i32>,
    distance: i32,
) -> bool {
    (target.x - center.x).abs().max((target.y - center.y).abs()) <= distance
}

#[allow(clippy::too_many_lines)]
pub fn update_position(player: &Arc<Player>) {
    let entity = &player.get_entity();
    let new_chunk_center = entity.chunk_pos.load();
    let old_cylindrical = player.watched_section.load();

    // 当有新玩家生成时，这确实会失效
    // if old_cylindrical.center == new_chunk_center {
    //     return;
    // }

    let view_distance = get_view_distance(player);
    let new_cylindrical = Cylindrical::new(new_chunk_center, view_distance);

    if old_cylindrical == new_cylindrical {
        return;
    }

    player.client.try_send_packet(&CCenterChunk {
        chunk_x: new_chunk_center.x.into(),
        chunk_z: new_chunk_center.y.into(),
    });
    let (loading_iter, unloading_iter) =
        Cylindrical::changed_chunks(old_cylindrical, new_cylindrical);
    let loading_chunks: Vec<_> = loading_iter.collect();
    let unloading_chunks: Vec<_> = unloading_iter.collect();

    let world = player.world();
    let level = &world.level;
    let mut held_tickets = player
        .held_chunk_tickets
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let is_spectator = player.is_spectator();
    let spectators_generate_chunks = world
        .level_info
        .load()
        .game_rules
        .spectators_generate_chunks;

    let new_view_level = (!is_spectator || spectators_generate_chunks).then(|| {
        papokin_world::chunk_system::ChunkLoading::get_level_from_view_distance(
            u8::from(view_distance) + 1,
        )
    });

    let new_sim_level = (!is_spectator || spectators_generate_chunks).then(|| {
        let sim_dist = world.server.upgrade().map_or(10, |s| {
            s.advanced_config.networking.java.simulation_distance.get()
        });
        papokin_world::chunk_system::ChunkLoading::get_level_from_simulation_distance(sim_dist)
    });

    {
        let mut lock = level
            .chunk_loading
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if let Some(view) = new_view_level {
            lock.add_ticket(new_chunk_center, view);
        }
        if let Some(sim) = new_sim_level {
            lock.add_ticket(new_chunk_center, sim);
        }

        if let Some((held_center, held_view, held_sim)) =
            held_tickets.replace((new_chunk_center, new_view_level, new_sim_level))
        {
            // 用加票时记录的中心移除（而非当前 chunk_pos），见
            // held_chunk_tickets 的字段注释。
            if let Some(view) = held_view {
                lock.remove_ticket(held_center, view);
            }
            if let Some(sim) = held_sim {
                lock.remove_ticket(held_center, sim);
            }
        }
        lock.send_change();
    };
    drop(held_tickets);

    {
        let mut sender = player
            .chunk_sender
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for pos in &unloading_chunks {
            sender.unload_chunk(&player.client, *pos);
        }
        for pos in &loading_chunks {
            sender.enqueue_chunk(*pos);
        }
    }
    player.watched_section.store(new_cylindrical);

    // 确保被监视的区块和区块监视器的更新是异步原子的。我们要
    // 确保玩家断开连接时我们卸载的内容是正确的。
    if !loading_chunks.is_empty() || !unloading_chunks.is_empty() {
        let level = world.level.clone();
        let world_clone = world.clone();
        let loading_chunks_clone = loading_chunks.clone();
        let unloading_chunks_clone = unloading_chunks;

        if let Some(server) = world.server.upgrade() {
            server.spawn_task(async move {
                level
                    .mark_chunks_as_newly_watched(&loading_chunks_clone)
                    .await;
                let chunks_to_clean = level
                    .mark_chunks_as_not_watched(&unloading_chunks_clone)
                    .await;

                if !chunks_to_clean.is_empty() {
                    world_clone
                        .remove_entities_in_chunks(&chunks_to_clean)
                        .await;
                    world_clone.level.clean_entity_chunks(&chunks_to_clean);
                }
            });
        }
    }

    if !loading_chunks.is_empty() {
        world.spawn_world_entity_chunks(player.clone(), loading_chunks);
    }
    world.entity_tracker.update_player_position(player, &world);
}
