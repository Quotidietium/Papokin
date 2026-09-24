use crate::{
    entity::item::ItemEntity,
    net::java::JavaClient,
    server::Server,
    world::{
        World,
        portal::{NetherPortal, PortalProcessor, PortalType, SourcePortalInfo},
    },
};
use arc_swap::ArcSwap;
use bytes::BufMut;
use crossbeam::atomic::AtomicCell;
use living::LivingEntity;
use papokin_data::BlockState;
use papokin_data::biome::Biome;
use papokin_data::block_properties::blocks_movement;
use papokin_data::data_component_impl::EquipmentSlot;
use papokin_data::dimension::Dimension;
use papokin_data::entity::EntityStatus;
use papokin_data::fluid::Fluid;
use papokin_data::item_stack::ItemStack;
use papokin_data::tag::{self, Taggable};
use papokin_data::tracked_data;
use papokin_data::{Block, BlockDirection};
use papokin_data::{
    block_properties::{Facing, HorizontalFacing},
    damage::DamageType,
    damage_ext::ResolvedDamageType,
    entity::{EntityPose, EntityType},
    sound::{Sound, SoundCategory},
};
use papokin_nbt::{compound::NbtCompound, tag::NbtTag};
use papokin_protocol::java::client::play::{CUpdateEntityPos, CUpdateEntityPosRot};
use papokin_protocol::{
    PositionFlag,
    codec::var_int::VarInt,
    java::client::play::{
        CEntityPositionSync, CEntityVelocity, CHeadRot, CPlayerPosition, CSetEntityMetadata,
        CSetPassengers, CSpawnEntity, CSpawnLivingEntity, CUpdateEntityRot, Metadata,
        MetadataSerializer,
    },
};
use papokin_util::math::vector3::Axis;
use papokin_util::math::{
    boundingbox::{BoundingBox, EntityDimensions},
    get_section_cord,
    position::BlockPos,
    vector2::Vector2,
    vector3::Vector3,
    wrap_degrees,
};
use papokin_util::text::TextComponent;
use papokin_util::text::hover::HoverEvent;
use papokin_util::version::JavaMinecraftVersion;
use player::Player;
use std::collections::{BTreeMap, HashSet};
use std::sync::{
    Arc, OnceLock, Weak,
    atomic::{
        AtomicBool, AtomicI32, AtomicU8, AtomicU32,
        Ordering::{self, Relaxed},
    },
};
use uuid::Uuid;

pub mod ageable;
pub mod ai;
pub mod area_effect_cloud;
pub mod attributes;
pub mod boss;
pub mod breath;
pub mod custom_sound;
pub mod decoration;
pub mod effect;
pub mod experience_orb;
pub mod falling;
pub mod hunger;
pub mod interaction;
pub mod item;
pub mod item_steerable;
pub mod lightning;
pub mod living;
pub mod marker;
pub mod mob;
pub mod passive;
pub mod player;
pub mod projectile;
pub mod projectile_deflection;
pub mod synched_entity_data;
pub mod tnt;
pub mod r#type;
pub mod vehicle;

pub use lightning::LightningBoltEntity;

pub(crate) mod combat;
pub mod predicate;

/// 实体可携带的记分板标签最大数量，与原版一致。
pub const MAX_SCOREBOARD_TAGS: usize = 1024;

/// 校验来自存档的双精度浮点：要求有限（NaN/Inf 一律拒绝），
/// 但允许任意符号（位置/速度分量为负是合法的）。
/// 与原版 `Entity.load` 的 `isFinite` 检查对齐，防止 NaN
/// 经运动与伤害算术传播并随存档持久化。
#[must_use]
pub(crate) const fn finite_f64_or(value: f64, default: f64) -> f64 {
    if value.is_finite() { value } else { default }
}

/// 装载坐标的世界水平边界（与跨世界传送校验一致）。
pub(crate) const LOADED_POS_HORIZONTAL_LIMIT: f64 = 2.999_99E7;
/// 装载坐标的垂直边界：覆盖 i8 section 范围（±128 区段 × 16 格）。
pub(crate) const LOADED_POS_VERTICAL_LIMIT: f64 = 2048.0;
/// 装载速度分量的幅度上限：巨大速度会把实体在单刻内推出
/// 上述边界（原版没有哪种机制能产生上百格/刻的速度）。
pub(crate) const LOADED_MOTION_LIMIT: f64 = 100.0;

/// 钳制从存档装载的坐标：编辑存档可注入任意有限值，越界坐标
/// 会让后续区块坐标换算（`chunk << 4`、section 索引等）整数
/// 溢出 panic 或越界索引。
#[must_use]
pub(crate) fn sanitize_loaded_position(pos: Vector3<f64>) -> Vector3<f64> {
    Vector3::new(
        pos.x
            .clamp(-LOADED_POS_HORIZONTAL_LIMIT, LOADED_POS_HORIZONTAL_LIMIT),
        pos.y
            .clamp(-LOADED_POS_VERTICAL_LIMIT, LOADED_POS_VERTICAL_LIMIT),
        pos.z
            .clamp(-LOADED_POS_HORIZONTAL_LIMIT, LOADED_POS_HORIZONTAL_LIMIT),
    )
}

/// 钳制从存档装载的速度分量，防止单刻内被推出世界边界。
#[must_use]
pub(crate) fn sanitize_loaded_motion(motion: Vector3<f64>) -> Vector3<f64> {
    Vector3::new(
        motion.x.clamp(-LOADED_MOTION_LIMIT, LOADED_MOTION_LIMIT),
        motion.y.clamp(-LOADED_MOTION_LIMIT, LOADED_MOTION_LIMIT),
        motion.z.clamp(-LOADED_MOTION_LIMIT, LOADED_MOTION_LIMIT),
    )
}

/// 校验来自存档的单精度浮点：要求有限但允许任意符号（如旋转角）。
#[must_use]
pub(crate) const fn finite_f32_or(value: f32, default: f32) -> f32 {
    if value.is_finite() { value } else { default }
}

/// 校验来自存档的单精度浮点：要求有限且非负（血量、速度系数
/// 等 gameplay 数值均不应为 NaN/Inf 或负数）。
#[must_use]
pub(crate) const fn finite_non_negative_f32_or(value: f32, default: f32) -> f32 {
    if value.is_finite() && value >= 0.0 {
        value
    } else {
        default
    }
}

///返回在给定情况下应广播的 [`EntityStatus`]
/// 装备槽位损坏。
#[must_use]
pub const fn equipment_break_status(slot: &EquipmentSlot) -> EntityStatus {
    match slot {
        EquipmentSlot::MainHand(_) => EntityStatus::MainhandBreak,
        EquipmentSlot::OffHand(_) => EntityStatus::OffhandBreak,
        EquipmentSlot::Head(_) => EntityStatus::HeadBreak,
        EquipmentSlot::Chest(_) => EntityStatus::ChestBreak,
        EquipmentSlot::Legs(_) => EntityStatus::LegsBreak,
        EquipmentSlot::Feet(_) => EntityStatus::FeetBreak,
        EquipmentSlot::Body(_) => EntityStatus::BodyBreak,
        EquipmentSlot::Saddle(_) => EntityStatus::SaddleBreak,
    }
}

impl dyn EntityBase + '_ {
    /// 在 trait 对象上固有提供，使双方都能作为 `&dyn EntityBase`。
    #[must_use]
    pub fn is_allied_to(&self, other: &dyn EntityBase) -> bool {
        self.get_entity().entity_id == other.get_entity().entity_id
            || self.considers_entity_as_ally(other)
            || other.considers_entity_as_ally(self)
    }
}

pub trait EntityBase: Send + Sync + std::any::Any {
    fn write_nbt(&self, nbt: &mut NbtCompound) {
        self.get_entity().write_nbt(nbt);
        if let Some(living) = self.get_living_entity() {
            living.write_living_nbt(nbt);
        }
        self.write_custom_nbt(nbt);
    }

    fn write_custom_nbt(&self, _nbt: &mut NbtCompound) {}

    fn read_nbt_non_mut(&self, nbt: &NbtCompound) {
        self.get_entity().read_nbt_non_mut(nbt);
        if let Some(living) = self.get_living_entity() {
            living.read_living_nbt_non_mut(nbt);
        }
        self.read_custom_nbt(nbt);
    }

    fn read_custom_nbt(&self, _nbt: &NbtCompound) {}
    /// 此实体的每个刻都会调用。
    ///
    /// `caller` 参数是对发起本次刻（tick）的实体的引用。
    /// 这可以是调用该方法（`self`）时所指的同一个实体，
    /// 但在某些场景下（如交互或事件），它可能是另一个实体。
    ///
    /// `server` 参数提供对游戏服务器实例的访问。
    fn tick(&self, caller: &dyn EntityBase, server: &Server) {
        if let Some(living) = self.get_living_entity() {
            living.tick(caller, server);
        } else {
            self.get_entity().tick(caller, server);
        }
    }

    fn get_job_site_pos(&self) -> Option<papokin_util::math::position::BlockPos> {
        None
    }

    fn get_home_pos(&self) -> Option<papokin_util::math::position::BlockPos> {
        None
    }

    fn as_any(&self) -> &dyn std::any::Any
    where
        Self: Sized,
    {
        self
    }

    fn get_item_steerable(&self) -> Option<&dyn crate::entity::item_steerable::ItemSteerable> {
        None
    }

    fn get_owner_id(&self) -> Option<i32> {
        None
    }

    fn get_eye_pos(&self) -> Vector3<f64> {
        self.get_entity().get_eye_pos()
    }

    fn get_looking_vector(&self) -> Vector3<f64> {
        let entity = self.get_entity();
        Vector3::from_yaw_pitch(entity.yaw.load(), entity.pitch.load())
    }

    fn init_data_tracker(&self) {
        let entity = self.get_entity();

        // 如果内部年龄为负，则是幼体
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;

        if is_baby {
            entity.set_synced_data(tracked_data::ageable_mob::DATA_BABY_ID, true);
        }
    }
    fn set_variant_name(&self, _name: &str) {}

    fn teleport(
        &self,
        position: Vector3<f64>,
        yaw: Option<f32>,
        pitch: Option<f32>,
        world: Arc<World>,
    ) {
        self.get_entity().teleport(position, yaw, pitch, &world);
    }

    /// 像 [`EntityBase::teleport`] 一样传送实体，并额外地标记
    /// 将发往客户端的位置数据包的各个分量作为相对
    /// （对应 Papo 的 `TeleportFlags`）。
    ///
    /// 相对标志只影响玩家客户端；非玩家实体没有
    /// 相对传送机制，因此默认实现会忽略
    /// `relatives`，并进行绝对传送。
    fn teleport_with_relatives(
        &self,
        position: Vector3<f64>,
        yaw: Option<f32>,
        pitch: Option<f32>,
        relatives: &[PositionFlag],
        world: Arc<World>,
    ) {
        let _ = relatives;
        self.teleport(position, yaw, pitch, world);
    }

    fn is_pushed_by_fluids(&self) -> bool {
        true
    }

    /// 实体是否免疫爆炸击退和伤害
    fn is_immune_to_explosion(&self) -> bool {
        false
    }

    fn get_gravity(&self) -> f64 {
        0.0
    }

    fn get_mob(&self) -> Option<&dyn mob::Mob> {
        None
    }

    /// 玩家按档案名称跟踪，其他所有实体按其 UUID 跟踪。
    fn get_scoreboard_name(&self) -> String {
        self.get_player().map_or_else(
            || self.get_entity().entity_uuid.to_string(),
            |player| player.gameprofile.name.clone(),
        )
    }

    fn get_team(&self) -> Option<crate::world::scoreboard::Team> {
        if let Some(player) = self.get_player() {
            return player.get_team();
        }
        let world = self.get_entity().world.load();
        let scoreboard = world
            .scoreboard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if scoreboard.get_teams().is_empty() {
            return None;
        }
        scoreboard
            .get_entity_team(&self.get_scoreboard_name())
            .cloned()
    }

    /// 仅团队名称，这正是盟友检查所需的全部。值得单独保存，因为
    /// 它们在每次目标搜索的每个候选上都会运行一次，而 `Team` 的开销很大
    /// 以便克隆。
    fn get_team_name(&self) -> Option<String> {
        if let Some(player) = self.get_player() {
            return player.get_team_name();
        }
        let world = self.get_entity().world.load();
        let scoreboard = world
            .scoreboard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if scoreboard.get_teams().is_empty() {
            return None;
        }
        scoreboard
            .get_entity_team(&self.get_scoreboard_name())
            .map(|team| team.name.clone())
    }

    fn considers_entity_as_ally(&self, other: &dyn EntityBase) -> bool {
        if let Some(tamable) = self.get_mob().and_then(mob::Mob::as_tamable)
            && let Some(considered) = tamable.tamable_considers_entity_as_ally(other)
        {
            return considered;
        }
        let Some(team) = self.get_team_name() else {
            return false;
        };
        other.get_team_name().is_some_and(|other| other == team)
    }

    fn tick_in_void(&self, _dyn_self: &dyn EntityBase) {
        self.get_entity().remove();
    }

    ///返回伤害是否成功
    fn damage(&self, caller: &dyn EntityBase, amount: f32, damage_type: DamageType) -> bool {
        caller.damage_with_context(caller, amount, damage_type, None, None, None)
    }

    ///返回伤害是否成功。接受已解析的（原版）伤害来源
    /// 或插件注册的自定义）伤害类型。
    fn damage_resolved(
        &self,
        caller: &dyn EntityBase,
        amount: f32,
        damage_type: &ResolvedDamageType,
    ) -> bool {
        caller.damage_with_resolved_context(caller, amount, damage_type, None, None, None)
    }

    fn on_lightning_strike(
        &self,
        caller: &dyn EntityBase,
        lightning: &lightning::LightningBoltEntity,
    ) {
        if self.get_living_entity().is_some() {
            self.set_on_fire_for(8.0);
            let cause = lightning.get_cause();
            self.damage_with_context(
                caller,
                5.0,
                DamageType::LIGHTNING_BOLT,
                None,
                Some(lightning),
                cause.as_deref().map(|p| p as &dyn EntityBase),
            );
        }
    }

    fn is_spectator(&self) -> bool {
        false
    }

    fn is_collidable(&self, _entity: Option<Box<dyn EntityBase>>) -> bool {
        false
    }

    fn can_hit(&self) -> bool {
        false
    }

    fn is_flutterer(&self) -> bool {
        false
    }

    fn set_sprinting(&self, is_sprinting: bool) {
        if let Some(living) = self.get_living_entity() {
            living.set_sprinting(is_sprinting);
        } else {
            self.get_entity().set_sprinting(is_sprinting);
        }
    }

    fn get_block_speed_factor(&self) -> f32 {
        self.get_living_entity().map_or_else(
            || self.get_entity().get_block_speed_factor(),
            LivingEntity::get_block_speed_factor,
        )
    }

    /// 在 `travel_in_air` 期间应用的自定义 Y 轴速度阻力乘数。
    /// 蝙蝠返回 `Some(0.6)`，以匹配原版对 `travel()` 的重写。
    fn get_y_velocity_drag(&self) -> Option<f64> {
        None
    }

    fn java_spawn_metadata(&self, version: JavaMinecraftVersion) -> Option<Box<[u8]>> {
        if version < JavaMinecraftVersion::V_1_9 {
            let entity = self.get_entity();
            let shared_flags = entity.flags.load(Ordering::Relaxed);
            return (shared_flags != 0).then(|| {
                // (0 << 5) | 0 = 0（类型：byte，索引：0，标志），值，127（终止符）
                Box::<[u8]>::from([0x00u8, shared_flags as u8, 127u8])
            });
        }
        self.get_mob().map_or_else(
            || {
                let entity = self.get_entity();
                let shared_flags = entity.flags.load(Ordering::Relaxed);
                (shared_flags != 0).then(|| {
                    let mut buf = Vec::new();
                    let _ = Metadata::new(
                        papokin_data::tracked_data::entity::DATA_SHARED_FLAGS_ID,
                        shared_flags,
                    )
                    .write(&mut buf, &version);
                    buf.put_u8(255);
                    buf.into_boxed_slice()
                })
            },
            |mob| mob.mob_java_spawn_metadata(version),
        )
    }

    fn send_java_spawn_packet(&self, client: &JavaClient) {
        let entity = self.get_entity();
        let version = client.version.load();
        let is_mob = entity.entity_type.mob || self.get_mob().is_some();
        let metadata = self.java_spawn_metadata(version);
        if version < JavaMinecraftVersion::V_1_19 && is_mob {
            let spawn_packet = entity.create_spawn_living_packet(metadata.clone());
            if let Ok(data) = client.serialize_packet(&spawn_packet) {
                client.try_enqueue_packet(data);
            }
            if version >= JavaMinecraftVersion::V_1_15
                && let Some(meta) = metadata
            {
                let meta_packet = CSetEntityMetadata::new(entity.entity_id.into(), meta);
                if let Ok(meta_data) = client.serialize_packet(&meta_packet) {
                    client.try_enqueue_packet(meta_data);
                }
            }
        } else {
            let spawn_packet = entity.create_spawn_packet();
            if let Ok(data) = client.serialize_packet(&spawn_packet) {
                client.try_enqueue_packet(data);
            }
            if let Some(meta) = metadata
                && (version >= JavaMinecraftVersion::V_1_9 || meta.last().copied() == Some(127))
            {
                let meta_packet = CSetEntityMetadata::new(entity.entity_id.into(), meta);
                if let Ok(meta_data) = client.serialize_packet(&meta_packet) {
                    client.try_enqueue_packet(meta_data);
                }
            }
        }
    }

    fn damage_with_context(
        &self,
        caller: &dyn EntityBase,
        amount: f32,
        damage_type: DamageType,
        position: Option<Vector3<f64>>,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) -> bool {
        if let Some(living) = caller.get_living_entity() {
            return living.damage_with_context(
                caller,
                amount,
                damage_type,
                position,
                source,
                cause,
            );
        }
        false
    }

