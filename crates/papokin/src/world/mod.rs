use crate::block::entities::{BlockEntity, block_entity_from_nbt};
use dashmap::DashMap;
use papokin_data::chunk::Biome;
use papokin_protocol::codec::data_component::data_to_proto_sound;
use papokin_world::generation::proto_chunk::GenerationCache;
use rayon::prelude::*;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, RwLock, Weak};
use std::{
    collections::{BTreeMap, HashMap},
    sync::atomic::Ordering,
};
use tracing::{debug, error, info, trace, warn};

mod active_chunks;
pub mod chunker;
pub mod explosion;
pub mod generation_cache;
pub mod loot;
pub mod map;
pub mod portal;
pub mod raid;
pub mod random_sequences;
pub mod stopwatches;
pub mod time;
pub mod villager_poi;

use crate::block::RandomTickArgs;
use crate::world::chunker::is_within_chebyshev_distance;
use crate::{block::BlockEvent, entity::item::ItemEntity};
use crate::{
    block::{
        registry::BlockRegistry,
        {OnNeighborUpdateArgs, OnScheduledTickArgs},
    },
    command::client_suggestions,
    entity::{Entity, EntityBase, RemovalReason, player::Player, r#type::from_type},
    error::PapokinError,
    net::java::JavaClient,
    plugin::{
        block::block_break::BlockBreakEvent,
        player::{
            player_change_world::PlayerChangeWorldEvent, player_join::PlayerJoinEvent,
            player_leave::PlayerLeaveEvent, player_respawn::PlayerRespawnEvent,
        },
    },
    server::Server,
};
use active_chunks::{ActiveChunkTracker, ActivePlayerArea};
use arc_swap::ArcSwap;
use border::Worldborder;
use bytes::BufMut;
pub use explosion::{
    BlockInteraction, DefaultExplosionDamageCalculator, Explosion, ExplosionDamageCalculator,
    ExplosionInteraction, SimpleExplosionDamageCalculator,
};
use papokin_config::BasicConfiguration;
use papokin_data::block_properties::{blocks_movement, is_air};
use papokin_data::block_rotation::{Mirror, Rotation};
use papokin_data::data_component_impl::EquipmentSlot;
use papokin_data::dimension::Dimension;
use papokin_data::entity::MobCategory;
use papokin_data::fluid::FluidState;
use papokin_data::game_rules::{GameRule, GameRuleValue};
use papokin_data::noise_settings::NoiseSettings;
use papokin_data::{
    Block, BlockStateId,
    entity::{EntityStatus, EntityType},
    fluid::Fluid,
    item_stack::ItemStack,
    particle::Particle,
    sound::{Sound, SoundCategory},
    sound_id_remap::remap_sound_id_for_version,
    world::{RAW, WorldEvent},
};
use papokin_data::{BlockDirection, BlockState, HorizontalFacingExt, translation};
use papokin_inventory::crafting::recipe_provider::RecipeProvider;
use papokin_inventory::screen_handler::InventoryPlayer;
use papokin_inventory::{Clearable, Inventory};
use papokin_nbt::compound::NbtCompound;
use papokin_protocol::java::client::play::{
    CBlockUpdate, CDisguisedChatMessage, CExplosion, CRespawn, CSetBlockDestroyStage, CWorldEvent,
    PlayerSpawnData,
};
use papokin_protocol::java::client::play::{
    CPlayerSpawnPosition, CRecipeBookAdd, CRecipeBookSettings, CSystemChatMessage,
};
use papokin_protocol::java::client::play::{CSetEntityMetadata, Metadata};
use papokin_protocol::{
    ClientPacket, IdOr, SoundEvent,
    codec::var_int::VarInt,
    java::{
        self,
        client::play::{
            CBlockEntityData, CDamageEvent, CEntityStatus, CGameEvent, CLogin, CMultiBlockUpdate,
            CPlayerInfoUpdate, CRemoveEntities, CRemovePlayerInfo, CSetSelectedSlot, CSoundEffect,
            CSpawnEntity, GameEvent, InitChat, PlayerAction, PlayerInfoFlags,
        },
        server::play::SChatMessage,
    },
};
use papokin_protocol::{
    codec::item_stack_seralizer::ItemStackSerializer,
    java::client::play::{
        CBlockEvent, CParticle, CRemoveMobEffect, CSetEquipment, CUpdateMobEffect,
    },
};
use papokin_util::resource_location::ResourceLocation;
use papokin_util::text::{TextComponent, color::NamedColor};
use papokin_util::version::JavaMinecraftVersion;
use papokin_util::{
    Difficulty,
    math::{boundingbox::BoundingBox, position::BlockPos, vector3::Vector3},
};
use papokin_util::{
    math::{get_section_cord, position::chunk_section_from_pos, vector2::Vector2},
    random::{RandomImpl, get_seed, xoroshiro128::Xoroshiro},
};
use papokin_world::world::{GetBlockError, WorldPortalExt};
use papokin_world::{biome, chunk::io::Dirtiable};
use papokin_world::{chunk::ChunkData, world::BlockAccessor};
use papokin_world::{level::Level, tick::TickPriority};
pub use papokin_world::{world::BlockFlags, world_info::LevelData};
use rand::seq::SliceRandom;
use rand::{RngExt, rng};
use scoreboard::Scoreboard;
use time::LevelTime;

pub mod block_placer;
pub mod border;
pub mod bossbar;
pub mod custom_bossbar;
pub mod dragon_fight;
pub mod end_podium;
pub mod entity_index;
pub mod entity_tracker;
pub mod environment;
pub mod natural_spawner;
pub mod scoreboard;
pub mod weather;

pub use environment::EnvironmentAttributes;
pub use papokin_data::environment_attribute::{Activity, MoonPhase};

use crate::world::natural_spawner::{SpawnState, spawn_for_chunk};
use papokin_config::lighting::LightingEngineConfig;
use papokin_data::effect::StatusEffect;
use papokin_world::chunk::ChunkHeightmapType::{self, MotionBlocking};
use uuid::Uuid;
use weather::Weather;

const MAX_LIGHT_LEVEL: u8 = 15;

use rustc_hash::{FxHashMap, FxHashSet};

impl PapokinError for GetBlockError {
    fn is_kick(&self) -> bool {
        false
    }

    fn severity(&self) -> tracing::Level {
        tracing::Level::WARN
    }

    fn client_kick_reason(&self) -> Option<String> {
        None
    }
}

/// 表示一个 Minecraft 世界，包含实体、玩家以及底层的世界数据。
///
/// 每个维度（主世界、下界、末地）通常都有各自的 `World`。
///
/// **主要职责：**
///
/// - 管理 `Level` 实例，用于处理区块相关操作。
/// - 存储并跟踪世界中的活跃 `Player` 实体。
/// - 提供与世界中的实体和环境交互的中心枢纽。
pub struct World {
    /// 表示世界的唯一标识符
    pub uuid: Uuid,
    /// 底层的世界（level），负责区块管理与地形生成。
    pub level: Arc<Level>,
    pub level_info: Arc<ArcSwap<LevelData>>,
    /// 世界中活跃玩家的映射，以各自的 UUID 为键。
    pub players: ArcSwap<Vec<Arc<Player>>>,
    /// 世界中活跃实体的映射，以各自的 UUID 为键。
    /// 这不包括玩家。
    pub entities: ArcSwap<Vec<Arc<dyn EntityBase>>>,
    /// 按区块分桶的实体空间索引（弱引用）：供 `get_entities_at_box`
    /// 的小范围查询替代全表线性扫描。见 `entity_index` 模块文档。
    pub entities_by_chunk: entity_index::ChunkedEntityIndex<dyn EntityBase>,
    /// 世界的记分板，用于跟踪分数、目标和显示信息。
    pub scoreboard: std::sync::Mutex<Scoreboard>,
    /// 世界的世界边界（worldborder），定义可玩区域并控制其扩张或收缩。
    pub worldborder: std::sync::Mutex<Worldborder>,
    /// 世界的时间，包括用于天气、昼夜循环和统计的刻计数。
    pub level_time: std::sync::Mutex<LevelTime>,
    /// 世界所处的维度类型。
    pub dimension: Dimension,
    pub sea_level: i32,
    pub min_y: i32,
    /// 世界的天气，包括降雨和雷暴等级。
    pub weather: std::sync::Mutex<Weather>,
    /// 方块行为
    pub block_registry: Arc<BlockRegistry>,
    pub server: Weak<Server>,
    synced_block_event_queue: std::sync::Mutex<Vec<BlockEvent>>,
    /// 未发送方块变更的映射，以方块位置为键。
    unsent_block_changes: std::sync::Mutex<HashMap<BlockPos, BlockStateId>>,
    /// 持久化的原版 POI 存储，用于传送门和村民查找。
    pub portal_poi: std::sync::Mutex<portal::PortalPoiStorage>,
    /// 村民的工作站点及其当前所有者。
    pub villager_poi: std::sync::Mutex<villager_poi::VillagerPoiStorage>,
    /// 此世界中正在进行的袭击。
    pub raids: std::sync::Mutex<raid::Raids>,
    /// 末影龙战斗管理器（仅存在于 `THE_END` 维度中）。
    pub dragon_fight: Option<std::sync::Mutex<dragon_fight::DragonFight>>,
    pub spawn_state: ArcSwap<SpawnState>,
    pub active_chunks: RwLock<FxHashSet<Vector2<i32>>>,
    active_chunk_tracker: std::sync::Mutex<ActiveChunkTracker>,
    pub forced_chunks: std::sync::Mutex<FxHashSet<Vector2<i32>>>,
    /// 按区块索引的方块实体，因此 tick 只会访问当前
    /// 活跃区块，而不是每刻扫描所有已加载的方块实体。
    pub block_entities: DashMap<Vector2<i32>, FxHashMap<BlockPos, Arc<dyn BlockEntity>>>,
    pending_block_entity_migrations: crossbeam::queue::SegQueue<Vector2<i32>>,
    /// 世界的持久自定义数据（对应 Bukkit 的 `PersistentDataHolder`）
    pub custom_data: std::sync::Mutex<NbtCompound>,
    /// 特定位置方块实体的持久自定义数据
    pub custom_block_entity_data: DashMap<BlockPos, NbtCompound>,
    /// 实体追踪器，负责跟踪实体可见性，并向观察者发送增量/状态数据包。
    pub entity_tracker: entity_tracker::EntityTracker,
}

#[derive(Clone, Copy)]
pub(crate) enum BlockBreakingProgress {
    Start { stage: i32 },
    Update { stage: i32 },
    Stop,
}

impl PartialEq for World {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid
    }
}

impl Eq for World {}

impl World {
    pub async fn get_block_state_id_async(&self, position: &BlockPos) -> BlockStateId {
        if !self.is_in_build_limit(*position) {
            return Block::AIR.default_state.id;
        }

        let (chunk_coordinate, relative) = position.chunk_and_chunk_relative_position();
        self.level
            .get_or_fetch_chunk(chunk_coordinate, |chunk| {
                chunk
                    .section
                    .get_block_absolute_y(relative.x as usize, relative.y, relative.z as usize)
                    .unwrap_or(Block::AIR.default_state.id)
            })
            .await
    }

