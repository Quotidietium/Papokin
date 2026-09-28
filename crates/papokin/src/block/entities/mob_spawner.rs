use std::sync::{
    Arc, Mutex,
    atomic::{AtomicI32, Ordering},
};

use crossbeam::atomic::AtomicCell;
use papokin_data::{entity::EntityType, world::WorldEvent};
use papokin_nbt::compound::NbtCompound;
use papokin_nbt::tag::NbtTag;
use papokin_util::math::{boundingbox::BoundingBox, position::BlockPos, vector3::Vector3};

use crate::{block::entities::BlockEntity, entity::EntityBase, world::World};

pub struct MobSpawnerBlockEntity {
    pub position: BlockPos,
    pub delay: AtomicI32,
    pub max_delay: i32,
    pub min_delay: i32,
    pub spawn_count: i32,
    pub spawn_range: i32,
    pub max_nearby_entities: i32,
    pub required_player_range: i32,
    entity_type: AtomicCell<Option<&'static EntityType>>,
    /// 从存档原样保留的生成负载：`SpawnData` 完整 NBT（可含位置、
    /// 装备等实体字段）与 `SpawnPotentials` 加权候选列表。写侧原样
    /// 写回——此前只重写裸 `entity{id}`，多候选/带装备的刷怪笼在
    /// 首次保存后即永久退化为单一裸实体。
    preserved_spawn_data: Mutex<Option<NbtCompound>>,
    preserved_potentials: Mutex<Option<Vec<NbtTag>>>,
}

impl MobSpawnerBlockEntity {
    pub const ID: &'static str = "minecraft:mob_spawner";
    pub const DEFAULT_DELAY: i32 = 20;
    pub const DEFAULT_MAX_SPAWN_DELAY: i32 = 800;
    pub const DEFAULT_MIN_SPAWN_DELAY: i32 = 200;
    pub const DEFAULT_SPAWN_COUNT: i32 = 4;
    pub const DEFAULT_SPAWN_RANGE: i32 = 4;
    pub const DEFAULT_MAX_NEARBY_ENTITIES: i32 = 6;
    pub const DEFAULT_REQUIRED_PLAYER_RANGE: i32 = 16;

    #[must_use]
    pub const fn new(position: BlockPos, entity_type: Option<&'static EntityType>) -> Self {
        Self {
            position,
            delay: AtomicI32::new(Self::DEFAULT_DELAY),
            max_delay: Self::DEFAULT_MAX_SPAWN_DELAY,
            min_delay: Self::DEFAULT_MIN_SPAWN_DELAY,
            spawn_count: Self::DEFAULT_SPAWN_COUNT,
            spawn_range: Self::DEFAULT_SPAWN_RANGE,
            max_nearby_entities: Self::DEFAULT_MAX_NEARBY_ENTITIES,
            required_player_range: Self::DEFAULT_REQUIRED_PLAYER_RANGE,
            entity_type: AtomicCell::new(entity_type),
            preserved_spawn_data: Mutex::new(None),
            preserved_potentials: Mutex::new(None),
        }
    }

    pub fn write_spawner_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_short("Delay", self.delay.load(Ordering::Relaxed) as i16);
        nbt.put_short("MinSpawnDelay", self.min_delay as i16);
        nbt.put_short("MaxSpawnDelay", self.max_delay as i16);
        nbt.put_short("SpawnCount", self.spawn_count as i16);
        nbt.put_short("SpawnRange", self.spawn_range as i16);
        nbt.put_short("MaxNearbyEntities", self.max_nearby_entities as i16);
        nbt.put_short("RequiredPlayerRange", self.required_player_range as i16);