    /// 伤害入口点，接受已解析的（原版或插件注册的）
    /// 自定义）伤害类型；与 [`Self::damage_with_context`] 等同。
    fn damage_with_resolved_context(
        &self,
        caller: &dyn EntityBase,
        amount: f32,
        damage_type: &ResolvedDamageType,
        position: Option<Vector3<f64>>,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) -> bool {
        if let Some(living) = caller.get_living_entity() {
            return living.damage_with_resolved_context(
                caller,
                amount,
                damage_type,
                position,
                source,
                cause,
            );
        }
        false
    }

    /// 当玩家用物品右键点击此实体时调用。
    /// 当玩家用物品右键点击此实体时调用。
    /// 若交互已被处理，则返回 true。
    fn interact(&self, _player: &Arc<Player>, _item_stack: &mut ItemStack) -> bool {
        false
    }

    fn set_on_fire_for(&self, seconds: f32) {
        let entity = self.get_entity();
        // 将免疫火焰的实体（如某些物品）排除在火焰伤害之外
        if !entity.fire_immune.load(Ordering::Relaxed) {
            self.set_on_fire_for_ticks((seconds * 20.0).floor() as u32);
        }
    }

    fn set_on_fire_for_ticks(&self, ticks: u32) {
        let entity = self.get_entity();
        let mut event = crate::plugin::api::events::entity::entity_combust::EntityCombustEvent::new(
            entity.entity_id,
            ticks as f32 / 20.0,
        );
        if let Some(server) = entity.world.load().server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return;
            }
        }
        if entity.fire_ticks.load(Ordering::Relaxed) < ticks as i32 {
            entity.fire_ticks.store(ticks as i32, Ordering::Relaxed);
        }
        // TODO: 解冻
    }

    /// 当玩家与实体发生碰撞时调用
    fn on_player_collision(&self, _player: &Arc<Player>) {}

    fn is_passenger(&self) -> bool {
        self.get_entity().has_vehicle()
    }

    fn is_vehicle(&self) -> bool {
        self.get_entity().has_passengers()
    }

    fn has_passenger(&self, other: &dyn EntityBase) -> bool {
        self.get_entity()
            .has_passenger(other.get_entity().entity_id)
    }

    fn move_entity(&self, caller: &dyn EntityBase, motion: Vector3<f64>) {
        self.get_entity().move_entity(caller, motion);
    }

    fn is_pushable(&self) -> bool {
        false
    }

    fn push(&self, entity: &dyn EntityBase) {
        let self_entity = self.get_entity();
        let other_entity = entity.get_entity();

        if self_entity.no_physics.load(Ordering::Relaxed)
            || other_entity.no_physics.load(Ordering::Relaxed)
        {
            return;
        }

        if self_entity.has_passenger(other_entity.entity_id)
            || other_entity.has_passenger(self_entity.entity_id)
        {
            return;
        }

        let mut dx = other_entity.pos.load().x - self_entity.pos.load().x;
        let mut dz = other_entity.pos.load().z - self_entity.pos.load().z;
        let mut d = dx.abs().max(dz.abs());
        if d >= 0.01 {
            d = d.sqrt();
            dx /= d;
            dz /= d;
            let mut d2 = 1.0 / d;
            if d2 > 1.0 {
                d2 = 1.0;
            }
            dx *= d2;
            dz *= d2;
            dx *= 0.05;
            dz *= 0.05;

            if !self_entity.has_passengers() && self.is_pushable() {
                let mut vel = self_entity.velocity.load();
                vel.x -= dx;
                vel.z -= dz;
                self_entity.velocity.store(vel);
                self_entity.velocity_dirty.store(true, Ordering::SeqCst);
            }

            if !other_entity.has_passengers() && entity.is_pushable() {
                let mut vel = other_entity.velocity.load();
                vel.x += dx;
                vel.z += dz;
                other_entity.velocity.store(vel);
                other_entity.velocity_dirty.store(true, Ordering::SeqCst);
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    fn push_entities(&self, dyn_self: &dyn EntityBase) -> bool {
        let mut picked_up = false;
        let mut pushed = false;
        let self_entity = self.get_entity();
        let entity_bb = self_entity.bounding_box.load();

        if !self.is_pushable() {
            return false;
        }

        let world = self_entity.world.load();

        let is_rideable_minecart = self_entity.entity_type.id == EntityType::MINECART.id;
        let is_abstract_minecart = is_rideable_minecart
            || self_entity.entity_type.id == EntityType::CHEST_MINECART.id
            || self_entity.entity_type.id == EntityType::COMMAND_BLOCK_MINECART.id
            || self_entity.entity_type.id == EntityType::FURNACE_MINECART.id
            || self_entity.entity_type.id == EntityType::HOPPER_MINECART.id
            || self_entity.entity_type.id == EntityType::SPAWNER_MINECART.id
            || self_entity.entity_type.id == EntityType::TNT_MINECART.id;

        let is_minecart_fn = |id| -> bool {
            id == EntityType::MINECART.id
                || id == EntityType::CHEST_MINECART.id
                || id == EntityType::COMMAND_BLOCK_MINECART.id
                || id == EntityType::FURNACE_MINECART.id
                || id == EntityType::HOPPER_MINECART.id
                || id == EntityType::SPAWNER_MINECART.id
                || id == EntityType::TNT_MINECART.id
        };

        if is_abstract_minecart {
            let is_vehicle = self.is_vehicle();

            if is_rideable_minecart && !is_vehicle {
                let pickup_bb = entity_bb.expand(0.2, 0.0, 0.2);
                let other_entities = world.get_entities_at_box(&pickup_bb);

                for other in other_entities {
                    if other.get_entity().entity_id != self_entity.entity_id {
                        let other_type = other.get_entity().entity_type.id;
                        let is_iron_golem = other_type == EntityType::IRON_GOLEM.id;
                        let is_other_minecart = is_minecart_fn(other_type);

                        if !is_iron_golem
                            && !is_other_minecart
                            && !other.is_passenger()
                            && other.is_pushable()
                            && other.get_entity().riding_cooldown.load(Relaxed) == 0
                            && let Some(self_arc) = world.get_entity_by_id(self_entity.entity_id)
                        {
                            self_entity.add_passenger(self_arc, other.clone());
                            picked_up = true;
                            break;
                        }
                    }
                }
            }

            let push_bb = entity_bb.expand(1.0e-7, 1.0e-7, 1.0e-7);

            let other_entities = world.get_entities_at_box(&push_bb);
            for other in other_entities {
                if other.get_entity().entity_id != self_entity.entity_id {
                    let other_type = other.get_entity().entity_type.id;
                    let is_other_minecart = is_minecart_fn(other_type);
                    let is_iron_golem = other_type == EntityType::IRON_GOLEM.id;

                    if is_rideable_minecart {
                        if (is_iron_golem
                            || is_other_minecart
                            || is_vehicle
                            || !other.get_entity().has_vehicle())
                            && other.is_pushable()
                        {
                            dyn_self.push(other.as_ref());
                            pushed = true;
                        }
                    } else if !self.has_passenger(other.as_ref())
                        && other.is_pushable()
                        && is_other_minecart
                    {
                        dyn_self.push(other.as_ref());
                        pushed = true;
                    }
                }
            }

            let players = world.get_players_at_box(&push_bb);
            for player in players {
                if player.get_entity().entity_id != self_entity.entity_id && is_rideable_minecart {
                    dyn_self.push(player.as_ref());
                    pushed = true;
                    // 原版中不可乘坐的矿车（漏斗、箱子矿车）不会推动玩家。
                }
            }
        } else {
            let other_entities = world.get_entities_at_box(&entity_bb);
            for other in other_entities {
                if other.get_entity().entity_id != self_entity.entity_id {
                    dyn_self.push(other.as_ref());
                    pushed = true;
                }
            }

            let players = world.get_players_at_box(&entity_bb);
            for player in players {
                if player.get_entity().entity_id != self_entity.entity_id {
                    dyn_self.push(player.as_ref());
                    pushed = true;
                }
            }
        }

        picked_up && !pushed
    }

    fn on_hit(&self, _hit: crate::entity::projectile::ProjectileHit) {}

    fn set_paddle_state(&self, _left: bool, _right: bool) {}

    fn is_in_love(&self) -> bool {
        false
    }

    fn is_breeding_ready(&self) -> bool {
        false
    }

    fn reset_love(&self) {}

    fn set_breeding_cooldown(&self, _ticks: i32) {}

    fn is_panicking(&self) -> bool {
        false
    }

    fn get_entity(&self) -> &Entity;

    fn get_living_entity(&self) -> Option<&LivingEntity>;

    fn cast_any(&self) -> &dyn std::any::Any;

    fn get_item_entity(&self) -> Option<&ItemEntity> {
        None
    }

    fn get_player(&self) -> Option<&Player> {
        None
    }

    /// 应返回不带点击或悬停事件的实体名称。
    fn get_name(&self) -> TextComponent {
        let entity = self.get_entity();
        entity
            .custom_name
            .load()
            .as_ref()
            .clone()
            .unwrap_or(TextComponent::translate(
                format!("entity.minecraft.{}", entity.entity_type.resource_name),
                [],
            ))
    }

    fn get_display_name(&self) -> TextComponent {
        // TODO: 队伍颜色
        let entity = self.get_entity();
        let mut name =
            entity
                .custom_name
                .load()
                .as_ref()
                .clone()
                .unwrap_or(TextComponent::translate(
                    format!("entity.minecraft.{}", entity.entity_type.resource_name),
                    [],
                ));
        let name_clone = name.clone();
        name = name.hover_event(HoverEvent::show_entity(
            entity.entity_uuid.to_string(),
            entity.entity_type.resource_name.into(),
            Some(name_clone),
        ));
        name = name.insertion(entity.entity_uuid.to_string());
        name
    }

    /// 杀死该实体。
    fn kill(&self, caller: &dyn EntityBase) {
        if self.get_living_entity().is_some() {
            caller.damage(caller, f32::MAX, DamageType::GENERIC_KILL);
        } else {
            // TODO 所有实体实现完成后应移除此处
            self.get_entity().remove();
        }
    }

    fn get_experience_reward(&self, _killer: Option<&dyn EntityBase>) -> u32 {
        0
    }

    fn get_base_experience_reward(&self) -> u32 {
        0
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum RemovalReason {
    Killed,
    Discarded,
    UnloadedToChunk,
    UnloadedWithPlayer,
    ChangedDimension,
}

impl RemovalReason {
    #[must_use]
    pub const fn should_destroy(&self) -> bool {
        match self {
            Self::Killed | Self::Discarded => true,
            Self::UnloadedToChunk | Self::UnloadedWithPlayer | Self::ChangedDimension => false,
        }
    }

    #[must_use]
    pub const fn should_save(&self) -> bool {
        match self {
            Self::Killed | Self::Discarded | Self::UnloadedWithPlayer | Self::ChangedDimension => {
                false
            }
            Self::UnloadedToChunk => true,
        }
    }
}

// IMPORTANT：此处必须是 1 而不是 0，因为 fetch_add 返回的是之前的值，0 会无效
static CURRENT_ID: AtomicI32 = AtomicI32::new(1);

/// 表示非生物实体（如物品、鸡蛋、雪球……）
pub struct Entity {
    /// 该实体的唯一标识符
    pub entity_id: i32,
    /// 该实体的持久且唯一的标识符
    pub entity_uuid: uuid::Uuid,
    /// 实体类型（例如玩家、僵尸、物品）
    pub entity_type: &'static EntityType,
    /// 实体所在的世界。
    /// 使用 `ArcSwap` 以便在切换维度时进行原子更新。
    pub world: ArcSwap<World>,
    /// 实体在世界中的当前位置
    pub pos: AtomicCell<Vector3<f64>>,
    /// 实体最后已知的位置。
    pub last_pos: AtomicCell<Vector3<f64>>,
    /// 最后一次移动向量
    pub movement: AtomicCell<Vector3<f64>>,
    /// 实体位置四舍五入到的最近方块坐标
    pub block_pos: AtomicCell<BlockPos>,
    /// 支撑该实体的方块
    pub supporting_block_pos: AtomicCell<Option<BlockPos>>,
    /// 实体当前位置的区块坐标
    pub chunk_pos: AtomicCell<Vector2<i32>>,
    /// 表示实体是否在潜行
    pub sneaking: AtomicBool,
    /// 表示实体是否在疾跑
    pub sprinting: AtomicBool,
    /// 表示实体是否在游泳
    pub swimming: AtomicBool,
    /// 表示实体是否隐形
    pub invisible: AtomicBool,
    /// 表示实体是否发光
    pub glowing: AtomicBool,
    /// 表示实体是否处于因坠落触发的飞行状态
    pub fall_flying: AtomicBool,
    /// 实体当前的速度向量，也称击退
    pub velocity: AtomicCell<Vector3<f64>>,
    /// 记录一次水平碰撞
    pub horizontal_collision: AtomicBool,
    /// 表示实体是否位于地面（不一定总是准确）。
    pub on_ground: AtomicBool,
    /// 表示实体是否接触水
    pub touching_water: AtomicBool,
    /// 表示流体高度
    pub water_height: AtomicCell<f64>,
    /// 表示实体是否接触熔岩
    pub touching_lava: AtomicBool,
    /// 表示流体高度
    pub lava_height: AtomicCell<f64>,
    /// 实体的偏航角（水平旋转）← →
    pub yaw: AtomicCell<f32>,
    /// 实体的头部偏航角（头部的水平旋转）
    pub head_yaw: AtomicCell<f32>,
    /// 实体的身体偏航角（身体的水平旋转）
    pub body_yaw: AtomicCell<f32>,
    /// 实体的俯仰角（垂直旋转）↑ ↓
    pub pitch: AtomicCell<f32>,
    /// 实体当前的姿态（例如站立、坐下、游泳）。
    pub pose: AtomicCell<EntityPose>,
    /// 实体的包围盒（碰撞箱）
    pub bounding_box: AtomicCell<BoundingBox>,
    ///边界框的尺寸（宽度和高度）
    pub entity_dimension: AtomicCell<EntityDimensions>,
    /// 此实体是否对所有伤害免疫
    pub invulnerable: AtomicBool,
    /// 此实体免疫的伤害类型列表
    pub damage_immunities: std::sync::Mutex<Vec<DamageType>>,
    // 实体是否免疫火焰（用于禁用视觉火焰与火焰伤害）
    pub fire_immune: AtomicBool,
    pub fire_ticks: AtomicI32,
    pub has_visual_fire: AtomicBool,
    /// 实体被冻结（处于细雪中）的刻数
    /// 最大值为 140 刻（7 秒）。在细雪中每刻增加 1，离开后每刻减少 2。
    pub frozen_ticks: AtomicI32,
    /// 在方块碰撞处理期间，当实体接触细雪时设置。
    pub is_in_powder_snow: AtomicBool,
    /// 若实体在上一刻处于细雪中，则为 true。
    pub was_in_powder_snow: AtomicBool,
    pub removal_reason: AtomicCell<Option<RemovalReason>>,
    // 该实体拥有的乘客
    pub passengers: std::sync::Mutex<Vec<Arc<dyn EntityBase>>>,

    /// 该实体可同时搭载的乘客数上限（原版 canAddPassenger/maxPassengers）。
    /// 绝大多数实体为 1（马、猪、炽足兽、矿车等），船与骆驼为 2。
    pub max_seats: usize,
    /// 该实体所在的载具
    pub vehicle: std::sync::Mutex<Option<Arc<dyn EntityBase>>>,
    /// 此实体被附着/拴系到的实体（如有）
    pub leashed_to: std::sync::Mutex<Option<Arc<dyn EntityBase>>>,
    /// 实体下坐骑后再次骑乘前的冷却时间
    pub riding_cooldown: AtomicI32,
    /// 实体的年龄（以刻为单位）。负值表示幼年。
    pub age: AtomicI32,

    pub current_biome: ArcSwap<&'static Biome>,
    pub last_biome_update_pos: AtomicCell<BlockPos>,

    pub portal_cooldown: AtomicU32,

    pub portal_manager: std::sync::Mutex<Option<PortalProcessor>>,
    /// 实体的自定义名称
    pub custom_name: ArcSwap<Option<TextComponent>>,
    /// 表示实体的自定义名称是否可见
    pub custom_name_visible: AtomicBool,
    pub silent: AtomicBool,
    pub has_no_gravity: AtomicBool,
    /// 附加到此实体上的记分板标签，通过 `/tag` 管理。
    /// 原版允许每个实体最多拥有 [`MAX_SCOREBOARD_TAGS`] 个标签。
    pub scoreboard_tags: std::sync::Mutex<HashSet<String>>,
    /// 实体生成数据包中发送的数据
    pub data: AtomicI32,
    /// 存储实体布尔标志（着火、潜行、隐身、发光等）
    pub flags: std::sync::atomic::AtomicI8,
    /// 如果为 true，实体将无视物理、碰撞和方块效果（如旁观模式、标记、展示实体）
    pub no_physics: AtomicBool,
    pub synched_data: synched_entity_data::SynchedEntityData,
    /// 在被重置前的一刻内对移动进行乘算
    pub movement_multiplier: AtomicCell<Vector3<f64>>,
    /// 判断是否需要发送实体的速度
    pub velocity_dirty: AtomicBool,
    /// 当实体即将被移除但仍可能被引用时设置
    pub removed: AtomicBool,
    /// 上次发送的偏航角值（编码为 u8），用于变化检测
    pub last_sent_yaw: AtomicU8,
    /// 上次发送的俯仰角值（编码为 u8），用于变化检测
    pub last_sent_pitch: AtomicU8,
    /// 缓存上次发送的位置，以优化实体位置更新数据包
    pub last_sent_pos: AtomicCell<Vector3<f64>>,
    /// 缓存上次发送的头部偏航角字节
    pub last_sent_head_yaw: AtomicU8,
    /// 面向插件的持久自定义数据容器（对应 Bukkit 的 `PersistentDataHolder`）
    pub custom_data: std::sync::Mutex<NbtCompound>,
    /// 自身作为 trait 对象的弱引用句柄，在被加入世界时登记
    /// （见 `World::register_entity_in_chunk_index`）。`set_pos` 跨块
    /// 移动时凭它向新桶插入，供 `get_entities_at_box` 分块索引查询。
    pub chunk_index_handle: OnceLock<Weak<dyn EntityBase>>,
}

impl Entity {
    pub fn new(
        world: Arc<World>,
        position: Vector3<f64>,
        entity_type: &'static EntityType,
    ) -> Self {
        Self::from_uuid(Uuid::new_v4(), world, position, entity_type)
    }

    pub fn reserve_ids(count: i32) -> i32 {
        CURRENT_ID.fetch_add(count, Relaxed)
    }

    pub fn from_uuid(
        entity_uuid: uuid::Uuid,
        world: Arc<World>,
        position: Vector3<f64>,
        entity_type: &'static EntityType,
    ) -> Self {
        Self::from_uuid_with_id(
            CURRENT_ID.fetch_add(1, Relaxed),
            entity_uuid,
            world,
            position,
            entity_type,
        )
    }

    pub fn from_uuid_with_id(
        entity_id: i32,
        entity_uuid: uuid::Uuid,
        world: Arc<World>,
        position: Vector3<f64>,
        entity_type: &'static EntityType,
    ) -> Self {
        let floor_x = position.x.floor() as i32;
        let floor_y = position.y.floor() as i32;
        let floor_z = position.z.floor() as i32;

        let bounding_box_size = EntityDimensions {
            width: entity_type.dimension[0],
            height: entity_type.dimension[1],
            eye_height: entity_type.eye_height,
        };

        let current_biome = world
            .level
            .get_rough_biome(&BlockPos::new(floor_x, floor_y, floor_z));

        Self {
            entity_id,
            entity_uuid,
            entity_type,
            on_ground: AtomicBool::new(false),
            touching_water: AtomicBool::new(false),
            water_height: AtomicCell::new(0.0),
            touching_lava: AtomicBool::new(false),
            lava_height: AtomicCell::new(0.0),
            horizontal_collision: AtomicBool::new(false),
            pos: AtomicCell::new(position),
            last_pos: AtomicCell::new(position),
            movement: AtomicCell::new(Vector3::default()),
            block_pos: AtomicCell::new(BlockPos(Vector3::new(floor_x, floor_y, floor_z))),
            supporting_block_pos: AtomicCell::new(None),
            chunk_pos: AtomicCell::new(Vector2::new(
                get_section_cord(floor_x),
                get_section_cord(floor_z),
            )),
            sneaking: AtomicBool::new(false),
            swimming: AtomicBool::new(false),
            invisible: AtomicBool::new(false),
            glowing: AtomicBool::new(false),
            world: ArcSwap::new(world),
            sprinting: AtomicBool::new(false),
            fall_flying: AtomicBool::new(false),
            yaw: AtomicCell::new(0.0),
            head_yaw: AtomicCell::new(0.0),
            body_yaw: AtomicCell::new(0.0),
            pitch: AtomicCell::new(0.0),
            velocity: AtomicCell::new(Vector3::new(0.0, 0.0, 0.0)),
            pose: AtomicCell::new(EntityPose::Standing),
            bounding_box: AtomicCell::new(BoundingBox::new_from_pos(
                position.x,
                position.y,
                position.z,
                &bounding_box_size,
            )),
            entity_dimension: AtomicCell::new(bounding_box_size),
            invulnerable: AtomicBool::new(false),
            damage_immunities: std::sync::Mutex::new(Vec::new()),
            data: AtomicI32::new(0),
            flags: std::sync::atomic::AtomicI8::new(0),
            fire_immune: AtomicBool::new(false),
            fire_ticks: AtomicI32::new(-1),
            has_visual_fire: AtomicBool::new(false),
            frozen_ticks: AtomicI32::new(0),
            is_in_powder_snow: AtomicBool::new(false),
            was_in_powder_snow: AtomicBool::new(false),
            removal_reason: AtomicCell::new(None),
            passengers: std::sync::Mutex::new(Vec::new()),
            max_seats: 1,
            vehicle: std::sync::Mutex::new(None),
            leashed_to: std::sync::Mutex::new(None),

            riding_cooldown: AtomicI32::new(0),
            age: AtomicI32::new(0),
            current_biome: ArcSwap::new(Arc::new(current_biome)),
            last_biome_update_pos: AtomicCell::new(BlockPos::new(floor_x, floor_y, floor_z)),
            portal_cooldown: AtomicU32::new(0),
            portal_manager: std::sync::Mutex::new(None),
            custom_name: ArcSwap::new(Arc::new(None)),
            custom_name_visible: AtomicBool::new(false),
            silent: AtomicBool::new(false),
            has_no_gravity: AtomicBool::new(false),
            scoreboard_tags: std::sync::Mutex::new(HashSet::new()),
            no_physics: AtomicBool::new(false),
            synched_data: synched_entity_data::SynchedEntityData::new(),
            movement_multiplier: AtomicCell::new(Vector3::default()),
            velocity_dirty: AtomicBool::new(true),
            removed: AtomicBool::new(false),
            last_sent_yaw: AtomicU8::new(0),
            last_sent_pitch: AtomicU8::new(0),
            last_sent_head_yaw: AtomicU8::new(0),
            last_sent_pos: AtomicCell::new(position),
            custom_data: std::sync::Mutex::new(NbtCompound::new()),
            chunk_index_handle: OnceLock::new(),
        }
    }

    pub fn add_velocity(&self, velocity: Vector3<f64>) {
        self.set_velocity(self.velocity.load() + velocity);
    }

    pub fn set_velocity(&self, velocity: Vector3<f64>) {
        self.velocity.store(velocity);
        self.send_velocity();
    }

    /// 更新此实体的世界引用。
    /// 当实体切换维度时调用（例如穿过下界传送门）。
    pub fn set_world(&self, world: Arc<World>) {
        let block_pos = self.block_pos.load();
        let biome = world.level.get_rough_biome(&block_pos);
        self.current_biome.store(Arc::new(biome));
        self.last_biome_update_pos.store(block_pos);
        self.world.store(world);
    }

    /// 以刻为单位设置实体的年龄。
    /// 负值表示该实体是幼年个体。
    pub fn set_age(&self, age: i32) {
        self.age.store(age, Relaxed);
    }

    /// 向此实体添加一个记分板标签。
    ///
    ///若实体已拥有该标签或已带有对应内容，则返回 `false`
    /// [`MAX_SCOREBOARD_TAGS`] 个标签。
    pub fn add_scoreboard_tag(&self, tag: &str) -> bool {
        let mut tags = self
            .scoreboard_tags
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        tags.len() < MAX_SCOREBOARD_TAGS && tags.insert(tag.to_owned())
    }

    /// 从此实体移除一个记分板标签。
    ///
    ///若实体原本没有该标签，则返回 `false`。
    pub fn remove_scoreboard_tag(&self, tag: &str) -> bool {
        self.scoreboard_tags
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(tag)
    }

    /// 为实体设置自定义名称，通常与命名牌配合使用
    pub fn set_custom_name(&self, name: TextComponent) {
        self.custom_name.store(Arc::new(Some(name.clone())));
        self.set_synced_data(tracked_data::entity::DATA_CUSTOM_NAME, Some(name));
    }

    pub fn set_custom_name_visible(&self, visible: bool) {
        self.custom_name_visible.store(visible, Ordering::Relaxed);
        self.set_synced_data(tracked_data::entity::DATA_CUSTOM_NAME_VISIBLE, visible);
    }

    pub fn is_silent(&self) -> bool {
        self.silent.load(Ordering::Relaxed)
    }

    pub fn set_silent(&self, silent: bool) {
        self.silent.store(silent, Ordering::Relaxed);
        self.set_synced_data(tracked_data::entity::DATA_SILENT, silent);
    }

    pub fn has_no_gravity(&self) -> bool {
        self.has_no_gravity.load(Ordering::Relaxed)
    }

    pub fn set_has_no_gravity(&self, no_gravity: bool) {
        self.has_no_gravity.store(no_gravity, Ordering::Relaxed);
        self.set_synced_data(tracked_data::entity::DATA_NO_GRAVITY, no_gravity);
    }

    pub fn send_velocity(&self) {
        let velocity = self.velocity.load();
        let chunk_pos = self.chunk_pos.load();
        self.world.load().broadcast_to_chunk(
            chunk_pos,
            &CEntityVelocity::new(self.entity_id.into(), velocity),
        );
    }

    #[must_use]
    pub const fn get_entity_dimensions(pose: EntityPose) -> EntityDimensions {
        match pose {
            EntityPose::Sleeping => EntityDimensions::new(0.2, 0.2, 0.2),
            EntityPose::FallFlying | EntityPose::Swimming | EntityPose::SpinAttack => {
                EntityDimensions::new(0.6, 0.6, 0.4)
            }
            EntityPose::Crouching => EntityDimensions::new(0.6, 1.5, 1.27),
            EntityPose::Dying => EntityDimensions::new(0.2, 0.2, 1.62),
            _ => EntityDimensions::new(0.6, 1.8, 1.62),
        }
    }

    pub fn get_eye_height(&self) -> f64 {
        f64::from(Self::get_entity_dimensions(self.pose.load()).eye_height)
    }

    /// 更新实体的位置、方块位置和区块位置。
    ///
    /// 此函数根据提供的坐标计算新位置、方块位置和区块位置。如果其中任何值发生变化，则更新对应的字段。
    pub fn set_pos(&self, new_position: Vector3<f64>) {
        let pos = self.pos.load();
        if pos != new_position {
            self.pos.store(new_position);
            self.bounding_box.store(BoundingBox::new_from_pos(
                new_position.x,
                new_position.y,
                new_position.z,
                &self.entity_dimension.load(),
            ));

            let floor_x = new_position.x.floor() as i32;
            let floor_y = new_position.y.floor() as i32;
            let floor_z = new_position.z.floor() as i32;

            let block_pos = self.block_pos.load();
            let block_pos_vec = block_pos.0;
            if floor_x != block_pos_vec.x
                || floor_y != block_pos_vec.y
                || floor_z != block_pos_vec.z
            {
                let new_block_pos = Vector3::new(floor_x, floor_y, floor_z);
                let new_bp = BlockPos(new_block_pos);
                self.block_pos.store(new_bp);

                let world = self.world.load();
                let biome = world.level.get_rough_biome(&new_bp);
                self.current_biome.store(Arc::new(biome));
                self.last_biome_update_pos.store(new_bp);

                let chunk_pos = self.chunk_pos.load();
                let new_chunk_x = get_section_cord(new_block_pos.x);
                let new_chunk_z = get_section_cord(new_block_pos.z);
                if get_section_cord(floor_x) != chunk_pos.x
                    || get_section_cord(floor_z) != chunk_pos.y
                {
                    self.chunk_pos.store(Vector2::new(new_chunk_x, new_chunk_z));
                    // 跨块移动：向新区块桶插入自身弱引用（旧桶的过期项
                    // 由查询侧 AABB 过滤排除并惰性清理）。凭加入世界时
                    // 登记的句柄取回自身的 Arc，无需全局查找。
                    if let Some(entity) = self
                        .chunk_index_handle
                        .get()
                        .and_then(std::sync::Weak::upgrade)
                    {
                        world
                            .entities_by_chunk
                            .insert(Vector2::new(new_chunk_x, new_chunk_z), &entity);
                    }
                }
            }
        }
    }

    ///以向量形式返回实体旋转
    pub fn rotation(&self) -> Vector3<f32> {
        let pitch_rad = self.pitch.load().to_radians();
        let yaw_rad = -self.yaw.load().to_radians();

        let cos_yaw = yaw_rad.cos();
        let sin_yaw = yaw_rad.sin();
        let cos_pitch = pitch_rad.cos();
        let sin_pitch = pitch_rad.sin();

        Vector3::new(sin_yaw * cos_pitch, -sin_pitch, cos_yaw * cos_pitch)
    }

    /// 更改此实体的俯仰角和偏航角以注视目标
    pub fn look_at(&self, target: Vector3<f64>) {
        let position = self.pos.load();
        let delta = target.sub(&position);
        let root = delta.x.hypot(delta.z);
        let pitch = wrap_degrees((-delta.y.atan2(root) as f32).to_degrees());
        let yaw = wrap_degrees((delta.z.atan2(delta.x) as f32).to_degrees() - 90.0);
        self.pitch.store(pitch);
        self.yaw.store(yaw);
    }

    pub fn send_rotation(&self) {
        let yaw = self.yaw.load();
        let pitch = self.pitch.load();
        let chunk_pos = self.chunk_pos.load();

        // 广播更新数据包。

        let yaw = (yaw * 256.0 / 360.0).rem_euclid(256.0) as u8;
        let pitch = (pitch * 256.0 / 360.0).rem_euclid(256.0) as u8;

        if yaw == self.last_sent_yaw.load(Relaxed) && pitch == self.last_sent_pitch.load(Relaxed) {
            return;
        }

        self.last_sent_yaw.store(yaw, Relaxed);
        self.last_sent_pitch.store(pitch, Relaxed);

        self.world.load().broadcast_to_chunk(
            chunk_pos,
            &CUpdateEntityRot::new(
                self.entity_id.into(),
                yaw,
                pitch,
                self.on_ground.load(Relaxed),
            ),
        );

        self.send_head_rot(yaw);
    }

    pub fn send_head_rot(&self, head_yaw: u8) {
        let chunk_pos = self.chunk_pos.load();
        if head_yaw == self.last_sent_head_yaw.load(Relaxed) {
            return;
        }
        self.last_sent_head_yaw.store(head_yaw, Relaxed);

        self.world
            .load()
            .broadcast_to_chunk(chunk_pos, &CHeadRot::new(self.entity_id.into(), head_yaw));
    }

    fn default_portal_cooldown(&self) -> u32 {
        if self.entity_type == &EntityType::PLAYER {
            10
        } else {
            300
        }
    }

    ///返回（非玩家）实体所站立方块的方块位置（如果有的话）。
    pub fn get_supporting_block_pos(&self) -> Option<BlockPos> {
        // 检查实体是否在地面上
        if !self.on_ground.load(Ordering::Relaxed) {
            return None;
        }

        self.supporting_block_pos.load()
    }

    #[expect(clippy::float_cmp)]
    fn adjust_movement_for_collisions(
        &self,
        movement: Vector3<f64>,
        caller: &dyn EntityBase,
    ) -> Vector3<f64> {
        if movement.length_squared() == 0.0 {
            return movement;
        }

        self.on_ground.store(false, Ordering::SeqCst);
        self.supporting_block_pos.store(None);
        self.horizontal_collision.store(false, Ordering::SeqCst);

        let bounding_box = self.bounding_box.load();

        let (collisions, block_positions) = self
            .world
            .load()
            .get_block_collisions(bounding_box.stretch(movement), caller);

        if collisions.is_empty() {
            return movement;
        }

        let mut adjusted_movement = movement;

        // Y 轴调整
        if movement.get_axis(Axis::Y) != 0.0 {
            let mut max_time = 1.0;
            let mut positions = block_positions.into_iter();
            if let Some((mut collisions_len, mut position)) = positions.next() {
                let mut supporting_block_pos = None;

                for (i, inert_box) in collisions.iter().enumerate() {
                    if i == collisions_len {
                        let Some((next_len, next_pos)) = positions.next() else {
                            break;
                        };
                        collisions_len = next_len;
                        position = next_pos;
                    }

                    if let Some(collision_time) = bounding_box.calculate_collision_time(
                        inert_box,
                        adjusted_movement,
                        Axis::Y,
                        max_time,
                    ) {
                        max_time = collision_time;

                        // 如果实体向下移动并发生碰撞，设置支撑方块位置
                        if movement.get_axis(Axis::Y) < 0.0 {
                            supporting_block_pos = Some(position);
                        }
                    }
                }

                if max_time != 1.0 {
                    let changed_component = adjusted_movement.get_axis(Axis::Y) * max_time;
                    adjusted_movement.set_axis(Axis::Y, changed_component);
                }

                self.on_ground
                    .store(supporting_block_pos.is_some(), Ordering::SeqCst);
                self.supporting_block_pos.store(supporting_block_pos);
            }
        }

        let mut horizontal_collision = false;

        for axis in Axis::horizontal() {
            if movement.get_axis(axis) == 0.0 {
                continue;
            }

            let mut max_time = 1.0;

            for inert_box in &collisions {
                if let Some(collision_time) = bounding_box.calculate_collision_time(
                    inert_box,
                    adjusted_movement,
                    axis,
                    max_time,
                ) {
                    max_time = collision_time;
                }
            }

            if max_time != 1.0 {
                let changed_component = adjusted_movement.get_axis(axis) * max_time;
                adjusted_movement.set_axis(axis, changed_component);
                horizontal_collision = true;
            }
        }

        self.horizontal_collision
            .store(horizontal_collision, Ordering::SeqCst);

        adjusted_movement
    }

    /// 按照原版 Minecraft 的机制对实体施加击退。
    /// `LivingEntity.takeKnockback()`
    /// 此函数根据指定的击退强度和方向计算实体的新速度。
    ///
    /// 此处不应用击退抗性，因为它是 `LivingEntity` 的
    /// 属性，而这是一个 `Entity` 方法。想模拟原版
    /// `LivingEntity.knockback` 会随
    /// `combat::knockback_after_resistance`；要建模原版原始
    /// `Entity.push`（例如末影龙）会以未缩放的形式传递 `strength`。
    pub fn apply_knockback(&self, strength: f64, mut x: f64, mut z: f64) {
        if strength <= 0.0 {
            return;
        }

        self.velocity_dirty.store(true, Ordering::SeqCst);

        // 这里包含一些原版的“魔法”处理

        while x.mul_add(x, z * z) < 1.0E-5 {
            x = (rand::random::<f64>() - rand::random::<f64>()) * 0.01;

            z = (rand::random::<f64>() - rand::random::<f64>()) * 0.01;
        }

        let var8 = Vector3::new(x, 0.0, z).normalize() * strength;

        let velocity = self.velocity.load();

        let new_velocity = Vector3::new(
            velocity.x / 2.0 - var8.x,
            if self.on_ground.load(Relaxed) {
                (velocity.y / 2.0 + strength).min(0.4)
            } else {
                velocity.y
            },
            velocity.z / 2.0 - var8.z,
        );

        let mut event =
            crate::plugin::api::events::entity::entity_knockback::EntityKnockbackEvent {
                entity_id: self.entity_id,
                hit_by_id: None,
                knockback: new_velocity.sub(&velocity),
                cancelled: false,
            };
        if let Some(server) = self.world.load().server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
        if event.cancelled {
            return;
        }

        self.velocity.store(new_velocity);
    }

    // 对应 yarn 中 LivingEntity.tickMovement() 的一部分

    pub fn check_zero_velo(&self) {
        let mut motion = self.velocity.load();

        if self.entity_type == &EntityType::PLAYER {
            if motion.horizontal_length_squared() < 9.0E-6 {
                motion.x = 0.0;

                motion.z = 0.0;
            }
        } else {
            if motion.x.abs() < 0.003 {
                motion.x = 0.0;
            }

            if motion.z.abs() < 0.003 {
                motion.z = 0.0;
            }
        }

        if motion.y.abs() < 0.003 {
            motion.y = 0.0;
        }

        self.velocity.store(motion);
    }

    #[expect(dead_code)]
    const fn tick_block_underneath() {
        // let world = self.world.read();

        // let (pos, block, state) = self.get_block_with_y_offset(0.2);

        // 世界
        //     .block_registry
        //     .on_stepped_on(&world, caller, pos, block, state)
        //     ;

        // TODO: 将此添加到 on_stepped_on

        /*


        if self.on_ground.load(Ordering::SeqCst) {


            let (_pos, block, state) = self.get_block_with_y_offset(0.2);


            if let Some(live) = living {


                if block == Block::CAMPFIRE


                    || block == Block::SOUL_CAMPFIRE


                        && CampfireLikeProperties::from_state_id(state.id).r#signal_fire


                {


                    let _ = live.damage(1.0, DamageType::CAMPFIRE);


                }





                if block == Block::MAGMA_BLOCK {


                    let _ = live.damage(1.0, DamageType::HOT_FLOOR);


                }


            }


        }


        */
    }

    pub fn tick_block_collisions(&self, caller: &dyn EntityBase) -> bool {
        if !self.is_affected_by_blocks() {
            return false;
        }

        let bounding_box = self.bounding_box.load();
        let aabb = bounding_box.expand(-1.0e-7, -1.0e-7, -1.0e-7);

        let min = aabb.min_block_pos();
        let max = aabb.max_block_pos();

        let eye_height = self.get_eye_height();
        let eye_width = f64::from(self.width()) * 0.8;
        let mut eye_level_box = aabb;
        let shrink_x = (aabb.max.x - aabb.min.x - eye_width) / 2.0;
        let shrink_z = (aabb.max.z - aabb.min.z - eye_width) / 2.0;
        eye_level_box.min.x += shrink_x;
        eye_level_box.max.x -= shrink_x;
        eye_level_box.min.z += shrink_z;
        eye_level_box.max.z -= shrink_z;
        eye_level_box.min.y += eye_height;
        eye_level_box.max.y = eye_level_box.min.y;

        let mut suffocating = false;
        let world = self.world.load();

        // 极热路径（每实体每刻每重叠方块）：仅
        // 仅在插件实际监听时才构建事件。
        let inside_block_server = world.server.upgrade().filter(|server| {
            server.plugin_manager.has_handlers::<
                crate::plugin::api::events::entity::entity_inside_block::EntityInsideBlockEvent,
            >()
        });

        for pos in BlockPos::iterate(min, max) {
            let (block, state) = world.get_block_and_state(&pos);
            if state.is_air() {
                continue;
            }

            // TODO: 这是默认谓词，原版会对某些方块覆盖它，
            // 见 Blocks.java 中的 .suffocates(...)
            let check_suffocation =
                !suffocating && blocks_movement(state, block.id) && state.is_full_cube();

            World::check_collision(
                &bounding_box,
                pos,
                state,
                check_suffocation,
                |collision_shape: &BoundingBox| {
                    if collision_shape.intersects(&eye_level_box) {
                        suffocating = true;
                    }
                },
            );

            let collision_shape = if block == &Block::POWDER_SNOW {
                crate::block::blocks::powder_snow::inside_collision_shape_for_entity(caller, &pos)
            } else {
                world
                    .block_registry
                    .get_inside_collision_shape(block, &world, state, &pos)
            };

            if bounding_box.intersects(&collision_shape.at_pos(pos)) {
                if block == &Block::POWDER_SNOW {
                    self.is_in_powder_snow.store(true, Relaxed);
                }
                if let Some(server) = &inside_block_server {
                    let mut event = crate::plugin::api::events::entity::entity_inside_block::EntityInsideBlockEvent::new(
                        self.entity_id,
                        pos,
                        format!("minecraft:{}", block.name),
                    );
                    // 纯通知：不支持取消。
                    server.plugin_manager.fire_blocking(server, &mut event);
                }
                if let Some(server_arc) = world.server.upgrade() {
                    world.block_registry.on_entity_collision(
                        block,
                        &world,
                        caller,
                        &pos,
                        state,
                        &server_arc,
                    );
                }
            }
        }

        suffocating
    }

    pub fn send_pos_rot(&self) {
        let old = self.last_sent_pos.load();
        let new = self.pos.load();
        let chunk_pos = self.chunk_pos.load();

        let converted = Vector3::new(
            new.x.mul_add(4096.0, -(old.x * 4096.0)) as i16,
            new.y.mul_add(4096.0, -(old.y * 4096.0)) as i16,
            new.z.mul_add(4096.0, -(old.z * 4096.0)) as i16,
        );

        let yaw = self.yaw.load();

        let pitch = self.pitch.load();
        let yaw = (yaw * 256.0 / 360.0).rem_euclid(256.0) as u8;
        let pitch = (pitch * 256.0 / 360.0).rem_euclid(256.0) as u8;

        // 仅在位置或旋转确实发生变化时才广播。
        let pos_changed = converted.x != 0 || converted.y != 0 || converted.z != 0;
        let rot_changed =
            yaw != self.last_sent_yaw.load(Relaxed) || pitch != self.last_sent_pitch.load(Relaxed);

        if !pos_changed && !rot_changed {
            return;
        }

        self.last_sent_pos.store(new);
        self.last_sent_yaw.store(yaw, Relaxed);
        self.last_sent_pitch.store(pitch, Relaxed);

        // 动态选择最高效的数据包
        if pos_changed && rot_changed {
            let je_packet = CUpdateEntityPosRot::new(
                self.entity_id.into(),
                Vector3::new(converted.x, converted.y, converted.z),
                yaw,
                pitch,
                self.on_ground.load(Relaxed),
            );
            self.world.load().broadcast_to_chunk(chunk_pos, &je_packet);
        } else if pos_changed {
            let je_packet = CUpdateEntityPos::new(
                self.entity_id.into(),
                Vector3::new(converted.x, converted.y, converted.z),
                self.on_ground.load(Relaxed),
            );
            self.world.load().broadcast_to_chunk(chunk_pos, &je_packet);
        } else if rot_changed {
            let je_packet = CUpdateEntityRot::new(
                self.entity_id.into(),
                yaw,
                pitch,
                self.on_ground.load(Relaxed),
            );
            self.world.load().broadcast_to_chunk(chunk_pos, &je_packet);
        }
        self.send_head_rot(yaw);
    }

    pub fn update_last_pos(&self) -> Vector3<f64> {
        let pos = self.pos.load();
        let old = self.last_pos.load();
        self.movement.store(pos - old);
        self.last_pos.store(pos);
        old
    }

    pub fn send_pos(&self) {
        let old = self.last_sent_pos.load();
        let new = self.pos.load();
        let chunk_pos = self.chunk_pos.load();

        let converted = Vector3::new(
            new.x.mul_add(4096.0, -(old.x * 4096.0)) as i16,
            new.y.mul_add(4096.0, -(old.y * 4096.0)) as i16,
            new.z.mul_add(4096.0, -(old.z * 4096.0)) as i16,
        );

        // 仅在位置确实发生变化时才广播。
        if converted.x == 0 && converted.y == 0 && converted.z == 0 {
            return;
        }

        self.last_sent_pos.store(new);

        let je_packet = CUpdateEntityPos::new(
            self.entity_id.into(),
            Vector3::new(converted.x, converted.y, converted.z),
            self.on_ground.load(Relaxed),
        );

        self.world.load().broadcast_to_chunk(chunk_pos, &je_packet);
    }

    // yarn 映射中的 updateWaterState()

    fn update_fluid_state(&self, caller: &dyn EntityBase) {
        let is_pushed = caller.is_pushed_by_fluids();
        let mut fluids = BTreeMap::new();

        let water_push = Vector3::default();

        let water_n = 0;

        let lava_push = Vector3::default();

        let lava_n = 0;

        let mut fluid_push = [water_push, lava_push];

        let mut fluid_n = [water_n, lava_n];

        let mut in_fluid = [false, false];

        // 所找到的最大流体高度

        let mut fluid_height: [f64; 2] = [0.0, 0.0];

        let entity_box = self.bounding_box.load();
        let bounding_box = entity_box.expand(-0.001, -0.001, -0.001);

        let min = bounding_box.min_block_pos();

        let max = bounding_box.max_block_pos();

        let world = self.world.load();

        for x in min.0.x..=max.0.x {
            for y in min.0.y..=max.0.y {
                for z in min.0.z..=max.0.z {
                    let pos = BlockPos::new(x, y, z);

                    let (fluid, state) = world.get_fluid_and_fluid_state(&pos);

                    if fluid.id != Fluid::EMPTY.id {
                        let surface_y =
                            f64::from(world.get_fluid_height(&pos, fluid, &state)) + f64::from(y);

                        if surface_y >= bounding_box.min.y {
                            let marginal_height = surface_y - entity_box.min.y;
                            let i = usize::from(
                                fluid.id == Fluid::FLOWING_LAVA.id || fluid.id == Fluid::LAVA.id,
                            );

                            fluid_height[i] = fluid_height[i].max(marginal_height);

                            in_fluid[i] = true;

                            if !is_pushed {
                                fluids.insert(fluid.id, fluid);

                                continue;
                            }

                            let mut fluid_velo = world.get_fluid_velocity(pos, fluid, &state);

                            if fluid_height[i] < 0.4 {
                                fluid_velo = fluid_velo * fluid_height[i];
                            }

                            fluid_push[i] += fluid_velo;

                            fluid_n[i] += 1;

                            fluids.insert(fluid.id, fluid);
                        }
                    }
                }
            }
        }

        // BTreeMap 会像原版一样自动将水排在岩浆之前

        for (_, fluid) in fluids {
            world
                .block_registry
                .on_entity_collision_fluid(fluid, caller);
        }

        let lava_speed = if world.dimension.fast_lava {
            0.007
        } else {
            0.002_333_333
        };

        self.push_by_fluid(0.014, fluid_push[0], fluid_n[0]);

        self.push_by_fluid(lava_speed, fluid_push[1], fluid_n[1]);

        let water_height = fluid_height[0];

        let in_water = in_fluid[0];

        if in_water {
            if let Some(living) = caller.get_living_entity() {
                living.fall_distance.store(0.0);
            }

            if !self.touching_water.load(Ordering::SeqCst) {

                // TODO: 生成水花粒子
            }
        }

        self.water_height.store(water_height);

        self.touching_water.store(in_water, Ordering::SeqCst);

        let lava_height = fluid_height[1];

        let in_lava = in_fluid[1];

        if in_lava && let Some(living) = caller.get_living_entity() {
            let halved_fall = living.fall_distance.load() / 2.0;

            if halved_fall != 0.0 {
                living.fall_distance.store(halved_fall);
            }
        }

        self.lava_height.store(lava_height);

        self.touching_lava.store(in_lava, Ordering::SeqCst);
    }

    fn push_by_fluid(&self, speed: f64, mut push: Vector3<f64>, n: usize) {
        if push.length_squared() != 0.0 {
            if n > 0 {
                push = push * (1.0 / (n as f64));
            }

            if self.entity_type != &EntityType::PLAYER {
                push = push.normalize();
            }

            push = push * speed;

            let velo = self.velocity.load();

            if velo.x.abs() < 0.003 && velo.z.abs() < 0.003 && velo.length_squared() < 0.000_020_25
            {
                push = push.normalize() * 0.0045;
            }

            self.velocity.store(velo + push);
        }
    }

    fn get_pos_with_y_offset(
        &self,
        offset: f64,
    ) -> (
        BlockPos,
        Option<&'static Block>,
        Option<&'static BlockState>,
    ) {
        if let Some(mut supporting_block) = self.supporting_block_pos.load() {
            if offset > 1.0e-5 {
                let (block, state) = self.world.load().get_block_and_state(&supporting_block);

                // if let Some(props) = block.properties(state.id) {
                //     let name = props.;

                //     if offset <= 0.5
                //         && (name == "OakFenceLikeProperties"
                //             || name == "ResinBrickWallLikeProperties"
                //             || name == "OakFenceGateLikeProperties"
                //                 && OakFenceGateLikeProperties::from_state_id(state.id)
                //                     .r#open)
                //     {
                //         return (supporting_block, Some(block), Some(state));
                //     }
                // }

                supporting_block.0.y = (self.pos.load().y - offset).floor() as i32;

                return (supporting_block, Some(block), Some(state));
            }

            return (supporting_block, None, None);
        }

        let mut block_pos = self.block_pos.load();

        block_pos.0.y = (self.pos.load().y - offset).floor() as i32;

        (block_pos, None, None)
    }

    fn get_block_with_y_offset(
        &self,
        offset: f64,
    ) -> (BlockPos, &'static Block, &'static BlockState) {
        let (pos, block, state) = self.get_pos_with_y_offset(offset);

        if let (Some(b), Some(s)) = (block, state) {
            (pos, b, s)
        } else {
            let (b, s) = self.world.load().get_block_and_state(&pos);

            (pos, b, s)
        }
    }

    // yarn 中的 Entity.updateVelocity

    fn update_velocity_from_input(&self, movement_input: Vector3<f64>, speed: f64) {
        let final_input = self.movement_input_to_velocity(movement_input, speed);

        self.velocity.store(self.velocity.load() + final_input);
    }

    // yarn 中的 Entity.movementInputToVelocity

    fn movement_input_to_velocity(&self, movement_input: Vector3<f64>, speed: f64) -> Vector3<f64> {
        let yaw = f64::from(self.yaw.load()).to_radians();

        let dist = movement_input.length_squared();

        if dist < 1.0e-7 {
            return Vector3::default();
        }

        let input = if dist > 1.0 {
            movement_input.normalize() * speed
        } else {
            movement_input * speed
        };

        let sin = yaw.sin();

        let cos = yaw.cos();

        Vector3::new(
            input.x.mul_add(cos, -(input.z * sin)),
            input.y,
            input.z.mul_add(cos, input.x * sin),
        )
    }

    #[must_use]
    pub fn get_block_pos_below_that_affects_my_movement(&self) -> BlockPos {
        self.get_pos_with_y_offset(0.500_001).0
    }

    #[must_use]
    #[expect(clippy::float_cmp)]
    pub fn get_block_speed_factor(&self) -> f32 {
        let world = self.world.load();
        let (block, _state) = world.get_block_and_state(&self.block_pos.load());
        let speed_factor_here = block.get_speed_factor();
        if block != &Block::WATER && block != &Block::BUBBLE_COLUMN {
            if speed_factor_here == 1.0 {
                let below_pos = self.get_block_pos_below_that_affects_my_movement();
                let (below_block, _below_state) = world.get_block_and_state(&below_pos);
                below_block.get_speed_factor()
            } else {
                speed_factor_here
            }
        } else {
            speed_factor_here
        }
    }

    #[expect(clippy::float_cmp)]
    fn get_jump_velocity_multiplier(&self) -> f32 {
        let f = self
            .world
            .load()
            .get_block(&self.block_pos.load())
            .jump_velocity_multiplier;

        let g = self
            .get_block_with_y_offset(0.500_001)
            .1
            .jump_velocity_multiplier;

        if f == 1f32 { g } else { f }
    }

    pub fn move_pos(&self, delta: Vector3<f64>) {
        self.set_pos(self.pos.load() + delta);
    }

    // 按增量移动，根据碰撞调整，并发送

    // 不发送移动，这必须单独完成
    pub fn move_entity(&self, caller: &dyn EntityBase, mut motion: Vector3<f64>) {
        if caller.get_player().is_some() {
            return;
        }

        if self.no_physics.load(Ordering::Relaxed) {
            self.move_pos(motion);
            self.horizontal_collision.store(false, Ordering::Relaxed);
            self.on_ground.store(false, Ordering::Relaxed);

            return;
        }

        let movement_multiplier = self.movement_multiplier.swap(Vector3::default());

        if movement_multiplier.length_squared() > 1.0e-7 {
            motion = motion.multiply(
                movement_multiplier.x,
                movement_multiplier.y,
                movement_multiplier.z,
            );

            self.velocity.store(Vector3::default());
        }

        let final_move = self.adjust_movement_for_collisions(motion, caller);

        // 极热路径：仅在插件实际监听时才构建该事件
        let world = self.world.load();
        let fire_move_event = final_move.length_squared() > 0.0
            && world.server.upgrade().is_some_and(|server| {
                server.plugin_manager.has_handlers::<
                    crate::plugin::api::events::entity::entity_move::EntityMoveEvent,
                >()
            });
        let from_position = fire_move_event.then(|| self.pos.load());

        self.move_pos(final_move);

        if let Some(from_position) = from_position
            && let Some(server) = world.server.upgrade()
        {
            let mut event = crate::plugin::api::events::entity::entity_move::EntityMoveEvent::new(
                self.entity_id,
                from_position,
                self.pos.load(),
            );
            server.plugin_manager.fire_blocking(&server, &mut event);
        }

        let velocity_multiplier = f64::from(caller.get_block_speed_factor());

        self.velocity.store(final_move * velocity_multiplier);

        if let Some(living) = caller.get_living_entity() {
            let on_ground = self.on_ground.load(Ordering::SeqCst);
            living.fall(caller, final_move.y, on_ground, false);
        }

        if motion.y != final_move.y {
            let world = self.world.load();
            let block = self.get_block_with_y_offset(0.2).1;
            world
                .block_registry
                .update_entity_movement_after_fall_on(block, caller);
        }
    }

    pub fn push_out_of_blocks(&self, center_pos: Vector3<f64>) {
        let block_pos = BlockPos::floored_v(center_pos);

        let delta = center_pos.sub(&block_pos.0.to_f64());

        let mut min_dist = f64::MAX;

        let mut direction = BlockDirection::Up;

        for dir in BlockDirection::all() {
            if dir == BlockDirection::Down {
                continue;
            }

            let offset = dir.to_offset();

            if self
                .world
                .load()
                .get_block_state(&block_pos.offset(offset))
                .is_full_cube()
            {
                continue;
            }

            let component = delta.get_axis(dir.to_axis().into());

            let dist = if dir.positive() {
                1.0 - component
            } else {
                component
            };

            if dist < min_dist {
                min_dist = dist;

                direction = dir;
            }
        }

        let amplitude = rand::random::<f64>().mul_add(0.2, 0.1);

        let axis = direction.to_axis().into();

        let sign = if direction.positive() { 1.0 } else { -1.0 };

        let mut velo = self.velocity.load();

        velo = velo * 0.75;

        velo.set_axis(axis, sign * amplitude);

        self.velocity.store(velo);
    }

    fn tick_portal(&self, caller: &dyn EntityBase) {
        if self.portal_cooldown.load(Ordering::Relaxed) > 0 {
            self.portal_cooldown.fetch_sub(1, Ordering::Relaxed);
        }
        let Ok(mut manager_guard) = self.portal_manager.try_lock() else {
            return;
        };
        let mut should_remove = false;
        if let Some(portal_processor) = manager_guard.as_mut() {
            if portal_processor.process_portal_teleportation(&self.world.load(), caller, true) {
                self.portal_cooldown
                    .store(self.default_portal_cooldown(), Ordering::Relaxed);

                let world_clone = self.world.load_full();
                let portal_type = portal_processor.portal_type;
                let dest_world_opt = portal_processor.destination_world.clone();
                let src_portal = portal_processor.source_portal.clone();
                let entry_position = portal_processor.entry_position;
                let entity_id = self.entity_id;
                let yaw = self.yaw.load();

                let rt_handle = world_clone.server.upgrade().map(|s| s.runtime.clone());
                rayon::spawn(move || {
                    let _guard = rt_handle.as_ref().map(tokio::runtime::Handle::enter);
                    let Some(entity_arc) = world_clone.get_entity_by_id(entity_id) else {
                        return;
                    };
                    let transition = portal_type.get_portal_destination(
                        &world_clone,
                        dest_world_opt,
                        entity_arc.as_ref(),
                        src_portal.as_ref(),
                    );

                    if let Some(transition) = transition {
                        let dest_world = transition.new_world.clone();
                        let yaw_val = transition.yaw;
                        let pitch = transition.pitch;
                        let teleport_pos = transition.position;

                        // 玩家使用玩家专属的传送门钩子；其他
                        // 实体会收到通用的退出事件。
                        if let Some(player) = world_clone.get_player_by_id(entity_id) {
                            let mut player_portal_event =
                                crate::plugin::api::events::player::player_portal::PlayerPortalEvent {
                                    player,
                                    from_pos: entry_position,
                                    to_pos: Some(BlockPos::floored_v(teleport_pos)),
                                    cancelled: false,
                                };
                            if let Some(server) = world_clone.server.upgrade() {
                                server
                                    .plugin_manager
                                    .fire_blocking(&server, &mut player_portal_event);
                            }
                            if player_portal_event.cancelled {
                                return;
                            }
                        }

                        let mut portal_exit_event = crate::plugin::api::events::entity::entity_portal_exit::EntityPortalExitEvent::new(
                            entity_id,
                            entry_position,
                            Some(BlockPos::floored_v(teleport_pos)),
                        );
                        if let Some(server) = world_clone.server.upgrade() {
                            server
                                .plugin_manager
                                .fire_blocking(&server, &mut portal_exit_event);
                        }
                        if portal_exit_event.cancelled {
                            return;
                        }

                        // 传送主实体
                        entity_arc.teleport(teleport_pos, yaw_val, pitch, dest_world.clone());

                        // 递归地将所有乘客随载具一起传送
                        let yaw_delta = yaw_val.map(|y| y - yaw);
                        Self::teleport_passengers_recursive(
                            entity_arc.get_entity(),
                            teleport_pos,
                            yaw_delta,
                            &dest_world,
                        );
                    }
                });
            } else if portal_processor.portal_time == 0 {
                should_remove = true;
            }
        }
        if should_remove {
            *manager_guard = None;
        }
    }

    /// 递归地将所有乘客（以及乘客的乘客）传送到目的地
    fn teleport_passengers_recursive(
        entity: &Self,
        position: Vector3<f64>,
        yaw_delta: Option<f32>,
        dest_world: &Arc<World>,
    ) {
        let passengers = entity
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        for passenger in passengers {
            let passenger_entity = passenger.get_entity();
            let passenger_yaw = yaw_delta.map(|delta| passenger_entity.yaw.load() + delta);
            passenger_entity.portal_cooldown.store(
                passenger_entity.default_portal_cooldown(),
                Ordering::Relaxed,
            );

            passenger.teleport(position, passenger_yaw, None, dest_world.clone());

            // 递归处理该乘客自己的乘客。此前实现对“乘客的乘客”
            // 只递归传送其下层乘客，漏掉了该层自身；骑乘链由
            // add_passenger 的成环校验保证无环，递归深度有界。
            Self::teleport_passengers_recursive(passenger_entity, position, yaw_delta, dest_world);
        }
    }

    pub fn try_use_portal(&self, portal_world: Arc<World>, pos: BlockPos) {
        let mut portal_event =
            crate::plugin::api::events::entity::entity_portal::EntityPortalEvent::new(
                self.entity_id,
                pos,
            );
        if let Some(server) = self.world.load().server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut portal_event);
        }
        if portal_event.cancelled {
            return;
        }

        // 乘客不会独立传送——它们会等待其载具
        if self.has_vehicle() {
            return;
        }

        if self.portal_cooldown.load(Ordering::Relaxed) > 0 {
            self.portal_cooldown
                .store(self.default_portal_cooldown(), Ordering::Relaxed);
            return;
        }

        let Some(server) = portal_world.server.upgrade() else {
            return;
        };

        if (portal_world.dimension == Dimension::THE_NETHER && !server.basic_config.allow_nether)
            || (portal_world.dimension == Dimension::THE_END && !server.basic_config.allow_end)
        {
            return;
        }

        let mut manager = self
            .portal_manager
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let world = self.world.load();
        if manager.is_none() {
            let mut portal_enter_event =
                crate::plugin::api::events::entity::entity_portal_enter::EntityPortalEnterEvent::new(
                    self.entity_id,
                    pos,
                );
            if let Some(server) = world.server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut portal_enter_event);
            }
            if portal_enter_event.cancelled {
                return;
            }

            let portal_type = if portal_world.dimension == Dimension::THE_END
                || self.world.load().dimension == Dimension::THE_END
            {
                PortalType::End
            } else {
                PortalType::Nether
            };

            let mut new_manager = PortalProcessor::new(portal_type, pos, portal_world);

            let (block, state) = world.get_block_and_state(&pos);
            let source_axis = (block == &papokin_data::Block::NETHER_PORTAL).then(|| {
                let props = <papokin_data::block_properties::NetherPortalLikeProperties as papokin_data::block_properties::BlockProperties>::from_state_id(state.id, block);
                props.axis
            });

            if let Some(axis) = source_axis
                && let Some(portal) = NetherPortal::get_on_axis(&world, &pos, axis)
                && portal.was_already_valid()
            {
                new_manager.set_source_portal(SourcePortalInfo {
                    lower_corner: portal.lower_corner(),
                    axis: portal.axis(),
                    width: portal.width(),
                    height: portal.height(),
                });
            }

            *manager = Some(new_manager);
        } else if let Some(manager) = manager.as_mut() {
            manager.entry_position = pos;
            manager.inside_portal_this_tick = true;
            if manager.source_portal.is_none() {
                let (block, state) = world.get_block_and_state(&pos);
                if block == &papokin_data::Block::NETHER_PORTAL {
                    let props = <papokin_data::block_properties::NetherPortalLikeProperties as papokin_data::block_properties::BlockProperties>::from_state_id(state.id, block);
                    if let Some(portal) = NetherPortal::get_on_axis(&world, &pos, props.axis)
                        && portal.was_already_valid()
                    {
                        manager.set_source_portal(SourcePortalInfo {
                            lower_corner: portal.lower_corner(),
                            axis: portal.axis(),
                            width: portal.width(),
                            height: portal.height(),
                        });
                    }
                }
            }
        }
    }

    /// 熄灭此实体。
    pub fn extinguish(&self) {
        self.fire_ticks.store(0, Ordering::Relaxed);
    }

    /// 最大冻结刻数（20 tps 下为 7 秒）
    pub const MAX_FROZEN_TICKS: i32 = 140;

    /// 完全冻结时每 40 刻受到一次冰冻伤害
    const FREEZE_DAMAGE_INTERVAL: i32 = 40;

    /// 检查实体当前是否处于细雪中。
    ///
    /// 该标志在每个刻开始时重置，并在处理期间置位
    /// 当前刻的方块碰撞。
    pub fn is_in_powder_snow(&self) -> bool {
        self.is_in_powder_snow.load(Ordering::Relaxed)
    }

    /// 检查此实体类型是否对冰冻免疫
    pub fn is_freeze_immune(&self) -> bool {
        self.entity_type
            .has_tag(&tag::EntityType::MINECRAFT_FREEZE_IMMUNE_ENTITY_TYPES)
    }

    /// 对应原版 `LivingEntity#canFreeze`：旁观者以及穿戴装备的实体
    /// 免疫冰冻的可穿戴物（如皮革盔甲）不会被冰冻。
    fn can_freeze(&self, caller: &dyn EntityBase) -> bool {
        if caller.is_spectator() || self.is_freeze_immune() {
            return false;
        }

        let Some(living) = caller.get_living_entity() else {
            return true;
        };

        if let Ok(equipment) = living.entity_equipment.try_lock() {
            for (slot, stack) in &equipment.equipment {
                if (*slot == EquipmentSlot::HEAD
                    || *slot == EquipmentSlot::CHEST
                    || *slot == EquipmentSlot::LEGS
                    || *slot == EquipmentSlot::FEET)
                    && stack
                        .get_item()
                        .has_tag(&tag::Item::MINECRAFT_FREEZE_IMMUNE_WEARABLES)
                {
                    return false;
                }
            }
        }

        true
    }

    /// 按刻推进实体的冻结状态。
    /// 在细雪中且可被冻结时：`frozen_ticks` 加 1（上限为 `MAX_FROZEN_TICKS`）
    /// 否则：`frozen_ticks` 每次减少 2（最低降至 0）
    /// 完全冻结时，每 40 刻造成 1 点伤害
    pub fn tick_frozen(&self, caller: &dyn EntityBase) {
        let can_freeze = self.can_freeze(caller);
        let in_powder_snow = self.is_in_powder_snow();
        let old_frozen_ticks = self.frozen_ticks.load(Ordering::Relaxed);

        let new_frozen_ticks = if in_powder_snow && can_freeze {
            // 处于细雪中时增加冰冻刻数
            (old_frozen_ticks + 1).min(Self::MAX_FROZEN_TICKS)
        } else {
            // 原版：不在细雪中或冰冻被阻止时解冻
            (old_frozen_ticks - 2).max(0)
        };

        // 仅在值发生变化时才更新并发送元数据
        if new_frozen_ticks != old_frozen_ticks {
            self.frozen_ticks.store(new_frozen_ticks, Ordering::Relaxed);
            self.set_synced_data(
                tracked_data::entity::DATA_TICKS_FROZEN,
                VarInt(new_frozen_ticks),
            );
        }

        // 原版对齐：完全冻结伤害按刻阶段处理。
        if can_freeze
            && new_frozen_ticks >= Self::MAX_FROZEN_TICKS
            && self.age.load(Ordering::Relaxed) % Self::FREEZE_DAMAGE_INTERVAL == 0
        {
            let world = self.world.load_full();
            if world.level_info.load().game_rules.freeze_damage
                && let Some(entity) = world.get_entity_by_id(self.entity_id)
            {
                entity.damage(entity.as_ref(), 1.0, DamageType::FREEZE);
            }
        }
    }

    /// 设置实体已被冻结的刻数。
    pub fn set_frozen_ticks(&self, ticks: i32) {
        let new_frozen_ticks = ticks.clamp(0, Self::MAX_FROZEN_TICKS);
        self.frozen_ticks.store(new_frozen_ticks, Ordering::Relaxed);
        self.set_synced_data(
            tracked_data::entity::DATA_TICKS_FROZEN,
            VarInt(new_frozen_ticks),
        );
    }

    /// 返回实体已被冻结的刻数。
    pub fn get_frozen_ticks(&self) -> i32 {
        self.frozen_ticks.load(Ordering::Relaxed)
    }

    /// 设置 `Entity` 的偏航角与俯仰角旋转
    pub fn set_rotation(&self, yaw: f32, pitch: f32) {
        // TODO
        self.yaw.store(yaw);
        self.set_pitch(pitch);
    }

    pub fn set_pitch(&self, pitch: f32) {
        self.pitch.store(pitch.clamp(-90.0, 90.0) % 360.0);
    }

    /// 将 `Entity` 从其当前的 `World` 中移除
    pub fn remove(&self) {
        self.world.load().remove_entity(self);
    }

    pub fn create_spawn_packet(&self) -> CSpawnEntity {
        let entity_loc = self.pos.load();
        let entity_vel = self.velocity.load();
        CSpawnEntity::new(
            VarInt(self.entity_id),
            self.entity_uuid,
            VarInt(i32::from(self.entity_type.id)),
            entity_loc,
            self.pitch.load(),
            self.yaw.load(),
            self.head_yaw.load(), // todo: head_yaw 和 yaw 互换了，需查明原因
            self.data.load(Relaxed).into(),
            entity_vel,
        )
    }

    pub fn create_spawn_living_packet(&self, metadata: Option<Box<[u8]>>) -> CSpawnLivingEntity {
        let entity_loc = self.pos.load();
        let entity_vel = self.velocity.load();
        CSpawnLivingEntity::new(
            VarInt(self.entity_id),
            self.entity_uuid,
            VarInt(i32::from(self.entity_type.id)),
            entity_loc,
            self.pitch.load(),
            self.yaw.load(),
            self.head_yaw.load(),
            entity_vel,
            metadata,
        )
    }
    pub fn width(&self) -> f32 {
        self.entity_dimension.load().width
    }

    pub fn height(&self) -> f32 {
        self.entity_dimension.load().height
    }

    /// 按照原版 Minecraft 的机制对实体施加击退。
    ///
    /// 此函数根据指定的击退强度和方向计算实体的新速度。
    pub fn knockback(&self, strength: f64, x: f64, z: f64) {
        // 这里包含一些原版的“魔法”处理
        let mut x = x;
        let mut z = z;
        while x.mul_add(x, z * z) < 1.0E-5 {
            x = (rand::random::<f64>() - rand::random::<f64>()) * 0.01;
            z = (rand::random::<f64>() - rand::random::<f64>()) * 0.01;
        }

        let var8 = Vector3::new(x, 0.0, z).normalize() * strength;
        let velocity = self.velocity.load();
        self.velocity.store(Vector3::new(
            velocity.x / 2.0 - var8.x,
            if self.on_ground.load(Relaxed) {
                (velocity.y / 2.0 + strength).min(0.4)
            } else {
                velocity.y
            },
            velocity.z / 2.0 - var8.z,
        ));
    }

    pub fn set_sneaking(&self, sneaking: bool) {
        //assert!(self.sneaking.load(Relaxed) != sneaking);
        self.sneaking.store(sneaking, Relaxed);
        self.set_flag(Flag::Sneaking, sneaking);
    }
    pub fn is_sneaking(&self) -> bool {
        self.sneaking.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn is_swimming(&self) -> bool {
        self.swimming.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn is_visually_swimming(&self) -> bool {
        self.pose.load() == EntityPose::Swimming
    }

    #[must_use]
    pub fn is_in_water(&self) -> bool {
        self.touching_water.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn is_submerged_in_water(&self) -> bool {
        let pos = self.pos.load();
        let eye_y = pos.y + self.get_eye_height();
        let eye_pos = BlockPos::floored(pos.x, eye_y, pos.z);
        let world = self.world.load();
        let (fluid, state) = world.get_fluid_and_fluid_state(&eye_pos);
        fluid.matches_type(&Fluid::WATER)
            && eye_y
                <= f64::from(eye_pos.0.y)
                    + f64::from(world.get_fluid_height(&eye_pos, fluid, &state))
    }

    #[must_use]
    pub fn is_under_water(&self) -> bool {
        self.is_in_water() && self.is_submerged_in_water()
    }

    #[must_use]
    pub fn is_visually_crawling(&self) -> bool {
        self.is_visually_swimming() && !self.is_in_water()
    }

    pub fn set_swimming(&self, swimming: bool) {
        if self.swimming.load(Ordering::Relaxed) != swimming {
            let mut event =
                crate::plugin::api::events::entity::entity_toggle_swim::EntityToggleSwimEvent::new(
                    self.entity_id,
                    swimming,
                );
            if let Some(server) = self.world.load().server.upgrade() {
                server.plugin_manager.fire_blocking(&server, &mut event);
            }
            if event.cancelled {
                return;
            }
            self.swimming.store(event.is_swimming, Relaxed);
            self.set_flag(Flag::Swimming, event.is_swimming);
        }
    }

    /// 设置实体是否隐身，并发送更新后的元数据。
    pub fn set_invisible(&self, invisible: bool) {
        if self.invisible.load(Ordering::Relaxed) != invisible {
            self.invisible.store(invisible, Relaxed);
            self.set_flag(Flag::Invisible, invisible);
        }
    }

    /// 设置实体是否发光，并发送更新后的元数据。
    pub fn set_glowing(&self, glowing: bool) {
        if self.glowing.load(Ordering::Relaxed) != glowing {
            self.glowing.store(glowing, Ordering::Relaxed);
            self.set_flag(Flag::Glowing, glowing);
        }
    }

    /// 设置实体在视觉与伤害层面是否处于着火状态。这与 `fire_ticks` 相互独立，后者跟踪着火在伤害方面的表现。
    pub fn set_on_fire(&self, on_fire: bool) {
        if self.has_visual_fire.load(Ordering::Relaxed) != on_fire {
            self.has_visual_fire.store(on_fire, Ordering::Relaxed);
            self.set_flag(Flag::OnFire, on_fire);
        }
    }

    #[must_use]
    pub fn is_on_fire(&self) -> bool {
        self.fire_ticks.load(Ordering::Relaxed) > 0 || self.has_visual_fire.load(Ordering::Relaxed)
    }

    pub fn get_horizontal_facing(&self) -> HorizontalFacing {
        let yaw = self.yaw.load();
        // 使用原版公式：floor(angle / 90.0 + 0.5) & 3
        let quarter_turns = ((yaw / 90.0) + 0.5).floor() as i32 & 3;
        match quarter_turns {
            0 => HorizontalFacing::South,
            1 => HorizontalFacing::West,
            2 => HorizontalFacing::North,
            _ => HorizontalFacing::East,
        }
    }

    pub fn get_rotation_16(&self) -> u8 {
        let adjusted_yaw = self.yaw.load().rem_euclid(360.0);

        ((adjusted_yaw / 22.5).round() as u8) % 16
    }

    pub fn get_flipped_rotation_16(&self) -> u8 {
        (self.get_rotation_16() + 8) % 16
    }

    pub fn get_facing(&self) -> Facing {
        let pitch = self.pitch.load().to_radians();
        let yaw = -self.yaw.load().to_radians();

        let (sin_p, cos_p) = pitch.sin_cos();
        let (sin_y, cos_y) = yaw.sin_cos();

        let x = sin_y * cos_p;
        let y = -sin_p;
        let z = cos_y * cos_p;

        let ax = x.abs();
        let ay = y.abs();
        let az = z.abs();

        if ax > ay && ax > az {
            if x > 0.0 { Facing::East } else { Facing::West }
        } else if ay > ax && ay > az {
            if y > 0.0 { Facing::Up } else { Facing::Down }
        } else if z > 0.0 {
            Facing::South
        } else {
            Facing::North
        }
    }

    pub fn get_entity_facing_order(&self) -> [Facing; 6] {
        let pitch = self.pitch.load().to_radians();
        let yaw = -self.yaw.load().to_radians();

        let sin_p = pitch.sin();
        let cos_p = pitch.cos();
        let sin_y = yaw.sin();
        let cos_y = yaw.cos();

        let east_west = if sin_y > 0.0 {
            Facing::East
        } else {
            Facing::West
        };
        let up_down = if sin_p < 0.0 {
            Facing::Up
        } else {
            Facing::Down
        };
        let south_north = if cos_y > 0.0 {
            Facing::South
        } else {
            Facing::North
        };

        let x_axis = sin_y.abs();
        let y_axis = sin_p.abs();
        let z_axis = cos_y.abs();
        let x_weight = x_axis * cos_p;
        let z_weight = z_axis * cos_p;

        let (first, second, third) = if x_axis > z_axis {
            if y_axis > x_weight {
                (up_down, east_west, south_north)
            } else if z_weight > y_axis {
                (east_west, south_north, up_down)
            } else {
                (east_west, up_down, south_north)
            }
        } else if y_axis > z_weight {
            (up_down, south_north, east_west)
        } else if x_weight > y_axis {
            (south_north, east_west, up_down)
        } else {
            (south_north, up_down, east_west)
        };

        [
            first,
            second,
            third,
            third.opposite(),
            second.opposite(),
            first.opposite(),
        ]
    }

    pub fn set_sprinting(&self, sprinting: bool) {
        //assert!(self.sprinting.load(Relaxed) != sprinting);
        self.sprinting.store(sprinting, Relaxed);
        self.set_flag(Flag::Sprinting, sprinting);
    }

    pub fn is_sprinting(&self) -> bool {
        self.sprinting.load(Ordering::Relaxed)
    }
    pub fn check_fall_flying(&self) -> bool {
        !self.on_ground.load(Relaxed)
    }

    pub fn set_fall_flying(&self, fall_flying: bool) {
        assert_ne!(self.fall_flying.load(Relaxed), fall_flying);
        self.fall_flying.store(fall_flying, Relaxed);
        self.set_flag(Flag::FallFlying, fall_flying);
    }
    pub fn is_fall_flying(&self) -> bool {
        self.fall_flying.load(Ordering::Relaxed)
    }

    fn set_flag(&self, flag: Flag, value: bool) {
        let index = flag as u8;
        let mask = (1i8).wrapping_shl(index as u32);
        let new_je_flags = if value {
            self.flags.fetch_or(mask, Ordering::Relaxed) | mask
        } else {
            self.flags.fetch_and(!mask, Ordering::Relaxed) & !mask
        };

        self.set_synced_data(tracked_data::entity::DATA_SHARED_FLAGS_ID, new_je_flags);
    }

    /// 在此实体的位置以该实体的声音类别播放声音
    pub fn play_sound(&self, sound: Sound) {
        self.world
            .load()
            .play_sound(sound, SoundCategory::Neutral, &self.pos.load());
    }

    pub fn set_synced_data<T: MetadataSerializer + Clone + Send + Sync + 'static>(
        &self,
        tracked: papokin_data::tracked_data::TrackedData,
        value: T,
    ) -> bool {
        if self.synched_data.set(tracked, value) {
            self.send_dirty_entity_data();
            true
        } else {
            false
        }
    }

    pub fn send_meta_data<T: MetadataSerializer>(&self, meta: &[Metadata<T>]) {
        let world = self.world.load();
        let players = world.players.load();

        let mut java_recipients = Vec::new();

        if let Some(tracked) = world.entity_tracker.get_tracked_entity(self.entity_id) {
            for player in players.iter() {
                if tracked.seen_by.contains(&player.gameprofile.id)
                    || player.entity_id() == self.entity_id
                {
                    java_recipients.push(player);
                }
            }
        } else {
            let chunk_pos = self.chunk_pos.load();
            for player in players.iter() {
                if player
                    .watched_section
                    .load()
                    .is_within_distance(chunk_pos.x, chunk_pos.y)
                {
                    java_recipients.push(player);
                }
            }
        }

        let recipients_by_version =
            World::collect_java_recipients_by_version(java_recipients.into_iter());

        for (version, recipients) in recipients_by_version {
            if version < JavaMinecraftVersion::V_1_21 {
                continue;
            }
            let mut buf = Vec::new();
            for m in meta {
                let _ = m.write(&mut buf, &version);
            }
            if buf.is_empty() {
                continue;
            }
            buf.put_u8(255);
            let packet = CSetEntityMetadata::new(self.entity_id.into(), buf.into());
            if let Ok(packet_data) = JavaClient::serialize_packet_for_version(&packet, version) {
                for recipient in recipients {
                    recipient.try_enqueue_packet(packet_data.clone());
                }
            }
        }
    }

    pub fn send_dirty_entity_data(&self) {
        if !self.synched_data.is_dirty() {
            return;
        }

        let world = self.world.load();
        let players = world.players.load();

        let mut java_recipients = Vec::new();

        if let Some(tracked) = world.entity_tracker.get_tracked_entity(self.entity_id) {
            for player in players.iter() {
                if tracked.seen_by.contains(&player.gameprofile.id)
                    || player.entity_id() == self.entity_id
                {
                    java_recipients.push(player);
                }
            }
        } else {
            let chunk_pos = self.chunk_pos.load();
            for player in players.iter() {
                if player
                    .watched_section
                    .load()
                    .is_within_distance(chunk_pos.x, chunk_pos.y)
                {
                    java_recipients.push(player);
                }
            }
        }

        if java_recipients.is_empty() {
            return;
        }

        let recipients_by_version =
            World::collect_java_recipients_by_version(java_recipients.into_iter());
        let versions: Vec<papokin_util::version::JavaMinecraftVersion> =
            recipients_by_version.keys().copied().collect();
        // 打包与清脏在同一把锁内完成，防止窗口期并发的 set 写入
        // 被清脏吞掉（见 pack_dirty_for_versions 文档）。
        for (version, buf) in self.synched_data.pack_dirty_for_versions(&versions) {
            if let Some(recipients) = recipients_by_version.get(&version) {
                let packet = CSetEntityMetadata::new(self.entity_id.into(), buf);
                if let Ok(packet_data) = JavaClient::serialize_packet_for_version(&packet, version)
                {
                    for recipient in recipients {
                        recipient.try_enqueue_packet(packet_data.clone());
                    }
                }
            }
        }
    }

    pub fn set_pose(&self, pose: EntityPose) {
        if self.pose.load() == pose {
            return;
        }

        let mut pose_event =
            crate::plugin::api::events::entity::entity_pose_change::EntityPoseChangeEvent::new(
                self.entity_id,
                (pose as u8).to_string(),
            );
        if let Some(server) = self.world.load().server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut pose_event);
            if pose_event.cancelled {
                return;
            }
        }

        let dimension = Self::get_entity_dimensions(pose);
        let position = self.pos.load();
        let aabb = BoundingBox::new_from_pos(position.x, position.y, position.z, &dimension);
        self.pose.store(pose);
        self.bounding_box.store(aabb);
        self.entity_dimension.store(dimension);
        let pose = pose as i32;
        self.set_synced_data(tracked_data::entity::DATA_POSE, VarInt(pose));
    }

    /// 检查实体是否对给定伤害类型免疫，同时考虑总体无敌与特定免疫。
    pub fn is_invulnerable_to(&self, damage_type: &DamageType) -> bool {
        // 没有任何东西能免疫虚空或 kill
        if matches!(
            *damage_type,
            DamageType::GENERIC_KILL | DamageType::OUT_OF_WORLD
        ) {
            return false;
        }

        // 通用无敌
        if self.invulnerable.load(Ordering::Relaxed) {
            return true;
        }

        // 特定类型免疫
        self.damage_immunities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(damage_type)
    }

    /// 检查实体是否对已解析的（原版或
    /// 插件注册的自定义）伤害类型。自定义类型绝不会
    /// 虚空/击杀伤害，且不匹配任何静态免疫条目，因此只有
    /// 通用无敌标记是否适用于它们。
    pub fn is_invulnerable_to_resolved(&self, damage_type: &ResolvedDamageType) -> bool {
        damage_type.vanilla().map_or_else(
            || self.invulnerable.load(Ordering::Relaxed),
            |vanilla| self.is_invulnerable_to(&vanilla),
        )
    }

    /// 设置实体是否对特定伤害类型免疫
    pub fn set_damage_immunity(&self, damage_type: DamageType, immune: bool) {
        let mut immunities = self
            .damage_immunities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if immune {
            if !immunities.contains(&damage_type) {
                immunities.push(damage_type);
            }
        } else {
            // retain 比查找索引再移除更简洁
            immunities.retain(|dt| dt != &damage_type);
        }
    }

    /// 设置实体是否对所有伤害类型免疫（`GENERIC_KILL` 与 `OUT_OF_WORLD` 除外）
    pub fn set_invulnerable(&self, invulnerable: bool) {
        self.invulnerable.store(invulnerable, Relaxed);
    }

    pub fn check_block_collision(entity: &dyn EntityBase, server: &Server) {
        let aabb = entity.get_entity().bounding_box.load();
        let blockpos = BlockPos::new(
            (aabb.min.x + 0.001).floor() as i32,
            (aabb.min.y + 0.001).floor() as i32,
            (aabb.min.z + 0.001).floor() as i32,
        );
        let blockpos1 = BlockPos::new(
            (aabb.max.x - 0.001).floor() as i32,
            (aabb.max.y - 0.001).floor() as i32,
            (aabb.max.z - 0.001).floor() as i32,
        );
        let world = entity.get_entity().world.load();

        for x in blockpos.0.x..=blockpos1.0.x {
            for y in blockpos.0.y..=blockpos1.0.y {
                for z in blockpos.0.z..=blockpos1.0.z {
                    let pos = BlockPos::new(x, y, z);
                    let (block, state) = world.get_block_and_state(&pos);
                    let block_outlines = state.get_block_outline_shapes_at(&pos);

                    if state.outline_shapes.is_empty() {
                        world
                            .block_registry
                            .on_entity_collision(block, &world, entity, &pos, state, server);
                        let fluid = world.get_fluid(&pos);
                        world
                            .block_registry
                            .on_entity_collision_fluid(fluid, entity);
                        continue;
                    }
                    for outline in block_outlines {
                        let outline_aabb = outline.at_pos(pos);
                        if outline_aabb.intersects(&aabb) {
                            world
                                .block_registry
                                .on_entity_collision(block, &world, entity, &pos, state, server);
                            let fluid = world.get_fluid(&pos);
                            world
                                .block_registry
                                .on_entity_collision_fluid(fluid, entity);
                            break;
                        }
                    }
                }
            }
        }
    }

    pub fn teleport(
        &self,
        position: Vector3<f64>,
        yaw: Option<f32>,
        pitch: Option<f32>,
        world: &World,
    ) {
        // 更新服务器端位置与包围盒
        self.set_pos(position);
        if let Some(yaw) = yaw {
            self.yaw.store(yaw);
        }
        if let Some(pitch) = pitch {
            self.set_pitch(pitch);
        }
        // 更新缓存，以免发送造成回弹的增量
        self.last_sent_pos.store(position);
        if let Some(yaw) = yaw {
            self.last_sent_yaw
                .store((yaw * 256.0 / 360.0).rem_euclid(256.0) as u8, Relaxed);
            self.last_sent_head_yaw
                .store((yaw * 256.0 / 360.0).rem_euclid(256.0) as u8, Relaxed);
        }
        if let Some(pitch) = pitch {
            self.last_sent_pitch
                .store((pitch * 256.0 / 360.0).rem_euclid(256.0) as u8, Relaxed);
        }
        let chunk_pos = self.chunk_pos.load();
        world.broadcast_to_chunk(
            chunk_pos,
            &CEntityPositionSync::new(
                self.entity_id.into(),
                position,
                self.velocity.load(),
                yaw.unwrap_or(self.yaw.load()),
                pitch.unwrap_or(self.pitch.load()),
                self.on_ground.load(Ordering::SeqCst),
            ),
        );
    }

    pub fn get_eye_pos(&self) -> Vector3<f64> {
        let pos = self.pos.load();
        Vector3::new(
            pos.x,
            pos.y + f64::from(self.entity_dimension.load().eye_height),
            pos.z,
        )
    }

    /// 两个眼睛位置之间没有固体方块。
    #[must_use]
    pub fn has_line_of_sight(&self, other: &Self) -> bool {
        let from = self.get_eye_pos();
        let to = other.get_eye_pos();
        if from.squared_distance_to_vec(&to) > 128.0 * 128.0 {
            return false;
        }
        self.world
            .load_full()
            .raycast(from, to, |block_pos, world| {
                world.get_block_state(block_pos).is_solid()
            })
            .is_none()
    }

    pub fn get_eye_y(&self) -> f64 {
        self.pos.load().y + f64::from(self.entity_dimension.load().eye_height)
    }

    pub fn is_removed(&self) -> bool {
        self.removal_reason.load().is_some()
    }

    pub fn is_alive(&self) -> bool {
        !self.is_removed()
    }

    #[must_use]
    pub fn is_affected_by_blocks(&self) -> bool {
        !self.is_removed() && !self.no_physics.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn is_in_wall(&self) -> bool {
        if self.no_physics.load(Ordering::Relaxed) {
            return false;
        }

        let eye_pos = self.get_eye_pos();
        let half_width = (f64::from(self.entity_dimension.load().width) * 0.8) / 2.0;
        let eye_bb = BoundingBox::new(
            Vector3::new(eye_pos.x - half_width, eye_pos.y, eye_pos.z - half_width),
            Vector3::new(
                eye_pos.x + half_width,
                eye_pos.y + 1.0e-6,
                eye_pos.z + half_width,
            ),
        );
        let min = eye_bb.min_block_pos();
        let max = eye_bb.max_block_pos();
        let world = self.world.load();

        for pos in BlockPos::iterate(min, max) {
            let (block, state) = world.get_block_and_state(&pos);
            if state.is_air() {
                continue;
            }

            if blocks_movement(state, block.id) && state.is_full_cube() {
                return true;
            }
        }

        false
    }

    pub const LEASH_SNAP_DISTANCE: f64 = 12.0;
    pub const LEASH_ELASTIC_DISTANCE: f64 = 6.0;

    pub fn leash_to(&self, holder: Arc<dyn EntityBase>) {
        let holder_entity_id = holder.get_entity().entity_id;
        let world = self.world.load();
        if let Some(server) = world.server.upgrade()
            && let Some(player) = holder.get_player()
            && let Some(player_arc) = world.get_player_by_uuid(player.gameprofile.id)
        {
            let mut event =
                crate::plugin::api::events::player::player_leash_entity::PlayerLeashEntityEvent {
                    player: player_arc,
                    entity_id: self.entity_id,
                    holder_id: holder_entity_id,
                    cancelled: false,
                };
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return;
            }
        }

        *self
            .leashed_to
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(holder);

        let je_packet = papokin_protocol::java::client::play::CSetEntityLink::new(
            self.entity_id,
            holder_entity_id,
            true,
        );

        self.world
            .load()
            .broadcast_to_chunk(self.chunk_pos.load(), &je_packet);
    }

    pub fn unleash(&self) {
        let world = self.world.load();
        if let Some(server) = world.server.upgrade() {
            let mut event =
                crate::plugin::api::events::entity::entity_unleash::EntityUnleashEvent::new(
                    self.entity_id,
                    "unleashed".to_string(),
                );
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return;
            }
        }

        let old_holder = self
            .leashed_to
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if old_holder.is_none() {
            return;
        }

        if let Some(holder) = &old_holder
            && let Some(server) = world.server.upgrade()
            && let Some(player) = holder.get_player()
            && let Some(player_arc) = world.get_player_by_uuid(player.gameprofile.id)
        {
            let mut event = crate::plugin::api::events::player::player_unleash_entity::PlayerUnleashEntityEvent {
                player: player_arc,
                entity_id: self.entity_id,
                cancelled: false,
            };
            server.plugin_manager.fire_blocking(&server, &mut event);
        }

        let je_packet =
            papokin_protocol::java::client::play::CSetEntityLink::new(self.entity_id, -1, true);

        self.world
            .load()
            .broadcast_to_chunk(self.chunk_pos.load(), &je_packet);
    }

    pub fn tick_leash(&self) {
        let holder = {
            let Ok(guard) = self.leashed_to.try_lock() else {
                return;
            };
            guard.clone()
        };

        if let Some(holder) = holder {
            let holder_entity = holder.get_entity();

            // 若实体或拴持者被移除或死亡，则掉落拴绳
            if !self.is_alive() || !holder_entity.is_alive() {
                self.unleash();
                return;
            }

            let self_pos = self.pos.load();
            let holder_pos = holder_entity.pos.load();
            let diff = self_pos - holder_pos;
            let distance = diff.length();

            if distance > Self::LEASH_SNAP_DISTANCE {
                // 距离过远：拴绳绷断，并掉落拴绳物品
                self.unleash();
                let lead_item =
                    papokin_data::item_stack::ItemStack::new(1, &papokin_data::item::Item::LEAD);
                self.world
                    .load()
                    .drop_stack(&self.block_pos.load(), lead_item);
            } else if distance > Self::LEASH_ELASTIC_DISTANCE {
                // 朝向拴绳持有者的弹性拉力
                let dir = (holder_pos - self_pos).normalize();
                let pull_strength = (distance - Self::LEASH_ELASTIC_DISTANCE) * 0.11;
                let current_vel = self.velocity.load();
                self.velocity.store(current_vel + dir * pull_strength);
                self.velocity_dirty.store(true, Relaxed);
            }
        }
    }

    pub fn has_passengers(&self) -> bool {
        !self
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    }

    pub fn has_passenger(&self, id: i32) -> bool {
        self.passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .any(|passenger| passenger.get_entity().entity_id == id)
    }

    pub fn has_vehicle(&self) -> bool {
        self.vehicle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
    }

    pub fn get_vehicle(&self) -> Option<Arc<dyn EntityBase>> {
        self.vehicle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn is_leashed(&self) -> bool {
        self.leashed_to
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
    }

    pub fn add_passenger(&self, vehicle: Arc<dyn EntityBase>, passenger: Arc<dyn EntityBase>) {
        self.add_passenger_checked(vehicle, passenger, false);
    }

    /// 强制上坐骑（对齐原版 `startRiding(entity, force)`）：
    /// 绕过骑乘冷却（供 `/ride` 与断线重连恢复载具使用），
    /// 但完整性校验（自骑、重复、满座、成环）不可绕过。
    pub fn add_passenger_force(
        &self,
        vehicle: Arc<dyn EntityBase>,
        passenger: Arc<dyn EntityBase>,
    ) {
        self.add_passenger_checked(vehicle, passenger, true);
    }

    fn add_passenger_checked(
        &self,
        vehicle: Arc<dyn EntityBase>,
        passenger: Arc<dyn EntityBase>,
        force: bool,
    ) {
        let passenger_entity = passenger.get_entity();

        // —— 完整性校验（force 也不可绕过）——

        // 自骑即长度为 1 的环，teleport_passengers_recursive 会无界递归。
        if self.entity_id == passenger_entity.entity_id {
            return;
        }

        {
            let passengers = self
                .passengers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            // 重复挂载会破坏乘客表（CSetPassengers 出现重复、下骑后残留幽灵条目）。
            if passengers
                .iter()
                .any(|p| p.get_entity().entity_id == passenger_entity.entity_id)
            {
                return;
            }
            // 座位上限（原版 canAddPassenger）：马/猪/矿车为 1，船/骆驼为 2。
            if passengers.len() >= self.max_seats {
                return;
            }
        }

        if self.would_create_vehicle_cycle(passenger_entity.entity_id) {
            tracing::warn!(
                "拒绝实体 {} 挂载到 {}：将构成骑乘环",
                passenger_entity.entity_id,
                self.entity_id
            );
            return;
        }

        // —— 常规校验（force 可绕过，对齐原版语义）——
        if !force {
            if let Some(current) = passenger_entity.get_vehicle()
                && current.get_entity().entity_id != self.entity_id
            {
                // 乘客已在其他载具上：先自动下骑（原版 stopRiding 后再 startRiding），
                // 避免出现“vehicle 指向新载具、旧载具乘客表仍含该乘客”的分裂状态。
                current
                    .get_entity()
                    .remove_passenger_sync(passenger_entity.entity_id);
            }
            // 原版 ridingCooldown > 0 时禁止 startRiding（防止下骑后立即重复上骑）。
            if passenger_entity.riding_cooldown.load(Relaxed) > 0 {
                return;
            }
        }

        let mut mount_event =
            crate::plugin::api::events::entity::entity_mount::EntityMountEvent::new(
                passenger.get_entity().entity_id,
                self.entity_id,
            );
        let mut vehicle_enter =
            crate::plugin::api::events::vehicle::vehicle_enter::VehicleEnterEvent::new(
                self.entity_id,
                passenger.get_entity().entity_id,
            );
        if let Some(server) = self.world.load().server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut mount_event);
            server
                .plugin_manager
                .fire_blocking(&server, &mut vehicle_enter);
        }
        if mount_event.cancelled || vehicle_enter.cancelled {
            return;
        }

        *passenger_entity
            .vehicle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(vehicle);

        let mut passengers = self
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        passengers.push(passenger);

        let passenger_ids: Vec<VarInt> = passengers
            .iter()
            .map(|p| VarInt(p.get_entity().entity_id))
            .collect();

        let world = self.world.load();
        let chunk_pos = self.chunk_pos.load();
        world.broadcast_to_chunk(
            chunk_pos,
            &CSetPassengers::new(VarInt(self.entity_id), &passenger_ids),
        );
    }

    /// 检查把 `passenger_id` 挂到 `self` 上是否会构成骑乘环：
    /// 从 `self` 沿其载具链向上走，若途中遇到该乘客即成环。
    /// 环一旦形成，`teleport_passengers_recursive` 会栈溢出崩服。
    fn would_create_vehicle_cycle(&self, passenger_id: i32) -> bool {
        let mut current = self.get_vehicle();
        vehicle_chain_reaches(
            || {
                let entity = current.take()?;
                let id = entity.get_entity().entity_id;
                current = entity.get_entity().get_vehicle();
                Some(id)
            },
            passenger_id,
        )
    }
}