    pub async fn get_block_state_async(&self, position: &BlockPos) -> &'static BlockState {
        let id = self.get_block_state_id_async(position).await;
        BlockState::from_id(id)
    }

    pub async fn get_heightmap_height_async(
        &self,
        height_map: ChunkHeightmapType,
        x: i32,
        z: i32,
    ) -> i32 {
        let chunk_pos = Vector2::new(x >> 4, z >> 4);
        self.level
            .get_or_fetch_chunk(chunk_pos, |chunk| {
                chunk
                    .heightmap
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get(height_map, x, z, self.min_y)
            })
            .await
    }

    #[must_use]
    pub fn load(
        level: Arc<Level>,
        level_info: Arc<ArcSwap<LevelData>>,
        dimension: Dimension,
        block_registry: Arc<BlockRegistry>,
        server: Weak<Server>,
    ) -> Self {
        // TODO
        let generation_settings = NoiseSettings::from_dimension(&dimension);

        // 从磁盘加载传送门 POI（如果文件存在，PoiStorage::new 会自动从磁盘加载）
        let portal_poi = portal::PortalPoiStorage::new(level.level_folder.poi_folder.clone());
        let dragon_fight = (dimension.minecraft_name == Dimension::THE_END.minecraft_name)
            .then(|| std::sync::Mutex::new(dragon_fight::DragonFight::new()));

        let custom_data_path = level
            .level_folder
            .root_folder
            .join("pumpkin_custom_data.nbt");
        let custom_data = if custom_data_path.exists()
            && let Ok(bytes) = std::fs::read(&custom_data_path)
            && let Ok(nbt) = papokin_nbt::Nbt::read_unnamed(
                &mut papokin_nbt::deserializer::NbtReadHelperJava::new(&mut std::io::Cursor::new(
                    bytes,
                )),
            ) {
            nbt.root_tag
        } else {
            NbtCompound::new()
        };

        Self {
            uuid: Uuid::new_v4(),
            level,
            level_info,
            players: ArcSwap::new(Arc::new(Vec::new())),
            entities: ArcSwap::new(Arc::new(Vec::new())),
            entities_by_chunk: entity_index::ChunkedEntityIndex::new(),
            scoreboard: std::sync::Mutex::new(Scoreboard::default()),
            worldborder: std::sync::Mutex::new(Worldborder::new(
                0.0,
                0.0,
                5.999_996_8E7,
                0,
                5,
                300,
            )),
            level_time: std::sync::Mutex::new(LevelTime::new()),
            dimension,
            weather: std::sync::Mutex::new(Weather::new()),
            block_registry,
            sea_level: generation_settings.sea_level,
            min_y: i32::from(generation_settings.shape.min_y),
            synced_block_event_queue: std::sync::Mutex::new(Vec::new()),
            unsent_block_changes: std::sync::Mutex::new(HashMap::new()),
            portal_poi: std::sync::Mutex::new(portal_poi),
            villager_poi: std::sync::Mutex::new(villager_poi::VillagerPoiStorage::default()),
            raids: std::sync::Mutex::new(raid::Raids::default()),
            dragon_fight,
            spawn_state: ArcSwap::new(Arc::new(SpawnState::empty())),
            active_chunks: RwLock::new(FxHashSet::default()),
            active_chunk_tracker: std::sync::Mutex::new(ActiveChunkTracker::default()),
            forced_chunks: std::sync::Mutex::new(FxHashSet::default()),
            server,
            block_entities: DashMap::new(),
            pending_block_entity_migrations: crossbeam::queue::SegQueue::new(),
            custom_data: std::sync::Mutex::new(custom_data),
            custom_block_entity_data: DashMap::new(),
            entity_tracker: entity_tracker::EntityTracker::new(),
        }
    }

    pub fn update_active_chunks(self: &Arc<Self>) {
        let sim_dist = self.server.upgrade().map_or(10, |s| {
            s.advanced_config.networking.java.simulation_distance.get()
        }) as i32;
        let players = self.players.load();
        let forced_chunks = self
            .forced_chunks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let mut tracker = self
            .active_chunk_tracker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut active_chunks = self
            .active_chunks
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut newly_active = Vec::new();
        let mut current_players = FxHashSet::default();

        let spectators_generate_chunks =
            self.level_info.load().game_rules.spectators_generate_chunks;

        for player in players.iter() {
            if player.is_spectator() && !spectators_generate_chunks {
                continue;
            }
            let id = player.gameprofile.id;
            current_players.insert(id);
            tracker.update_player(
                id,
                ActivePlayerArea {
                    center: player.get_entity().chunk_pos.load(),
                    simulation_distance: sim_dist,
                },
                &mut active_chunks,
                &mut newly_active,
            );
        }
        let removed_players: Vec<_> = tracker
            .players
            .keys()
            .filter(|id| !current_players.contains(id))
            .copied()
            .collect();
        for id in removed_players {
            tracker.remove_player(id, &mut active_chunks);
        }
        tracker.sync_forced_chunks(&forced_chunks, &mut active_chunks, &mut newly_active);

        for pos in newly_active {
            if self.level.is_chunk_loaded(&pos) && tracker.loaded_active_chunks.insert(pos) {
                self.migrate_pending_block_entities(pos);
            }
        }
        for change in self.level.loaded_chunk_changes() {
            match change {
                papokin_world::level::LoadedChunkChange::Loaded(pos) => {
                    // 宿主区块系统只分发 `Arc<ChunkData>` 快照，而
                    // 事件负载需要 `Arc<RwLock<ChunkData>>`。WIT 层仅暴露
                    // 区块坐标传给插件，因此带有正确坐标的区块存根
                    // 在这里填充（与 WASM 侧派发时构建的结构相同）。
                    let mut event = crate::plugin::api::events::world::chunk_load::ChunkLoad {
                        world: self.clone(),
                        chunk: Arc::new(tokio::sync::RwLock::new(
                            papokin_world::chunk::ChunkData::empty(pos.x, pos.y),
                        )),
                        cancelled: false,
                    };
                    if let Some(server) = self.server.upgrade() {
                        server.plugin_manager.fire_blocking(&server, &mut event);
                    }
                    if active_chunks.contains(&pos)
                        && self.level.is_chunk_loaded(&pos)
                        && tracker.loaded_active_chunks.insert(pos)
                    {
                        self.migrate_pending_block_entities(pos);
                    }
                }
                papokin_world::level::LoadedChunkChange::Unloaded(pos) => {
                    if !self.level.is_chunk_loaded(&pos) {
                        tracker.loaded_active_chunks.remove(&pos);
                    }
                }
            }
        }
        let mut pending_migrations = FxHashSet::default();
        while let Some(pos) = self.pending_block_entity_migrations.pop() {
            pending_migrations.insert(pos);
        }
        for pos in pending_migrations {
            if active_chunks.contains(&pos) && self.level.is_chunk_loaded(&pos) {
                self.migrate_pending_block_entities(pos);
            }
        }
        let spawnable_chunks = tracker.loaded_active_chunks.len() as i32;
        drop(active_chunks);
        drop(tracker);

        self.spawn_state.store(Arc::new(SpawnState::new(
            spawnable_chunks,
            &self.entities,
            self,
        )));
    }

    /// 将区块标记为强制加载（或取消强制加载），并保留相应记录
    /// 区块系统的加载凭证保持同步，因此被强制的区块绝不会
    /// 在没有玩家注视时被卸载。返回该强制状态是否
    /// 实际发生更改；以相同状态重复调用是空操作。
    ///
    /// 活动区块（参与刻计算的）视图见下一
    /// [`World::update_active_chunks`] 得以运行。
    pub fn set_chunk_forced(&self, chunk_pos: Vector2<i32>, forced: bool) -> bool {
        let changed = {
            let mut forced_chunks = self
                .forced_chunks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if forced {
                forced_chunks.insert(chunk_pos)
            } else {
                forced_chunks.remove(&chunk_pos)
            }
        };
        if changed {
            let mut chunk_loading = self
                .level
                .chunk_loading
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if forced {
                chunk_loading.add_force_ticket(chunk_pos);
            } else {
                chunk_loading.remove_force_ticket(chunk_pos);
            }
            chunk_loading.send_change();
        }
        changed
    }

    /// 返回区块是否被标记为强制加载（无需
    /// 玩家观察者）。
    #[must_use]
    pub fn is_chunk_forced(&self, chunk_pos: &Vector2<i32>) -> bool {
        self.forced_chunks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(chunk_pos)
    }

    pub fn get_lighting_config(&self) -> LightingEngineConfig {
        self.server
            .upgrade()
            .map(|s| s.advanced_config.world.lighting)
            .unwrap_or_default()
    }

    /// 获取世界文件夹名称（例如 `world`、`world_nether`、`world_the_end`）。
    /// 若无法确定名称，则回退到“world”。
    pub fn get_world_name(&self) -> &str {
        self.level
            .level_folder
            .root_folder
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("world")
    }

    ///返回已配置的共享世界出生点的方块位置与朝向。
    #[must_use]
    pub fn get_spawn_location(&self) -> (BlockPos, f32, f32) {
        let level_info = self.level_info.load();
        (
            BlockPos::new(level_info.spawn_x, level_info.spawn_y, level_info.spawn_z),
            level_info.spawn_yaw,
            level_info.spawn_pitch,
        )
    }

    #[must_use]
    pub fn is_in_spawn_protection(&self, player: &Player, position: &BlockPos) -> bool {
        if player.permission_lvl.load() == papokin_util::permission::PermissionLvl::Four {
            return false;
        }

        let Some(server) = self.server.upgrade() else {
            return false;
        };

        let radius = server.basic_config.spawn_protection;
        if radius == 0 {
            return false;
        }

        let radius = i32::try_from(radius).unwrap_or(i32::MAX);
        let spawn = self.get_spawn_location().0;
        let dx = (spawn.0.x - position.0.x).abs();
        let dz = (spawn.0.z - position.0.z).abs();

        dx <= radius && dz <= radius
    }

    pub async fn shutdown(&self) {
        let entities = self.entities.load_full();
        self.save_entities_by_chunk(&entities, self.level.live_entity_chunk_positions())
            .await;

        let chunks: Vec<Vector2<i32>> = self
            .block_entities
            .iter()
            .map(|chunk_block_entities| *chunk_block_entities.key())
            .collect();
        for chunk_pos in chunks {
            self.save_block_entities(chunk_pos);
        }

        // 将传送门 POI 保存到磁盘
        let save_result = self
            .portal_poi
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .save_all();
        if let Err(e) = save_result {
            error!("保存传送门 POI 失败：{e}");
        }

        self.level.shutdown().await;
    }

    /// 将 `entities` 写入其所在区块的保存数据中。活跃区块会
    /// 从头重建，因此 `snapshot_chunks` 列出必须重写的存活区块
    /// 即使里面已空无一物；从未上线的区块会保留其记录。
    async fn save_entities_by_chunk(
        &self,
        entities: &[Arc<dyn EntityBase>],
        snapshot_chunks: impl IntoIterator<Item = Vector2<i32>>,
    ) {
        let mut groups: FxHashMap<Vector2<i32>, Vec<NbtCompound>> = FxHashMap::default();
        for entity in entities {
            let base_entity = entity.get_entity();
            if base_entity.is_removed() {
                continue;
            }
            let mut nbt = NbtCompound::new();
            entity.write_nbt(&mut nbt);
            groups
                .entry(base_entity.chunk_pos.load())
                .or_default()
                .push(nbt);
        }
        for pos in snapshot_chunks {
            groups.entry(pos).or_default();
        }

        for (pos, records) in groups {
            let chunk = if records.is_empty() {
                let Some(chunk) = self.level.get_entity_chunk_sync(&pos) else {
                    continue;
                };
                chunk
            } else {
                self.level.get_entity_chunk(pos).await
            };
            let live = chunk.live.load(Relaxed);
            if !live && records.is_empty() {
                continue;
            }
            let mut data = chunk
                .data
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            merge_entity_records(&mut data, live, records);
            drop(data);
            chunk.mark_dirty(true);
        }
    }

    /// 将区块中现存的方块实体序列化回该区块的方块
    /// 实体数据。区块加载期间，内存中的映射是事实来源——
    /// `get_block_entity` 在唤醒实体时会从区块中取出保存的 NBT
    /// 实体——因此必须在区块被丢弃之前运行，否则一切
    /// 实体自加载以来所做的一切都会丢失。
    fn save_block_entities(&self, chunk_pos: Vector2<i32>) {
        let Some(block_entities) = self
            .block_entities
            .get(&chunk_pos)
            .map(|chunk_block_entities| chunk_block_entities.values().cloned().collect::<Vec<_>>())
        else {
            return;
        };

        for block_entity in block_entities {
            let mut nbt = NbtCompound::new();
            block_entity.write_internal(&mut nbt);
            if let Some(custom_data) = self
                .custom_block_entity_data
                .get(&block_entity.get_position())
                && !custom_data.is_empty()
            {
                nbt.put_compound("PumpkinCustomData", custom_data.clone());
            }
            self.add_block_entity_nbt(block_entity.get_position(), &nbt);
        }
    }

    /// 向正在跟踪指定实体的所有玩家广播实体状态更新/事件，
    /// 若该实体是玩家，还会发送给实体本身。
    /// 对应原版的 `ServerLevel.broadcastEntityEvent(entity, event)`。
    pub fn broadcast_entity_event(&self, entity: &Entity, java_status: EntityStatus) {
        let je_packet = CEntityStatus::new(entity.entity_id, java_status as i8);
        self.send_to_tracking_players_and_self(entity, &je_packet);
    }

    /// 向正在跟踪指定实体的所有玩家广播伤害事件，
    /// 若该实体是玩家，还会发送给实体本身。
    /// 对应原版的 `ServerLevel.broadcastDamageEvent(entity, source)`。
    pub fn broadcast_damage_event(
        &self,
        entity: &Entity,
        damage_type_id: i32,
        source_entity_id: Option<i32>,
        cause_entity_id: Option<i32>,
        position: Option<Vector3<f64>>,
    ) {
        let je_packet = CDamageEvent::new(
            entity.entity_id.into(),
            damage_type_id.into(),
            source_entity_id.map(Into::into),
            cause_entity_id.map(Into::into),
            position,
        );
        self.send_to_tracking_players_and_self(entity, &je_packet);
    }

    /// 向所有正在跟踪指定实体的玩家发送实体状态更新。
    pub fn send_entity_status(&self, entity: &Entity, java_status: EntityStatus) {
        self.broadcast_entity_event(entity, java_status);
    }

    pub fn send_remove_mob_effect(&self, entity: &Entity, effect_type: &'static StatusEffect) {
        let je_packet =
            CRemoveMobEffect::new(entity.entity_id.into(), VarInt(i32::from(effect_type.id)));

        self.send_to_tracking_players_and_self(entity, &je_packet);
    }

    pub fn send_add_mob_effect(&self, entity: &Entity, effect: &papokin_data::potion::Effect) {
        let mut flags: i8 = 0;
        if effect.ambient {
            flags |= 0x01;
        }
        if effect.show_particles {
            flags |= 0x02;
        }
        if effect.show_icon {
            flags |= 0x04;
        }

        let je_packet = CUpdateMobEffect::new(
            VarInt(entity.entity_id),
            VarInt(i32::from(effect.effect_type.id)),
            VarInt(i32::from(effect.amplifier)),
            VarInt(effect.duration),
            flags,
        );

        self.send_to_tracking_players_and_self(entity, &je_packet);
    }

    pub fn send_to_tracking_players<P: ClientPacket + Sync>(&self, entity: &Entity, packet: &P) {
        if let Some(tracked) = self.entity_tracker.get_tracked_entity(entity.entity_id) {
            tracked.send_to_tracking_players(packet, self);
        }
    }

    pub fn send_to_tracking_players_and_self<P: ClientPacket + Sync>(
        &self,
        entity: &Entity,
        packet: &P,
    ) {
        if let Some(tracked) = self.entity_tracker.get_tracked_entity(entity.entity_id) {
            tracked.send_to_tracking_players_and_self(packet, self);
        }
    }

    pub fn send_to_tracking_players_filtered<P: ClientPacket + Sync, F: Fn(&Player) -> bool>(
        &self,
        entity: &Entity,
        packet: &P,
        filter: F,
    ) {
        if let Some(tracked) = self.entity_tracker.get_tracked_entity(entity.entity_id) {
            tracked.send_to_tracking_players_filtered(packet, self, filter);
        }
    }

    #[must_use]
    pub fn is_tracked_by_any_player(&self, entity: &Entity) -> bool {
        self.entity_tracker
            .is_tracked_by_any_player(entity.entity_id)
    }

    pub fn set_difficulty(&self, difficulty: Difficulty) {
        let current_info = self.level_info.load();
        let mut new_info = (**current_info).clone();
        new_info.difficulty = difficulty;
        self.level_info.store(Arc::new(new_info));
    }

    pub fn get_game_rule(&self, rule: &GameRule) -> GameRuleValue<i64, bool> {
        let level_info = self.level_info.load();
        match level_info.game_rules.get(rule) {
            GameRuleValue::Int(v) => GameRuleValue::Int(*v),
            GameRuleValue::Bool(v) => GameRuleValue::Bool(*v),
        }
    }

    pub fn set_game_rule(&self, rule: &GameRule, value: GameRuleValue<i64, bool>) {
        let current_info = self.level_info.load();
        let mut new_info = (**current_info).clone();
        match (new_info.game_rules.get_mut(rule), value) {
            (GameRuleValue::Int(target), GameRuleValue::Int(val)) => {
                *target = val;
            }
            (GameRuleValue::Bool(target), GameRuleValue::Bool(val)) => {
                *target = val;
            }
            _ => {}
        }
        self.level_info.store(Arc::new(new_info));
    }

    pub fn add_synced_block_event(&self, pos: BlockPos, r#type: u8, data: u8) {
        let mut queue = self
            .synced_block_event_queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        queue.push(BlockEvent { pos, r#type, data });
    }

    pub fn flush_synced_block_events(self: &Arc<Self>) {
        // 这非常重要
        // 它既能防止死锁，也免去了添加新同步方块时等待锁的需要
        let events = {
            let mut queue = self
                .synced_block_event_queue
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            std::mem::take(&mut *queue)
        };

        for event in events {
            let block = self.get_block(&event.pos);
            if !self.block_registry.on_synced_block_event(
                block,
                self,
                &event.pos,
                event.r#type,
                event.data,
            ) {
                continue;
            }
            let chunk_pos = event.pos.chunk_position();
            self.broadcast_to_chunk(
                chunk_pos,
                &CBlockEvent::new(
                    event.pos,
                    event.r#type,
                    event.data,
                    VarInt(block.id.as_u16() as i32),
                ),
            );
        }
    }

    pub(crate) fn collect_java_recipients_by_version<'a>(
        players: impl Iterator<Item = &'a Arc<Player>>,
    ) -> BTreeMap<JavaMinecraftVersion, Vec<&'a JavaClient>> {
        let mut recipients_by_version: BTreeMap<JavaMinecraftVersion, Vec<&'a JavaClient>> =
            BTreeMap::new();
        for player in players {
            let java_client = &player.client;
            recipients_by_version
                .entry(java_client.version.load())
                .or_default()
                .push(java_client);
        }
        recipients_by_version
    }

    pub fn broadcast_java_clients<'a, P: ClientPacket>(
        packet: &P,
        recipients: impl Iterator<Item = &'a JavaClient>,
    ) {
        let mut recipients_by_version: BTreeMap<JavaMinecraftVersion, Vec<&JavaClient>> =
            BTreeMap::new();
        for client in recipients {
            recipients_by_version
                .entry(client.version.load())
                .or_default()
                .push(client);
        }
        Self::broadcast_java_grouped(packet, recipients_by_version);
    }

    fn broadcast_java_grouped<P: ClientPacket>(
        packet: &P,
        recipients_by_version: BTreeMap<JavaMinecraftVersion, Vec<&JavaClient>>,
    ) {
        for (version, recipients) in recipients_by_version {
            let packet_data = match JavaClient::serialize_packet_for_version(packet, version) {
                Ok(packet_data) => packet_data,
                Err(papokin_protocol::ser::WritingError::UnsupportedVersion(_)) => {
                    continue;
                }
                Err(err) => {
                    error!(
                        "序列化数据包 {}（版本 {:?}）失败：{}",
                        std::any::type_name::<P>(),
                        version,
                        err
                    );
                    continue;
                }
            };

            for recipient in recipients {
                recipient.try_enqueue_packet(packet_data.clone());
            }
        }
    }

    /// 向世界内所有已连接的玩家广播数据包。
    ///
    /// 向当前已登录到该世界的每个玩家发送指定数据包。
    ///
    /// **注意：** 此函数会获取 `current_players` 映射上的锁，以确保线程安全。
    pub fn broadcast_packet_all<P: ClientPacket>(&self, packet: &P) {
        let players = self.players.load();
        let recipients_by_version = Self::collect_java_recipients_by_version(players.iter());
        Self::broadcast_java_grouped(packet, recipients_by_version);
    }

    pub fn broadcast_system_message(&self, message: &TextComponent, overlay: bool) {
        let je_packet = CSystemChatMessage::new(message, overlay);
        self.broadcast_packet_all(&je_packet);
    }

    pub fn broadcast_message(
        &self,
        message: &TextComponent,
        sender_name: &TextComponent,
        chat_type: u8,
        target_name: Option<&TextComponent>,
    ) {
        let je_packet =
            CDisguisedChatMessage::new(message, (chat_type + 1).into(), sender_name, target_name);

        self.broadcast_packet_all(&je_packet);
    }

    pub fn broadcast_chat_message(
        &self,
        message: &crate::net::chat::PlayerChatMessage,
        is_filtered: impl Fn(&Player) -> bool,
        sender_player: Option<&Arc<Player>>,
        chat_type: VarInt,
        sender_name: &TextComponent,
        target_name: Option<&TextComponent>,
    ) {
        let tracked = crate::net::chat::OutgoingChatMessage::create(message.clone());
        let mut was_fully_filtered = false;

        let players = self.players.load();
        for player in players.iter() {
            let filtered = is_filtered(player);
            tracked.send_to_player(player, filtered, chat_type, sender_name, target_name);
            was_fully_filtered |= filtered && message.is_fully_filtered();
        }

        if was_fully_filtered && let Some(sender) = sender_player {
            let filter_notice =
                TextComponent::translate(papokin_data::translation::java::CHAT_FILTERED_FULL, [])
                    .color_named(papokin_util::text::color::NamedColor::Red)
                    .italic();
            sender.send_system_message(&filter_notice);
        }
    }

    pub fn broadcast_secure_player_chat(
        &self,
        sender: &Arc<Player>,
        chat_message: &SChatMessage<'_>,
        decorated_message: &TextComponent,
    ) {
        let messages_sent: i32 = sender
            .chat_session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .messages_sent;
        let sender_last_seen = {
            let cache = sender
                .signature_cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            cache.last_seen.as_ref().to_vec()
        };

        let link = crate::net::chat::SignedMessageLink::new(
            messages_sent,
            sender.gameprofile.id,
            Uuid::nil(),
        );
        let signed_body = crate::net::chat::SignedMessageBody::new(
            chat_message.message.to_string(),
            chat_message.timestamp,
            chat_message.salt,
            sender_last_seen,
        );
        let player_chat_msg = crate::net::chat::PlayerChatMessage::new(
            link,
            chat_message.signature.map(std::convert::Into::into),
            signed_body,
            Some(decorated_message.clone()),
            crate::net::chat::FilterMask::PassThrough,
        );

        self.broadcast_chat_message(
            &player_chat_msg,
            Player::is_text_filtering_enabled,
            Some(sender),
            (RAW + 1).into(),
            &TextComponent::empty(),
            None,
        );

        sender
            .chat_session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .messages_sent += 1;
    }

    /// 广播玩家的皮肤层，并为每个 Java 客户端自身的（版本）编码元数据
    /// 协议版本，因为被追踪数据的索引在不同版本间存在差异。
    fn broadcast_skin_parts(&self, except: &[uuid::Uuid], entity_id: i32, skin_parts: u8) {
        let players = self.players.load();
        let recipients_by_version = Self::collect_java_recipients_by_version(
            players
                .iter()
                .filter(|p| !except.contains(&p.gameprofile.id)),
        );

        for (version, recipients) in recipients_by_version {
            if version < JavaMinecraftVersion::V_1_21 {
                continue;
            }
            let mut buf = Vec::new();
            for meta in [
                Metadata::new(
                    papokin_data::tracked_data::player::PLAYER_MODE_CUSTOMISATION,
                    skin_parts,
                ),
                Metadata::new(
                    papokin_data::tracked_data::player::PLAYER_MODE_CUSTOMIZATION_ID,
                    skin_parts,
                ),
            ] {
                let _ = meta.write(&mut buf, &version);
            }
            buf.put_u8(255);
            let packet = CSetEntityMetadata::new(entity_id.into(), buf.into());
            if let Ok(packet_data) = JavaClient::serialize_packet_for_version(&packet, version) {
                for recipient in recipients {
                    recipient.try_enqueue_packet(packet_data.clone());
                }
            }
        }
    }

    /// 向世界内所有已连接的玩家广播数据包，但排除指定玩家。
    ///
    /// 向当前已登录到该世界的每个玩家发送指定数据包，但 `except` 参数中列出的玩家除外。
    ///
    /// **注意：** 此函数会获取 `current_players` 映射上的锁，以确保线程安全。
    pub fn broadcast_packet_except<P: ClientPacket>(&self, except: &[uuid::Uuid], packet: &P) {
        let players = self.players.load();
        let recipients_by_version = Self::collect_java_recipients_by_version(
            players
                .iter()
                .filter(|candidate| !except.contains(&candidate.gameprofile.id)),
        );
        Self::broadcast_java_grouped(packet, recipients_by_version);
    }

    pub fn spawn_particle(
        &self,
        position: Vector3<f64>,
        offset: Vector3<f32>,
        max_speed: f32,
        particle_count: i32,
        particle: Particle,
    ) {
        for player in self.players.load().iter() {
            player.spawn_particle(position, offset, max_speed, particle_count, particle);
        }
    }

    pub fn play_sound(&self, sound: Sound, category: SoundCategory, position: &Vector3<f64>) {
        self.play_sound_raw(sound as u16, category, position, 1.0, 1.0);
    }

    pub fn play_sound_event(
        &self,
        sound: &papokin_data::data_component_impl::IdOr<
            papokin_data::data_component_impl::SoundEvent,
        >,
        category: SoundCategory,
        position: &Vector3<f64>,
    ) {
        let seed = rng().random::<i64>();
        let packet = CSoundEffect::new(
            data_to_proto_sound(sound),
            category,
            position,
            1.0,
            1.0,
            seed,
        );
        self.broadcast_packet_all(&packet);
    }

    pub fn play_sound_event_expect(
        &self,
        player: &Player,
        sound: &papokin_data::data_component_impl::IdOr<
            papokin_data::data_component_impl::SoundEvent,
        >,
        category: SoundCategory,
        position: &Vector3<f64>,
    ) {
        let seed = rng().random::<i64>();
        let packet = CSoundEffect::new(
            data_to_proto_sound(sound),
            category,
            position,
            1.0,
            1.0,
            seed,
        );
        self.broadcast_packet_except(&[player.gameprofile.id], &packet);
    }

    pub fn play_sound_fine(
        &self,
        sound: Sound,
        category: SoundCategory,
        position: &Vector3<f64>,
        volume: f32,
        pitch: f32,
    ) {
        self.play_sound_raw(sound as u16, category, position, volume, pitch);
    }

    /// 按标识符为范围内的所有玩家播放自定义音效事件。
    pub fn play_custom_sound(
        &self,
        sound_name: &str,
        category: SoundCategory,
        position: &Vector3<f64>,
        volume: f32,
        pitch: f32,
    ) {
        let seed = rand::random::<i64>();
        let packet = CSoundEffect::new(
            papokin_protocol::IdOr::Value(papokin_protocol::SoundEvent {
                sound_name: sound_name.into(),
                range: None,
            }),
            category,
            position,
            volume,
            pitch,
            seed,
        );
        self.broadcast_packet_all(&packet);
    }

    /// 在世界中生成一簇粒子，对范围内的所有玩家可见。
    pub fn spawn_particles(
        &self,
        particle: papokin_data::particle::Particle,
        pos: Vector3<f64>,
        count: u32,
        offset: Vector3<f32>,
        max_speed: f32,
    ) {
        let packet = CParticle::new(
            false,
            false,
            pos,
            offset,
            max_speed,
            count as i32,
            (particle.to_id() as i32).into(),
            &[],
        );
        self.broadcast_packet_all(&packet);
    }

    pub fn play_sound_expect(
        &self,
        player: &Player,
        sound: Sound,
        category: SoundCategory,
        position: &Vector3<f64>,
    ) {
        self.play_sound_raw_expect(player, sound as u16, category, position, 1.0, 1.0);
    }

    pub fn play_sound_raw(
        &self,
        sound_id: u16,
        category: SoundCategory,
        position: &Vector3<f64>,
        volume: f32,
        pitch: f32,
    ) {
        let seed = rand::rng().random::<i64>();
        let packet = CSoundEffect::new(IdOr::Id(sound_id), category, position, volume, pitch, seed);

        // 根据音量计算该声音可被听到的区块数量。
        let audible_chunks = f64::from(volume.max(1.0)).ceil() as i32;
        let chunk_pos = BlockPos::floored_v(*position).chunk_position();

        let players = self.players.load();
        let recipients = players.iter().filter(|p| {
            let center = p.get_entity().chunk_pos.load();
            // 如果声音到达其所在区块，就发送它！
            is_within_chebyshev_distance(chunk_pos, center, audible_chunks)
        });

        let recipients_by_version = Self::collect_java_recipients_by_version(recipients);
        Self::broadcast_java_grouped(&packet, recipients_by_version);
    }

    pub fn play_sound_raw_expect(
        &self,
        player: &Player,
        sound_id: u16,
        category: SoundCategory,
        position: &Vector3<f64>,
        volume: f32,
        pitch: f32,
    ) {
        let seed = rand::rng().random::<i64>();
        let packet = CSoundEffect::new(IdOr::Id(sound_id), category, position, volume, pitch, seed);

        let audible_chunks = f64::from(volume.max(1.0)).ceil() as i32;
        let chunk_pos = BlockPos::floored_v(*position).chunk_position();

        let players = self.players.load();
        let recipients = players.iter().filter(|p| {
            // 跳过预期的玩家
            if p.gameprofile.id == player.gameprofile.id {
                return false;
            }

            let center = p.get_entity().chunk_pos.load();
            is_within_chebyshev_distance(chunk_pos, center, audible_chunks)
        });

        let recipients_by_version = Self::collect_java_recipients_by_version(recipients);
        Self::broadcast_java_grouped(&packet, recipients_by_version);
    }

    pub fn play_block_sound(&self, sound: Sound, category: SoundCategory, position: BlockPos) {
        let new_vec = Vector3::new(
            f64::from(position.0.x) + 0.5,
            f64::from(position.0.y) + 0.5,
            f64::from(position.0.z) + 0.5,
        );
        self.play_sound(sound, category, &new_vec);
    }

    pub fn play_block_sound_expect(
        &self,
        player: &Player,
        sound: Sound,
        category: SoundCategory,
        position: BlockPos,
    ) {
        let new_vec = Vector3::new(
            f64::from(position.0.x) + 0.5,
            f64::from(position.0.y) + 0.5,
            f64::from(position.0.z) + 0.5,
        );
        self.play_sound_expect(player, sound, category, &new_vec);
    }

    #[expect(clippy::too_many_lines)]
    pub fn tick(self: &Arc<Self>, server: &Arc<Server>) {
        const ENTITY_TICK_BATCH_SIZE: usize = 16;

        let start = std::time::Instant::now();

        self.flush_block_updates();
        self.flush_synced_block_events();
        self.update_active_chunks();
        self.tick_environment();
        let mut raids = {
            let mut guard = self
                .raids
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            std::mem::take(&mut *guard)
        };
        raids.tick(self);
        {
            let mut guard = self
                .raids
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for (id, raid) in guard.raid_map.drain() {
                raids.raid_map.insert(id, raid);
            }
            raids.next_id = raids.next_id.max(guard.next_id);
            *guard = raids;
        };

        let t_chunks = std::time::Instant::now();
        self.tick_chunks(server);
        let chunk_elapsed = t_chunks.elapsed();

        let handle = server.runtime.clone();

        let players = self.players.load();
        let player_count = players.len();
        let players_cache: Vec<_> = players
            .par_iter()
            .map(|player| {
                let entity = player.get_entity();
                let pos = entity.pos.load();
                let bb = entity.bounding_box.load().expand(1.0, 0.5, 1.0);
                let chunk_pos = Vector2::new(
                    get_section_cord(pos.x.floor() as i32),
                    get_section_cord(pos.z.floor() as i32),
                );
                (player, pos, bb, chunk_pos)
            })
            .collect();

        let t_players = std::time::Instant::now();
        let player_handle = handle.clone();
        players.par_iter().for_each(|player| {
            let _guard = player_handle.enter();
            player.tick(server);
        });
        let player_elapsed = t_players.elapsed();

        let entities_to_tick = self.entities.load();
        let entity_count = entities_to_tick.len();
        let active_chunks = self
            .active_chunks
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let level_for_entities = self.level.clone();
        let entity_handle = handle.clone();

        let t_entities = std::time::Instant::now();
        let tickable: Vec<_> = entities_to_tick
            .par_iter()
            .filter_map(|entity| {
                let entity_pos = entity.get_entity().pos.load();
                let entity_chunk = Vector2::new(
                    get_section_cord(entity_pos.x.floor() as i32),
                    get_section_cord(entity_pos.z.floor() as i32),
                );
                if !active_chunks.contains(&entity_chunk) {
                    return None;
                }
                if !level_for_entities.is_chunk_loaded(&entity_chunk) {
                    return None;
                }
                Some((entity, entity_chunk))
            })
            .collect();

        let server_ref = server.as_ref();
        tickable
            .par_chunks(ENTITY_TICK_BATCH_SIZE)
            .for_each(|batch| {
                let _guard = entity_handle.enter();

                for (entity, entity_chunk) in batch {
                    entity.get_entity().age.fetch_add(1, Relaxed);
                    entity.tick(entity.as_ref(), server_ref);

                    let entity_inner = entity.get_entity();
                    let entity_pos = entity_inner.pos.load();
                    let entity_bb = entity_inner.bounding_box.load();

                    for (player, player_pos, player_bb, player_chunk) in &players_cache {
                        if (player_chunk.x - entity_chunk.x).abs() <= 1
                            && (player_chunk.y - entity_chunk.y).abs() <= 1
                            && (player_pos.x - entity_pos.x).abs() < 5.0
                            && (player_pos.y - entity_pos.y).abs() < 5.0
                            && (player_pos.z - entity_pos.z).abs() < 5.0
                            && player_bb.intersects(&entity_bb)
                        {
                            entity.on_player_collision(player);
                            break;
                        }
                    }
                }
            });
        let entity_elapsed = t_entities.elapsed();

        self.entity_tracker.update_all(self);

        let mut block_entities: Vec<Arc<dyn BlockEntity>> = Vec::new();
        if self.block_entities.len() < active_chunks.len() {
            for chunk_block_entities in &self.block_entities {
                if active_chunks.contains(chunk_block_entities.key()) {
                    block_entities.extend(chunk_block_entities.values().cloned());
                }
            }
        } else {
            for chunk_pos in active_chunks.iter() {
                if let Some(chunk_block_entities) = self.block_entities.get(chunk_pos) {
                    block_entities.extend(chunk_block_entities.values().cloned());
                }
            }
        }
        let block_entity_count = block_entities.len();

        let t_be = std::time::Instant::now();
        let be_handle = handle;
        block_entities.par_chunks(16).for_each(|batch| {
            let _guard = be_handle.enter();
            for be in batch {
                be.tick(self);
            }
        });
        // 在所有刻之后排空，因此变化（漏斗 -> 箱子）落在同一刻内。
        let guard = be_handle.enter();
        self.flush_comparator_updates(&block_entities);
        drop(guard);
        let block_entity_elapsed = t_be.elapsed();

        self.level
            .chunk_loading
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .send_change();

        if let Some(ref fight_mutex) = self.dragon_fight {
            dragon_fight::DragonFight::tick(fight_mutex, self);
        }

        let total_elapsed = start.elapsed();
        if total_elapsed.as_millis() > 50 {
            debug!(
                "刻耗时过长 [{}ms]：区块：{:?} | 玩家({})：{:?} | 实体({})：{:?} | 方块实体({})：{:?}",
                total_elapsed.as_millis(),
                chunk_elapsed,
                player_count,
                player_elapsed,
                entity_count,
                entity_elapsed,
                block_entity_count,
                block_entity_elapsed,
            );
        }
    }

    pub fn register_block_change(&self, position: BlockPos, block_state_id: BlockStateId) {
        self.unsent_block_changes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(position, block_state_id);
    }

    /// 将方块状态变更加入队列，以便广播给附近的玩家。
    ///
    /// 之后调用 [`flush_block_updates`](Self::flush_block_updates) 来发送数据包。
    pub fn queue_block_updates(&self, changes: &[(BlockPos, BlockStateId)]) {
        let mut guard = self
            .unsent_block_changes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (pos, state_id) in changes {
            guard.insert(*pos, *state_id);
        }
    }

    pub fn flush_block_updates(&self) {
        let mut block_state_updates_by_chunk_section: HashMap<
            Vector3<i32>,
            Vec<(BlockPos, BlockStateId)>,
        > = HashMap::new();
        let changes = {
            let mut guard = self
                .unsent_block_changes
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            std::mem::take(&mut *guard)
        };
        for (position, block_state_id) in changes {
            let chunk_section = chunk_section_from_pos(&position);
            block_state_updates_by_chunk_section
                .entry(chunk_section)
                .or_default()
                .push((position, block_state_id));
        }

        // TODO: 只向已加载相应区块的玩家发送数据包
        // TODO: 发送光照更新，以更新紧邻被破坏方块的导线
        for (chunk_section, updates) in block_state_updates_by_chunk_section {
            if updates.is_empty() {
                continue;
            }
            let chunk_pos = Vector2::new(chunk_section.x, chunk_section.z);
            if updates.len() == 1 {
                let (block_pos, block_state_id) = updates[0];
                self.broadcast_to_chunk(
                    chunk_pos,
                    &CBlockUpdate::new(block_pos, i32::from(block_state_id.as_u16()).into()),
                );
                if let Some(block_entity) = self.get_block_entity(&block_pos)
                    && let Some(nbt) = block_entity.chunk_data_nbt()
                {
                    let bytes = papokin_nbt::Nbt::from(nbt).write_unnamed();
                    self.broadcast_to_chunk(
                        chunk_pos,
                        &CBlockEntityData::new(
                            block_pos,
                            VarInt(block_entity.get_id() as i32),
                            bytes.as_ref().into(),
                        ),
                    );
                }
            } else {
                let players = self.players.load();

                let recipients = players.iter().filter(|p| {
                    p.watched_section
                        .load()
                        .is_within_distance(chunk_pos.x, chunk_pos.y)
                });

                let recipients_by_version = Self::collect_java_recipients_by_version(recipients);
                Self::broadcast_java_grouped(
                    &CMultiBlockUpdate::new(&updates),
                    recipients_by_version,
                );

                for (block_pos, _) in &updates {
                    if let Some(block_entity) = self.get_block_entity(block_pos)
                        && let Some(nbt) = block_entity.chunk_data_nbt()
                    {
                        let bytes = papokin_nbt::Nbt::from(nbt).write_unnamed();
                        self.broadcast_to_chunk(
                            chunk_pos,
                            &CBlockEntityData::new(
                                *block_pos,
                                VarInt(block_entity.get_id() as i32),
                                bytes.as_ref().into(),
                            ),
                        );
                    }
                }
            }
        }
    }

    pub fn tick_environment(self: &Arc<Self>) {
        let (world_age, is_night, time_of_day) = {
            let mut level_time = self
                .level_time
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let advance_time = self.level_info.load().game_rules.advance_time;
            level_time.tick(advance_time);

            // 自动保存逻辑
            if level_time.world_age % 100 == 0 {
                self.level.should_unload.store(true, Relaxed);
                let cleaned_chunks = self.level.clean_memory();
                if !cleaned_chunks.is_empty() {
                    let world_clone = self.clone();
                    if let Some(server) = self.server.upgrade() {
                        server.spawn_task(async move {
                            world_clone.remove_entities_in_chunks(&cleaned_chunks).await;
                            world_clone.level.clean_entity_chunks(&cleaned_chunks);
                        });
                    }
                }
                // 如果配置了自动保存且本刻会触发自动保存，则不要重复通知
                if self.level.autosave_ticks == 0 {
                    self.level.level_channel.notify();
                } else {
                    let autosave = self.level.autosave_ticks as i64;
                    if autosave == 0 || level_time.world_age % autosave != 0 {
                        self.level.level_channel.notify();
                    }
                }
            }
            if self.level.autosave_ticks > 0 && self.level.save_enabled.load(Relaxed) {
                let autosave = self.level.autosave_ticks as i64;
                if autosave > 0 && level_time.world_age % autosave == 0 {
                    self.level.should_save.store(true, Relaxed);
                    self.level.level_channel.notify();
                }
            }
            (
                level_time.world_age,
                level_time.is_night(),
                level_time.time_of_day,
            )
        };

        let (should_reset_weather, weather_cycle_enabled) = {
            let mut weather = self
                .weather
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            weather.tick_weather(self);
            (
                weather.raining || weather.thundering,
                weather.weather_cycle_enabled,
            )
        };

        if self.should_skip_night() && is_night {
            let level_time = {
                let mut guard = self
                    .level_time
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let time = time_of_day + 24000;
                guard.set_time(time - time % 24000);
                guard.clone()
            };
            level_time.send_time(self);

            for player in self.players.load().iter() {
                player.wake_up();
            }

            if weather_cycle_enabled && should_reset_weather {
                let mut weather = self
                    .weather
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                weather.reset_weather_cycle(self);
            }
        } else if world_age % 20 == 0 {
            let level_time = self
                .level_time
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            level_time.send_time(self);
        }
    }

    #[expect(clippy::too_many_lines)]
    pub fn tick_chunks(self: &Arc<Self>, server: &Arc<Server>) {
        const BATCH_SIZE: usize = 32;
        const INHABITED_TIME_BATCH_SIZE: usize = 1024;
        let random_tick_speed = self.level_info.load().game_rules.random_tick_speed;

        let active_chunks = self
            .active_chunks
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let tick_data = self.level.get_tick_data(&active_chunks, random_tick_speed);
        let handle = server.runtime.clone();

        // 1. 通过 Rayon 并行执行方块刻
        let world = self.clone();
        let block_handle = handle.clone();
        tick_data
            .block_ticks
            .par_chunks(BATCH_SIZE)
            .for_each(|batch| {
                let _guard = block_handle.enter();
                let world = world.clone();
                for scheduled_tick in batch {
                    let pos = scheduled_tick.position;
                    let block = world.get_block(&pos);
                    if let Some(pumpkin_block) = world.block_registry.get_pumpkin_block(block.id) {
                        pumpkin_block.on_scheduled_tick(OnScheduledTickArgs {
                            world: &world,
                            block,
                            position: &pos,
                        });
                    }
                }
            });

        // 2. 通过 Rayon 并行执行流体刻
        let world = self.clone();
        let fluid_handle = handle.clone();
        tick_data
            .fluid_ticks
            .par_chunks(BATCH_SIZE)
            .for_each(|batch| {
                let _guard = fluid_handle.enter();
                let world = world.clone();
                for scheduled_tick in batch {
                    let pos = scheduled_tick.position;
                    let fluid = world.get_fluid(&pos);
                    if let Some(pumpkin_fluid) = world.block_registry.get_pumpkin_fluid(fluid.id) {
                        pumpkin_fluid.on_scheduled_tick(&world, fluid, &pos);
                    }
                }
            });

        // 3. 通过 Rayon 并行执行随机刻
        let world = self.clone();
        let random_handle = handle.clone();
        tick_data
            .random_ticks
            .par_chunks(BATCH_SIZE)
            .for_each(|batch| {
                let _guard = random_handle.enter();
                let world = world.clone();
                for scheduled_tick in batch {
                    let pos = scheduled_tick.position;
                    let (block, fluid) =
                        match (scheduled_tick.tick_block, scheduled_tick.tick_fluid) {
                            (true, true) => {
                                let (b, f) = world.get_block_and_fluid(&pos);
                                (Some(b), Some(f))
                            }
                            (true, false) => (Some(world.get_block(&pos)), None),
                            (false, true) => (None, Some(world.get_fluid(&pos))),
                            (false, false) => (None, None),
                        };

                    if let Some(block) = block
                        && let Some(pumpkin_block) =
                            world.block_registry.get_pumpkin_block(block.id)
                    {
                        pumpkin_block.random_tick(RandomTickArgs {
                            world: &world,
                            block,
                            position: &pos,
                        });
                    }

                    if let Some(fluid) = fluid
                        && let Some(pumpkin_fluid) =
                            world.block_registry.get_pumpkin_fluid(fluid.id)
                    {
                        pumpkin_fluid.random_tick(fluid, &world, &pos);
                    }
                }
            });

        // 4. 计算生成列表（顺序设置）
        let spawn_state = self.spawn_state.load();
        let (spawn_mobs, spawn_monsters, peaceful) = {
            let lock = self.level_info.load();
            (
                lock.game_rules.spawn_mobs,
                lock.game_rules.spawn_monsters,
                lock.difficulty == Difficulty::Peaceful,
            )
        };
        let spawn_passives = self.get_time_of_day() % 400 == 0;
        let spawn_enemies = !peaceful && spawn_monsters && spawn_mobs;
        let spawn_passives = spawn_passives && spawn_mobs;

        let spawn_list = Arc::new(natural_spawner::get_filtered_spawning_categories(
            &spawn_state,
            spawn_mobs,
            spawn_enemies,
            spawn_passives,
        ));

        // 5. 通过 Rayon 并行执行区块生成器
        if !spawn_list.is_empty() {
            let mut spawning_chunks = Vec::new();
            for pos in active_chunks.iter() {
                if let Some(chunk) = self.level.read_chunk_sync(pos, std::clone::Clone::clone) {
                    spawning_chunks.push((*pos, chunk));
                }
            }

            spawning_chunks.shuffle(&mut rng());

            let world = self.clone();
            let spawn_handle = handle;
            spawning_chunks.par_chunks(8).for_each(|batch| {
                let _guard = spawn_handle.enter();
                let world = world.clone();
                let s_list = spawn_list.clone();
                let s_state = spawn_state.clone();
                for (pos, chunk) in batch {
                    world.tick_spawning_chunk(*pos, chunk, &s_list, &s_state);
                }
            });
        }

        // 批量执行这些开销小的查找和原子自增，避免唤醒 Rayon
        // 为每个刻的微小任务生成工作线程，同时对大集合保留并行性。
        let loaded_chunks = self.level.loaded_chunks.clone();
        let active_chunks_vec: Vec<_> = active_chunks.iter().copied().collect();
        active_chunks_vec
            .par_iter()
            .with_min_len(INHABITED_TIME_BATCH_SIZE)
            .for_each(|pos| {
                if let Some(chunk) = loaded_chunks.get(pos) {
                    chunk.inhabited_time.fetch_add(1, Relaxed);
                }
            });
    }

    pub fn check_fluid_collision(&self, bounding_box: BoundingBox) -> bool {
        let min = bounding_box.min_block_pos();

        let max = bounding_box.max_block_pos();

        for x in min.0.x..=max.0.x {
            for y in min.0.y..=max.0.y {
                for z in min.0.z..=max.0.z {
                    let pos = BlockPos::new(x, y, z);

                    let (fluid, state) = self.get_fluid_and_fluid_state(&pos);

                    if fluid.id != Fluid::EMPTY.id {
                        let height = f64::from(state.height);

                        if height >= bounding_box.min.y {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    pub fn contains_any_liquid(&self, bounding_box: BoundingBox) -> bool {
        let min_x = bounding_box.min.x.floor() as i32;
        let max_x = bounding_box.max.x.ceil() as i32;
        let min_y = bounding_box.min.y.floor() as i32;
        let max_y = bounding_box.max.y.ceil() as i32;
        let min_z = bounding_box.min.z.floor() as i32;
        let max_z = bounding_box.max.z.ceil() as i32;

        for x in min_x..max_x {
            for y in min_y..max_y {
                for z in min_z..max_z {
                    let pos = BlockPos::new(x, y, z);
                    if self.get_fluid_and_fluid_state(&pos).0.id != Fluid::EMPTY.id {
                        return true;
                    }
                }
            }
        }

        false
    }

    // FlowingFluid.getFlow()
    pub fn get_fluid_velocity(
        &self,
        pos0: BlockPos,
        fluid0: &Fluid,
        state0: &FluidState,
    ) -> Vector3<f64> {
        let mut velo = Vector3::default();

        for dir in BlockDirection::horizontal() {
            let offset = dir.to_offset();
            let pos = pos0.offset(offset);

            let (neighbor_fluid, neighbor_state) = self.get_fluid_and_fluid_state(&pos);

            if neighbor_fluid.matches_type(fluid0) {
                let mut neighbor_height = neighbor_state.height;
                let mut amplitude = 0.0;

                if neighbor_height == 0.0 {
                    let state_id = self.get_block_state_id(&pos);
                    let block_id = state_id.to_block_id();
                    let block_state = state_id.to_state();

                    let blocks_movement = blocks_movement(block_state, block_id);

                    if !blocks_movement {
                        let down_pos = pos.down();
                        let (down_fluid, down_state) = self.get_fluid_and_fluid_state(&down_pos);

                        if down_fluid.matches_type(fluid0) {
                            neighbor_height = down_state.height;
                            if neighbor_height > 0.0 {
                                amplitude = f64::from(state0.height)
                                    - (f64::from(neighbor_height) - 0.888_888_9);
                            }
                        }
                    }
                } else if neighbor_height > 0.0 {
                    amplitude = f64::from(state0.height) - f64::from(neighbor_height);
                }

                if amplitude != 0.0 {
                    velo.x += f64::from(offset.x) * amplitude;
                    velo.z += f64::from(offset.z) * amplitude;
                }
            }
        }

        if state0.falling {
            for dir in BlockDirection::horizontal() {
                let pos = pos0.offset(dir.to_offset());

                if self.is_solid_face(fluid0.id, pos, dir.to_block_direction())
                    || self.is_solid_face(fluid0.id, pos.up(), dir.to_block_direction())
                {
                    if velo.length_squared() != 0.0 {
                        velo = velo.normalize();
                    }

                    velo.y -= 6.0;
                    break;
                }
            }
        }

        if velo.length_squared() == 0.0 {
            velo
        } else {
            velo.normalize()
        }
    }

    // FlowingFluid.isSolidFace()
    fn is_solid_face(&self, fluid0_id: u16, pos: BlockPos, direction: BlockDirection) -> bool {
        let id = self.get_block_state_id(&pos);

        let fluid = Fluid::from_state_id(id).unwrap_or(&Fluid::EMPTY);

        if Fluid::same_fluid_type(fluid.id, fluid0_id) {
            return false;
        }

        if direction == BlockDirection::Up {
            return true;
        }

        let block = Block::from_state_id(id);
        let state = BlockState::from_id(id);

        // 不计算蓝冰或浮冰

        if block == &Block::ICE || block == &Block::FROSTED_ICE {
            return false;
        }

        state.is_side_solid(direction)
    }

    pub fn check_outline<F>(
        bounding_box: &BoundingBox,
        pos: BlockPos,
        state: &BlockState,
        use_outline_shape: bool,
        mut using_outline_shape: F,
    ) -> bool
    where
        F: FnMut(&BoundingBox),
    {
        if state.outline_shapes.is_empty() {
            // 显然空气和移动的活塞需要这个

            return true;
        }

        let mut inside = false;
        'shapes: for shape in state.get_block_outline_shapes_at(&pos) {
            let outline_shape = shape.at_pos(pos);

            if outline_shape.intersects(bounding_box) {
                inside = true;

                if !use_outline_shape {
                    break 'shapes;
                }

                using_outline_shape(&outline_shape);
            }
        }

        inside
    }

    pub fn check_collision<F>(
        bounding_box: &BoundingBox,
        pos: BlockPos,
        state: &BlockState,
        use_collision_shape: bool,
        mut on_collision: F,
    ) -> bool
    where
        F: FnMut(&BoundingBox),
    {
        if state.is_air() || !state.is_solid() {
            return false;
        }

        let mut shapes = state
            .get_block_collision_shapes_at(&pos)
            .map(|shape| shape.at_pos(pos));

        if use_collision_shape {
            let mut collided = false;
            for collision_shape in shapes {
                if collision_shape.intersects(bounding_box) {
                    collided = true;
                    // 转换为 BB 并触发回调
                    on_collision(&collision_shape);
                }
            }
            collided
        } else {
            shapes.any(|s| s.intersects(bounding_box))
        }
    }

    // 用于调整移动
    pub fn get_block_collisions(
        &self,
        bounding_box: BoundingBox,
        entity: &dyn EntityBase,
    ) -> (Vec<BoundingBox>, Vec<(usize, BlockPos)>) {
        let mut collisions = Vec::new();

        let mut positions = Vec::new();

        let min = BlockPos::floored_v(bounding_box.min.add_raw(0.0, -0.50001, 0.0));
        let max = bounding_box.max_block_pos();
        let pos_iter = BlockPos::iterate(min, max);

        for pos in pos_iter {
            let state = self.get_block_state(&pos);

            if state.is_air() {
                continue;
            }

            let block = Block::from_state_id(state.id);
            let mut collided = false;

            if block == &Block::POWDER_SNOW {
                if let Some(shape) =
                    crate::block::blocks::powder_snow::collision_shape_for_entity(entity, &pos)
                {
                    let shape = shape.at_pos(pos);
                    if shape.intersects(&bounding_box) {
                        collided = true;
                        collisions.push(shape);
                    }
                }
            } else {
                for shape in state.get_block_collision_shapes_at(&pos) {
                    let shape = shape.at_pos(pos);
                    if shape.intersects(&bounding_box) {
                        collided = true;
                        collisions.push(shape);
                    }
                }
            }

            if collided {
                positions.push((collisions.len(), pos));
            }
        }

        (collisions, positions)
    }

    pub fn is_space_empty(&self, bounding_box: BoundingBox) -> bool {
        let min = bounding_box.min_block_pos();
        let max = bounding_box.max_block_pos();

        for pos in BlockPos::iterate(min, max) {
            let state = self.get_block_state(&pos);
            let collided = Self::check_collision(&bounding_box, pos, state, false, |_| ());

            if collided {
                return false;
            }
        }
        true
    }

    /// 原版的 `BlockView.getDismountHeight()`。
    ///返回在给定方块位置下坐骑时使用的 Y 表面高度，
    /// 若不存在有效表面则返回 `f64::NEG_INFINITY`。
    pub fn get_dismount_height(&self, pos: &BlockPos) -> f64 {
        let state = self.get_block_state(pos);
        let max_y = state
            .get_block_collision_shapes_at(pos)
            .map(|s| s.max.y)
            .fold(f64::NEG_INFINITY, f64::max);
        if max_y != f64::NEG_INFINITY {
            return max_y;
        }
        // pos 处无碰撞 — 检查下方方块
        let below = BlockPos(Vector3::new(pos.0.x, pos.0.y - 1, pos.0.z));
        let below_state = self.get_block_state(&below);
        let below_max_y = below_state
            .get_block_collision_shapes_at(&below)
            .map(|s| s.max.y)
            .fold(f64::NEG_INFINITY, f64::max);
        if below_max_y >= 1.0 {
            below_max_y - 1.0
        } else {
            f64::NEG_INFINITY
        }
    }

    pub fn tick_spawning_chunk(
        self: &Arc<Self>,
        chunk_pos: Vector2<i32>,
        chunk: &Arc<ChunkData>,
        spawn_list: &Vec<&'static MobCategory>,
        spawn_state: &Arc<SpawnState>,
    ) {
        // this.level.tickThunder(chunk);
        //TODO 在模拟距离内进行检查
        let (is_raining, is_thundering) = (self.is_raining(), self.is_thundering());

        if is_raining && is_thundering && rng().random_range(0..100_000) == 0 {
            let rand_value = rng().random::<i32>() >> 2;
            let delta = Vector3::new(rand_value & 15, rand_value >> 16 & 15, rand_value >> 8 & 15);
            let random_pos = Vector3::new(
                chunk_pos.x << 4,
                chunk
                    .heightmap
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get(
                        MotionBlocking,
                        chunk_pos.x << 4,
                        chunk_pos.y << 4,
                        self.min_y,
                    ),
                chunk_pos.y << 4,
            )
            .add(&delta);
            // TODO this.getBrightness(LightLayer.SKY, blockPos) >= 15;
            // TODO 高度图

            // TODO findLightningRod(blockPos)
            // TODO encapsulatingFullBlocks
            if true {
                // TODO biome.getPrecipitationAt(pos, this.getSeaLevel()) == Biome.Precipitation.RAIN
                // TODO this.getCurrentDifficultyAt(blockPos);
                if rng().random::<f32>() < 0.0675
                    && self.get_block(&random_pos.to_block_pos().down()) != &Block::LIGHTNING_ROD
                {
                    let entity = Entity::new(
                        self.clone(),
                        random_pos.to_f64(),
                        &EntityType::SKELETON_HORSE,
                    );
                    self.spawn_entity_non_save(Arc::new(entity));
                }
                let entity = Entity::new(
                    self.clone(),
                    random_pos.to_f64().add_raw(0.5, 0., 0.5),
                    &EntityType::LIGHTNING_BOLT,
                );
                self.spawn_entity_non_save(Arc::new(entity));
            }
        }

        if spawn_list.is_empty() {
            return;
        }
        // TODO this.level.canSpawnEntitiesInChunk(chunkPos)
        let entities = spawn_for_chunk(
            self,
            chunk_pos,
            chunk,
            spawn_state,
            spawn_list,
            is_thundering,
        );
        for entity in entities {
            // 自然生成闸门：插件可否决个别生成。
            let mut spawn_event =
                crate::plugin::api::events::entity::creature_spawn::CreatureSpawnEvent {
                    entity_id: entity.get_entity().entity_id,
                    entity_type: entity.get_entity().entity_type.resource_name.to_string(),
                    position: entity.get_entity().pos.load(),
                    world: self.clone(),
                    spawn_reason: "NATURAL".to_string(),
                    cancelled: false,
                };
            if let Some(server) = self.server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut spawn_event);
            }
            if spawn_event.cancelled {
                continue;
            }
            self.spawn_entity_non_save(entity);
        }
    }

    pub fn get_world_age(&self) -> i64 {
        self.level_time
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .world_age
    }

    pub fn get_time_of_day(&self) -> i64 {
        self.level_time
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .time_of_day
    }

    pub fn set_time_of_day(&self, time: i64) {
        let level_time = {
            let mut guard = self
                .level_time
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            guard.set_time(time);
            guard.clone()
        };
        level_time.send_time(self);
    }

    pub fn is_raining(&self) -> bool {
        self.weather
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .raining
    }

    pub fn is_raining_at(&self, pos: &BlockPos) -> bool {
        if !self.is_raining() {
            return false;
        }
        if self.get_heightmap_height(MotionBlocking, pos.0.x, pos.0.z) + 1 > pos.0.y {
            return false;
        }
        self.can_see_sky(pos)
            && self
                .get_biome(pos)
                .weather
                .is_rain_at(pos.0.x, pos.0.y, pos.0.z, self.sea_level)
    }

    pub fn set_raining(&self, raining: bool) {
        if let Some(server) = self.server.upgrade() {
            let world_arc = server.get_world_from_dimension(&self.dimension);
            let mut event =
                crate::plugin::api::events::world::weather_change::WeatherChangeEvent::new(
                    world_arc, raining,
                );
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return;
            }
        }
        let mut weather = self
            .weather
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if weather.raining != raining {
            let thunder = weather.thundering;
            weather.set_weather_parameters(self, 0, 0, raining, thunder);
        }
    }

    pub fn is_thundering(&self) -> bool {
        self.weather
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .thundering
    }

    pub fn set_thundering(&self, thundering: bool) {
        if let Some(server) = self.server.upgrade() {
            let world_arc = server.get_world_from_dimension(&self.dimension);
            let mut event =
                crate::plugin::api::events::world::weather_change::ThunderChangeEvent::new(
                    world_arc, thundering,
                );
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return;
            }
        }
        let mut weather = self
            .weather
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if weather.thundering != thundering {
            let raining = weather.raining;
            weather.set_weather_parameters(self, 0, 0, raining, thundering);
        }
    }

    /// 自顶向下获取第一个非空气方块的 y 坐标
    pub fn get_top_block(&self, position: Vector2<i32>) -> i32 {
        let chunk_pos = Vector2::new(position.x >> 4, position.y >> 4);
        let relative_x = (position.x & 15) as usize;
        let relative_z = (position.y & 15) as usize;

        self.level
            .read_chunk_sync(&chunk_pos, |chunk| {
                let height = chunk
                    .heightmap
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get(
                        ChunkHeightmapType::WorldSurface,
                        position.x,
                        position.y,
                        self.dimension.min_y,
                    );

                if height >= self.dimension.min_y {
                    return height;
                }

                for y in (self.dimension.min_y..self.dimension.min_y + self.dimension.height).rev()
                {
                    if let Some(block_id) = chunk
                        .section
                        .get_block_absolute_y(relative_x, y, relative_z)
                        && !is_air(block_id)
                    {
                        return y;
                    }
                }
                self.dimension.min_y
            })
            .unwrap_or(self.dimension.min_y)
    }

    pub fn get_heightmap_height(&self, height_map: ChunkHeightmapType, x: i32, z: i32) -> i32 {
        let chunk_pos = Vector2::new(x >> 4, z >> 4);
        self.level
            .read_chunk_sync(&chunk_pos, |chunk| {
                chunk
                    .heightmap
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get(height_map, x, z, self.min_y)
            })
            .unwrap_or(self.min_y)
    }

    #[expect(clippy::too_many_lines)]
    pub async fn spawn_java_player(
        &self,
        base_config: &BasicConfiguration,
        player: &Arc<Player>,
        server: &Arc<Server>,
    ) {
        let dimensions: Vec<ResourceLocation> = server
            .dimensions
            .iter()
            .map(|d| ResourceLocation::from(d.minecraft_name))
            .collect();

        // 这段代码遵循原版的数据包顺序
        let entity_id = player.entity_id();
        let gamemode = player.gamemode.load();
        debug!(
            "正在生成玩家 {}，实体 ID {}",
            player.gameprofile.name, entity_id
        );

        let client = &player.client;
        // 为我们的新玩家发送登录数据包
        client
            .send_packet(&CLogin::new(
                entity_id,
                base_config.hardcore,
                &dimensions,
                server
                    .advanced_config
                    .networking
                    .java
                    .max_players
                    .try_into()
                    .unwrap_or(u16::MAX.into()),
                server
                    .advanced_config
                    .networking
                    .java
                    .view_distance
                    .get()
                    .into(), //  TODO: 视距
                server
                    .advanced_config
                    .networking
                    .java
                    .simulation_distance
                    .get()
                    .into(), // TODO: 模拟视距
                false,
                true,
                false,
                PlayerSpawnData::new(
                    self.dimension.clone(),
                    biome::hash_seed(self.level.seed.0), // 种子
                    gamemode as u8,
                    player
                        .previous_gamemode
                        .load()
                        .map_or(-1, |gamemode| gamemode as i8),
                    false,
                    false,
                    None,
                    VarInt(player.get_entity().portal_cooldown.load(Ordering::Relaxed) as i32),
                    self.sea_level.into(),
                ),
                server.advanced_config.networking.java.online_mode,
                // 即使报告功能被禁用，这里也应保持为真。
                // 它防止加入服务器时弹出烦人的提示。
                true,
            ))
            .await;

        self.pair_new_player_with_tracked_entities(player);

        // 将当前的 ticking 状态发送给新玩家，使其保持同步。
        server.tick_rate_manager.update_joining_player(player).await;

        // 权限，即玩家可以使用的命令。
        player.send_permission_lvl_update();

        // 世界难度
        player.send_difficulty_update();
        {
            let command_dispatcher = server.command_dispatcher.load();

            client_suggestions::send_c_commands_packet(player, server, &command_dispatcher);
        };
        if client.version.load() < JavaMinecraftVersion::V_1_20_2
            && client.version.load() >= JavaMinecraftVersion::V_1_13
        {
            let version = client.version.load();
            let tags = server.tag_manager.network_tag_keys(version);
            // 将插件标签覆盖层合并到静态表中（None 当
            // 没有插件修改过这些标签）。
            let merged_tags = server.tag_manager.snapshot(version);
            let packet = papokin_protocol::java::client::play::CUpdateTagsPlay::with_merged(
                &tags,
                merged_tags.as_ref(),
            );
            if let Ok(packet_data) = JavaClient::serialize_packet_for_version(&packet, version) {
                client.send_packet_now(packet_data).await;
            }
        }

        let (position, yaw, pitch) = if player.has_played_before.load(Ordering::Relaxed) {
            let position = player.position();
            let yaw = player.get_entity().yaw.load(); //info.spawn_angle;
            let pitch = player.get_entity().pitch.load();

            (position, yaw, pitch)
        } else {
            let info = &self.level_info.load();
            let spawn_position = Vector2::new(info.spawn_x, info.spawn_z);
            let chunk_pos = Vector2::new(info.spawn_x >> 4, info.spawn_z >> 4);
            self.level.get_or_fetch_chunk(chunk_pos, |_| ()).await;
            let top = self.get_top_block(spawn_position);
            let pos_y = if top > self.dimension.min_y {
                top + 1
            } else {
                info.spawn_y
            };

            let position = Vector3::new(
                f64::from(info.spawn_x) + 0.5,
                f64::from(pos_y),
                f64::from(info.spawn_z) + 0.5,
            );
            (position, info.spawn_yaw, info.spawn_pitch)
        };

        // 在将客户端传送到真实出生位置之前，先加载其周围的区块。
        player.living_entity.entity.set_pos(position);
        player.living_entity.entity.set_rotation(yaw, pitch);
        player.living_entity.entity.last_pos.store(position);
        chunker::update_position(player);

        let center_chunk = player.living_entity.entity.chunk_pos.load();
        let chunk = self
            .level
            .get_or_fetch_chunk(center_chunk, std::clone::Clone::clone)
            .await;
        if let Some(server) = self.server.upgrade() {
            let mut event =
                crate::plugin::world::chunk_send::ChunkSend::new(player.world(), chunk.clone());
            server.plugin_manager.fire(&server, &mut event).await;
            if event.cancelled {
                return;
            }
        }
        client.send_chunks(&[chunk]).await;

        let velocity = player.living_entity.entity.velocity.load();

        debug!("正在向 {} 发送玩家传送", player.gameprofile.name);
        player.request_teleport(position, yaw, pitch);

        let gameprofile = &player.gameprofile;
        let player_actions = [
            PlayerAction::AddPlayer {
                name: &gameprofile.name,
                properties: &gameprofile.properties.load(),
            },
            PlayerAction::UpdateGameMode(VarInt(gamemode as i32)),
            PlayerAction::UpdateListed(true),
            PlayerAction::UpdateLatency(VarInt(0)),
            PlayerAction::UpdateListOrder(VarInt(0)),
            PlayerAction::UpdateHat(true),
        ];
        let java_player = [papokin_protocol::java::client::play::Player {
            uuid: gameprofile.id,
            actions: &player_actions,
        }];
        let player_info_update = CPlayerInfoUpdate::new(
            (PlayerInfoFlags::ADD_PLAYER
                | PlayerInfoFlags::UPDATE_GAME_MODE
                | PlayerInfoFlags::UPDATE_LISTED
                | PlayerInfoFlags::UPDATE_LATENCY
                | PlayerInfoFlags::UPDATE_LIST_PRIORITY
                | PlayerInfoFlags::UPDATE_HAT)
                .bits(),
            &java_player,
        );

        self.broadcast_packet_all(&player_info_update);

        // 如果玩家有自定义 tab_list_name，为其发送更新
        if let Some(tab_list_name) = player.get_tab_list_name() {
            let actions = [PlayerAction::UpdateDisplayName(Some(&tab_list_name))];
            let java_player = [papokin_protocol::java::client::play::Player {
                uuid: gameprofile.id,
                actions: &actions,
            }];
            self.broadcast_packet_all(&CPlayerInfoUpdate::new(
                PlayerInfoFlags::UPDATE_DISPLAY_NAME.bits(),
                &java_player,
            ));
        }

        // 在这里，我们发送所有已加入玩家的信息。
        let mut players_tab_list_names = Vec::new();
        {
            let players = self.players.load();
            let mut data_to_process = Vec::new();
            for p in players
                .iter()
                .filter(|p| p.gameprofile.id != player.gameprofile.id)
            {
                let props_guard = p.gameprofile.properties.load();
                data_to_process.push((props_guard, p));
            }

            let mut current_player_data = Vec::new();
            for (properties, player) in &data_to_process {
                let chat_session = player
                    .chat_session
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let tab_list_name = player.get_tab_list_name();

                let mut player_actions = vec![PlayerAction::AddPlayer {
                    name: &player.gameprofile.name,
                    properties,
                }];

                if base_config.allow_chat_reports {
                    let initialized = chat_session.session_id != uuid::Uuid::nil()
                        && !chat_session.public_key.is_empty()
                        && !chat_session.signature.is_empty();
                    player_actions.push(PlayerAction::InitializeChat(initialized.then(|| {
                        InitChat {
                            session_id: chat_session.session_id,
                            expires_at: chat_session.expires_at,
                            public_key: chat_session.public_key.clone(),
                            signature: chat_session.signature.clone(),
                        }
                    })));
                }

                player_actions.extend([
                    PlayerAction::UpdateGameMode(VarInt(player.gamemode.load() as i32)),
                    PlayerAction::UpdateListed(player.tab_list_listed.load(Ordering::Relaxed)),
                    PlayerAction::UpdateLatency(VarInt(
                        player.tab_list_latency.load(Ordering::Relaxed),
                    )),
                    PlayerAction::UpdateListOrder(VarInt(
                        player.tab_list_order.load(Ordering::Relaxed),
                    )),
                    PlayerAction::UpdateHat(true),
                ]);
                drop(chat_session);

                current_player_data.push((&player.gameprofile.id, player_actions));

                // 收集 tab_list_names，稍后发送
                if tab_list_name.is_some() {
                    players_tab_list_names.push((player.gameprofile.id, tab_list_name));
                }
            }

            let mut action_flags = PlayerInfoFlags::ADD_PLAYER
                | PlayerInfoFlags::UPDATE_LISTED
                | PlayerInfoFlags::UPDATE_LATENCY
                | PlayerInfoFlags::UPDATE_LIST_PRIORITY
                | PlayerInfoFlags::UPDATE_GAME_MODE
                | PlayerInfoFlags::UPDATE_HAT;
            if base_config.allow_chat_reports {
                action_flags |= PlayerInfoFlags::INITIALIZE_CHAT;
            }

            let entries = current_player_data
                .iter()
                .map(|(id, actions)| java::client::play::Player {
                    uuid: **id,
                    actions,
                })
                .collect::<Vec<_>>();

            debug!("正在向 {} 发送玩家信息", player.gameprofile.name);
            client
                .enqueue_client_packet(&CPlayerInfoUpdate::new(action_flags.bits(), &entries))
                .await;

            // 为拥有自定义名称的现有玩家发送 tab_list_names
            for (player_id, tab_list_name) in &players_tab_list_names {
                if let Some(name) = tab_list_name {
                    let actions = [PlayerAction::UpdateDisplayName(Some(name))];
                    let java_player = [papokin_protocol::java::client::play::Player {
                        uuid: *player_id,
                        actions: &actions,
                    }];
                    client
                        .enqueue_client_packet(&CPlayerInfoUpdate::new(
                            PlayerInfoFlags::UPDATE_DISPLAY_NAME.bits(),
                            &java_player,
                        ))
                        .await;
                }
            }
        };

        let gameprofile = &player.gameprofile;

        // 为每个客户端生成该玩家。
        let spawn_entity = CSpawnEntity::new(
            entity_id.into(),
            gameprofile.id,
            i32::from(EntityType::PLAYER.id).into(),
            position,
            pitch,
            yaw,
            yaw,
            0.into(),
            velocity,
        );

        self.broadcast_packet_except(&[player.gameprofile.id], &spawn_entity);

        // 向 Java 玩家广播元数据，以便他们能与新玩家正常交互
        let skin_parts = player.config.load().skin_parts;

        self.broadcast_skin_parts(&[gameprofile.id], entity_id, skin_parts);

        // 为我们的客户端生成玩家。
        let id = player.gameprofile.id;
        for existing_player in self
            .players
            .load()
            .iter()
            .filter(|c| c.gameprofile.id != id)
        {
            let entity = &existing_player.get_entity();
            let pos = entity.pos.load();
            let gameprofile = &existing_player.gameprofile;
            let actions = [
                PlayerAction::AddPlayer {
                    name: &gameprofile.name,
                    properties: &gameprofile.properties.load(),
                },
                PlayerAction::UpdateGameMode(VarInt(existing_player.gamemode.load() as i32)),
                PlayerAction::UpdateListed(existing_player.tab_list_listed.load(Ordering::Relaxed)),
                PlayerAction::UpdateLatency(VarInt(
                    existing_player.tab_list_latency.load(Ordering::Relaxed),
                )),
                PlayerAction::UpdateListOrder(VarInt(
                    existing_player.tab_list_order.load(Ordering::Relaxed),
                )),
                PlayerAction::UpdateHat(true),
            ];
            let java_player = [papokin_protocol::java::client::play::Player {
                uuid: gameprofile.id,
                actions: &actions,
            }];
            player
                .client
                .enqueue_client_packet(&CPlayerInfoUpdate::new(
                    (PlayerInfoFlags::ADD_PLAYER
                        | PlayerInfoFlags::UPDATE_LISTED
                        | PlayerInfoFlags::UPDATE_GAME_MODE
                        | PlayerInfoFlags::UPDATE_LATENCY
                        | PlayerInfoFlags::UPDATE_LIST_PRIORITY
                        | PlayerInfoFlags::UPDATE_HAT)
                        .bits(),
                    &java_player,
                ))
                .await;

            player
                .client
                .enqueue_client_packet(&CSpawnEntity::new(
                    existing_player.entity_id().into(),
                    gameprofile.id,
                    i32::from(EntityType::PLAYER.id).into(),
                    pos,
                    entity.pitch.load(),
                    entity.yaw.load(),
                    entity.head_yaw.load(),
                    0.into(),
                    entity.velocity.load(),
                ))
                .await;

            if client.version.load() >= JavaMinecraftVersion::V_1_21 {
                let config = existing_player.config.load();
                let mut buf = Vec::new();
                {
                    let meta = Metadata::new(
                        papokin_data::tracked_data::player::PLAYER_MODE_CUSTOMISATION,
                        config.skin_parts,
                    );
                    let _ = meta.write(&mut buf, &client.version.load());
                };
                {
                    let meta = Metadata::new(
                        papokin_data::tracked_data::player::PLAYER_MODE_CUSTOMIZATION_ID,
                        config.skin_parts,
                    );
                    let _ = meta.write(&mut buf, &client.version.load());
                };
                drop(config);
                // END
                buf.put_u8(255);
                client
                    .enqueue_client_packet(&CSetEntityMetadata::new(
                        existing_player.get_entity().entity_id.into(),
                        buf.into(),
                    ))
                    .await;
            }

            {
                let held_item = existing_player.inventory.held_item();
                let equipment_list = {
                    let mut equipment_list =
                        vec![(EquipmentSlot::MAIN_HAND.discriminant(), held_item.clone())];

                    let equipment_guard = existing_player
                        .inventory
                        .entity_equipment
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    for (slot, item_stack) in &equipment_guard.equipment {
                        equipment_list.push((slot.discriminant(), item_stack.clone()));
                    }
                    equipment_list
                };

                let equipment: Vec<(i8, ItemStackSerializer)> = equipment_list
                    .iter()
                    .map(|(slot, stack)| (*slot, ItemStackSerializer::from(stack.clone())))
                    .collect();

                let je_packet = CSetEquipment::new(existing_player.entity_id().into(), equipment);

                player.client.enqueue_client_packet(&je_packet).await;
            }
        }
        player.send_client_information();

        player.send_abilities_update();

        // 同步选中的槽位
        player.enqueue_set_held_item_packet(&CSetSelectedSlot::new(
            player.get_inventory().get_selected_slot() as i8,
        ));

        if client.version.load() >= JavaMinecraftVersion::V_1_20_2 {
            // 开始等待区块。设置 "Loading Terrain" 界面（1.20.2 新增）
            debug!("正在向 {} 发送等待区块事件", player.gameprofile.name);
            client
                .send_packet(&CGameEvent::new(GameEvent::StartWaitingChunks, 0.0))
                .await;
        }

        self.worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .init_client(client);

        // 发送初始时间
        player.send_time(self);

        // 发送初始记分板状态
        player.send_scoreboard();

        let (spawn_block_pos, yaw, pitch) = {
            let level_info_lock = self.level_info.load();
            (
                BlockPos::new(
                    level_info_lock.spawn_x,
                    level_info_lock.spawn_y,
                    level_info_lock.spawn_z,
                ),
                level_info_lock.spawn_yaw,
                level_info_lock.spawn_pitch,
            )
        };

        client
            .send_packet(&CPlayerSpawnPosition::new(
                spawn_block_pos,
                yaw,
                pitch,
                self.dimension.minecraft_name.to_owned(),
            ))
            .await;

        // 发送初始天气状态
        let (is_raining, rain_level, thunder_level) = {
            let weather = self
                .weather
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (
                weather.raining,
                weather.rain_level.clamp(0.0, 1.0),
                weather.thunder_level.clamp(0.0, 1.0),
            )
        };
        if is_raining {
            client
                .enqueue_client_packet(&CGameEvent::new(GameEvent::BeginRaining, 0.0))
                .await;

            client
                .enqueue_client_packet(&CGameEvent::new(GameEvent::RainLevelChange, rain_level))
                .await;
            client
                .enqueue_client_packet(&CGameEvent::new(
                    GameEvent::ThunderLevelChange,
                    thunder_level,
                ))
                .await;
        }

        let player_bossbars = server
            .bossbars
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_player_bars(&player.gameprofile.id)
            .map(|bars| bars.into_iter().cloned().collect::<Vec<_>>());
        if let Some(bossbars) = player_bossbars {
            for bossbar in &bossbars {
                player.send_bossbar(bossbar);
            }
        }

        player.has_played_before.store(true, Ordering::Relaxed);
        player.on_screen_handler_opened(&player.player_screen_handler);

        player.send_active_effects();
        player.breath_manager.send_air_supply(player);
        self.send_player_equipment(player);
        player
            .living_entity
            .send_current_equipment_attribute_modifiers();

        let java_client = &player.client;
        if server.advanced_config.recipe.send_recipes
            && java_client.version.load() >= JavaMinecraftVersion::V_1_21_2
        {
            let settings_packet = CRecipeBookSettings::default_closed();
            if let Ok(data) = java_client.serialize_packet(&settings_packet) {
                java_client.send_packet_now(data).await;
            }
            let dynamic_recipes = server.recipe_manager.get_dynamic_recipes();
            let add_packet = CRecipeBookAdd::new(true, &dynamic_recipes);
            if let Ok(data) = java_client.serialize_packet(&add_packet) {
                java_client.send_packet_now(data).await;
            }
        }
        let msg_comp = TextComponent::translate(
            translation::java::MULTIPLAYER_PLAYER_JOINED,
            [TextComponent::text(player.gameprofile.name.clone())],
        )
        .color_named(NamedColor::Yellow);
        let mut event = PlayerJoinEvent::new(player.clone(), msg_comp);

        server.plugin_manager.fire(server, &mut event).await;

        if !event.cancelled {
            self.broadcast_system_message(&event.join_message, false);
            // TODO: 改用结构化日志，例如 info!(player = %name, "connected")
            info!("{}", event.join_message.to_pretty_console());
        }
    }

    fn send_player_equipment(&self, from: &Player) {
        let held_item = from.inventory.held_item();
        let mut equipment_list = vec![(EquipmentSlot::MAIN_HAND.discriminant(), held_item)];

        let equipment_guard = from
            .inventory
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (slot, item_stack) in &equipment_guard.equipment {
            equipment_list.push((slot.discriminant(), item_stack.clone()));
        }
        drop(equipment_guard);

        let equipment: Vec<(i8, ItemStackSerializer)> = equipment_list
            .iter()
            .map(|(slot, stack)| (*slot, ItemStackSerializer::from(stack.clone())))
            .collect();
        let je_packet = CSetEquipment::new(from.entity_id().into(), equipment);

        self.send_to_tracking_players(from.get_entity(), &je_packet);
    }

    pub fn send_world_info(
        &self,
        player: &Arc<Player>,
        position: Vector3<f64>,
        yaw: f32,
        pitch: f32,
    ) {
        self.worldborder
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .init_client(&player.client);

        // TODO: 世界出生点（指南针相关）

        if player.client.version.load() >= JavaMinecraftVersion::V_1_20_2 {
            player.try_send_client_packet(&CGameEvent::new(GameEvent::StartWaitingChunks, 0.0));
        }

        let entity = &player.get_entity();

        self.broadcast_packet_except(
            &[player.gameprofile.id],
            // TODO: 添加速度
            &CSpawnEntity::new(
                entity.entity_id.into(),
                player.gameprofile.id,
                i32::from(EntityType::PLAYER.id).into(),
                position,
                pitch,
                yaw,
                yaw,
                0.into(),
                Vector3::new(0.0, 0.0, 0.0),
            ),
        );

        player.send_client_information();

        chunker::update_position(player);
        // 更新命令

        player.set_health(20.0);
    }

    pub fn explode(
        self: &Arc<Self>,
        position: Vector3<f64>,
        power: f32,
        interaction: ExplosionInteraction,
    ) {
        self.explode_with_calculator(position, power, interaction, None);
    }

    pub fn explode_with_calculator(
        self: &Arc<Self>,
        position: Vector3<f64>,
        power: f32,
        interaction: ExplosionInteraction,
        damage_calculator: Option<Arc<dyn ExplosionDamageCalculator>>,
    ) {
        let block_interaction = self.get_block_interaction(interaction);
        let mut explosion = Explosion::new(power, position, block_interaction);
        if let Some(calc) = damage_calculator {
            explosion = explosion.with_damage_calculator(calc);
        }
        self.run_explosion(&explosion, position, power);
    }

    pub fn explode_tnt_minecart(self: &Arc<Self>, position: Vector3<f64>, power: f32) {
        let block_interaction = self.get_block_interaction(ExplosionInteraction::Tnt);
        let explosion = Explosion::new(power, position, block_interaction).preserving_rails();
        self.run_explosion(&explosion, position, power);
    }

    #[must_use]
    pub fn get_block_interaction(&self, interaction: ExplosionInteraction) -> BlockInteraction {
        let game_rules = &self.level_info.load().game_rules;
        match interaction {
            ExplosionInteraction::None => BlockInteraction::Keep,
            ExplosionInteraction::Block => {
                Self::get_destroy_type(game_rules.block_explosion_drop_decay)
            }
            ExplosionInteraction::Mob => {
                if game_rules.mob_griefing {
                    Self::get_destroy_type(game_rules.mob_explosion_drop_decay)
                } else {
                    BlockInteraction::Keep
                }
            }
            ExplosionInteraction::Tnt => {
                Self::get_destroy_type(game_rules.tnt_explosion_drop_decay)
            }
            ExplosionInteraction::Trigger => BlockInteraction::TriggerBlock,
        }
    }

    #[must_use]
    pub const fn get_destroy_type(drop_decay: bool) -> BlockInteraction {
        if drop_decay {
            BlockInteraction::DestroyWithDecay
        } else {
            BlockInteraction::Destroy
        }
    }

    fn run_explosion(self: &Arc<Self>, explosion: &Explosion, position: Vector3<f64>, power: f32) {
        let mut event = crate::plugin::api::events::entity::entity_explode::EntityExplodeEvent::new(
            0, position, power,
        );
        if let Some(server) = self.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
        if event.cancelled {
            return;
        }

        let block_count = explosion.explode(self);
        // 音效与客户端渲染半径使用钳制后的威力，与实际破坏效果一致
        let power = explosion.power();
        let particle = if power < 2.0 {
            Particle::Explosion
        } else {
            Particle::ExplosionEmitter
        };
        for player in self.players.load().iter() {
            let sound_id = remap_sound_id_for_version(
                Sound::EntityGenericExplode as u16,
                player.client.version.load(),
            );
            let sound = IdOr::<SoundEvent>::Id(sound_id);
            if player.position().squared_distance_to_vec(&position) > 4096.0 {
                continue;
            }
            player.try_send_client_packet(&CExplosion::new(
                position,
                power,
                block_count as i32,
                None,
                VarInt(particle as i32),
                sound,
            ));
        }
    }

    #[allow(clippy::too_many_lines)]
    pub async fn respawn_player(self: &Arc<Self>, player: &Arc<Player>, alive: bool) {
        let last_pos = player.get_entity().last_pos.load();
        let death_dimension = ResourceLocation::from(player.world().dimension.minecraft_name);
        let death_location = BlockPos(Vector3::new(
            last_pos.x.round() as i32,
            last_pos.y.round() as i32,
            last_pos.z.round() as i32,
        ));

        let data_kept = u8::from(alive);

        // 死亡重生时客户端已重置并脱离载具，服务端必须同步清理骑乘
        // 状态：否则 vehicle 指针残留旧载具（状态分裂——客户端已步行、
        // 服务端仍按乘客处理，后续移动包还会被套用到旧载具上）。
        if let Some(vehicle) = player.get_entity().get_vehicle() {
            vehicle
                .get_entity()
                .remove_passenger_sync(player.entity_id());
        }

        let server = self.server.upgrade();
        let default_world = server.as_ref().map_or_else(
            || self.clone(),
            |s| s.get_world_from_dimension(&Dimension::OVERWORLD),
        );

        // 从默认世界 level_info 复制生成信息，避免跨 await 持有锁
        let (spawn_x, spawn_y, spawn_z, spawn_yaw, spawn_pitch, keep_inventory) = {
            let info = default_world.level_info.load();
            (
                info.spawn_x,
                info.spawn_y,
                info.spawn_z,
                info.spawn_yaw,
                info.spawn_pitch,
                info.game_rules.keep_inventory,
            )
        };

        // 获取重生位置和维度
        let (position, yaw, pitch, respawn_dimension) = if let Some(respawn) =
            player.calculate_respawn_point().await
        {
            (
                respawn.position,
                respawn.yaw,
                respawn.pitch,
                respawn.dimension,
            )
        } else {
            // 无有效重生点 - 若玩家曾设置过则发送通知
            if player
                .respawn_point
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_some()
            {
                player
                    .send_client_packet(&CGameEvent::new(GameEvent::NoRespawnBlockAvailable, 0.0))
                    .await;
                let mut guard = player
                    .respawn_point
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if let Some(point) = guard.as_ref()
                    && !point.force
                {
                    *guard = None;
                }
            }

            // FIXME: 此生成位置计算不正确，应使用原版的
            // 正确的生成位置计算（见 #1381）。y 高度的计算
            // 需要考虑生成半径并寻找安全的生成位置。
            let chunk_pos = Vector2::new(spawn_x >> 4, spawn_z >> 4);
            default_world
                .level
                .get_or_fetch_chunk(chunk_pos, |_| ())
                .await;
            let top = default_world.get_top_block(Vector2::new(spawn_x, spawn_z));
            let pos_y = if top > default_world.dimension.min_y {
                top + 1
            } else {
                spawn_y
            };

            (
                Vector3::new(
                    f64::from(spawn_x) + 0.5,
                    f64::from(pos_y),
                    f64::from(spawn_z) + 0.5,
                ),
                spawn_yaw,
                spawn_pitch,
                default_world.dimension.clone(),
            )
        };

        let mut spawn_loc_event = crate::plugin::api::events::player::player_spawn_location::PlayerSpawnLocationEvent::new(
            player.clone(),
            position,
        );
        if let Some(ref s) = server {
            s.plugin_manager.fire(s, &mut spawn_loc_event).await;
        }
        let position = spawn_loc_event.spawn_pos;

        // 跨维度重生的候选目标世界。
        let candidate_world = if respawn_dimension == self.dimension {
            None
        } else {
            server.as_ref().map_or_else(
                || {
                    warn!("无法获取服务器以进行跨维度重生");
                    None
                },
                |s| {
                    let worlds = s.worlds.load();
                    worlds
                        .iter()
                        .find(|w| w.dimension == respawn_dimension)
                        .cloned()
                },
            )
        };

        // 在传送前触发 PlayerChangeWorldEvent（可取消）；它先于
        // 不可取消的 PlayerRespawnEvent，它会观察已解析的世界。
        let (resolved_world, position, yaw, pitch) = if let Some(new_world) = candidate_world {
            if let Some(ref s) = server {
                let mut event = PlayerChangeWorldEvent {
                    player: player.clone(),
                    previous_world: self.clone(),
                    new_world: new_world.clone(),
                    position,
                    yaw,
                    pitch,
                    cancelled: false,
                };
                s.plugin_manager.fire(s, &mut event).await;

                if event.cancelled {
                    (None, position, yaw, pitch)
                } else {
                    let destination = event.new_world;
                    let position = event.position;
                    let yaw = event.yaw;
                    let pitch = event.pitch;

                    // 若被重定向回当前世界，则跳过该传送。
                    if destination.uuid != self.uuid {
                        debug!(
                            "跨维度重生：{} -> {}",
                            self.dimension.minecraft_name, destination.dimension.minecraft_name
                        );

                        // 在发布到新世界之前先从旧世界分离，以避免
                        // 观察者在区块管理器不匹配的世界中看到该玩家。
                        self.remove_player(player, false).await;
                        player.unload_watched_chunks(self).await;
                        player.change_world_chunks(&self.level, &destination);
                        player.living_entity.entity.set_world(destination.clone());
                        destination.players.rcu(|current_list| {
                            let mut new_list = (**current_list).clone();
                            new_list.push(player.clone());
                            new_list
                        });
                    }

                    (Some(destination), position, yaw, pitch)
                }
            } else {
                warn!("跨维度重生期间服务器已被释放");
                (None, position, yaw, pitch)
            }
        } else {
            if respawn_dimension != self.dimension {
                warn!(
                    "未找到目标世界 {:?}，改用 {:?} 维度的世界出生点",
                    respawn_dimension, self.dimension
                );
            }
            (None, position, yaw, pitch)
        };

        // 已取消或未解决的跨维度重生将回退到当前的
        // 世界出生点；否则应用事件解析出的值。
        let (target_world, position, yaw, pitch) = resolved_world.as_ref().map_or_else(
            || (self.clone(), position, yaw, pitch),
            |new_world| (new_world.clone(), position, yaw, pitch),
        );

        // 通知插件玩家已重生（不可取消）。
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire(
                    &server,
                    &mut PlayerRespawnEvent::new(
                        player.clone(),
                        self.clone(),
                        target_world.clone(),
                        position,
                        yaw,
                        pitch,
                        alive,
                    ),
                )
                .await;
        }

        // 发送带有目标维度的重生数据包（使用 send_packet_now 确保顺序正确）
        player
            .send_client_packet(&CRespawn::new(
                PlayerSpawnData::new(
                    target_world.dimension.clone(),
                    biome::hash_seed(target_world.level.seed.0),
                    player.gamemode.load() as u8,
                    player.gamemode.load() as i8,
                    false,
                    false,
                    Some((death_dimension, death_location)),
                    VarInt(player.get_entity().portal_cooldown.load(Ordering::Relaxed) as i32),
                    target_world.sea_level.into(),
                ),
                data_kept,
            ))
            .await;

        // 告知客户端默认生成位置，以免客户端
        // 世界重新加载期间回退到 (0, 2, 0)（修复橡皮筋回弹问题）。
        // 此数据包必须在 CRespawn 之后发送，客户端才能正确定位。
        let spawn_block_pos = BlockPos(Vector3::new(
            position.x.round() as i32,
            position.y.round() as i32,
            position.z.round() as i32,
        ));
        player
            .client
            .send_packet(&CPlayerSpawnPosition::new(
                spawn_block_pos,
                yaw,
                pitch,
                target_world.dimension.minecraft_name.to_string(),
            ))
            .await;

        player.living_entity.reset_state();

        player.send_permission_lvl_update();

        player.hunger_manager.restart();

        if !keep_inventory {
            player.set_experience(0, 0.0, 0);
            player.inventory.clear();
        }

        // 在加载区块之前设置实体位置，使区块在正确的位置加载
        // 这与初始生成流程相同：在传送之前先调用 update_position
        player.get_entity().set_pos(position);
        player.get_entity().set_rotation(yaw, pitch);
        player.get_entity().last_pos.store(position);

        // TODO: 难度、经验条、状态效果

        // 首先加载区块并发送世界信息（先于传送数据包）
        target_world.send_world_info(player, position, yaw, pitch);

        // 确保在传送前至少同步发送中心区块
        let center_chunk = player.get_entity().chunk_pos.load();
        let chunk = target_world
            .level
            .get_or_fetch_chunk(center_chunk, std::clone::Clone::clone)
            .await;
        player.client.send_chunks(&[chunk]).await;

        // 至少在中心区块交付后发送传送数据包
        player.request_teleport(position, yaw, pitch);
    }

    /// 若足够多的玩家正在睡觉且应跳过夜晚，则返回 true。
    pub fn should_skip_night(&self) -> bool {
        let players = self.players.load();

        let player_count = players.len();
        let sleeping_player_count = players
            .iter()
            .filter(|player| {
                player
                    .sleeping_since
                    .load()
                    .is_some_and(|since| since >= 100)
            })
            .count();
        drop(players);

        if player_count == 0 {
            return false;
        }

        let sleep_percentage = self
            .level_info
            .load()
            .game_rules
            .players_sleeping_percentage
            .clamp(0, 100);
        let required_sleeping =
            ((player_count as f64 * sleep_percentage as f64) / 100.0).ceil() as usize;
        let required_sleeping = required_sleeping.max(1);

        sleeping_player_count >= required_sleeping
    }

    // NOTE: 此函数实际上不 await 任何东西，它只是启动两个 tokio 任务
    /// IMPORTANT: 区块必须非空
    fn spawn_world_entity_chunks(self: &Arc<Self>, player: Arc<Player>, chunks: Vec<Vector2<i32>>) {
        #[cfg(debug_assertions)]
        let inst = std::time::Instant::now();

        // Note: `chunks` 源自 `Cylindrical::changed_chunks`，它
        // 已由预编译结果按距中心从近到远排好序
        // 圆柱形区块视野查找表。无需重新排序。

        let mut entity_receiver = self.level.receive_entity_chunks(chunks);
        let level = self.level.clone();
        let world = self.clone();

        player.clone().spawn_task(async move {
            'main: loop {
                let recv_result = tokio::select! {
                    () = player.client.await_close_interrupt() => {
                        debug!("正在取消玩家数据包处理");
                        None
                    },
                    recv_result = entity_receiver.recv() => {
                        recv_result
                    }
                };

                let Some((chunk_weak, first_load)) = recv_result else {
                    break;
                };

                let Some(chunk) = chunk_weak.upgrade() else {
                    continue;
                };

                let position = Vector2::new(chunk.x, chunk.z);

                if !level.is_chunk_watched(&position) {
                    // 不再被监视：不要使其实体活跃。保留
                    // 序列化数据保持原样，从而让正常的卸载路径得以持久化
                    // 原样保留（没有任何内容生效，因此也无需保存）。
                    trace!(
                        "收到实体区块 {:?}，但它已不再被监视；留给卸载流程处理",
                        &position
                    );
                    continue 'main;
                }

                if first_load {
                    // 第一个监视者：消费序列化的实体并使其
                    // 存活。活动实体列表将成为唯一的
                    // 事实来源，因此会取走（清空）区块的 NBT，以避免一直保留
                    // 一个会在下次卸载时被重新追加的重复副本
                    // 且每次重载都会翻倍。
                    let entity_nbts = std::mem::take(
                        &mut *chunk
                            .data
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner),
                    );
                    chunk.live.store(true, Relaxed);
                    for entity_nbt in &entity_nbts {
                        let Some(id) = entity_nbt.get_string("id") else {
                            debug!("实体没有 ID");
                            continue;
                        };
                        let Some(entity_type) =
                            EntityType::from_name(id.strip_prefix("minecraft:").unwrap_or(id))
                        else {
                            warn!("实体没有有效的实体类型 {id}");
                            continue;
                        };

                        // 保留持久化的 UUID，使实体保持其身份
                        // 跨重载保持不变（与原版一致）；仅在以下情况回退到
                        // 在缺失或损坏时生成新的。
                        let uuid = entity_nbt.get_uuid("UUID").unwrap_or_else(Uuid::new_v4);
                        // Pos 为零，因为它会从 NBT 读取。
                        let entity =
                            from_type(entity_type, Vector3::new(0.0, 0.0, 0.0), &world, uuid);
                        entity.read_nbt_non_mut(entity_nbt);
                        entity.init_data_tracker();

                        let base_entity = entity.get_entity();
                        // 清除速度，使客户端不会重放下落
                        // 动画；来自原始下落的残余速度
                        // 过期数据。
                        base_entity.velocity.store(Vector3::default());

                        // 若另一个观察者已加载此实体，则按 UUID 去重。
                        // 追踪器持有配对信息（生成数据包 + 载具恢复）。
                        world.add_entity_silent(entity.clone());
                        player.try_restore_vehicle(&entity);
                    }
                } else {
                    // 对其他观察者已是 live：现在配对这位玩家，以便
                    // 生成数据包与载具恢复不必等待追踪刻。
                    world
                        .entity_tracker
                        .update_player_chunks(&player, &world, &[position]);
                }
            }

            #[cfg(debug_assertions)]
            debug!("区块在 {}ms 后完成排队", inst.elapsed().as_millis());
        });
    }

    /// 根据实体 ID 获取 `Player`
    pub fn get_player_by_id(&self, id: i32) -> Option<Arc<Player>> {
        for player in self.players.load().iter() {
            if player.entity_id() == id {
                return Some(player.clone());
            }
        }
        None
    }

    /// 根据实体 ID 获取实体
    pub fn get_entity_by_id(&self, id: i32) -> Option<Arc<dyn EntityBase>> {
        for entity in self.entities.load().iter() {
            if entity.get_entity().entity_id == id {
                return Some(entity.clone());
            }
        }
        for player in self.players.load().iter() {
            if player.get_entity().entity_id == id {
                return Some(player.clone() as Arc<dyn EntityBase>);
            }
        }
        None
    }

    /// 根据用户名获取 `Player`
    pub fn get_player_by_name(&self, name: &str) -> Option<Arc<Player>> {
        for player in self.players.load().iter() {
            if player.gameprofile.name.eq_ignore_ascii_case(name) {
                return Some(player.clone());
            }
        }
        None
    }

    // 获取某个 Box 中的所有实体
    pub fn get_all_at_box(&self, aabb: &BoundingBox) -> Vec<Arc<dyn EntityBase>> {
        let entities_guard = self.entities.load();
        let players_guard = self.players.load();

        entities_guard
            .iter()
            .map(|e| e.clone() as Arc<dyn EntityBase>)
            .chain(
                players_guard
                    .iter()
                    .map(|p| p.clone() as Arc<dyn EntityBase>),
            )
            .filter(|entity| entity.get_entity().bounding_box.load().intersects(aabb))
            .collect()
    }

    // 获取某个 Box 中的所有非玩家实体
    pub fn get_entities_at_box(&self, aabb: &BoundingBox) -> Vec<Arc<dyn EntityBase>> {
        // 小范围盒子（漏斗/投掷物/压力板/绊线等高频调用）走按区块
        // 分桶的索引，代价 O(命中实体数)；范围超过阈值（64 桶，
        // 即 128×128 格）时回退全表线性扫描，保证任意大盒子仍正确。
        let min_chunk = Vector2::new(
            get_section_cord(aabb.min.x.floor() as i32),
            get_section_cord(aabb.min.z.floor() as i32),
        );
        let max_chunk = Vector2::new(
            get_section_cord(aabb.max.x.floor() as i32),
            get_section_cord(aabb.max.z.floor() as i32),
        );
        if let Some(candidates) = self.entities_by_chunk.query(min_chunk, max_chunk, 64) {
            return candidates
                .into_iter()
                .filter(|entity| entity.get_entity().bounding_box.load().intersects(aabb))
                .collect();
        }
        self.entities
            .load()
            .iter()
            .filter(|entity| entity.get_entity().bounding_box.load().intersects(aabb))
            .cloned()
            .collect()
    }

    // 获取某个 Box 中的所有玩家实体
    pub fn get_players_at_box(&self, aabb: &BoundingBox) -> Vec<Arc<Player>> {
        let players_guard = self.players.load();
        players_guard
            .iter()
            .filter(|player| player.get_entity().bounding_box.load().intersects(aabb))
            .cloned()
            .collect()
    }

    /// 通过唯一 UUID 检索玩家。
    ///
    /// 此函数在世界的活跃玩家列表中查找具有指定 UUID 的玩家。
    /// 若找到，则返回该玩家的 `Arc<Player>` 引用；否则返回 `None`。
    ///
    /// # Arguments
    ///
    /// * `id`: 要获取的玩家的 UUID。
    ///
    /// # Returns
    ///
    /// 一个 `Option<Arc<Player>>`，若找到则包含该玩家，否则为 `None`。
    pub fn get_player_by_uuid(&self, id: uuid::Uuid) -> Option<Arc<Player>> {
        self.players
            .load()
            .iter()
            .find(|p| p.gameprofile.id == id)
            .cloned()
    }

    /// 通过唯一 UUID 检索实体。
    ///
    /// 此函数在世界实体中查找具有指定 UUID 的实体。
    /// 若找到，则返回该实体的 `Arc<dyn EntityBase>` 引用；否则返回 `None`。
    ///
    /// # Arguments
    ///
    /// * `id`: 要获取的实体的 UUID。
    ///
    /// # Returns
    ///
    /// 一个 `Option<Arc<dyn EntityBase>>`，若找到则包含该玩家，否则为 `None`。
    pub fn get_entity_by_uuid(&self, id: uuid::Uuid) -> Option<Arc<dyn EntityBase>> {
        self.entities
            .load()
            .iter()
            .find(|p| p.get_entity().entity_uuid == id)
            .cloned()
    }

    /// 获取位置等于世界中给定坐标点的玩家列表。
    ///
    /// 它遍历世界中的玩家并检查其位置。如果玩家的位置与
    /// 给定位置，它会将其加入一个 `Vec` 并稍后返回。如果没有
    /// 玩家在该位置，则只会返回空的 `Vec`。
    ///
    /// # Arguments
    ///
    /// * `position`: 该函数要检查的位置。
    pub fn get_players_by_pos(&self, position: BlockPos) -> Vec<Arc<Player>> {
        self.players
            .load()
            .iter()
            .filter_map(|player| {
                let player_block_pos = player.get_entity().block_pos.load().0;
                (position.0.x == player_block_pos.x
                    && position.0.y == player_block_pos.y
                    && position.0.z == player_block_pos.z)
                    .then(|| Arc::clone(player))
            })
            .collect::<_>()
    }

    /// 获取给定世界位置附近的玩家。
    /// 它“创建”一个球体，并检查玩家是否在球体内部
    /// 并返回一个 `HashMap`，其中 UUID 作为键，`Player`
    /// 对象则是值。
    ///
    /// # Arguments
    /// * `pos`: 球体的中心。
    /// * `radius`: 球体的半径。半径越大，检查的范围越大（朝各个方向）。
    pub fn get_nearby_players(&self, pos: Vector3<f64>, radius: f64) -> Vec<Arc<Player>> {
        let radius_squared = radius.powi(2);

        self.players
            .load()
            .iter()
            .filter_map(|player| {
                let player_pos = player.get_entity().pos.load();
                (player_pos.squared_distance_to_vec(&pos) <= radius_squared).then(|| player.clone())
            })
            .collect()
    }

    pub fn get_nearby_entities(
        &self,
        pos: Vector3<f64>,
        radius: f64,
    ) -> HashMap<uuid::Uuid, Arc<dyn EntityBase>> {
        let radius_squared = radius.powi(2);

        // AI 目标选择等高频调用走分块索引（代价 O(候选数)）；
        // 大半径（半径 64+，跨度超过索引阈值）回退全表线性扫描，
        // 保证任意半径仍然正确。见 `entity_index` 模块文档。
        let min_chunk = Vector2::new(
            get_section_cord((pos.x - radius).floor() as i32),
            get_section_cord((pos.z - radius).floor() as i32),
        );
        let max_chunk = Vector2::new(
            get_section_cord((pos.x + radius).floor() as i32),
            get_section_cord((pos.z + radius).floor() as i32),
        );
        let candidates: Box<dyn Iterator<Item = Arc<dyn EntityBase>>> = self
            .entities_by_chunk
            .query(min_chunk, max_chunk, 64)
            .map_or_else(
                || {
                    Box::new(
                        self.entities
                            .load()
                            .iter()
                            .cloned()
                            .collect::<Vec<_>>()
                            .into_iter(),
                    )
                },
                |indexed| Box::new(indexed.into_iter()),
            );

        candidates
            .filter_map(|entity| {
                let entity_pos = entity.get_entity().pos.load();
                (entity_pos.squared_distance_to_vec(&pos) <= radius_squared)
                    .then(|| (entity.get_entity().entity_uuid, entity))
            })
            .collect()
    }

    /// 满足 `predicate` 的最近玩家。与 [`Self::get_closest_player`] 不同，更近的
    /// 未通过谓词的玩家不会遮蔽更远处通过谓词的玩家。
    pub fn get_nearest_player(
        &self,
        pos: Vector3<f64>,
        radius: f64,
        predicate: impl Fn(&Arc<Player>) -> bool,
    ) -> Option<Arc<Player>> {
        self.get_nearby_players(pos, radius)
            .into_iter()
            .filter(|player| predicate(player))
            .min_by(|a, b| {
                a.get_entity()
                    .pos
                    .load()
                    .squared_distance_to_vec(&pos)
                    .total_cmp(&b.get_entity().pos.load().squared_distance_to_vec(&pos))
            })
    }

    /// 满足 `predicate` 的最近实体。为何如此，参见 [`Self::get_nearest_player`]
    /// 并非 [`Self::get_closest_entity`] 再加一次检查。
    pub fn get_nearest_entity(
        &self,
        pos: Vector3<f64>,
        radius: f64,
        entity_types: Option<&[&'static EntityType]>,
        predicate: impl Fn(&Arc<dyn EntityBase>) -> bool,
    ) -> Option<Arc<dyn EntityBase>> {
        self.get_nearby_entities(pos, radius)
            .into_values()
            .filter(|entity| {
                entity_types.is_none_or(|types| types.contains(&entity.get_entity().entity_type))
                    && predicate(entity)
            })
            .min_by(|a, b| {
                a.get_entity()
                    .pos
                    .load()
                    .squared_distance_to_vec(&pos)
                    .total_cmp(&b.get_entity().pos.load().squared_distance_to_vec(&pos))
            })
    }

    pub fn get_closest_player(&self, pos: Vector3<f64>, radius: f64) -> Option<Arc<Player>> {
        self.get_nearest_player(pos, radius, |_| true)
    }

    /// 获取距离某位置最近的实体，可按实体类型过滤。
    ///
    /// # Arguments
    ///
    /// * `pos` - 要在其周围搜索的位置。
    /// * `radius` - 搜索范围半径。
    /// * `entity_types` - 用于过滤的可选实体类型数组。若为 None，则包含所有实体类型。
    ///
    /// # Returns
    ///
    /// 符合筛选条件的最近实体，如果未找到任何实体则为 None。
    pub fn get_closest_entity(
        &self,
        pos: Vector3<f64>,
        radius: f64,
        entity_types: Option<&[&'static EntityType]>,
    ) -> Option<Arc<dyn EntityBase>> {
        self.get_nearest_entity(pos, radius, entity_types, |_| true)
    }

    /// 向给定的 [`Vec`] 添加满足特定条件且
    /// 存在于所提供的 [`BoundingBox`] 中。
    ///
    /// # Arguments
    ///
    /// * `list`: 要添加到的 `Vec`。
    /// * `max_list_capacity`: 向 `list` 添加实体的最大容量。若达到此上限，将不再
    ///   实体将被加入列表。如果 `list` 已达到该上限，则不会发生任何事。
    /// * `bounding_box`: 用于过滤所添加实体的包围盒。
    /// * `predicate`: 谓词函数，实体必须使其为 `true` 才会被加入列表。
    pub fn extend_entities_in_box_where(
        &self,
        list: &mut Vec<Arc<dyn EntityBase>>,
        max_list_capacity: usize,
        bounding_box: BoundingBox,
        predicate: impl Fn(&dyn EntityBase) -> bool,
    ) {
        self.extend_entities_where(list, max_list_capacity, |e| {
            bounding_box.intersects(&e.get_entity().bounding_box.load()) && predicate(e)
        });
    }

    /// 将满足特定条件的实体添加到给定的 [`Vec`] 中。
    ///
    /// # Arguments
    ///
    /// * `list`: 要添加到的 `Vec`。
    /// * `max_list_capacity`: 向 `list` 添加实体的最大容量。若达到此上限，将不再
    ///   实体将被加入列表。如果 `list` 已达到该上限，则不会发生任何事。
    /// * `predicate`: 谓词函数，实体必须使其为 `true` 才会被加入列表。
    pub fn extend_entities_where(
        &self,
        list: &mut Vec<Arc<dyn EntityBase>>,
        max_list_capacity: usize,
        predicate: impl Fn(&dyn EntityBase) -> bool,
    ) {
        if list.len() >= max_list_capacity {
            return;
        }
        // 遍历玩家。
        for player in self.players.load().iter() {
            if !predicate(player.as_ref()) {
                continue;
            }
            // 我们把该玩家加入列表。
            list.push(player.clone());
            // 检查列表是否过大。
            if list.len() >= max_list_capacity {
                return;
            }
        }
        // 实体同理。
        for entity in self.entities.load().iter() {
            if !predicate(entity.as_ref()) {
                continue;
            }
            list.push(entity.clone());
            if list.len() >= max_list_capacity {
                return;
            }
            // TODO: 实现末影龙处理
        }
    }

    /// 将玩家添加到世界，并在启用时广播加入消息。
    ///
    /// 此函数接收玩家的 UUID 和一个 `Arc<Player>` 引用。
    /// 它以 UUID 为键将玩家插入世界的 `current_players` 映射。
    /// 此外，它还会向世界中所有已连接的玩家广播加入消息。
    ///
    /// # Arguments
    ///
    /// * `player`: 指向玩家对象的 `Arc<Player>` 引用。
    pub fn add_player(&self, player: &Arc<Player>) -> Result<(), String> {
        self.players.rcu(|current_list| {
            let mut new_list = (**current_list).clone();
            new_list.push(player.clone());
            new_list
        });
        self.entity_tracker
            .add_entity(&(player.clone() as Arc<dyn EntityBase>), self);
        Ok(())
    }

    /// 只能在玩家自身的 `CLogin` 数据包发送之后调用。
    pub fn pair_new_player_with_tracked_entities(&self, player: &Arc<Player>) {
        self.entity_tracker
            .pair_new_player_with_tracked_entities(player, self);
    }

    /// 将玩家从世界中移除，并在启用时广播断开连接消息。
    ///
    /// 此函数根据玩家的 `Player` 引用将其从世界中移除。
    /// 它执行以下操作：
    ///
    /// 1. 使用玩家的 UUID 从 `current_players` 映射中移除该玩家。
    /// 2. 向所有已连接的玩家广播 `CRemovePlayerInfo` 数据包，告知他们有玩家离开。
    /// 3. 使用实体 ID 将玩家的实体从世界中移除。
    /// 4. 可选地向所有其他玩家发送断开连接消息，通知他们有玩家离开。
    ///
    /// # Arguments
    ///
    /// * `player`: 要移除的 `Player` 对象的引用。
    /// * `fire_event`: 一个布尔标志，指示是否触发 `PlayerLeaveEvent` 事件。
    ///
    /// # Notes
    ///
    /// - 此函数假定 `broadcast_packet_expect` 和 `remove_entity` 已在其他地方定义。
    /// - 断开连接消息的发送目前是可选的。建议将其做成可配置项。
    pub async fn remove_player(
        &self,
        player: &Arc<Player>,
        fire_event: bool,
    ) -> Option<Arc<Player>> {
        let mut removed_player: Option<Arc<Player>> = None;

        self.players.rcu(|current_list| {
            let mut new_list = (**current_list).clone();
            // 在过滤掉玩家之前先找到玩家
            let pos = new_list
                .iter()
                .position(|p| p.gameprofile.id == player.gameprofile.id);
            if let Some(pos) = pos {
                removed_player = Some(new_list.remove(pos));
            }
            new_list
        });
        if let Some(ref player) = removed_player {
            self.entity_tracker
                .remove_entity(player.as_ref() as &dyn EntityBase, self);
            let uuid = player.gameprofile.id;
            let entity_id = player.entity_id();

            self.broadcast_packet_all(&CRemovePlayerInfo::new(&[uuid]));

            self.broadcast_packet_all(&CRemoveEntities::new(&[entity_id.into()]));

            if fire_event {
                let msg_comp = TextComponent::translate(
                    translation::java::MULTIPLAYER_PLAYER_LEFT,
                    [TextComponent::text(player.gameprofile.name.clone())],
                )
                .color_named(NamedColor::Yellow);
                let mut event = PlayerLeaveEvent::new(player.clone(), msg_comp);

                if let Some(server) = self.server.upgrade() {
                    server.plugin_manager.fire(&server, &mut event).await;

                    if !event.cancelled {
                        for player in self.players.load().iter() {
                            player.send_system_message(&event.leave_message);
                        }
                        info!("{}", event.leave_message.to_pretty_console());
                    }
                }
            }
        }
        removed_player
    }

    #[expect(clippy::needless_pass_by_value)]
    pub fn spawn_entity_non_save(&self, entity: Arc<dyn EntityBase>) {
        let _base_entity = entity.get_entity();
        self.entity_tracker.add_entity(&entity, self);
        self.spawn_state.load().add_entity(self, entity.as_ref());
        self.register_entity_in_chunk_index(&entity);

        self.entities.rcu(|current_entities| {
            let mut new_entities = (**current_entities).clone();
            new_entities.push(entity.clone());
            new_entities
        });
    }

    pub fn spawn_entity(self: &Arc<Self>, entity: Arc<dyn EntityBase>) {
        let mut event = crate::plugin::api::events::entity::entity_spawn::EntitySpawnEvent::new(
            entity.get_entity().entity_id,
            entity.get_entity().entity_type.id.to_string(),
            entity.get_entity().pos.load(),
            self.clone(),
        );
        if let Some(server) = self.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
        if event.cancelled {
            return;
        }

        entity.init_data_tracker();
        self.add_entity_silent(entity);
    }

    #[expect(clippy::needless_pass_by_value)]
    pub fn add_entity_silent(&self, entity: Arc<dyn EntityBase>) {
        let base_entity = entity.get_entity();

        // 防止出现 UUID 相同的重复实体。
        // 当区块实体数据被加载而该实体
        // 已存在于世界中（例如另一名玩家仍在追踪它）。
        let already_exists = self
            .entities
            .load()
            .iter()
            .any(|e| e.get_entity().entity_uuid == base_entity.entity_uuid);
        if already_exists {
            return;
        }

        // 该实体仅存在于内存中：只在特定时点才写入其所属区块的已保存数据，
        // 卸载（见 `save_entities_by_chunk`），绝不会发生在生成时，因此它不可能既存活又
        // 一次性序列化（这会在下次重载时使其翻倍）。
        self.spawn_state.load().add_entity(self, entity.as_ref());
        self.entity_tracker.add_entity(&entity, self);
        self.register_entity_in_chunk_index(&entity);

        self.entities.rcu(|current_entities| {
            let mut new_entities = (**current_entities).clone();
            new_entities.push(entity.clone());
            new_entities
        });
    }

    /// 将实体登记进按区块分桶的空间索引，并在实体基座上记录自身的
    /// 弱引用句柄——`Entity::set_pos` 跨块移动时凭该句柄向新桶插入，
    /// 保证盒查询不会漏掉已移动的实体。
    fn register_entity_in_chunk_index(&self, entity: &Arc<dyn EntityBase>) {
        let base = entity.get_entity();
        // 实体只会被加入世界一次（`add_entity_silent` 有 UUID 去重），
        // 但跨维度迁移会复用同一 Arc 重新登记：句柄不变，幂等无害。
        let _ = base
            .chunk_index_handle
            .set(Arc::downgrade(entity) as Weak<dyn EntityBase>);
        // 桶位从当前位置直接计算：`chunk_pos` 依赖 set_pos 驱动，
        // 构造后未移动过的实体可能仍是默认值。
        let pos = base.pos.load();
        let chunk = Vector2::new(
            get_section_cord(pos.x.floor() as i32),
            get_section_cord(pos.z.floor() as i32),
        );
        self.entities_by_chunk.insert(chunk, entity);
    }

    pub fn remove_entity(&self, entity: &dyn EntityBase) {
        let base_entity = entity.get_entity();
        if base_entity
            .removal_reason
            .swap(Some(RemovalReason::Discarded))
            .is_some()
        {
            return;
        }
        base_entity.removed.store(true, Ordering::Release);

        // 原版 Entity.remove() 语义：移除前先脱离自身载具并弹出
        // 全部乘客。否则乘客表/载具指针会残留指向已移除实体的强
        // 引用——幽灵骑乘（客户端永久骑在已删实体上）且 Arc 无法释放。
        if let Some(vehicle) = base_entity.get_vehicle() {
            vehicle
                .get_entity()
                .remove_passenger_sync(base_entity.entity_id);
        }
        let passenger_ids: Vec<i32> = base_entity
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|passenger| passenger.get_entity().entity_id)
            .collect();
        for passenger_id in passenger_ids {
            base_entity.remove_passenger_sync(passenger_id);
        }

        self.spawn_state.load().remove_entity(self, entity);
        self.entity_tracker.remove_entity(entity, self);
        self.entities.rcu(|current_entities| {
            let mut new_entities = (**current_entities).clone();
            new_entities.retain(|e| e.get_entity().entity_uuid != base_entity.entity_uuid);
            new_entities
        });
    }

    pub async fn remove_entities_in_chunks(
        &self,
        chunks: impl IntoIterator<Item = impl std::borrow::Borrow<Vector2<i32>>>,
    ) {
        let chunks_set: FxHashSet<_> = chunks.into_iter().map(|c| *c.borrow()).collect();
        if chunks_set.is_empty() {
            return;
        }
        let mut entities_to_remove = Vec::new();

        self.entities.rcu(|current_entities| {
            entities_to_remove.clear();
            let mut new_entities = (**current_entities).clone();
            new_entities.retain(|entity| {
                let base_entity = entity.get_entity();
                let pos = base_entity.chunk_pos.load();
                if chunks_set.contains(&pos) {
                    entities_to_remove.push(entity.clone());
                    false
                } else {
                    true
                }
            });
            new_entities
        });

        self.save_entities_by_chunk(&entities_to_remove, chunks_set.iter().copied())
            .await;

        for entity in entities_to_remove {
            // 统一走 remove_entity：区块卸载同样要清理骑乘状态（弹出
            // 乘客、脱离载具）并置 removed 标记；实体表项已在上面的
            // rcu 中移除，remove_entity 内的再次 retain 是无害空操作。
            self.remove_entity(entity.as_ref());
        }

        for chunk_pos in &chunks_set {
            self.save_block_entities(*chunk_pos);
            self.block_entities.remove(chunk_pos);
        }
    }

    pub(crate) fn set_block_breaking(
        &self,
        from: &Entity,
        location: BlockPos,
        progress: BlockBreakingProgress,
    ) {
        let chunk_pos = location.chunk_position(); // papokin 的 BlockPos 已有此方法
        let stage = match progress {
            BlockBreakingProgress::Start { stage, .. }
            | BlockBreakingProgress::Update { stage, .. } => stage,
            BlockBreakingProgress::Stop => -1,
        };
        let je_packet = CSetBlockDestroyStage::new(from.entity_id.into(), location, stage as i8);

        self.broadcast_to_chunk_except(chunk_pos, &[from.entity_uuid], &je_packet);
    }

    #[expect(clippy::too_many_lines)]
    pub fn set_block_state(
        self: &Arc<Self>,
        position: &BlockPos,
        block_state_id: BlockStateId,
        flags: BlockFlags,
    ) -> BlockStateId {
        if !self.is_in_build_limit(*position) {
            return Block::AIR.default_state.id;
        }

        let (chunk_coordinate, relative) = position.chunk_and_chunk_relative_position();
        let replaced_block_state_id = self
            .level
            .read_chunk_sync(&chunk_coordinate, |chunk| {
                let replaced_block_state_id = chunk.set_block_absolute_y(
                    relative.x as usize,
                    relative.y,
                    relative.z as usize,
                    block_state_id,
                );
                // 若尚未脏则标记区块为脏
                if replaced_block_state_id != block_state_id && !chunk.is_dirty() {
                    chunk.mark_dirty(true);
                }
                replaced_block_state_id
            })
            .unwrap_or(Block::AIR.default_state.id);

        if !flags.contains(BlockFlags::FORCE_STATE) && replaced_block_state_id == block_state_id {
            return block_state_id;
        }

        let old_block = Block::from_state_id(replaced_block_state_id);
        let new_block = Block::from_state_id(block_state_id);
        let is_new_block = old_block != new_block;
        let block_moved = flags.contains(BlockFlags::MOVED);

        if is_new_block
            && old_block.default_state.block_entity_type != u16::MAX
            && let Some(entity) = self.get_block_entity(position)
        {
            if !flags.contains(BlockFlags::SKIP_BLOCK_ENTITY_REPLACED_CALLBACK) {
                entity.on_block_replaced(self, position);
            }
            self.remove_block_entity(position);
        }

        if is_new_block && (flags.contains(BlockFlags::NOTIFY_NEIGHBORS) || block_moved) {
            self.block_registry.on_state_replaced(
                self,
                old_block,
                position,
                replaced_block_state_id,
                block_moved,
            );
        }

        if !flags.contains(BlockFlags::SKIP_BLOCK_ADDED_CALLBACK) && is_new_block {
            self.block_registry.on_placed(
                self,
                new_block,
                block_state_id,
                position,
                replaced_block_state_id,
                block_moved,
            );
            let new_fluid = self.get_fluid(position);
            self.block_registry.on_placed_fluid(
                self,
                new_fluid,
                block_state_id,
                position,
                replaced_block_state_id,
                block_moved,
            );
        }

        // Level.java 的 setBlock
        if self.get_block_state_id(position) == block_state_id {
            if flags.contains(BlockFlags::NOTIFY_LISTENERS) {
                self.unsent_block_changes
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(*position, block_state_id);
            }

            if flags.contains(BlockFlags::NOTIFY_NEIGHBORS) {
                self.update_neighbors_at(position, old_block, None);
                if block_state_id.has_analog_output_signal() {
                    self.update_neighbour_for_output_signal(position, new_block);
                }
            }

            if !flags.contains(BlockFlags::MOVED) {
                let mut neighbour_update_flags = flags;
                neighbour_update_flags.remove(BlockFlags::NOTIFY_NEIGHBORS);
                neighbour_update_flags.remove(BlockFlags::SKIP_REDSTONE_WIRE_STATE_REPLACEMENT);
                self.block_registry.prepare(
                    self,
                    position,
                    old_block,
                    replaced_block_state_id,
                    neighbour_update_flags,
                );
                self.block_registry
                    .update_neighbors(self, position, neighbour_update_flags);
                self.block_registry.prepare(
                    self,
                    position,
                    new_block,
                    block_state_id,
                    neighbour_update_flags,
                );
            }

            self.villager_poi
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .update_block(*position, new_block);

            if is_new_block {
                let mut poi = self
                    .portal_poi
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if villager_poi::profession_for_block(old_block).is_some() {
                    poi.remove(position);
                }
                if let Some(poi_type) = villager_poi::poi_type_for_block(new_block) {
                    poi.add_with_free_tickets(*position, poi_type, 1);
                }
            }
        }

        let old_state = replaced_block_state_id.to_state();
        let new_state = block_state_id.to_state();
        if papokin_world::lighting::LightEngine::has_different_light_properties(
            old_state, new_state,
        ) {
            self.level
                .light_engine
                .update_lighting_at(&self.level, *position);
        }

        replaced_block_state_id
    }

    pub fn break_block(
        self: &Arc<Self>,
        position: &BlockPos,
        cause: Option<&Arc<Player>>,
        flags: BlockFlags,
    ) -> Option<BlockStateId> {
        if let Some(player) = cause
            && self.is_in_spawn_protection(player, position)
        {
            player.send_system_message(&TextComponent::translate(
                papokin_data::translation::java::BUILD_SPAWN_PROTECTION,
                [TextComponent::text(player.gameprofile.name.clone())],
            ));
            return None;
        }

        let (broken_block, broken_block_state) = self.get_block_and_state(position);
        if broken_block_state.is_air() {
            return None;
        }

        let mut flags = flags;
        if flags.contains(BlockFlags::SKIP_DROPS)
            && cause.is_some_and(|p| p.gamemode.load() == papokin_util::GameMode::Creative)
            && self
                .get_block_entity(position)
                .is_some_and(|entity| entity.drops_for_creative_player())
        {
            flags.remove(BlockFlags::SKIP_DROPS);
        }

        let mut event = BlockBreakEvent::new(
            cause.cloned(),
            broken_block,
            *position,
            0,
            !flags.contains(BlockFlags::SKIP_DROPS),
        );
        if let Some(server) = self.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
        if event.cancelled {
            return None;
        }

        if event.drop {
            flags.remove(BlockFlags::SKIP_DROPS);
        } else {
            flags.insert(BlockFlags::SKIP_DROPS);
        }

        if !flags.contains(BlockFlags::SKIP_DROPS) {
            let tool = cause.as_ref().and_then(|p| {
                let item = p.inventory().held_item();
                if item.is_empty() { None } else { Some(item) }
            });
            let params = crate::world::loot::LootContextParameters {
                tool,
                block_state: Some(broken_block_state),
                position: Some(position.to_f64()),
                killed_by_player: Some(cause.is_some()),
                ..Default::default()
            };
            crate::block::drop_loot(self, broken_block, position, true, &params);
        }

        let new_state_id = if broken_block.is_waterlogged(broken_block_state.id) {
            Block::WATER.default_state.id
        } else {
            Block::AIR.default_state.id
        };

        let broken_state_id = self.set_block_state(position, new_state_id, flags);
        let broken_block = Block::from_state_id(broken_state_id);
        if !broken_block.is_air()
            && broken_state_id != new_state_id
            && broken_block != &Block::FIRE
            && broken_block != &Block::SOUL_FIRE
        {
            let je_packet = CWorldEvent::new(
                WorldEvent::ParticlesDestroyBlock as i32,
                *position,
                broken_state_id.as_u16().into(),
                false,
            );
            let chunk_pos = position.chunk_position();
            if let Some(player) = cause {
                // Java 会预测其自身的破坏效果。
                self.broadcast_to_chunk_except(
                    chunk_pos,
                    &[player.get_entity().entity_uuid],
                    &je_packet,
                );
            } else {
                self.broadcast_to_chunk(chunk_pos, &je_packet);
            }
        }

        Some(broken_state_id)
    }

    #[must_use]
    pub const fn environment_attributes(&self) -> EnvironmentAttributes<'_> {
        EnvironmentAttributes::new(self)
    }

    #[must_use]
    pub fn get_sky_darken(&self) -> i32 {
        let sky_light_level = self.environment_attributes().get_dimension_value_f32(
            papokin_data::environment_attribute::EnvironmentAttribute::GameplaySkyLightLevel,
        );
        (15.0 - sky_light_level).clamp(0.0, 15.0) as i32
    }

    #[must_use]
    pub fn is_bright_outside(&self) -> bool {
        !self.dimension.has_fixed_time && self.get_sky_darken() < 4
    }

    #[must_use]
    pub fn is_dark_outside(&self) -> bool {
        !self.dimension.has_fixed_time && !self.is_bright_outside()
    }

    /// 检查亡灵怪物是否会在日光下燃烧（`EnvironmentAttributes.MONSTERS_BURN`）。
    #[must_use]
    pub fn monsters_burn(&self, pos: &BlockPos) -> bool {
        self.environment_attributes().get_value_bool(
            papokin_data::environment_attribute::EnvironmentAttribute::GameplayMonstersBurn,
            pos,
        )
    }

    /// 检查蜜蜂是否应留在蜂箱/蜂巢内（`EnvironmentAttributes.BEES_STAY_IN_HIVE`）。
    #[must_use]
    pub fn bees_stay_in_hive(&self, pos: &BlockPos) -> bool {
        self.environment_attributes().get_value_bool(
            papokin_data::environment_attribute::EnvironmentAttribute::GameplayBeesStayInHive,
            pos,
        )
    }

    /// 检查嘎枝之心是否处于激活状态（`EnvironmentAttributes.CREAKING_ACTIVE`）。
    #[must_use]
    pub fn creaking_active(&self, pos: &BlockPos) -> bool {
        self.environment_attributes().get_value_bool(
            papokin_data::environment_attribute::EnvironmentAttribute::GameplayCreakingActive,
            pos,
        )
    }

    /// 检查眼眸花是否应当开放（`EnvironmentAttributes.EYEBLOSSOM_OPEN`）。
    #[must_use]
    pub fn eyeblossom_open(&self, pos: &BlockPos) -> Option<bool> {
        self.environment_attributes().get_value_tri_state(
            papokin_data::environment_attribute::EnvironmentAttribute::GameplayEyeblossomOpen,
            pos,
        )
    }

    #[must_use]
    pub fn get_effective_sky_brightness(&self, pos: &BlockPos) -> i32 {
        let sky_light = self.get_sky_light_level(pos) as i32;
        sky_light - self.get_sky_darken()
    }

    #[must_use]
    pub fn get_sun_angle(&self, pos: &BlockPos) -> f32 {
        let sun_angle_deg = self.environment_attributes().get_value_f32(
            papokin_data::environment_attribute::EnvironmentAttribute::VisualSunAngle,
            pos,
        );
        sun_angle_deg * (std::f32::consts::PI / 180.0)
    }

    #[must_use]
    pub fn get_moon_phase(&self) -> MoonPhase {
        self.environment_attributes()
            .get_dimension_value_moon_phase()
    }

    #[must_use]
    pub fn can_pillager_patrol_spawn(&self, pos: &BlockPos) -> bool {
        self.environment_attributes().get_value_bool(
            papokin_data::environment_attribute::EnvironmentAttribute::GameplayCanPillagerPatrolSpawn,
            pos,
        )
    }

    #[must_use]
    pub fn surface_slime_spawn_chance(&self, pos: &BlockPos) -> f32 {
        self.environment_attributes().get_value_f32(
            papokin_data::environment_attribute::EnvironmentAttribute::GameplaySurfaceSlimeSpawnChance,
            pos,
        )
    }

    #[must_use]
    pub fn villager_activity(&self, pos: &BlockPos, baby: bool) -> Activity {
        self.environment_attributes().get_value_activity(baby, pos)
    }

    pub fn get_raw_brightness(&self, pos: &BlockPos, sky_darken: u8) -> u8 {
        let sky_light = self.get_sky_light_level(pos).saturating_sub(sky_darken);
        let block_light = self.get_block_light_level(pos).unwrap_or(0);
        sky_light.max(block_light)
    }

    pub fn get_max_local_raw_brightness(&self, pos: &BlockPos) -> u8 {
        self.get_raw_brightness(pos, self.get_sky_darken() as u8)
    }

    pub fn get_block_light_level(&self, position: &BlockPos) -> Option<u8> {
        self.level
            .light_engine
            .get_block_light_level(&self.level, position)
    }

    pub fn get_sky_light_level(&self, position: &BlockPos) -> u8 {
        self.level
            .light_engine
            .get_sky_light_level(&self.level, position)
    }

    #[must_use]
    pub fn can_see_sky(&self, position: &BlockPos) -> bool {
        position.0.y >= self.dimension.min_y
            && position.0.y < self.dimension.min_y + self.dimension.height
            && self.get_sky_light_level(position) >= MAX_LIGHT_LEVEL
    }

    pub fn set_block_light_level(&self, position: &BlockPos, light_level: u8) {
        let _ = self
            .level
            .light_engine
            .set_block_light_level(&self.level, position, light_level);
    }

    pub fn set_sky_light_level(&self, position: &BlockPos, light_level: u8) {
        let _ = self
            .level
            .light_engine
            .set_sky_light_level(&self.level, position, light_level);
    }

    pub fn get_biome(&self, position: &BlockPos) -> &'static Biome {
        let chunk_pos = position.chunk_position();
        if let Some(chunk) = self.level.loaded_chunks.get(&chunk_pos) {
            let id = chunk
                .section
                .get_rough_biome_absolute_y(
                    (position.0.x & 15) as usize,
                    position.0.y,
                    (position.0.z & 15) as usize,
                )
                .unwrap_or(0);
            Biome::from_id(id).unwrap_or(&Biome::PLAINS)
        } else {
            &Biome::PLAINS
        }
    }

    pub fn schedule_block_tick(
        &self,
        block: &Block,
        block_pos: BlockPos,
        delay: u8,
        priority: TickPriority,
    ) {
        self.level
            .schedule_block_tick(block, block_pos, delay, priority);
    }

    pub fn schedule_fluid_tick(
        &self,
        fluid: &Fluid,
        block_pos: BlockPos,
        delay: u8,
        priority: TickPriority,
    ) {
        self.level
            .schedule_fluid_tick(fluid, block_pos, delay, priority);
    }

    pub fn is_block_tick_scheduled(&self, block_pos: &BlockPos, block: &Block) -> bool {
        self.level.is_block_tick_scheduled(block_pos, block)
    }

    pub fn is_fluid_tick_scheduled(&self, block_pos: &BlockPos, fluid: &Fluid) -> bool {
        self.level.is_fluid_tick_scheduled(block_pos, fluid)
    }

    /// 为所有在给定方块位置处打开着容器的玩家关闭容器界面。
    pub fn close_container_screens_at(&self, position: &BlockPos) {
        let players = self.players.load();
        for player in players.iter() {
            if player.open_container_pos.load() == Some(*position) {
                player.close_handled_screen();
            }
        }
    }

    pub fn drop_stack(self: &Arc<Self>, pos: &BlockPos, stack: ItemStack) {
        if stack.is_empty() {
            return;
        }

        let half_height = f64::from(EntityType::ITEM.dimension[1]) / 2.0;
        let spawn_pos = {
            let mut r = rand::rng();
            Vector3::new(
                f64::from(pos.0.x) + 0.5 + r.random_range(-0.25..0.25),
                f64::from(pos.0.y) + 0.5 + r.random_range(-0.25..0.25) - half_height,
                f64::from(pos.0.z) + 0.5 + r.random_range(-0.25..0.25),
            )
        };

        let entity = Entity::new(self.clone(), spawn_pos, &EntityType::ITEM);
        let mut item_event = crate::plugin::api::events::entity::item_spawn::ItemSpawnEvent::new(
            entity.entity_id,
            spawn_pos,
            stack.item.registry_key.to_string(),
        );
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut item_event);
        }
        if item_event.cancelled {
            return;
        }

        let item_entity = Arc::new(ItemEntity::new(entity, stack));
        self.spawn_entity(item_entity);
    }

    pub fn drop_stack_from_face(
        self: &Arc<Self>,
        pos: &BlockPos,
        face: BlockDirection,
        stack: ItemStack,
    ) {
        if stack.is_empty() {
            return;
        }

        let offset = face.to_offset();
        let step_x = offset.x;
        let step_y = offset.y;
        let step_z = offset.z;

        let half_width = f64::from(EntityType::ITEM.dimension[0]) / 2.0;
        let half_height = f64::from(EntityType::ITEM.dimension[1]) / 2.0;

        let (spawn_pos, velocity) = {
            let mut r = rand::rng();
            let x = f64::from(pos.0.x)
                + 0.5
                + if step_x == 0 {
                    r.random_range(-0.25..0.25)
                } else {
                    f64::from(step_x) * (0.5 + half_width)
                };
            let y = f64::from(pos.0.y)
                + 0.5
                + if step_y == 0 {
                    r.random_range(-0.25..0.25)
                } else {
                    f64::from(step_y) * (0.5 + half_height)
                }
                - half_height;
            let z = f64::from(pos.0.z)
                + 0.5
                + if step_z == 0 {
                    r.random_range(-0.25..0.25)
                } else {
                    f64::from(step_z) * (0.5 + half_width)
                };

            let delta_x = if step_x == 0 {
                r.random_range(-0.1..0.1)
            } else {
                f64::from(step_x) * 0.1
            };
            let delta_y = if step_y == 0 {
                r.random_range(0.0..0.1)
            } else {
                f64::from(step_y) * 0.1 + 0.1
            };
            let delta_z = if step_z == 0 {
                r.random_range(-0.1..0.1)
            } else {
                f64::from(step_z) * 0.1
            };

            (
                Vector3::new(x, y, z),
                Vector3::new(delta_x, delta_y, delta_z),
            )
        };

        let entity = Entity::new(self.clone(), spawn_pos, &EntityType::ITEM);
        let mut item_event = crate::plugin::api::events::entity::item_spawn::ItemSpawnEvent::new(
            entity.entity_id,
            spawn_pos,
            stack.item.registry_key.to_string(),
        );
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut item_event);
        }
        if item_event.cancelled {
            return;
        }

        let item_entity = Arc::new(ItemEntity::new_with_velocity(entity, stack, velocity, 10));
        self.spawn_entity(item_entity);
    }

    pub fn strike_lightning(self: &Arc<Self>, pos: Vector3<f64>, effect_only: bool) {
        use papokin_data::entity::EntityType;
        use uuid::Uuid;
        let server_ref = self.server.upgrade();
        if let Some(server_ref) = server_ref {
            let mut event =
                crate::plugin::api::events::world::lightning_strike::LightningStrikeEvent::new(
                    pos,
                    effect_only,
                );
            server_ref
                .plugin_manager
                .fire_blocking(&server_ref, &mut event);
            if event.cancelled {
                return;
            }
        }

        let lightning = crate::entity::r#type::from_type(
            &EntityType::LIGHTNING_BOLT,
            pos,
            self,
            Uuid::new_v4(),
        );

        if let Some(bolt) = lightning
            .cast_any()
            .downcast_ref::<crate::entity::lightning::LightningBoltEntity>()
        {
            bolt.set_visual_only(effect_only);
        }

        self.spawn_entity(lightning);
    }

    /* ItemScatterer.java */
    pub fn scatter_inventory(
        self: &Arc<Self>,
        position: &BlockPos,
        inventory: &Arc<dyn Inventory>,
    ) {
        for i in 0..inventory.size() {
            self.scatter_stack(
                f64::from(position.0.x),
                f64::from(position.0.y),
                f64::from(position.0.z),
                inventory.remove_stack(i),
            );
        }
    }
    pub fn scatter_stack(self: &Arc<Self>, x: f64, y: f64, z: f64, mut stack: ItemStack) {
        const TRIANGULAR_DEVIATION: f64 = 0.114_850_001_711_398_36;

        const XZ_MODE: f64 = 0.0;
        const Y_MODE: f64 = 0.2;

        let width = f64::from(EntityType::ITEM.dimension[0]);
        let half_width = width / 2.0;
        let spawn_area = 1.0 - width;

        let mut rng = Xoroshiro::from_seed(get_seed());

        // TODO: 在此使用世界随机数：world.random.nextDouble()
        let x = rng.next_f64().mul_add(spawn_area, x.floor()) + half_width;
        let y = rng.next_f64().mul_add(spawn_area, y.floor());
        let z = rng.next_f64().mul_add(spawn_area, z.floor()) + half_width;

        while !stack.is_empty() {
            let item = stack.split((rng.next_bounded_i32(21) + 10) as u8);
            let velocity = Vector3::new(
                rng.next_triangular(XZ_MODE, TRIANGULAR_DEVIATION),
                rng.next_triangular(Y_MODE, TRIANGULAR_DEVIATION),
                rng.next_triangular(XZ_MODE, TRIANGULAR_DEVIATION),
            );

            let entity = Entity::new(self.clone(), Vector3::new(x, y, z), &EntityType::ITEM);
            let entity = Arc::new(ItemEntity::new_with_velocity(entity, item, velocity, 10));
            self.spawn_entity(entity);
        }
    }
    /* End ItemScatterer.java */

    pub fn sync_world_event(&self, world_event: WorldEvent, position: BlockPos, data: i32) {
        let chunk_pos = position.chunk_position();
        self.broadcast_to_chunk(
            chunk_pos,
            &CWorldEvent::new(world_event as i32, position, data, false),
        );
    }

    pub fn sync_global_world_event(&self, world_event: WorldEvent, position: BlockPos, data: i32) {
        self.broadcast_packet_all(&CWorldEvent::new(world_event as i32, position, data, true));
    }

    pub fn set_block_destroy_stage(&self, entity_id: i32, location: BlockPos, stage: i8) {
        let chunk_pos = location.chunk_position();
        let packet = CSetBlockDestroyStage::new(entity_id.into(), location, stage);
        self.broadcast_to_chunk(chunk_pos, &packet);
    }
    #[must_use]
    pub fn is_valid(dest: BlockPos) -> bool {
        Self::is_valid_horizontally(dest) && Self::is_valid_vertically(dest.0.y)
    }
    #[must_use]
    pub fn is_valid_horizontally(dest: BlockPos) -> bool {
        // Note: 30_000_000 无效，但 -30_000_000 有效。
        (-30_000_000..30_000_000).contains(&dest.0.x)
            && (-30_000_000..30_000_000).contains(&dest.0.z)
    }
    #[must_use]
    pub fn is_valid_vertically(y: i32) -> bool {
        // Note: 20_000_000 无效，但 -20_000_000 有效。
        (-20_000_000..20_000_000).contains(&y)
    }
    #[must_use]
    pub fn is_in_build_limit(&self, dest: BlockPos) -> bool {
        self.is_in_height_limit(dest.0.y) && Self::is_valid_horizontally(dest)
    }
    #[must_use]
    pub fn is_in_height_limit(&self, y: i32) -> bool {
        (self.get_bottom_y()..=self.get_top_y()).contains(&y)
    }
    pub const fn get_bottom_y(&self) -> i32 {
        self.dimension.min_y
    }
    pub const fn get_top_y(&self) -> i32 {
        self.dimension.min_y + self.dimension.height - 1
    }
    /// 从方块注册表获取 `Block`。若未找到该方块则返回 `Block::AIR`。
    pub fn get_block(&self, position: &BlockPos) -> &'static Block {
        self.get_block_state_id_if_loaded(position)
            .map_or(&Block::AIR, Block::from_state_id)
    }

    #[must_use]
    pub fn get_block_state_id_if_loaded(&self, position: &BlockPos) -> Option<BlockStateId> {
        if !self.is_in_build_limit(*position) {
            return None;
        }

        let (chunk_coordinate, relative) = position.chunk_and_chunk_relative_position();
        self.level.read_chunk_sync(&chunk_coordinate, |chunk| {
            chunk
                .section
                .get_block_absolute_y(relative.x as usize, relative.y, relative.z as usize)
        })?
    }

    #[must_use]
    pub fn get_block_state_if_loaded(&self, position: &BlockPos) -> Option<&'static BlockState> {
        self.get_block_state_id_if_loaded(position)
            .map(BlockState::from_id)
    }

    #[must_use]
    pub fn is_loaded(&self, position: &BlockPos) -> bool {
        self.get_block_state_id_if_loaded(position).is_some()
    }

    fn get_fluid_from_state_id(id: BlockStateId) -> &'static papokin_data::fluid::Fluid {
        if let Some(fluid) = Fluid::from_state_id(id) {
            return fluid.to_flowing();
        }
        // 这些方块包含水源，但没有 `waterlogged` 属性。
        if matches!(
            id.to_block_id(),
            papokin_data::BlockId::KELP
                | papokin_data::BlockId::KELP_PLANT
                | papokin_data::BlockId::SEAGRASS
                | papokin_data::BlockId::TALL_SEAGRASS
                | papokin_data::BlockId::BUBBLE_COLUMN
        ) || id.is_waterlogged()
        {
            &Fluid::FLOWING_WATER
        } else {
            &Fluid::EMPTY
        }
    }

    fn fluid_state_from_block_state(id: BlockStateId) -> (&'static Fluid, FluidState) {
        let fluid = Self::get_fluid_from_state_id(id);
        let source = if fluid.matches_type(&Fluid::WATER) {
            &Fluid::WATER
        } else if fluid.matches_type(&Fluid::LAVA) {
            &Fluid::LAVA
        } else {
            &Fluid::EMPTY
        };
        let mut state = source.states[source.default_state_index as usize].clone();

        if matches!(
            id.to_block_id(),
            papokin_data::BlockId::WATER | papokin_data::BlockId::LAVA
        ) {
            // LiquidBlock#getFluidState：源，量 7..1，然后下落量为 8。
            // 流体族的第一个状态并非实际方块的状态。
            let level =
                papokin_data::block_properties::WaterLikeProperties::from_state_id(id).level;
            let amount = if level == 0 || level >= 8 {
                8
            } else {
                8 - level
            };
            state.height = f32::from(amount) / 9.0;
            state.level = i16::from(amount);
            state.is_source = level == 0;
            state.is_still = state.is_source;
            state.falling = level >= 8;
            state.block_state_id = papokin_data::block_properties::WaterLikeProperties {
                level: level.min(8),
            }
            .to_state_id(id.to_block());
        }

        // 保留流体回调使用的归一化族，与源状态无关。
        (fluid, state)
    }

    pub fn get_fluid(&self, position: &BlockPos) -> &'static papokin_data::fluid::Fluid {
        let id = self.get_block_state_id(position);
        Self::get_fluid_from_state_id(id)
    }

    pub fn get_block_and_fluid(
        &self,
        position: &BlockPos,
    ) -> (
        &'static papokin_data::Block,
        &'static papokin_data::fluid::Fluid,
    ) {
        let id = self.get_block_state_id(position);
        (id.to_block(), Self::get_fluid_from_state_id(id))
    }

    pub fn get_fluid_and_fluid_state(&self, position: &BlockPos) -> (&'static Fluid, FluidState) {
        let id = self.get_block_state_id(position);
        Self::fluid_state_from_block_state(id)
    }

    /// 当同种流体位于上方时，`FluidState#getHeight` 会计入整个方块。
    /// 将 `state.height` 保留为流动速度计算所用的自身高度。
    pub fn get_fluid_height(&self, position: &BlockPos, fluid: &Fluid, state: &FluidState) -> f32 {
        if state.is_empty {
            0.0
        } else if fluid.matches_type(self.get_fluid(&position.up())) {
            1.0
        } else {
            state.height
        }
    }

    pub fn get_block_state_id(&self, position: &BlockPos) -> BlockStateId {
        self.get_block_state_id_if_loaded(position)
            .unwrap_or(Block::AIR.default_state.id)
    }

    /// 从方块注册表中获取 `BlockState`。若未找到方块状态，则返回空气。
    pub fn get_block_state(&self, position: &BlockPos) -> &'static BlockState {
        let id = self.get_block_state_id(position);
        BlockState::from_id(id)
    }

    /// 从方块注册表中获取方块及方块状态，若未找到方块状态则返回空气
    pub fn get_block_and_state(
        &self,
        position: &BlockPos,
    ) -> (&'static Block, &'static BlockState) {
        let id = self.get_block_state_id(position);
        BlockState::from_id_with_block(id)
    }

    /// 从方块注册表中获取方块及状态 ID，若未找到方块状态则返回空气
    pub fn get_block_and_state_id(&self, position: &BlockPos) -> (&'static Block, BlockStateId) {
        let id = self.get_block_state_id(position);
        (Block::from_state_id(id), id)
    }

    /// 用指定的源方块更新某方块的邻近方块
    pub fn update_neighbors_at(
        self: &Arc<Self>,
        block_pos: &BlockPos,
        source_block: &Block,
        except: Option<BlockDirection>,
    ) {
        for direction in BlockDirection::update_order() {
            if except.is_some_and(|d| d == direction) {
                continue;
            }

            let neighbor_pos = block_pos.offset(direction.to_offset());
            let (neighbor_block, neighbor_fluid) = self.get_block_and_fluid(&neighbor_pos);

            let mut event =
                crate::plugin::api::events::block::block_physics::BlockPhysicsEvent::new(
                    neighbor_pos,
                    *block_pos,
                );
            if let Some(server) = self.server.upgrade() {
                server.plugin_manager.fire_blocking(&server, &mut event);
            }
            if event.cancelled {
                continue;
            }

            if let Some(neighbor_pumpkin_block) =
                self.block_registry.get_pumpkin_block(neighbor_block.id)
            {
                neighbor_pumpkin_block.on_neighbor_update(OnNeighborUpdateArgs {
                    world: self,
                    block: neighbor_block,
                    position: &neighbor_pos,
                    source_block,
                    notify: false,
                });
            }

            if let Some(neighbor_pumpkin_fluid) =
                self.block_registry.get_pumpkin_fluid(neighbor_fluid.id)
            {
                neighbor_pumpkin_fluid.on_neighbor_update(
                    self,
                    neighbor_fluid,
                    &neighbor_pos,
                    false,
                );
            }
        }
    }

    /// 更新某方块的邻近方块
    pub fn update_neighbors(
        self: &Arc<Self>,
        block_pos: &BlockPos,
        except: Option<BlockDirection>,
    ) {
        let source_block = self.get_block(block_pos);
        self.update_neighbors_at(block_pos, source_block, except);
    }

    pub fn update_neighbor(self: &Arc<Self>, neighbor_block_pos: &BlockPos, source_block: &Block) {
        let neighbor_block = self.get_block(neighbor_block_pos);

        let mut event = crate::plugin::api::events::block::block_physics::BlockPhysicsEvent::new(
            *neighbor_block_pos,
            *neighbor_block_pos,
        );
        if let Some(server) = self.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
        if event.cancelled {
            return;
        }

        if let Some(neighbor_pumpkin_block) =
            self.block_registry.get_pumpkin_block(neighbor_block.id)
        {
            neighbor_pumpkin_block.on_neighbor_update(OnNeighborUpdateArgs {
                world: self,
                block: neighbor_block,
                position: neighbor_block_pos,
                source_block,
                notify: false,
            });
        }
    }

    pub fn update_neighbour_for_output_signal(
        self: &Arc<Self>,
        pos: &BlockPos,
        changed_block: &Block,
    ) {
        for direction in BlockDirection::horizontal() {
            let mut relative_pos = pos.offset(direction.to_offset());
            if self.is_loaded(&relative_pos) {
                let state = self.get_block_state(&relative_pos);
                if state.id.to_block() == &Block::COMPARATOR {
                    self.update_neighbor(&relative_pos, changed_block);
                } else if state.is_solid_block() {
                    relative_pos = relative_pos.offset(direction.to_offset());
                    if self.is_loaded(&relative_pos) {
                        let second_state = self.get_block_state(&relative_pos);
                        if second_state.id.to_block() == &Block::COMPARATOR {
                            self.update_neighbor(&relative_pos, changed_block);
                        }
                    }
                }
            }
        }
    }

    /// 按列表顺序为本刻中发生变化的方块实体发送输出信号更新。
    fn flush_comparator_updates(self: &Arc<Self>, block_entities: &[Arc<dyn BlockEntity>]) {
        for be in block_entities {
            // 原版 `BlockEntity.setChanged` -> `Level.updateNeighbourForOutputSignal`。
            if !be.is_comparator_dirty() {
                continue;
            }
            be.clear_comparator_dirty();
            let pos = be.get_position();
            // 该列表是快照，因此此时区块可能已经不存在。
            if let Some(state_id) = self.get_block_state_id_if_loaded(&pos) {
                self.update_neighbour_for_output_signal(&pos, state_id.to_block());
            }
        }
    }

    pub fn update_from_neighbor_shapes(
        self: &Arc<Self>,
        state_id: BlockStateId,
        pos: &BlockPos,
    ) -> BlockStateId {
        let mut current_state_id = state_id;
        let block = Block::from_state_id(state_id);
        for direction in BlockDirection::all() {
            let neighbor_pos = pos.offset(direction.to_offset());
            let neighbor_state_id = self.get_block_state_id(&neighbor_pos);
            current_state_id = self.block_registry.get_state_for_neighbor_update(
                self,
                block,
                current_state_id,
                pos,
                direction,
                &neighbor_pos,
                neighbor_state_id,
            );
        }
        current_state_id
    }

    pub fn replace_with_state_for_neighbor_update(
        self: &Arc<Self>,
        block_pos: &BlockPos,
        direction: BlockDirection,
        flags: BlockFlags,
    ) {
        let (block, block_state_id) = self.get_block_and_state_id(block_pos);

        if flags.contains(BlockFlags::SKIP_REDSTONE_WIRE_STATE_REPLACEMENT)
            && *block == Block::REDSTONE_WIRE
        {
            return;
        }

        let neighbor_pos = block_pos.offset(direction.to_offset());
        let neighbor_state_id = self.get_block_state_id(&neighbor_pos);

        let new_state_id = self.block_registry.get_state_for_neighbor_update(
            self,
            block,
            block_state_id,
            block_pos,
            direction,
            &neighbor_pos,
            neighbor_state_id,
        );

        if new_state_id != block_state_id {
            if is_air(new_state_id) {
                self.break_block(block_pos, None, flags | BlockFlags::NOTIFY_ALL);
            } else {
                self.set_block_state(block_pos, new_state_id, flags);
            }
        }
    }

    /// 返回怪物是否可以在世界中生成
    pub fn should_spawn_monsters(&self) -> bool {
        let level_data = self.level_info.load();
        level_data.game_rules.spawn_mobs
            && level_data.game_rules.spawn_monsters
            && level_data.difficulty != Difficulty::Peaceful
    }

    pub fn get_block_entity(&self, block_pos: &BlockPos) -> Option<Arc<dyn BlockEntity>> {
        let chunk_pos = block_pos.chunk_position();
        if let Some(entity) = self
            .block_entities
            .get(&chunk_pos)
            .and_then(|m| m.get(block_pos).cloned())
        {
            return Some(entity);
        }

        let nbt = self
            .level
            .read_chunk_sync(&chunk_pos, |chunk| {
                chunk
                    .pending_block_entities
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get(block_pos)
                    .cloned()
            })
            .flatten()?;
        if let Some(custom_data) = nbt
            .get_compound("PumpkinCustomData")
            .or_else(|| nbt.get_compound("BukkitValues"))
        {
            self.custom_block_entity_data
                .insert(*block_pos, custom_data.clone());
        }
        let entity = block_entity_from_nbt(&nbt)?;
        self.block_entities
            .entry(chunk_pos)
            .or_default()
            .insert(*block_pos, entity.clone());
        Some(entity)
    }

    pub fn add_block_entity(&self, block_entity: Arc<dyn BlockEntity>) {
        let block_pos = block_entity.get_position();
        let chunk_pos = block_pos.chunk_position();
        let block_entity_nbt = block_entity.chunk_data_nbt();
        let entity_id = block_entity.resource_location().to_string();

        if let Some(nbt) = &block_entity_nbt {
            let bytes = papokin_nbt::Nbt::from(nbt.clone()).write_unnamed();
            self.broadcast_to_chunk(
                chunk_pos,
                &CBlockEntityData::new(
                    block_entity.get_position(),
                    VarInt(block_entity.get_id() as i32),
                    bytes.as_ref().into(),
                ),
            );
        }

        self.block_entities
            .entry(chunk_pos)
            .or_default()
            .insert(block_pos, block_entity);

        if let Some(nbt) = block_entity_nbt {
            let mut full_nbt = nbt;
            full_nbt.put_string("id", entity_id);
            full_nbt.put_int("x", block_pos.0.x);
            full_nbt.put_int("y", block_pos.0.y);
            full_nbt.put_int("z", block_pos.0.z);
            self.add_block_entity_nbt(block_pos, &full_nbt);
        }

        self.level.read_chunk_sync(&chunk_pos, |chunk| {
            chunk.mark_dirty(true);
        });
    }

    pub(crate) fn add_block_entity_nbt(&self, block_pos: BlockPos, nbt: &NbtCompound) {
        if self
            .level
            .read_chunk_sync(&block_pos.chunk_position(), |chunk| {
                chunk
                    .pending_block_entities
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(block_pos, nbt.clone());
                chunk.mark_dirty(true);
            })
            .is_some()
        {
            self.pending_block_entity_migrations
                .push(block_pos.chunk_position());
        }
    }

    pub fn remove_block_entity(&self, block_pos: &BlockPos) {
        let chunk_pos = block_pos.chunk_position();
        let removed =
            self.block_entities
                .get_mut(&chunk_pos)
                .is_some_and(|mut chunk_block_entities| {
                    chunk_block_entities.remove(block_pos).is_some()
                });
        if removed {
            self.custom_block_entity_data.remove(block_pos);
            // 当区块的最后一个方块实体消失后，丢弃该区块的映射
            self.block_entities
                .remove_if(&chunk_pos, |_, entities| entities.is_empty());
            self.level.read_chunk_sync(&chunk_pos, |chunk| {
                chunk.mark_dirty(true);
            });
        }
    }

    fn migrate_pending_block_entities(&self, chunk_pos: Vector2<i32>) {
        let positions: Vec<BlockPos> = self
            .level
            .read_chunk_sync(&chunk_pos, |chunk| {
                chunk
                    .pending_block_entities
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .keys()
                    .copied()
                    .collect()
            })
            .unwrap_or_default();
        for pos in positions {
            let already_loaded = self
                .block_entities
                .get(&chunk_pos)
                .is_some_and(|m| m.contains_key(&pos));
            if !already_loaded && let Some(entity) = self.get_block_entity(&pos) {
                self.update_block_entity(&entity);
            }
        }
    }

    pub fn update_block_entity(&self, block_entity: &Arc<dyn BlockEntity>) {
        let block_pos = block_entity.get_position();
        let chunk_pos = block_pos.chunk_position();
        let block_entity_nbt = block_entity.chunk_data_nbt();

        if let Some(nbt) = &block_entity_nbt {
            let bytes = papokin_nbt::Nbt::from(nbt.clone()).write_unnamed();
            self.broadcast_to_chunk(
                chunk_pos,
                &CBlockEntityData::new(
                    block_entity.get_position(),
                    VarInt(block_entity.get_id() as i32),
                    bytes.as_ref().into(),
                ),
            );
            let mut full_nbt = nbt.clone();
            full_nbt.put_string("id", block_entity.resource_location().to_string());
            let pos = block_entity.get_position();
            full_nbt.put_int("x", pos.0.x);
            full_nbt.put_int("y", pos.0.y);
            full_nbt.put_int("z", pos.0.z);
            self.add_block_entity_nbt(block_pos, &full_nbt);
        }
        self.level.read_chunk_sync(&chunk_pos, |chunk| {
            chunk.mark_dirty(true);
        });
    }

    #[must_use]
    pub fn intersects_aabb_with_hit(
        from: Vector3<f64>,
        to: Vector3<f64>,
        min: Vector3<f64>,
        max: Vector3<f64>,
    ) -> Option<(f64, BlockDirection, Vector3<f64>)> {
        let dir = to.sub(&from);
        let mut tmin: f64 = 0.0;
        let mut tmax: f64 = 1.0;

        let mut hit_axis = None;
        let mut hit_is_min = false;

        macro_rules! check_axis {
            ($axis:ident, $dir_axis:ident, $min_axis:ident, $max_axis:ident) => {{
                if dir.$dir_axis.abs() < 1e-8 {
                    if from.$dir_axis < min.$min_axis || from.$dir_axis > max.$max_axis {
                        return None;
                    }
                } else {
                    let inv_d = 1.0 / dir.$dir_axis;
                    let t_near = (min.$min_axis - from.$dir_axis) * inv_d;
                    let t_far = (max.$max_axis - from.$dir_axis) * inv_d;

                    let (t_entry, t_exit, is_min_face) = if inv_d >= 0.0 {
                        (t_near, t_far, true)
                    } else {
                        (t_far, t_near, false)
                    };

                    if t_entry > tmin {
                        tmin = t_entry;
                        hit_axis = Some(stringify!($axis));
                        hit_is_min = is_min_face;
                    }
                    tmax = tmax.min(t_exit);
                    if tmax < tmin {
                        return None;
                    }
                }
            }};
        }

        check_axis!(x, x, x, x);
        check_axis!(y, y, y, y);
        check_axis!(z, z, z, z);

        if tmax < 0.0 || tmin > 1.0 {
            return None;
        }

        let direction = match (hit_axis, hit_is_min) {
            (Some("x"), true) => BlockDirection::West,
            (Some("x"), false) => BlockDirection::East,
            (Some("y"), true) => BlockDirection::Down,
            (Some("y"), false) => BlockDirection::Up,
            (Some("z"), true) => BlockDirection::North,
            (Some("z"), false) => BlockDirection::South,
            _ => {
                if dir.y < 0.0 {
                    BlockDirection::Up
                } else if dir.y > 0.0 {
                    BlockDirection::Down
                } else {
                    BlockDirection::North
                }
            }
        };

        let t_hit = tmin.max(0.0);
        let hit_pos = from + dir * t_hit;
        Some((t_hit, direction, hit_pos))
    }

    /// 使用 `state` 的轮廓形状对线段进行裁剪。没有形状的方块
    /// 上方均为空气，无法被命中。
    fn clip_outline_shapes(
        state: &BlockState,
        block_pos: &BlockPos,
        from: Vector3<f64>,
        to: Vector3<f64>,
    ) -> Option<(BlockDirection, Vector3<f64>)> {
        let mut closest_hit: Option<(f64, BlockDirection, Vector3<f64>)> = None;

        for shape in state.get_block_outline_shapes_at(block_pos) {
            let world_min = shape.min.add(&block_pos.0.to_f64());
            let world_max = shape.max.add(&block_pos.0.to_f64());

            if let Some((t, dir, hit_pos)) =
                Self::intersects_aabb_with_hit(from, to, world_min, world_max)
                && closest_hit
                    .as_ref()
                    .is_none_or(|(closest_t, _, _)| t < *closest_t)
            {
                closest_hit = Some((t, dir, hit_pos));
            }
        }

        closest_hit.map(|(_, dir, hit_pos)| (dir, hit_pos))
    }

    pub fn ray_outline_check_detailed(
        &self,
        block_pos: &BlockPos,
        from: Vector3<f64>,
        to: Vector3<f64>,
    ) -> Option<(BlockDirection, Vector3<f64>)> {
        Self::clip_outline_shapes(self.get_block_state(block_pos), block_pos, from, to)
    }

    fn ray_outline_check(
        &self,
        block_pos: &BlockPos,
        from: Vector3<f64>,
        to: Vector3<f64>,
    ) -> (bool, Option<BlockDirection>) {
        if let Some((dir, _)) = self.ray_outline_check_detailed(block_pos, from, to) {
            (true, Some(dir))
        } else {
            let state = self.get_block_state(block_pos);
            if state.outline_shapes.is_empty() {
                (true, None)
            } else {
                (false, None)
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn ray_trace_block(
        &self,
        start_pos: Vector3<f64>,
        end_pos: Vector3<f64>,
        include_fluids: bool,
    ) -> Option<(BlockPos, BlockDirection, Vector3<f64>)> {
        if start_pos == end_pos {
            return None;
        }

        let adjust = -1.0e-7f64;
        let to = end_pos.lerp(&start_pos, adjust);
        let from = start_pos.lerp(&end_pos, adjust);

        let mut block = BlockPos::floored(from.x, from.y, from.z);

        let state = self.get_block_state(&block);
        let valid_start = if include_fluids {
            !state.is_air()
        } else {
            !state.is_air() && !state.is_liquid()
        };
        if valid_start
            && let Some((dir, hit_pos)) = self.ray_outline_check_detailed(&block, from, to)
        {
            return Some((block, dir, hit_pos));
        }

        let difference = to.sub(&from);
        let step = difference.sign();

        let delta = Vector3::new(
            if step.x == 0 {
                f64::MAX
            } else {
                (f64::from(step.x)) / difference.x
            },
            if step.y == 0 {
                f64::MAX
            } else {
                (f64::from(step.y)) / difference.y
            },
            if step.z == 0 {
                f64::MAX
            } else {
                (f64::from(step.z)) / difference.z
            },
        );

        let mut next = Vector3::new(
            delta.x
                * (if step.x > 0 {
                    1.0 - (from.x - from.x.floor())
                } else {
                    from.x - from.x.floor()
                }),
            delta.y
                * (if step.y > 0 {
                    1.0 - (from.y - from.y.floor())
                } else {
                    from.y - from.y.floor()
                }),
            delta.z
                * (if step.z > 0 {
                    1.0 - (from.z - from.z.floor())
                } else {
                    from.z - from.z.floor()
                }),
        );

        while next.x <= 1.0 || next.y <= 1.0 || next.z <= 1.0 {
            let block_direction = match (next.x, next.y, next.z) {
                (x, y, z) if x < y && x < z => {
                    block.0.x += step.x;
                    next.x += delta.x;
                    if step.x > 0 {
                        BlockDirection::West
                    } else {
                        BlockDirection::East
                    }
                }
                (_, y, z) if y < z => {
                    block.0.y += step.y;
                    next.y += delta.y;
                    if step.y > 0 {
                        BlockDirection::Down
                    } else {
                        BlockDirection::Up
                    }
                }
                _ => {
                    block.0.z += step.z;
                    next.z += delta.z;
                    if step.z > 0 {
                        BlockDirection::North
                    } else {
                        BlockDirection::South
                    }
                }
            };

            let state = self.get_block_state(&block);
            let hit = if include_fluids {
                !state.is_air()
            } else {
                !state.is_air() && !state.is_liquid()
            };

            if hit {
                if let Some((dir, hit_pos)) = self.ray_outline_check_detailed(&block, from, to) {
                    return Some((block, dir, hit_pos));
                }
                let block_min = block.0.to_f64();
                let block_max = block_min.add_raw(1.0, 1.0, 1.0);
                if let Some((_, dir, hit_pos)) =
                    Self::intersects_aabb_with_hit(from, to, block_min, block_max)
                {
                    return Some((block, dir, hit_pos));
                }
                return Some((block, block_direction, to));
            }
        }

        None
    }

    pub fn ray_trace_entities(
        &self,
        start: Vector3<f64>,
        end: Vector3<f64>,
    ) -> Vec<(Arc<dyn EntityBase>, Vector3<f64>, f64)> {
        if start == end {
            return Vec::new();
        }

        let min_x = start.x.min(end.x) - 1.0;
        let max_x = start.x.max(end.x) + 1.0;
        let min_y = start.y.min(end.y) - 1.0;
        let max_y = start.y.max(end.y) + 1.0;
        let min_z = start.z.min(end.z) - 1.0;
        let max_z = start.z.max(end.z) + 1.0;
        let ray_box = BoundingBox::new(
            Vector3::new(min_x, min_y, min_z),
            Vector3::new(max_x, max_y, max_z),
        );

        let mut hits = Vec::new();

        for entity in self.entities.load().iter() {
            let bb = entity.get_entity().bounding_box.load();
            if bb.intersects(&ray_box)
                && let Some((t, _, hit_pos)) =
                    Self::intersects_aabb_with_hit(start, end, bb.min, bb.max)
            {
                let distance = (hit_pos - start).length();
                hits.push((entity.clone(), hit_pos, distance, t));
            }
        }

        for player in self.players.load().iter() {
            let bb = player.get_entity().bounding_box.load();
            if bb.intersects(&ray_box)
                && let Some((t, _, hit_pos)) =
                    Self::intersects_aabb_with_hit(start, end, bb.min, bb.max)
            {
                let distance = (hit_pos - start).length();
                hits.push((player.clone() as Arc<dyn EntityBase>, hit_pos, distance, t));
            }
        }

        hits.sort_by(|a, b| a.3.partial_cmp(&b.3).unwrap_or(std::cmp::Ordering::Equal));
        hits.into_iter()
            .map(|(ent, hit_pos, dist, _)| (ent, hit_pos, dist))
            .collect()
    }

    ///返回从 `start` 到 `end` 的线段命中的最近实体，若无则返回 `None`
    /// `None`。它是 [`Self::ray_trace_entities`] 的便捷封装。
    pub fn ray_trace_entity(
        &self,
        start: Vector3<f64>,
        end: Vector3<f64>,
    ) -> Option<(Arc<dyn EntityBase>, Vector3<f64>, f64)> {
        self.ray_trace_entities(start, end).into_iter().next()
    }

    /// 从 `start_pos` 到 `end_pos` 追踪方块网格（原版
    /// `Block.clip` 语义），并返回射线实际
    /// 穿过其轮廓发生碰撞且 `hit_check` 返回
    /// true，以及该命中所报告的方向。起点
    /// 方块也像其他方块一样被检测；由于射线从其内部开始，
    /// 报告的方向时，存在回退而非真实的进入面。
    ///当未命中任何对象，或射线起点与终点处于同一方块内时，返回 `None`
    /// 同一个方块。
    pub fn raycast(
        self: &Arc<Self>,
        start_pos: Vector3<f64>,
        end_pos: Vector3<f64>,
        hit_check: impl Fn(&BlockPos, &Arc<Self>) -> bool,
    ) -> Option<(BlockPos, BlockDirection)> {
        if start_pos == end_pos {
            return None;
        }

        let adjust = -1.0e-7f64;
        let to = end_pos.lerp(&start_pos, adjust);
        let from = start_pos.lerp(&end_pos, adjust);

        let mut block = BlockPos::floored(from.x, from.y, from.z);

        if hit_check(&block, self) {
            let (collision, direction) = self.ray_outline_check(&block, from, to);
            if let Some(dir) = direction
                && collision
            {
                return Some((block, dir));
            }
        }

        let difference = to.sub(&from);

        let step = difference.sign();

        let delta = Vector3::new(
            if step.x == 0 {
                f64::MAX
            } else {
                (f64::from(step.x)) / difference.x
            },
            if step.y == 0 {
                f64::MAX
            } else {
                (f64::from(step.y)) / difference.y
            },
            if step.z == 0 {
                f64::MAX
            } else {
                (f64::from(step.z)) / difference.z
            },
        );

        let mut next = Vector3::new(
            delta.x
                * (if step.x > 0 {
                    1.0 - (from.x - from.x.floor())
                } else {
                    from.x - from.x.floor()
                }),
            delta.y
                * (if step.y > 0 {
                    1.0 - (from.y - from.y.floor())
                } else {
                    from.y - from.y.floor()
                }),
            delta.z
                * (if step.z > 0 {
                    1.0 - (from.z - from.z.floor())
                } else {
                    from.z - from.z.floor()
                }),
        );

        while next.x <= 1.0 || next.y <= 1.0 || next.z <= 1.0 {
            let block_direction = match (next.x, next.y, next.z) {
                (x, y, z) if x < y && x < z => {
                    block.0.x += step.x;
                    next.x += delta.x;
                    if step.x > 0 {
                        BlockDirection::West
                    } else {
                        BlockDirection::East
                    }
                }
                (_, y, z) if y < z => {
                    block.0.y += step.y;
                    next.y += delta.y;
                    if step.y > 0 {
                        BlockDirection::Down
                    } else {
                        BlockDirection::Up
                    }
                }
                _ => {
                    block.0.z += step.z;
                    next.z += delta.z;
                    if step.z > 0 {
                        BlockDirection::North
                    } else {
                        BlockDirection::South
                    }
                }
            };

            if hit_check(&block, self) {
                let (collision, direction) = self.ray_outline_check(&block, from, to);
                if collision {
                    if let Some(dir) = direction {
                        return Some((block, dir));
                    }
                    return Some((block, block_direction));
                }
            }
        }

        None
    }

    /// 向当前已加载目标区块的所有玩家广播数据包。
    pub fn broadcast_to_chunk<P: ClientPacket>(&self, chunk_pos: Vector2<i32>, packet: &P) {
        let players = self.players.load();

        let recipients = players.iter().filter(|p| {
            p.watched_section
                .load()
                .is_within_distance(chunk_pos.x, chunk_pos.y)
        });

        let recipients_by_version = Self::collect_java_recipients_by_version(recipients);
        Self::broadcast_java_grouped(packet, recipients_by_version);
    }

    /// 向区块观察者广播数据包，但排除特定玩家。
    pub fn broadcast_to_chunk_except<P: ClientPacket>(
        &self,
        chunk_pos: Vector2<i32>,
        except: &[uuid::Uuid],
        packet: &P,
    ) {
        let players = self.players.load();

        let recipients = players.iter().filter(|p| {
            if except.contains(&p.get_entity().entity_uuid) {
                return false;
            }
            p.watched_section
                .load()
                .is_within_distance(chunk_pos.x, chunk_pos.y)
        });

        let recipients_by_version = Self::collect_java_recipients_by_version(recipients);
        Self::broadcast_java_grouped(packet, recipients_by_version);
    }

    pub fn emit_game_event(self: &Arc<Self>, event_key: impl Into<String>, position: Vector3<f64>) {
        let mut receive_event =
            crate::plugin::api::events::block::block_receive_game::BlockReceiveGameEvent::new(
                position.to_block_pos(),
                self.clone(),
                event_key.into(),
                None,
            );
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut receive_event);
        }
        if receive_event.cancelled {
            return;
        }

        let mut event = crate::plugin::api::events::world::generic_game::GenericGameEvent::new(
            receive_event.game_event,
            position,
        );
        if let Some(server) = self.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
    }

    pub async fn unload(self: &Arc<Self>) {
        let mut event =
            crate::plugin::api::events::world::world_load::WorldUnloadEvent::new(self.clone());
        if let Some(server) = self.server.upgrade() {
            server.plugin_manager.fire(&server, &mut event).await;
        }
    }

    pub async fn save(&self) {
        let entities = self.entities.load_full();
        self.save_entities_by_chunk(&entities, self.level.live_entity_chunk_positions())
            .await;

        let chunks: Vec<Vector2<i32>> = self
            .block_entities
            .iter()
            .map(|chunk_block_entities| *chunk_block_entities.key())
            .collect();
        for chunk_pos in chunks {
            self.save_block_entities(chunk_pos);
        }

        if let Ok(mut portal_poi) = self.portal_poi.try_lock() {
            let _ = portal_poi.save_all();
        }

        {
            let custom_data = self
                .custom_data
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !custom_data.is_empty() {
                let custom_data_path = self
                    .level
                    .level_folder
                    .root_folder
                    .join("pumpkin_custom_data.nbt");
                let nbt = papokin_nbt::Nbt::from(custom_data.clone());
                let _ = std::fs::write(custom_data_path, nbt.write());
            }
        }

        self.level
            .should_save
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.level.level_channel.notify();

        let mut save_event = crate::plugin::api::events::world::world_save::WorldSaveEvent::new(
            format!("{:?}", self.dimension),
        );
        if let Some(server) = self.server.upgrade() {
            server.plugin_manager.fire(&server, &mut save_event).await;
        }
    }

    pub fn set_custom_data(&self, namespace: &str, key: &str, value: papokin_nbt::tag::NbtTag) {
        let mut custom_data = self
            .custom_data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let mut namespace_data = custom_data
            .child_tags
            .remove(namespace)
            .and_then(|tag| match tag {
                papokin_nbt::tag::NbtTag::Compound(compound) => Some(compound),
                _ => None,
            })
            .unwrap_or_default();

        namespace_data.child_tags.insert(key.into(), value);
        custom_data.child_tags.insert(
            namespace.into(),
            papokin_nbt::tag::NbtTag::Compound(namespace_data),
        );
    }

    pub fn get_custom_data(&self, namespace: &str, key: &str) -> Option<papokin_nbt::tag::NbtTag> {
        let custom_data = self
            .custom_data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        custom_data
            .get(namespace)?
            .extract_compound()?
            .get(key)
            .cloned()
    }

    pub fn remove_custom_data(&self, namespace: &str, key: &str) {
        let mut custom_data = self
            .custom_data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let Some(papokin_nbt::tag::NbtTag::Compound(mut namespace_data)) =
            custom_data.child_tags.remove(namespace)
        else {
            return;
        };

        namespace_data.child_tags.remove(key);
        if !namespace_data.is_empty() {
            custom_data.child_tags.insert(
                namespace.into(),
                papokin_nbt::tag::NbtTag::Compound(namespace_data),
            );
        }
    }

    pub fn has_custom_data(&self, namespace: &str, key: &str) -> bool {
        self.get_custom_data(namespace, key).is_some()
    }

    pub fn set_block_entity_custom_data(
        &self,
        pos: &BlockPos,
        namespace: &str,
        key: &str,
        value: papokin_nbt::tag::NbtTag,
    ) {
        let mut entry = self.custom_block_entity_data.entry(*pos).or_default();
        let mut namespace_data = entry
            .child_tags
            .remove(namespace)
            .and_then(|tag| match tag {
                papokin_nbt::tag::NbtTag::Compound(compound) => Some(compound),
                _ => None,
            })
            .unwrap_or_default();

        namespace_data.child_tags.insert(key.into(), value);
        entry.child_tags.insert(
            namespace.into(),
            papokin_nbt::tag::NbtTag::Compound(namespace_data),
        );
    }

    pub fn get_block_entity_custom_data(
        &self,
        pos: &BlockPos,
        namespace: &str,
        key: &str,
    ) -> Option<papokin_nbt::tag::NbtTag> {
        self.custom_block_entity_data
            .get(pos)?
            .get(namespace)?
            .extract_compound()?
            .get(key)
            .cloned()
    }

    pub fn remove_block_entity_custom_data(&self, pos: &BlockPos, namespace: &str, key: &str) {
        if let Some(mut entry) = self.custom_block_entity_data.get_mut(pos) {
            let Some(papokin_nbt::tag::NbtTag::Compound(mut namespace_data)) =
                entry.child_tags.remove(namespace)
            else {
                return;
            };

            namespace_data.child_tags.remove(key);
            if !namespace_data.is_empty() {
                entry.child_tags.insert(
                    namespace.into(),
                    papokin_nbt::tag::NbtTag::Compound(namespace_data),
                );
            }
        }
    }

    pub fn has_block_entity_custom_data(&self, pos: &BlockPos, namespace: &str, key: &str) -> bool {
        self.get_block_entity_custom_data(pos, namespace, key)
            .is_some()
    }

    pub fn populate_chunk(&self, chunk_pos: Vector2<i32>) {
        let mut populate_event =
            crate::plugin::api::events::world::chunk_populate::ChunkPopulateEvent::new(chunk_pos);
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut populate_event);
        }
    }

    pub fn unload_chunk(&self, chunk_pos: Vector2<i32>) {
        let mut unload_event =
            crate::plugin::api::events::world::chunk_unload::ChunkUnloadEvent::new(chunk_pos);
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut unload_event);
        }
    }

    pub fn load_entities(&self, chunk_pos: Vector2<i32>, entity_count: usize) {
        let mut load_event =
            crate::plugin::api::events::world::entities_load::EntitiesLoadEvent::new(
                chunk_pos,
                entity_count,
            );
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut load_event);
        }
    }

    pub fn unload_entities(&self, chunk_pos: Vector2<i32>, entity_count: usize) {
        let mut unload_event =
            crate::plugin::api::events::world::entities_unload::EntitiesUnloadEvent::new(
                chunk_pos,
                entity_count,
            );
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut unload_event);
        }
    }

    /// 战利品生成事件。返回 false 表示插件已取消本次生成，调用方应跳过战利品产出。
    pub fn generate_loot(&self, loot_table: &str) -> bool {
        let mut loot_event =
            crate::plugin::api::events::world::loot_generate::LootGenerateEvent::new(
                loot_table.to_string(),
            );
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut loot_event);
            if loot_event.cancelled {
                return false;
            }
        }
        true
    }

    pub fn skip_time(&self, skip_amount: i64) {
        let mut time_event =
            crate::plugin::api::events::world::time_skip::TimeSkipEvent::new(skip_amount);
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut time_event);
        }
    }

    pub fn trigger_raid(&self, pos: BlockPos) {
        let mut raid_event =
            crate::plugin::api::events::raid::raid_trigger::RaidTriggerEvent::new(pos);
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut raid_event);
        }
    }

    pub fn spawn_raid_wave(&self, wave: u32, pos: BlockPos) {
        let mut wave_event =
            crate::plugin::api::events::raid::raid_spawn_wave::RaidSpawnWaveEvent::new(wave, pos);
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut wave_event);
        }
    }

    pub fn finish_raid(&self, victory: bool) {
        let mut raid_event =
            crate::plugin::api::events::raid::raid_finish::RaidFinishEvent::new(victory);
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut raid_event);
        }
    }

    pub fn stop_raid(&self, reason: String) {
        let mut raid_event =
            crate::plugin::api::events::raid::raid_stop::RaidStopEvent::new(reason);
        if let Some(server) = self.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut raid_event);
        }
    }

    pub fn async_structure_generate(
        &self,
        world_name: String,
        structure_name: String,
        pos: BlockPos,
    ) {
        let mut event = crate::plugin::api::events::world::async_structure_generate::AsyncStructureGenerateEvent::new(
            world_name,
            structure_name,
            pos,
        );
        if let Some(server) = self.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
    }

    pub fn async_structure_spawn(&self, world_name: String, structure_name: String, pos: BlockPos) {
        let mut event =
            crate::plugin::api::events::world::async_structure_spawn::AsyncStructureSpawnEvent::new(
                world_name,
                structure_name,
                pos,
            );
        if let Some(server) = self.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
    }
}