        // 原样写回保留的生成负载；仅在结构体没有保留副本时（如
        // 刷怪蛋刚设置的新类型）才重写最小 SpawnData。
        let preserved_data = self
            .preserved_spawn_data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if let Some(spawn_data) = preserved_data {
            nbt.put_compound("SpawnData", spawn_data);
        } else if let Some(entity_type) = self.entity_type.load() {
            let mut spawn_entry = NbtCompound::new();

            let mut entity_nbt = NbtCompound::new();
            entity_nbt.put_string("id", format!("minecraft:{}", entity_type.resource_name));

            spawn_entry.put_compound("entity", entity_nbt);

            nbt.put_compound("SpawnData", spawn_entry);
        }

        let preserved_potentials = self
            .preserved_potentials
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if let Some(potentials) = preserved_potentials
            && !potentials.is_empty()
        {
            nbt.put_list("SpawnPotentials", potentials);
        }
    }
}

impl MobSpawnerBlockEntity {
    /// 统计感应范围内同类型实体数（MaxNearbyEntities 判定）。
    ///
    /// 走按区块分桶的索引：此前对全服实体做全表线性扫描，
    /// 多刷怪笼场景每刻放大。
    fn count_nearby_entities(&self, world: &World, type_id: u16, center: Vector3<f64>) -> usize {
        let radius_horiz = (self.spawn_range * 2) as f64;
        let radius_vert = 4.0;
        let search_box = BoundingBox::new(
            Vector3::new(
                center.x - radius_horiz,
                center.y - radius_vert,
                center.z - radius_horiz,
            ),
            Vector3::new(
                center.x + radius_horiz,
                center.y + radius_vert,
                center.z + radius_horiz,
            ),
        );
        world
            .get_entities_at_box(&search_box)
            .iter()
            .filter(|e| {
                let ent = e.get_entity();
                if ent.entity_type.id != type_id {
                    return false;
                }
                let pos = ent.pos.load();
                (pos.x - center.x).abs() <= radius_horiz
                    && (pos.z - center.z).abs() <= radius_horiz
                    && (pos.y - center.y).abs() <= radius_vert
            })
            .count()
    }
    fn update_spawns(&self, world: &Arc<World>) {
        let min_delay = self.min_delay;
        let max_delay = self.max_delay;

        self.delay.store(
            if max_delay <= min_delay {
                min_delay
            } else {
                min_delay + rand::random_range(0..max_delay - min_delay)
            },
            Ordering::Relaxed,
        );
        world.add_synced_block_event(self.position, 1, 0);
    }

    pub fn set_entity_type(&self, entity_type: &'static EntityType) {
        self.entity_type.store(Some(entity_type));
        // 保留副本描述的是旧类型；不清除会在下次保存时把旧
        // SpawnData/SpawnPotentials 写回（重载后类型回退）。
        if let Ok(mut preserved) = self.preserved_spawn_data.lock() {
            *preserved = None;
        }
        if let Ok(mut preserved) = self.preserved_potentials.lock() {
            *preserved = None;
        }
    }
}