/// 沿载具链逐跳取实体 id，判断链上是否出现 `passenger_id`。
/// 纯函数便于单测；`next` 每调用一次返回链上的下一跳（链尽返回 `None`）。
/// 跳数硬上限防御并发窗口或异常状态下的超长/退化链。
fn vehicle_chain_reaches(mut next: impl FnMut() -> Option<i32>, passenger_id: i32) -> bool {
    let mut hops = 0;
    while let Some(id) = next() {
        if id == passenger_id {
            return true;
        }
        hops += 1;
        if hops >= MAX_VEHICLE_CHAIN_HOPS {
            return true;
        }
    }
    false
}

/// 载具链跳数上限：正常骑乘链远短于此；到达上限按成环处理。
const MAX_VEHICLE_CHAIN_HOPS: u32 = 128;

impl Entity {
    pub(crate) fn remove_passenger_on_disconnect(&self, passenger_id: i32) {
        let mut passengers = self
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(index) = passengers
            .iter()
            .position(|passenger| passenger.get_entity().entity_id == passenger_id)
        {
            let passenger = passengers.remove(index);
            *passenger
                .get_entity()
                .vehicle
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        }

        let passenger_ids: Vec<VarInt> = passengers
            .iter()
            .map(|passenger| VarInt(passenger.get_entity().entity_id))
            .collect();
        drop(passengers);

        self.world.load().broadcast_to_chunk(
            self.chunk_pos.load(),
            &CSetPassengers::new(VarInt(self.entity_id), &passenger_ids),
        );
    }