impl BlockAccessor for World {
    fn get_block(&self, position: &BlockPos) -> &'static Block {
        self.get_block_state_id_if_loaded(position)
            .map_or(&Block::AIR, Block::from_state_id)
    }
    fn get_block_state(&self, position: &BlockPos) -> &'static BlockState {
        self.get_block_state_id_if_loaded(position)
            .map_or(Block::AIR.default_state, BlockState::from_id)
    }

    fn get_block_state_id(&self, position: &BlockPos) -> BlockStateId {
        self.get_block_state_id_if_loaded(position)
            .unwrap_or(Block::AIR.default_state.id)
    }

    fn get_block_and_state(&self, position: &BlockPos) -> (&'static Block, &'static BlockState) {
        let id = self
            .get_block_state_id_if_loaded(position)
            .unwrap_or(Block::AIR.default_state.id);
        BlockState::from_id_with_block(id)
    }
}

pub struct WorldPortal(pub Arc<World>);

// 纯粹之美 :cap:
impl WorldPortalExt for WorldPortal {
    fn can_place_at(
        &self,
        block: &papokin_data::Block,
        state: &BlockState,
        block_accessor: &dyn BlockAccessor,
        block_pos: &BlockPos,
    ) -> bool {
        self.0.block_registry.can_place_at(
            None,
            Some(&self.0),
            block_accessor,
            None,
            block,
            state,
            block_pos,
            None,
            None,
        )
    }

