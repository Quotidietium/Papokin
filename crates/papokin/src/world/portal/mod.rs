use std::sync::Arc;

use papokin_data::Block;
use papokin_data::block_properties::HorizontalAxis;
use papokin_data::dimension::Dimension;
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector2::Vector2;
use papokin_util::math::vector3::Vector3;
use papokin_world::world::BlockFlags;

use super::World;

pub mod end;
pub mod nether;
pub mod poi;

pub use nether::{NetherPortal, PortalSearchResult};
pub use poi::PortalPoiStorage;

#[derive(Clone)]
pub struct SourcePortalInfo {
    pub lower_corner: BlockPos,
    pub axis: HorizontalAxis,
    pub width: u32,
    pub height: u32,
}

impl From<&PortalSearchResult> for SourcePortalInfo {
    fn from(result: &PortalSearchResult) -> Self {
        Self {
            lower_corner: result.lower_corner,
            axis: result.axis,
            width: result.width,
            height: result.height,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PortalType {
    Nether,
    End,
}

impl PortalType {
    pub fn get_portal_transition_time(
        &self,
        current_world: &World,
        entity: &dyn crate::entity::EntityBase,
    ) -> u32 {
        match self {
            Self::End => 0,
            Self::Nether => {
                let entity_type = entity.get_entity().entity_type;
                let level_info = current_world.level_info.load();
                match entity_type.id {
                    id if id == papokin_data::entity::EntityType::PLAYER.id => (current_world
                        .get_player_by_id(entity.get_entity().entity_id))
                    .map_or(80, |player| match player.gamemode.load() {
                        papokin_util::GameMode::Creative => {
                            level_info.game_rules.players_nether_portal_creative_delay as u32
                        }
                        _ => level_info.game_rules.players_nether_portal_default_delay as u32,
                    }),
                    _ => 0,
                }
            }
        }
    }

    #[expect(clippy::too_many_lines)]
    pub fn get_portal_destination(
        &self,
        current_level: &World,
        dest_world: Arc<World>,
        caller: &dyn crate::entity::EntityBase,
        source_portal: Option<&SourcePortalInfo>,
    ) -> Option<TeleportTransition> {
        match self {
            Self::End => {
                let is_end_portal = dest_world.dimension == Dimension::THE_END
                    || current_level.dimension == Dimension::THE_END;

                if is_end_portal {
                    if dest_world.dimension == Dimension::THE_END {
                        // 进入末地：玩家生成在 (100, 49, 0) 的黑曜石平台上，其他实体生成在 (100, 50, 0)
                        let is_player = caller
                            .get_living_entity()
                            .is_some_and(crate::entity::living::LivingEntity::is_player);
                        let y = if is_player { 49.0 } else { 50.0 };

                        let platform_pos = BlockPos::new(100, 49, 0);

                        // 确保覆盖平台的区块已加载/生成
                        if let Ok(handle) = tokio::runtime::Handle::try_current() {
                            tokio::task::block_in_place(|| {
                                handle.block_on(async {
                                    let center_chunk =
                                        Vector2::new(platform_pos.0.x >> 4, platform_pos.0.z >> 4);
                                    dest_world
                                        .level
                                        .get_or_fetch_chunk(center_chunk, |_| ())
                                        .await;
                                });
                            });
                        }

                        // 生成/重新生成黑曜石平台（Y=48 处 5x5 黑曜石，上方为 5x5x3 空气）
                        for dx in -2..=2 {
                            for dz in -2..=2 {
                                for dy in -1..3 {
                                    let block = if dy == -1 {
                                        Block::OBSIDIAN
                                    } else {
                                        Block::AIR
                                    };
                                    let target_pos = BlockPos::new(
                                        platform_pos.0.x + dx,
                                        platform_pos.0.y + dy,
                                        platform_pos.0.z + dz,
                                    );
                                    dest_world.set_block_state(
                                        &target_pos,
                                        block.default_state.id,
                                        BlockFlags::NOTIFY_ALL,
                                    );
                                }
                            }
                        }

                        Some(TeleportTransition {
                            new_world: dest_world,
                            position: Vector3::new(100.5f64, y, 0.5f64),
                            yaw: Some(90.0f32),
                            pitch: None,
                        })
                    } else {
                        // 通过出口传送门离开末地：返回主世界出生点
                        if let Some(player) =
                            current_level.get_player_by_id(caller.get_entity().entity_id)
                        {
                            let client = &player.client;
                            if let Ok(data) = client.serialize_packet(
                                &papokin_protocol::java::client::play::CGameEvent::new(
                                    papokin_protocol::java::client::play::GameEvent::WinGame,
                                    1.0,
                                ),
                            ) {
                                client.try_enqueue_packet(data);
                            }
                        }

                        let info = dest_world.level_info.load();
                        Some(TeleportTransition {
                            new_world: dest_world,
                            position: Vector3::new(
                                f64::from(info.spawn_x) + 0.5,
                                f64::from(info.spawn_y),
                                f64::from(info.spawn_z) + 0.5,
                            ),
                            yaw: None,
                            pitch: None,
                        })
                    }
                } else {
                    None
                }
            }
            Self::Nether => {
                let pos = caller.get_entity().pos.load();
                let current_yaw = caller.get_entity().yaw.load();
                let dimensions = caller.get_entity().entity_dimension.load();
                let scale_factor_new = dest_world.dimension.coordinate_scale;
                let scale_factor_current = current_level.dimension.coordinate_scale;

                let teleportation_scale = scale_factor_current / scale_factor_new;
                let worldborder = dest_world
                    .worldborder
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let (clamped_x, clamped_z) = worldborder.clamp_block(
                    (pos.x * teleportation_scale).floor() as i32,
                    (pos.z * teleportation_scale).floor() as i32,
                );
                drop(worldborder);

                let approximate_exit_pos =
                    BlockPos::new(clamped_x, pos.y.floor() as i32, clamped_z);
                let source_portal_axis = source_portal.map_or(HorizontalAxis::X, |p| p.axis);

                let exit_portal = NetherPortal::search_for_portal(
                    &dest_world,
                    approximate_exit_pos,
                )
                .or_else(|| {
                    // 确保 dest_world 中 approximate_exit_pos 周围的区块已生成/加载
                    if let Ok(handle) = tokio::runtime::Handle::try_current() {
                        tokio::task::block_in_place(|| {
                            handle.block_on(async {
                                let center_chunk = Vector2::new(
                                    approximate_exit_pos.0.x >> 4,
                                    approximate_exit_pos.0.z >> 4,
                                );
                                for dx in -1..=1 {
                                    for dz in -1..=1 {
                                        let chunk_pos =
                                            Vector2::new(center_chunk.x + dx, center_chunk.y + dz);
                                        dest_world
                                            .level
                                            .get_or_fetch_chunk(chunk_pos, |_| ())
                                            .await;
                                    }
                                }
                            });
                        });
                    }

                    if let Some((build_pos, axis, is_fallback)) = NetherPortal::find_safe_location(
                        &dest_world,
                        approximate_exit_pos,
                        source_portal_axis,
                    ) {
                        NetherPortal::build_portal_frame(&dest_world, build_pos, axis, is_fallback);
                        Some(PortalSearchResult {
                            lower_corner: build_pos,
                            axis,
                            width: 2,
                            height: 3,
                        })
                    } else {
                        None
                    }
                });

                let (final_pos, yaw) = exit_portal.map_or_else(
                    || (approximate_exit_pos.0.to_f64(), None),
                    |exit_portal| {
                        let relative_offset = source_portal.map_or_else(
                            || Vector3::new(0.5, 0.0, 0.0),
                            |source| {
                                let source_result = PortalSearchResult {
                                    lower_corner: source.lower_corner,
                                    axis: source.axis,
                                    width: source.width,
                                    height: source.height,
                                };
                                source_result.entity_pos_in_portal(pos, &dimensions)
                            },
                        );
                        let target_pos =
                            exit_portal.calculate_exit_position(relative_offset, &dimensions);
                        let collision_free_pos =
                            exit_portal.find_open_position(&dest_world, target_pos, &dimensions);
                        let yaw = exit_portal
                            .calculate_teleport_yaw(current_yaw, source_portal.map(|p| p.axis));
                        (collision_free_pos, Some(yaw))
                    },
                );

                Some(TeleportTransition {
                    new_world: dest_world,
                    position: final_pos,
                    yaw,
                    pitch: None,
                })
            }
        }
    }
}

pub struct TeleportTransition {
    pub new_world: Arc<World>,
    pub position: Vector3<f64>,
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
}

pub struct PortalProcessor {
    pub portal_type: PortalType,
    pub entry_position: BlockPos,
    pub portal_time: u32,
    pub inside_portal_this_tick: bool,
    pub destination_world: Arc<World>,
    pub source_portal: Option<SourcePortalInfo>,
}

impl PortalProcessor {
    pub const fn new(
        portal_type: PortalType,
        entry_position: BlockPos,
        destination_world: Arc<World>,
    ) -> Self {
        Self {
            portal_type,
            entry_position,
            portal_time: 0,
            inside_portal_this_tick: true,
            destination_world,
            source_portal: None,
        }
    }

    pub const fn set_source_portal(&mut self, info: SourcePortalInfo) {
        self.source_portal = Some(info);
    }

    pub fn process_portal_teleportation(
        &mut self,
        current_world: &World,
        entity: &dyn crate::entity::EntityBase,
        allowed_to_teleport: bool,
    ) -> bool {
        if self.inside_portal_this_tick {
            self.inside_portal_this_tick = false;
            if allowed_to_teleport {
                self.portal_time += 1;
                let transition_time = self
                    .portal_type
                    .get_portal_transition_time(current_world, entity);
                self.portal_time >= transition_time
            } else {
                false
            }
        } else {
            self.decay_tick();
            false
        }
    }

    pub const fn decay_tick(&mut self) {
        self.portal_time = self.portal_time.saturating_sub(4);
    }

    #[must_use]
    pub const fn has_expired(&self) -> bool {
        self.portal_time == 0
    }
}