    pub fn remove_passenger_sync(&self, passenger_id: i32) {
        self.remove_passenger_on_disconnect(passenger_id);
    }

    pub fn remove_passenger(&self, passenger_id: i32) {
        self.remove_passenger_internal(passenger_id, true);
    }

    pub fn remove_passenger_before_teleport(&self, passenger_id: i32) {
        self.remove_passenger_internal(passenger_id, false);
    }

    #[allow(clippy::too_many_lines)]
    fn remove_passenger_internal(&self, passenger_id: i32, reposition: bool) {
        let mut dismount_event =
            crate::plugin::api::events::entity::entity_dismount::EntityDismountEvent::new(
                passenger_id,
                self.entity_id,
            );
        let mut vehicle_exit =
            crate::plugin::api::events::vehicle::vehicle_exit::VehicleExitEvent::new(
                self.entity_id,
                passenger_id,
            );
        if let Some(server) = self.world.load().server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut dismount_event);
            server
                .plugin_manager
                .fire_blocking(&server, &mut vehicle_exit);
        }
        if dismount_event.cancelled || vehicle_exit.cancelled {
            return;
        }

        let (removed_passenger, passenger_ids) = {
            let mut passengers = self
                .passengers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let removed_passenger = passengers
                .iter()
                .position(|p| p.get_entity().entity_id == passenger_id)
                .map(|idx| {
                    let passenger = passengers.remove(idx);
                    *passenger
                        .get_entity()
                        .vehicle
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
                    passenger
                });

            let passenger_ids: Vec<VarInt> = passengers
                .iter()
                .map(|p| VarInt(p.get_entity().entity_id))
                .collect();
            (removed_passenger, passenger_ids)
        };

        let chunk_pos = self.chunk_pos.load();

        if let Some(passenger) = removed_passenger {
            let vehicle_box = self.bounding_box.load();
            let passenger_entity = passenger.get_entity();

            // 在发送之前预分配传送 ID 和方块移动数据包
            // CSetPassengers。这可以防止一种竞态条件：客户端收到
            // 下乘坐数据包，会从旧的骑乘位置发送过期位置数据包
            // 位置，而服务器会在传送到达之前处理它们。
            let teleport_id = if reposition && let Some(player) = passenger.get_player() {
                let id = player
                    .teleport_id_count
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                    + 1;
                // 使用回退位置作为占位符——下方会用真实位置更新
                let placeholder =
                    Vector3::new(self.pos.load().x, vehicle_box.max.y, self.pos.load().z);
                *player
                    .awaiting_teleport
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) =
                    Some((id.into(), placeholder));
                Some(id)
            } else {
                None
            };

            // 原版：ridingCooldown = 60（防止立即重新骑乘）
            passenger_entity.riding_cooldown.store(60, Relaxed);
            // TODO: world.emitGameEvent(passenger, GameEvent.ENTITY_DISMOUNT, vehicle.pos)

            // 在广播之前，直接将 CSetPassengers 发送给正在下坐骑的玩家。
            let world = self.world.load();
            let passengers_packet = CSetPassengers::new(VarInt(self.entity_id), &passenger_ids);
            if let Some(player) = passenger.get_player() {
                player.try_send_client_packet(&passengers_packet);
                world.broadcast_to_chunk_except(
                    chunk_pos,
                    &[player.get_entity().entity_uuid],
                    &passengers_packet,
                );
            } else {
                world.broadcast_to_chunk(chunk_pos, &passengers_packet);
            }

            if !reposition {
                return;
            }

            // 计算下骑方向与偏移（原版 DismountHelper）
            let vehicle_yaw = self.yaw.load();
            // 把 yaw 折叠到 0..360 范围内
            let wrapped_yaw = (vehicle_yaw % 360.0 + 360.0) % 360.0;
            let forward_dir = if !(45.0..315.0).contains(&wrapped_yaw) {
                BlockDirection::South
            } else if (45.0..135.0).contains(&wrapped_yaw) {
                BlockDirection::West
            } else if (135.0..225.0).contains(&wrapped_yaw) {
                BlockDirection::North
            } else {
                BlockDirection::East
            };

            let get_step = |dir: BlockDirection| -> (i32, i32) {
                match dir {
                    BlockDirection::North => (0, -1),
                    BlockDirection::South => (0, 1),
                    BlockDirection::East => (1, 0),
                    BlockDirection::West => (-1, 0),
                    _ => (0, 0),
                }
            };

            let get_clockwise = |dir: BlockDirection| -> BlockDirection {
                match dir {
                    BlockDirection::North => BlockDirection::East,
                    BlockDirection::East => BlockDirection::South,
                    BlockDirection::South => BlockDirection::West,
                    BlockDirection::West => BlockDirection::North,
                    other => other,
                }
            };

            let get_opposite = |dir: BlockDirection| -> BlockDirection {
                match dir {
                    BlockDirection::North => BlockDirection::South,
                    BlockDirection::South => BlockDirection::North,
                    BlockDirection::East => BlockDirection::West,
                    BlockDirection::West => BlockDirection::East,
                    other => other,
                }
            };

            let right_dir = get_clockwise(forward_dir);
            let left_dir = get_opposite(right_dir);
            let back_dir = get_opposite(forward_dir);

            let (fx, fz) = get_step(forward_dir);
            let (rx, rz) = get_step(right_dir);
            let (lx, lz) = get_step(left_dir);
            let (bx, bz) = get_step(back_dir);

            let offsets = [
                (rx, rz),
                (lx, lz),
                (bx + rx, bz + rz),
                (bx + lx, bz + lz),
                (fx + rx, fz + rz),
                (fx + lx, fz + lz),
                (bx, bz),
                (fx, fz),
            ];

            let target_block_y = vehicle_box.max.y.floor() as i32;
            let below_pos = BlockPos(Vector3::new(
                self.pos.load().x.floor() as i32,
                target_block_y - 1,
                self.pos.load().z.floor() as i32,
            ));

            let below_state_id = world.get_block_state_id(&below_pos);
            // 原版：isWater 专门检查水流体，而不是任意流体
            let is_water = Fluid::from_state_id(below_state_id)
                .is_some_and(|f| f.id == Fluid::WATER.id || f.id == Fluid::FLOWING_WATER.id);

            let fallback_pos =
                Vector3::new(self.pos.load().x, vehicle_box.max.y, self.pos.load().z);

            let dismount_pos = if is_water {
                fallback_pos
            } else {
                // 原版检查站立、潜行、游泳姿态及各自对应的高度检测
                let poses_and_heights = [
                    (EntityPose::Standing, vec![0, 1, -1]),
                    (EntityPose::Crouching, vec![0, 1, -1]),
                    (EntityPose::Swimming, vec![0, 1]),
                ];

                let vehicle_block_pos = self.block_pos.load();
                let mut found = None;

                'search: for (pose, y_offsets) in poses_and_heights {
                    let dims = Self::get_entity_dimensions(pose);

                    for y_offset in y_offsets {
                        for &(ox, oz) in &offsets {
                            let target_block_x = vehicle_block_pos.0.x + ox;
                            let target_block_y = vehicle_block_pos.0.y + y_offset;
                            let target_block_z = vehicle_block_pos.0.z + oz;

                            let target_pos = BlockPos(Vector3::new(
                                target_block_x,
                                target_block_y,
                                target_block_z,
                            ));
                            let height = world.get_dismount_height(&target_pos);

                            if height.is_finite() && height < 1.0 {
                                let location = Vector3::new(
                                    f64::from(target_block_x) + 0.5,
                                    f64::from(target_block_y) + height,
                                    f64::from(target_block_z) + 0.5,
                                );

                                let bbox = BoundingBox::new_from_pos(
                                    location.x, location.y, location.z, &dims,
                                );
                                if world.is_space_empty(bbox) {
                                    found = Some((location, pose));
                                    break 'search;
                                }
                            }
                        }
                    }
                }

                if let Some((pos, pose)) = found {
                    if pose != EntityPose::Standing {
                        passenger_entity.set_pose(pose);
                    }
                    pos
                } else {
                    // 作为回退，尝试直接在载具顶部下坐骑
                    let mut found_fallback = None;
                    let vehicle_top = vehicle_box.max.y;

                    let poses = [
                        EntityPose::Standing,
                        EntityPose::Crouching,
                        EntityPose::Swimming,
                    ];

                    for pose in poses {
                        let dims = Self::get_entity_dimensions(pose);
                        let bbox = BoundingBox::new_from_pos(
                            self.pos.load().x,
                            vehicle_top,
                            self.pos.load().z,
                            &dims,
                        );
                        if world.is_space_empty(bbox) {
                            found_fallback = Some((
                                Vector3::new(self.pos.load().x, vehicle_top, self.pos.load().z),
                                pose,
                            ));
                            break;
                        }
                    }

                    if let Some((pos, pose)) = found_fallback {
                        if pose != EntityPose::Standing {
                            passenger_entity.set_pose(pose);
                        }
                        pos
                    } else {
                        fallback_pos
                    }
                }
            };

            // 清理对已下骑乘客的任何残留引用。
            passenger_entity.set_pos(dismount_pos);

            // 阶段 2：传送到安全位置（解除移动阻塞）
            if let Some(player) = passenger.get_player() {
                if let Some(id) = teleport_id {
                    player.get_entity().set_pos(dismount_pos);
                    // 用真实的下坐骑位置更新 awaiting_teleport
                    *player
                        .awaiting_teleport
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) =
                        Some((id.into(), dismount_pos));
                    // 使用 send_client_packet 让传送真正生效
                    // 与 CSetPassengers 相同的数据包队列，以保持发送顺序。
                    // 原版使用 DELTA | ROT 标志：位置为绝对值，增量/旋转为相对值。
                    // 使用相对旋转且 yaw/pitch=0 时，客户端会保持当前视角。
                    player.try_send_client_packet(&CPlayerPosition::new(
                        id.into(),
                        dismount_pos,
                        Vector3::new(0.0, 0.0, 0.0),
                        0.0,
                        0.0,
                        vec![
                            PositionFlag::DeltaX,
                            PositionFlag::DeltaY,
                            PositionFlag::DeltaZ,
                            PositionFlag::YRot,
                            PositionFlag::XRot,
                        ],
                    ));
                }

                // 原版：通过潜行输入下坐骑后调用 setSneaking(false)
                if passenger_entity.sneaking.load(Relaxed) {
                    passenger_entity.set_sneaking(false);
                }
            } else {
                passenger_entity.set_pos(dismount_pos);
            }
        } else {
            // 没有乘客被移除，仍需广播乘客列表
            let world = self.world.load();
            world.broadcast_to_chunk(
                chunk_pos,
                &CSetPassengers::new(VarInt(self.entity_id), &passenger_ids),
            );
        }
    }

    pub fn check_out_of_world(&self, dyn_self: &dyn EntityBase) {
        if self.pos.load().y < f64::from(self.world.load().dimension.min_y) - 64.0 {
            dyn_self.tick_in_void(dyn_self);
        }
    }

    pub fn reset_state(&self) {
        self.pose.store(EntityPose::Standing);
        self.fall_flying.store(false, Relaxed);
        self.extinguish();
        self.set_on_fire(false);
    }

    pub fn slow_movement(&self, state: &BlockState, multiplier: Vector3<f64>) {
        match self.entity_type.id {
            v if v == EntityType::PLAYER.id => {
                if let Some(player_entity) = self.get_player()
                    && player_entity.is_flying()
                {
                    return;
                }
            }
            v if (v == EntityType::SPIDER.id || v == EntityType::CAVE_SPIDER.id)
                && Block::from_state_id(state.id).id == Block::COBWEB.id =>
            {
                return;
            }
            v if v == EntityType::WITHER.id => {
                return;
            }
            _ => {}
        }
        if let Some(living) = self.get_living_entity() {
            living.fall_distance.store(0f32);
        }
        self.movement_multiplier.store(multiplier);
    }

    pub fn set_custom_data(&self, namespace: &str, key: &str, value: NbtTag) {
        let mut custom_data = self
            .custom_data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let mut namespace_data = custom_data
            .child_tags
            .remove(namespace)
            .and_then(|tag| match tag {
                NbtTag::Compound(compound) => Some(compound),
                _ => None,
            })
            .unwrap_or_default();

        namespace_data.child_tags.insert(key.into(), value);
        custom_data
            .child_tags
            .insert(namespace.into(), NbtTag::Compound(namespace_data));
    }

    pub fn get_custom_data(&self, namespace: &str, key: &str) -> Option<NbtTag> {
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

        let Some(NbtTag::Compound(mut namespace_data)) = custom_data.child_tags.remove(namespace)
        else {
            return;
        };

        namespace_data.child_tags.remove(key);
        if !namespace_data.is_empty() {
            custom_data
                .child_tags
                .insert(namespace.into(), NbtTag::Compound(namespace_data));
        }
    }

    pub fn has_custom_data(&self, namespace: &str, key: &str) -> bool {
        self.get_custom_data(namespace, key).is_some()
    }
}