    fn mirror(&self, block: &Block, state_id: BlockStateId, mirror: Mirror) -> &'static BlockState {
        self.0.block_registry.mirror(block, state_id, mirror)
    }

    fn rotate(
        &self,
        block: &Block,
        state_id: BlockStateId,
        rotation: Rotation,
    ) -> &'static BlockState {
        self.0.block_registry.rotate(block, state_id, rotation)
    }

    fn spawn_mobs_for_chunk_generation(
        &self,
        cache: &mut dyn GenerationCache,
        biome: &'static Biome,
        chunk_x: i32,
        chunk_z: i32,
    ) {
        natural_spawner::spawn_mobs_for_chunk_generation(&self.0, cache, biome, chunk_x, chunk_z);
    }

    fn spawn_structure_entities(&self, entities: Vec<NbtCompound>) {
        for nbt in entities {
            let Some(id) = nbt.get_string("id") else {
                continue;
            };
            let Some(entity_type) =
                EntityType::from_name(id.strip_prefix("minecraft:").unwrap_or(id))
            else {
                warn!("未知的结构实体类型：{id}");
                continue;
            };
            let entity = from_type(
                entity_type,
                Vector3::new(0.0, 0.0, 0.0),
                &self.0,
                Uuid::new_v4(),
            );
            entity.get_entity().read_nbt_non_mut(&nbt);
            entity.read_nbt_non_mut(&nbt);
            self.0.spawn_entity(entity);
        }
    }
}