impl BlockEntity for MobSpawnerBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn tick(&self, world: &Arc<World>) {
        if let Some(entity_type) = &self.entity_type.load() {
            let center = self.position.to_centered_f64();
            let max_player_dist_sq = (self.required_player_range as f64).powi(2);
            let player_nearby = world.players.load().iter().any(|p| {
                p.get_entity().pos.load().squared_distance_to_vec(&center) <= max_player_dist_sq
            });

            if !player_nearby {
                return;
            }

            if self.delay.load(Ordering::Relaxed) < 0 {
                self.update_spawns(world);
                return;
            }
            if self.delay.load(Ordering::Relaxed) > 0 {
                self.delay.fetch_sub(1, Ordering::Relaxed);
                return;
            }

            let nearby_count = self.count_nearby_entities(world, entity_type.id, center);

            if nearby_count as i32 >= self.max_nearby_entities {
                self.update_spawns(world);
                return;
            }

            let spawn_range = self.spawn_range;

            // 在任何生成尝试之前触发的早期过滤钩子
            let mut pre_event =
                crate::plugin::api::events::entity::pre_spawner_spawn::PreSpawnerSpawnEvent::new(
                    self.position,
                    format!("minecraft:{}", entity_type.resource_name),
                );
            if let Some(server) = world.server.upgrade() {
                server.plugin_manager.fire_blocking(&server, &mut pre_event);
            }
            if pre_event.cancelled {
                // 将被取消的生成轮次计为已完成，以便刷怪笼
                // 进入冷却，而不是每刻重试。
                self.update_spawns(world);
                return;
            }

            for _ in 0..self.spawn_count {
                let pos = self.position.0;

                let spawn_pos = Vector3::new(
                    pos.x as f64
                        + (rand::random::<f64>() - rand::random::<f64>()) * spawn_range as f64
                        + 0.5,
                    (pos.y + rand::random_range(0..3) - 1) as f64,
                    pos.z as f64
                        + (rand::random::<f64>() - rand::random::<f64>()) * spawn_range as f64
                        + 0.5,
                );
                if !world.is_space_empty(entity_type.get_spawn_bounding_box(
                    spawn_pos.x,
                    spawn_pos.y,
                    spawn_pos.z,
                )) {
                    continue;
                }
                let entity = crate::entity::r#type::from_type(
                    entity_type,
                    spawn_pos,
                    world,
                    uuid::Uuid::new_v4(),
                );
                let yaw = rand::random::<f32>() * 360.0;
                entity.get_entity().set_rotation(yaw, 0.0);

                let mut event =
                    crate::plugin::api::events::entity::spawner_spawn::SpawnerSpawnEvent::new(
                        entity.get_entity().entity_id,
                        self.position,
                    );
                if let Some(server) = world.server.upgrade() {
                    server.plugin_manager.fire_blocking(&server, &mut event);
                }
                if event.cancelled {
                    // 将被取消的生成计为一次已完成的尝试：冷却在
                    // 循环外统一进入
                    continue;
                }

                world.spawn_entity(entity);
                world.sync_world_event(WorldEvent::ParticlesMobblockSpawn, self.position, 0);
            }
            // 无论是否生成（含全部位置因空间不足失败、插件取消）：
            // 都进入冷却。全部失败时若不冷却，被封死的刷怪笼会在
            // 玩家于感应范围内时每刻重试 spawn_count 次空间检查并
            // 每刻触发一次 PreSpawnerSpawnEvent。
            self.update_spawns(world);
        }
    }

    fn from_nbt(nbt: &papokin_nbt::compound::NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        // 数值字段来自存档 NBT（可被外部编辑），且 get_int 回退接受
        // 全域 i32：若不钳制，update_spawns 的 `max_delay - min_delay`
        // 与 `min_delay + 随机数` 会整数溢出 panic，单轮生成尝试数与
        // 生成半径也会被放大到卡死 tick。统一钳制到 i16 short 语义
        // （原版字段本身即 short）。
        const FIELD_CAP: i32 = i16::MAX as i32;
        const SPAWN_COUNT_CAP: i32 = 255;
        const SPAWN_RANGE_CAP: i32 = 64;
        let get_num = |name: &str| {
            nbt.get_short(name)
                .map(i32::from)
                .or_else(|| nbt.get_int(name))
                .or_else(|| nbt.get_byte(name).map(i32::from))
        };

        let delay = get_num("Delay")
            .unwrap_or(Self::DEFAULT_DELAY)
            .clamp(-1, FIELD_CAP);
        let min_delay = get_num("MinSpawnDelay")
            .unwrap_or(Self::DEFAULT_MIN_SPAWN_DELAY)
            .clamp(0, FIELD_CAP);
        let max_delay = get_num("MaxSpawnDelay")
            .unwrap_or(Self::DEFAULT_MAX_SPAWN_DELAY)
            .clamp(min_delay, FIELD_CAP);
        let spawn_count = get_num("SpawnCount")
            .unwrap_or(Self::DEFAULT_SPAWN_COUNT)
            .clamp(0, SPAWN_COUNT_CAP);
        let spawn_range = get_num("SpawnRange")
            .unwrap_or(Self::DEFAULT_SPAWN_RANGE)
            .clamp(0, SPAWN_RANGE_CAP);
        let max_nearby_entities = get_num("MaxNearbyEntities")
            .unwrap_or(Self::DEFAULT_MAX_NEARBY_ENTITIES)
            .clamp(0, FIELD_CAP);
        let required_player_range = get_num("RequiredPlayerRange")
            .unwrap_or(Self::DEFAULT_REQUIRED_PLAYER_RANGE)
            .clamp(0, FIELD_CAP);

        let entity_type = nbt
            .get_compound("SpawnData")
            .and_then(|data| {
                data.get_compound("entity")
                    .and_then(|entity| entity.get_string("id"))
                    .or_else(|| data.get_string("id"))
            })
            .or_else(|| {
                nbt.get_list("SpawnPotentials")
                    .and_then(|list| list.first())
                    .and_then(|tag| tag.extract_compound())
                    .and_then(|entry| {
                        entry
                            .get_compound("data")
                            .and_then(|data| {
                                data.get_compound("entity")
                                    .and_then(|entity| entity.get_string("id"))
                                    .or_else(|| data.get_string("id"))
                            })
                            .or_else(|| {
                                entry
                                    .get_compound("entity")
                                    .and_then(|entity| entity.get_string("id"))
                            })
                    })
            })
            .or_else(|| nbt.get_string("EntityId"))
            .and_then(EntityType::from_name);

        Self {
            position,
            delay: AtomicI32::new(delay),
            max_delay,
            min_delay,
            spawn_count,
            spawn_range,
            max_nearby_entities,
            required_player_range,
            entity_type: AtomicCell::new(entity_type),
            // 保留原始生成负载：多候选（SpawnPotentials）与带完整
            // 实体字段的 SpawnData 在写侧原样回写，避免保存即退化。
            preserved_spawn_data: Mutex::new(nbt.get_compound("SpawnData").cloned()),
            preserved_potentials: Mutex::new(
                nbt.get_list("SpawnPotentials").map(<[NbtTag]>::to_vec),
            ),
        }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_spawner_nbt(nbt);
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut final_nbt = NbtCompound::new();
        final_nbt.put_short("Delay", self.delay.load(Ordering::Relaxed) as i16);
        final_nbt.put_short("MinSpawnDelay", self.min_delay as i16);
        final_nbt.put_short("MaxSpawnDelay", self.max_delay as i16);
        final_nbt.put_short("SpawnCount", self.spawn_count as i16);
        final_nbt.put_short("SpawnRange", self.spawn_range as i16);
        final_nbt.put_short("MaxNearbyEntities", self.max_nearby_entities as i16);
        final_nbt.put_short("RequiredPlayerRange", self.required_player_range as i16);

        if let Some(entity_type) = self.entity_type.load() {
            let mut spawn_entry = NbtCompound::new();

            let mut entity_nbt = NbtCompound::new();
            entity_nbt.put_string("id", format!("minecraft:{}", entity_type.resource_name));

            spawn_entry.put_compound("entity", entity_nbt);

            final_nbt.put_compound("SpawnData", spawn_entry);
        }
        Some(final_nbt)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 编辑存档可注入全域 i32：装载时必须钳制，否则 `update_spawns`
    /// 的 `max_delay - min_delay` 与 `min_delay + 随机数` 会整数
    /// 溢出 panic，单轮生成尝试数也会被放大到卡死 tick。
    #[test]
    fn extreme_nbt_values_are_clamped() {
        let mut nbt = NbtCompound::new();
        nbt.put_int("Delay", i32::MIN);
        nbt.put_int("MinSpawnDelay", i32::MIN);
        nbt.put_int("MaxSpawnDelay", i32::MAX);
        nbt.put_int("SpawnCount", i32::MAX);
        nbt.put_int("SpawnRange", i32::MAX);

        let spawner = MobSpawnerBlockEntity::from_nbt(&nbt, BlockPos::new(0, 0, 0));

        assert_eq!(spawner.delay.load(Ordering::Relaxed), -1);
        assert_eq!(spawner.min_delay, 0);
        assert_eq!(spawner.max_delay, i16::MAX as i32);
        assert_eq!(spawner.spawn_count, 255);
        assert_eq!(spawner.spawn_range, 64);
    }

    /// 多候选刷怪笼（`SpawnPotentials`）与带完整实体字段的 `SpawnData`
    /// 必须在保存时原样写回——此前写侧只写裸 `entity{id}`，外部
    /// 工具/数据包生成的刷怪笼在首次保存后即退化为单一裸实体。
    #[test]
    fn spawn_potentials_and_rich_spawn_data_survive_roundtrip() {
        let mut spawn_data = NbtCompound::new();
        let mut entity = NbtCompound::new();
        entity.put_string("id", "minecraft:zombie".to_string());
        entity.put_float("Health", 40.0);
        spawn_data.put_compound("entity", entity);

        let mut p1_data = NbtCompound::new();
        let mut p1_entity = NbtCompound::new();
        p1_entity.put_string("id", "minecraft:skeleton".to_string());
        p1_data.put_int("weight", 1);
        p1_data.put_compound("entity", p1_entity);

        let mut p2_data = NbtCompound::new();
        let mut p2_entity = NbtCompound::new();
        p2_entity.put_string("id", "minecraft:creeper".to_string());
        p2_data.put_int("weight", 3);
        p2_data.put_compound("entity", p2_entity);

        let mut nbt = NbtCompound::new();
        nbt.put_compound("SpawnData", spawn_data);
        nbt.put_list(
            "SpawnPotentials",
            vec![NbtTag::Compound(p1_data), NbtTag::Compound(p2_data)],
        );

        let spawner = MobSpawnerBlockEntity::from_nbt(&nbt, BlockPos::new(0, 0, 0));

        let mut out = NbtCompound::new();
        spawner.write_spawner_nbt(&mut out);

        let roundtrip_data = out.get_compound("SpawnData").expect("SpawnData 应写回");
        let health = roundtrip_data
            .get_compound("entity")
            .and_then(|e| e.get_float("Health"));
        assert_eq!(health, Some(40.0), "SpawnData 的完整实体字段不得丢失");

        let potentials = out
            .get_list("SpawnPotentials")
            .expect("SpawnPotentials 应写回");
        assert_eq!(potentials.len(), 2, "候选列表不得丢失");
    }

    /// 刷怪蛋设置新类型后，旧的保留负载必须清除，否则重载后
    /// 类型会回退到旧 `SpawnData` 描述的实体。
    #[test]
    fn set_entity_type_clears_preserved_payload() {
        let mut spawn_data = NbtCompound::new();
        let mut entity = NbtCompound::new();
        entity.put_string("id", "minecraft:zombie".to_string());
        spawn_data.put_compound("entity", entity);

        let mut nbt = NbtCompound::new();
        nbt.put_compound("SpawnData", spawn_data);

        let spawner = MobSpawnerBlockEntity::from_nbt(&nbt, BlockPos::new(0, 0, 0));
        let skeleton = EntityType::from_name("minecraft:skeleton").expect("类型应存在");
        spawner.set_entity_type(skeleton);

        let mut out = NbtCompound::new();
        spawner.write_spawner_nbt(&mut out);

        let id = out
            .get_compound("SpawnData")
            .and_then(|d| d.get_compound("entity"))
            .and_then(|e| e.get_string("id"))
            .expect("应写入新类型的最小 SpawnData");
        assert_eq!(id, "minecraft:skeleton");
        assert!(out.get_list("SpawnPotentials").is_none());
    }
}