impl Entity {
    pub fn write_nbt(&self, nbt: &mut NbtCompound) {
        let position = self.pos.load();
        nbt.put_string(
            "id",
            format!("minecraft:{}", self.entity_type.resource_name),
        );
        nbt.put_uuid("UUID", self.entity_uuid);
        nbt.put(
            "Pos",
            NbtTag::List(vec![
                position.x.into(),
                position.y.into(),
                position.z.into(),
            ]),
        );
        let velocity = self.velocity.load();
        nbt.put(
            "Motion",
            NbtTag::List(vec![
                velocity.x.into(),
                velocity.y.into(),
                velocity.z.into(),
            ]),
        );
        nbt.put(
            "Rotation",
            NbtTag::List(vec![self.yaw.load().into(), self.pitch.load().into()]),
        );
        nbt.put_short("Fire", self.fire_ticks.load(Relaxed) as i16);
        nbt.put_bool("OnGround", self.on_ground.load(Relaxed));
        nbt.put_bool("Invulnerable", self.invulnerable.load(Relaxed));
        nbt.put_int("PortalCooldown", self.portal_cooldown.load(Relaxed) as i32);
        if self.has_visual_fire.load(Relaxed) {
            nbt.put_bool("HasVisualFire", true);
        }
        nbt.put_int("TicksFrozen", self.frozen_ticks.load(Relaxed));
        if let Some(custom_name) = &**self.custom_name.load()
            && let Ok(name_json) = papokin_util::serde_json::to_string(custom_name)
        {
            nbt.put_string("CustomName", name_json);
        }
        nbt.put_bool("CustomNameVisible", self.custom_name_visible.load(Relaxed));

        let tags = self
            .scoreboard_tags
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !tags.is_empty() {
            nbt.put(
                "Tags",
                NbtTag::List(
                    tags.iter()
                        .map(|tag| NbtTag::String(tag.as_str().into()))
                        .collect(),
                ),
            );
        }

        let custom_data = self
            .custom_data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !custom_data.is_empty() {
            nbt.put_compound("PumpkinCustomData", custom_data.clone());
        }

        // todo 更多...
    }