struct CubicCurve {
    a: f32,
    b: f32,
    c: f32,
}

impl CubicCurve {
    fn new(v1: f32, v2: f32) -> Self {
        Self {
            a: 3.0 * v1 - 3.0 * v2 + 1.0,
            b: -6.0 * v1 + 3.0 * v2,
            c: 3.0 * v1,
        }
    }

    fn sample(&self, t: f32) -> f32 {
        ((self.a * t + self.b) * t + self.c) * t
    }

    fn sample_gradient(&self, t: f32) -> f32 {
        (3.0 * self.a * t + 2.0 * self.b) * t + self.c
    }
}

/// 计算天体（太阳）角度比例，范围 `[0.0, 1.0]`。
/// 使用 `symmetricCubicBezier(0.362, 0.241)` 匹配原版 26.2 的 `EnvironmentAttributes.SUN_ANGLE` 缓动。
#[must_use]
pub fn calculate_celestial_angle(time_of_day: i64) -> f32 {
    let ticks = time_of_day.rem_euclid(24000);
    let alpha = if ticks < 6000 {
        (ticks + 18000) as f32 / 24000.0
    } else {
        (ticks - 6000) as f32 / 24000.0
    };

    let x_curve = CubicCurve::new(0.362, 0.638);
    let y_curve = CubicCurve::new(0.241, 0.759);

    let mut t = alpha;
    let mut solved = false;
    for _ in 0..4 {
        let error = x_curve.sample(t) - alpha;
        if error.abs() < 1e-5 {
            solved = true;
            break;
        }
        let gradient = x_curve.sample_gradient(t);
        if gradient < 1e-5 {
            break;
        }
        t -= (error / gradient).clamp(-0.25, 0.25);
    }

    if !solved {
        let mut t0 = 0.0f32;
        let mut t1 = 1.0f32;
        for _ in 0..64 {
            if t0 >= t1 {
                break;
            }
            let error = x_curve.sample(t) - alpha;
            if error.abs() < 1e-5 {
                break;
            }
            if error < 0.0 {
                t0 = t;
            } else {
                t1 = t;
            }
            t = f32::midpoint(t1, t0);
        }
    }

    y_curve.sample(t)
}

/// 存活区块的记录已经生成过，因此 `fresh` 会替换它们。否则
/// 记录是未生成实体的唯一副本，因此会保留且只能按 UUID 替换。
fn merge_entity_records(data: &mut Vec<NbtCompound>, live: bool, fresh: Vec<NbtCompound>) {
    if live {
        *data = fresh;
        return;
    }

    for record in fresh {
        if let Some(uuid) = record.get_uuid("UUID") {
            data.retain(|existing| existing.get_uuid("UUID") != Some(uuid));
        }
        data.push(record);
    }
}

#[cfg(test)]
mod tests {
    use papokin_data::{Block, block_properties::WaterLikeProperties, fluid::Fluid};
    use papokin_nbt::compound::NbtCompound;
    use papokin_util::math::position::BlockPos;
    use uuid::Uuid;

    use super::{World, merge_entity_records};

    fn record(uuid: Option<Uuid>, id: &str) -> NbtCompound {
        let mut nbt = NbtCompound::new();
        nbt.put_string("id", id.to_string());
        if let Some(uuid) = uuid {
            nbt.put_uuid("UUID", uuid);
        }
        nbt
    }

    fn ids(data: &[NbtCompound]) -> Vec<&str> {
        data.iter()
            .filter_map(|record| record.get_string("id"))
            .collect()
    }