    pub fn read_nbt_non_mut(&self, nbt: &NbtCompound) {
        if let Some(position) = nbt.get_list("Pos")
            && position.len() >= 3
        {
            let pos = sanitize_loaded_position(Vector3::new(
                finite_f64_or(position[0].extract_double().unwrap_or(0.0), 0.0),
                finite_f64_or(position[1].extract_double().unwrap_or(0.0), 0.0),
                finite_f64_or(position[2].extract_double().unwrap_or(0.0), 0.0),
            ));
            self.set_pos(pos);
            self.last_sent_pos.store(pos);
        }
        if let Some(velocity) = nbt.get_list("Motion")
            && velocity.len() >= 3
        {
            self.velocity.store(sanitize_loaded_motion(Vector3::new(
                finite_f64_or(velocity[0].extract_double().unwrap_or(0.0), 0.0),
                finite_f64_or(velocity[1].extract_double().unwrap_or(0.0), 0.0),
                finite_f64_or(velocity[2].extract_double().unwrap_or(0.0), 0.0),
            )));
        }
        if let Some(rotation) = nbt.get_list("Rotation")
            && rotation.len() >= 2
        {
            let yaw = finite_f32_or(rotation[0].extract_float().unwrap_or(0.0), 0.0);
            let pitch = finite_f32_or(rotation[1].extract_float().unwrap_or(0.0), 0.0);
            self.set_rotation(yaw, pitch);
            let yaw_byte = (yaw * 256.0 / 360.0).rem_euclid(256.0) as u8;
            let pitch_byte = (pitch * 256.0 / 360.0).rem_euclid(256.0) as u8;
            self.last_sent_yaw.store(yaw_byte, Relaxed);
            self.last_sent_pitch.store(pitch_byte, Relaxed);
            self.head_yaw.store(yaw);
            self.last_sent_head_yaw.store(yaw_byte, Relaxed);
        }
        self.fire_ticks
            .store(i32::from(nbt.get_short("Fire").unwrap_or(0)), Relaxed);
        self.on_ground
            .store(nbt.get_bool("OnGround").unwrap_or(false), Relaxed);
        self.invulnerable
            .store(nbt.get_bool("Invulnerable").unwrap_or(false), Relaxed);
        self.portal_cooldown
            .store(nbt.get_int("PortalCooldown").unwrap_or(0) as u32, Relaxed);
        self.has_visual_fire
            .store(nbt.get_bool("HasVisualFire").unwrap_or(false), Relaxed);
        self.frozen_ticks
            .store(nbt.get_int("TicksFrozen").unwrap_or(0), Relaxed);
        if let Some(name_json) = nbt.get_string("CustomName")
            && let Ok(component) = papokin_util::serde_json::from_str(name_json)
        {
            self.custom_name.store(Arc::new(Some(component)));
        }
        self.custom_name_visible
            .store(nbt.get_bool("CustomNameVisible").unwrap_or(false), Relaxed);

        if let Some(tag_list) = nbt.get_list("Tags") {
            let mut tags = self
                .scoreboard_tags
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            tags.clear();
            tags.extend(
                tag_list
                    .iter()
                    .filter_map(|tag| tag.extract_string().map(str::to_owned))
                    .take(MAX_SCOREBOARD_TAGS),
            );
        }

        if let Some(custom_data) = nbt
            .get_compound("PumpkinCustomData")
            .or_else(|| nbt.get_compound("BukkitValues"))
        {
            let mut data = self
                .custom_data
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *data = custom_data.clone();
        }

        // todo 更多...
    }
}