    #[test]
    fn merge_entity_records_replaces_everything_in_a_live_chunk() {
        let stale = Uuid::from_u128(1);
        let fresh = Uuid::from_u128(2);
        let mut data = vec![record(Some(stale), "minecraft:piglin")];

        merge_entity_records(
            &mut data,
            true,
            vec![record(Some(fresh), "minecraft:zombie")],
        );

        assert_eq!(ids(&data), ["minecraft:zombie"]);
        assert_eq!(data[0].get_uuid("UUID"), Some(fresh));
    }

    #[test]
    fn merge_entity_records_empties_a_live_chunk_with_no_entities_left() {
        let mut data = vec![
            record(Some(Uuid::from_u128(1)), "minecraft:piglin"),
            record(Some(Uuid::from_u128(2)), "minecraft:zombie"),
        ];

        merge_entity_records(&mut data, true, Vec::new());

        assert!(data.is_empty());
    }

    #[test]
    fn merge_entity_records_keeps_unspawned_records_of_a_dormant_chunk() {
        let dormant = Uuid::from_u128(1);
        let mut data = vec![record(Some(dormant), "minecraft:piglin")];

        merge_entity_records(
            &mut data,
            false,
            vec![record(Some(Uuid::from_u128(2)), "minecraft:zombie")],
        );

        assert_eq!(ids(&data), ["minecraft:piglin", "minecraft:zombie"]);
    }

    #[test]
    fn merge_entity_records_replaces_a_same_uuid_record_of_a_dormant_chunk() {
        let shared = Uuid::from_u128(1);
        let mut data = vec![
            record(Some(shared), "minecraft:piglin"),
            record(Some(Uuid::from_u128(2)), "minecraft:zombie"),
        ];

        merge_entity_records(
            &mut data,
            false,
            vec![record(Some(shared), "minecraft:hoglin")],
        );

        assert_eq!(ids(&data), ["minecraft:zombie", "minecraft:hoglin"]);
        assert_eq!(
            data.iter()
                .filter(|r| r.get_uuid("UUID") == Some(shared))
                .count(),
            1
        );
    }

    #[test]
    fn merge_entity_records_never_matches_a_record_without_a_uuid() {
        let mut data = vec![record(None, "minecraft:piglin")];

        merge_entity_records(&mut data, false, vec![record(None, "minecraft:zombie")]);

        assert_eq!(ids(&data), ["minecraft:piglin", "minecraft:zombie"]);
    }

    #[test]
    fn merge_entity_records_is_idempotent_on_a_dormant_chunk() {
        let uuid = Uuid::from_u128(1);
        let mut data = Vec::new();

        for _ in 0..3 {
            merge_entity_records(
                &mut data,
                false,
                vec![record(Some(uuid), "minecraft:piglin")],
            );
        }

        assert_eq!(ids(&data), ["minecraft:piglin"]);
    }

    #[test]
    fn liquid_block_states_preserve_source_flow_and_falling_depths() {
        // 液体方块等级与流体量不同：所有下落等级
        // 解析为数量 8，而不是环绕经过流体状态数组。
        let amounts = [8, 7, 6, 5, 4, 3, 2, 1, 8, 8, 8, 8, 8, 8, 8, 8];
        for (block, expected_fluid) in [
            (&Block::WATER, &Fluid::FLOWING_WATER),
            (&Block::LAVA, &Fluid::FLOWING_LAVA),
        ] {
            for (level, amount) in amounts.into_iter().enumerate() {
                let id = WaterLikeProperties { level: level as u8 }.to_state_id(block);
                let (fluid, state) = World::fluid_state_from_block_state(id);
                assert_eq!(fluid.id, expected_fluid.id);
                assert_eq!(state.level, amount);
                assert!((state.height - f32::from(amount) / 9.0).abs() < f32::EPSILON);
                assert_eq!(state.is_source, level == 0);
                assert_eq!(state.is_still, level == 0);
                assert_eq!(state.falling, level >= 8);
                assert!(!state.is_empty);
                assert_eq!(
                    state.block_state_id,
                    WaterLikeProperties {
                        level: level.min(8) as u8
                    }
                    .to_state_id(block)
                );
            }
        }
    }

    #[test]
    fn aquatic_and_waterlogged_blocks_contain_source_water() {
        let wet_stairs = Block::OAK_STAIRS
            .set_waterlogged(Block::OAK_STAIRS.default_state.id, true)
            .unwrap();
        for id in [
            Block::KELP.default_state.id,
            Block::KELP_PLANT.default_state.id,
            Block::SEAGRASS.default_state.id,
            Block::TALL_SEAGRASS.default_state.id,
            Block::BUBBLE_COLUMN.default_state.id,
            wet_stairs,
        ] {
            let (fluid, state) = World::fluid_state_from_block_state(id);
            assert!(fluid.matches_type(&Fluid::WATER));
            assert!(state.is_source && state.is_still && !state.is_empty && !state.falling);
            assert_eq!(state.level, 8);
            assert!((state.height - 8.0 / 9.0).abs() < f32::EPSILON);
        }

        for block in [&Block::AIR, &Block::OAK_STAIRS, &Block::WATER_CAULDRON] {
            let (fluid, state) = World::fluid_state_from_block_state(block.default_state.id);
            assert_eq!(fluid.id, Fluid::EMPTY.id);
            assert!(state.is_empty);
            assert_eq!(state.height, 0.0);
        }
    }

    #[test]
    fn a_shapeless_block_never_stops_a_ray() {
        // 射线总是起始于某个方块内部（通常是空气），且该方块不能
        // 计为命中，否则每次射线检测都会在起点处停止。
        let pos = BlockPos::new(10, 64, 10);
        let from = papokin_util::math::vector3::Vector3::new(10.5, 64.5, 10.5);
        let to = papokin_util::math::vector3::Vector3::new(20.5, 64.5, 10.5);

        assert!(
            super::World::clip_outline_shapes(Block::AIR.default_state, &pos, from, to).is_none()
        );
        assert!(
            super::World::clip_outline_shapes(Block::STONE.default_state, &pos, from, to).is_some()
        );
    }

    #[test]
    fn game_rules_registry() {
        use papokin_data::game_rules::{GameRule, GameRuleRegistry, GameRuleValue};

        let mut registry = GameRuleRegistry::default();
        match registry.get(&GameRule::KeepInventory) {
            GameRuleValue::Bool(v) => assert!(!v),
            GameRuleValue::Int(_) => panic!("应为 bool"),
        }

        match registry.get_mut(&GameRule::KeepInventory) {
            GameRuleValue::Bool(v) => *v = true,
            GameRuleValue::Int(_) => panic!("应为 bool"),
        }

        match registry.get(&GameRule::KeepInventory) {
            GameRuleValue::Bool(v) => assert!(v),
            GameRuleValue::Int(_) => panic!("应为 bool"),
        }

        match registry.get(&GameRule::RandomTickSpeed) {
            GameRuleValue::Int(v) => assert_eq!(*v, 3),
            GameRuleValue::Bool(_) => panic!("应为 int"),
        }

        match registry.get_mut(&GameRule::RandomTickSpeed) {
            GameRuleValue::Int(v) => *v = 20,
            GameRuleValue::Bool(_) => panic!("应为 int"),
        }

        match registry.get(&GameRule::RandomTickSpeed) {
            GameRuleValue::Int(v) => assert_eq!(*v, 20),
            GameRuleValue::Bool(_) => panic!("应为 int"),
        }
    }
}