impl EntityBase for Entity {
    fn tick(&self, caller: &dyn EntityBase, _server: &Server) {
        // 在同一刻的移动/方块碰撞处理中重新计算。
        let was_in_powder_snow = self.is_in_powder_snow.load(Ordering::Relaxed);
        self.was_in_powder_snow
            .store(was_in_powder_snow, Ordering::Relaxed);
        self.is_in_powder_snow.store(false, Ordering::Relaxed);

        self.update_last_pos();
        self.tick_portal(caller);
        self.update_fluid_state(caller);
        self.check_out_of_world(caller);
        let fire_ticks = self.fire_ticks.load(Ordering::Relaxed);

        // 检查抗火（或该特定实体是否具有）
        let is_immune = self.entity_type.fire_immune || self.fire_immune.load(Ordering::Relaxed);
        if fire_ticks > 0 {
            if is_immune {
                self.fire_ticks.store(fire_ticks - 4, Ordering::Relaxed);
                if self.fire_ticks.load(Ordering::Relaxed) < 0 {
                    self.extinguish();
                }
            } else {
                if fire_ticks % 20 == 0 {
                    caller.damage(caller, 1.0, DamageType::ON_FIRE);
                }

                self.fire_ticks.store(fire_ticks - 1, Ordering::Relaxed);
            }
        }

        // 检查是否应发送视觉火焰
        let should_render_fire = self.fire_ticks.load(Ordering::Relaxed) > 0 && !is_immune;
        self.set_on_fire(should_render_fire);

        let riding_cooldown = self.riding_cooldown.load(Ordering::Relaxed);
        if riding_cooldown > 0 {
            self.riding_cooldown
                .store(riding_cooldown - 1, Ordering::Relaxed);
        }
    }

    fn get_entity(&self) -> &Entity {
        self
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl<T: EntityBase + ?Sized> NBTStorage for T {
    fn write_nbt(&self, nbt: &mut NbtCompound) {
        EntityBase::write_nbt(self, nbt);
    }

    fn read_nbt_non_mut(&self, nbt: &NbtCompound) {
        EntityBase::read_nbt_non_mut(self, nbt);
    }
}

pub trait NBTStorage: Send + Sync {
    fn write_nbt(&self, _nbt: &mut NbtCompound) {}

    fn read_nbt(&mut self, nbt: &mut NbtCompound) {
        self.read_nbt_non_mut(nbt);
    }

    fn read_nbt_non_mut(&self, _nbt: &NbtCompound) {}
}

pub trait NBTStorageInit: Send + Sync + Sized {
    fn create_from_nbt(_nbt: &mut NbtCompound) -> Option<Self> {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// 表示在实体元数据中发送的各种实体标志。
///
/// 客户端使用这些标志根据实体的当前状态修改实体的渲染。
///
/// **目的：**
///
/// 与使用原始整数值相比，此枚举提供了一种更类型安全、更易读的实体标志表示方式。
pub enum Flag {
    /// 表示实体是否着火。
    OnFire = 0,
    /// 表示实体是否在潜行。
    Sneaking = 1,
    /// 表示实体是否在疾跑。
    Sprinting = 3,
    /// 表示实体是否在游泳。
    Swimming = 4,
    /// 表示实体是否隐形。
    Invisible = 5,
    /// 表示实体是否发光。
    Glowing = 6,
    /// 表示实体是否处于因坠落触发的飞行状态。
    FallFlying = 7,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equipment_break_status_maps_all_slots() {
        // 来自原版 EntityEvent 的状态字节：mainhand=47、offhand=48、
        // 头部=49，胸部=50，腿部=51，脚部=52，身体=65，鞍=68。
        let cases: &[(&EquipmentSlot, u8)] = &[
            (&EquipmentSlot::MAIN_HAND, EntityStatus::MainhandBreak as u8),
            (&EquipmentSlot::OFF_HAND, EntityStatus::OffhandBreak as u8),
            (&EquipmentSlot::HEAD, EntityStatus::HeadBreak as u8),
            (&EquipmentSlot::CHEST, EntityStatus::ChestBreak as u8),
            (&EquipmentSlot::LEGS, EntityStatus::LegsBreak as u8),
            (&EquipmentSlot::FEET, EntityStatus::FeetBreak as u8),
            (&EquipmentSlot::BODY, EntityStatus::BodyBreak as u8),
            (&EquipmentSlot::SADDLE, EntityStatus::SaddleBreak as u8),
        ];
        for (i, (slot, expected)) in cases.iter().enumerate() {
            assert_eq!(
                equipment_break_status(slot) as u8,
                *expected,
                "status mismatch at index {i}"
            );
        }
    }

    #[test]
    fn finite_sanitizers_reject_nan_and_infinity() {
        // 存档中的 NaN/Inf 必须回退到默认值，正常值（含负数）原样通过。
        assert_eq!(finite_f64_or(f64::NAN, 1.5), 1.5);
        assert_eq!(finite_f64_or(f64::INFINITY, 1.5), 1.5);
        assert_eq!(finite_f64_or(f64::NEG_INFINITY, 1.5), 1.5);
        assert_eq!(finite_f64_or(-3.25, 1.5), -3.25);
        assert_eq!(finite_f64_or(0.0, 1.5), 0.0);

        assert_eq!(finite_f32_or(f32::NAN, 2.0), 2.0);
        assert_eq!(finite_f32_or(f32::INFINITY, 2.0), 2.0);
        assert_eq!(finite_f32_or(-180.0, 2.0), -180.0);

        assert_eq!(finite_non_negative_f32_or(f32::NAN, 0.5), 0.5);
        assert_eq!(finite_non_negative_f32_or(f32::INFINITY, 0.5), 0.5);
        assert_eq!(finite_non_negative_f32_or(-1.0, 0.5), 0.5);
        assert_eq!(finite_non_negative_f32_or(0.05, 0.5), 0.05);
    }

    #[test]
    fn loaded_position_is_clamped_to_world_bounds() {
        // 编辑存档注入的极端有限坐标必须被钳制，正常坐标原样通过。
        let clamped = sanitize_loaded_position(Vector3::new(1.0e300, -1.0e300, 5.0e9));
        assert_eq!(clamped.x, LOADED_POS_HORIZONTAL_LIMIT);
        assert_eq!(clamped.y, -LOADED_POS_VERTICAL_LIMIT);
        assert_eq!(clamped.z, LOADED_POS_HORIZONTAL_LIMIT);

        let normal = sanitize_loaded_position(Vector3::new(-123.5, 64.0, 321.0));
        assert_eq!(normal, Vector3::new(-123.5, 64.0, 321.0));
    }

    #[test]
    fn loaded_motion_is_clamped_to_sane_magnitude() {
        let clamped = sanitize_loaded_motion(Vector3::new(1.0e300, -500.0, 42.0));
        assert_eq!(clamped.x, LOADED_MOTION_LIMIT);
        assert_eq!(clamped.y, -LOADED_MOTION_LIMIT);
        assert_eq!(clamped.z, 42.0);

        let normal = sanitize_loaded_motion(Vector3::new(-3.0, 0.5, 2.0));
        assert_eq!(normal, Vector3::new(-3.0, 0.5, 2.0));
    }

    #[test]
    fn vehicle_chain_reaches_detects_cycles() {
        // 线性链 B→A（A 为链尾）查找 C：不可达，不成环。
        let mut linear_chain = vec![2, 1];
        assert!(!vehicle_chain_reaches(|| linear_chain.pop(), 3));

        // 环 A→B→A 查找 A：可达，成环。
        let cycle = [2, 1];
        let mut next_index = 0usize;
        assert!(vehicle_chain_reaches(
            || {
                let id = cycle[next_index % cycle.len()];
                next_index += 1;
                Some(id)
            },
            1
        ));
    }

    #[test]
    fn vehicle_chain_reaches_caps_degenerate_chains() {
        // 无环但超过跳数上限的退化链按成环处理，遍历必然终止。
        let mut remaining = MAX_VEHICLE_CHAIN_HOPS;
        assert!(vehicle_chain_reaches(
            || {
                remaining -= 1;
                Some(remaining as i32)
            },
            i32::MAX
        ));
        assert_eq!(remaining, 0);
    }
}
