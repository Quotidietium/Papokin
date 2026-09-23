use papokin_data::item::Item;
use papokin_data::particle::Particle;
use papokin_data::potion::Effect;
use papokin_data::tag::{self, Taggable};
use papokin_data::tracked_data;
use papokin_inventory::build_equipment_slots;
use papokin_inventory::player::player_inventory::PlayerInventory;
use papokin_inventory::screen_handler::InventoryPlayer;
use papokin_util::Difficulty;
use papokin_util::GameMode;
use papokin_util::Hand;
use papokin_util::math::position::BlockPos;
use rustc_hash::FxHashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::atomic::{
    AtomicBool, AtomicI32, AtomicI64, AtomicU8,
    Ordering::{Relaxed, SeqCst},
};
use tracing::warn;

use super::experience_orb::ExperienceOrbEntity;
use super::{Entity, EntityBase, NBTStorageInit};
use crate::block::OnLandedUponArgs;
use crate::entity::NBTStorage;
use crate::entity::ageable::AgeableMob;
use crate::entity::attributes::AttributeInstance;
use crate::entity::attributes::Modifier;
use crate::entity::attributes::ModifierOperation;
use crate::entity::combat::{CombatRules, CombatTracker, FallLocation, knockback_after_resistance};
use crate::entity::mob::equipment::DEFAULT_EQUIPMENT_DROP_CHANCE;
use crate::entity::player::statistics::{CustomStatistic, StatisticCategory};
use crate::server::Server;
use crate::world::loot::LootContextParameters;
use crossbeam::atomic::AtomicCell;
use papokin_data::AttributeModifierSlot;
use papokin_data::attributes::Attributes;
use papokin_data::damage_ext::ResolvedDamageType;
use papokin_data::data_component_impl::Operation;
use papokin_data::data_component_impl::food::{ConsumableImpl, ConsumeEffect};
use papokin_data::data_component_impl::{
    AttributeModifiersImpl, BlocksAttacksImpl, DeathProtectionImpl, EnchantmentsImpl,
    EquipmentSlot, EquippableImpl, FoodImpl,
};
use papokin_data::effect::StatusEffect;
use papokin_data::entity::{EntityPose, EntityStatus, EntityType};
use papokin_data::fluid::Fluid;
use papokin_data::game_rules::{GameRule, GameRuleValue};
use papokin_data::item_stack::{DamageResult, ItemStack};
use papokin_data::sound::SoundCategory;
use papokin_data::{Block, Enchantment};
use papokin_data::{damage::DamageType, sound::Sound};
use papokin_inventory::entity_equipment::EntityEquipment;
use papokin_nbt::compound::NbtCompound;
use papokin_nbt::tag::NbtTag;
use papokin_protocol::codec::var_int::VarInt;
use papokin_protocol::java::client::play::{
    CHurtAnimation, CSetPlayerInventory, CTakeItemEntity, CUpdateMobEffect,
};
use papokin_protocol::{
    codec::item_stack_seralizer::ItemStackSerializer,
    java::client::play::{CSetEquipment, MetadataSerializer},
    ser::{NetworkWriteExt, WritingError},
};
use papokin_util::math::boundingbox::BoundingBox;
use papokin_util::math::vector3::Vector3;
use papokin_util::text::TextComponent;
use papokin_world::tick::TickPriority;
use papokin_world::world::BlockFlags;
use rand::RngExt;
use std::sync::RwLock;

/// 表示游戏世界中的一个生物实体。
///
/// 此结构体封装了活体实体的核心属性和行为，包括玩家、怪物和其他生物。
pub struct LivingEntity {
    /// 底层实体对象，提供基本的实体信息与功能。
    pub entity: Entity,
    /// 跟踪实体恢复生命值前剩余的时间。
    pub hurt_cooldown: AtomicI32,
    /// 存储实体最近一次受到的伤害量。
    pub last_damage_taken: AtomicCell<f32>,
    /// 实体当前的生命值。
    pub health: AtomicCell<f32>,
    /// 实体当前的伤害吸收值（黄色爱心）。
    pub absorption: AtomicCell<f32>,
    pub item_use_time: AtomicI32,
    pub item_in_use: std::sync::Mutex<Option<ItemStack>>,
    pub active_hand: std::sync::Mutex<Option<Hand>>,
    pub recent_kinetic_enemies: std::sync::Mutex<FxHashMap<i32, i32>>,
    pub death_time: AtomicU8,
    /// 表示实体是否已死亡。（已调用 `on_death`）
    pub dead: AtomicBool,
    /// 实体已下落的距离。
    pub fall_distance: AtomicCell<f32>,
    pub active_effects: std::sync::Mutex<FxHashMap<&'static StatusEffect, Effect>>,
    pub entity_equipment: Arc<std::sync::Mutex<EntityEquipment>>,
    pub equipment_drop_chances: Arc<std::sync::Mutex<FxHashMap<EquipmentSlot, f32>>>,
    pub movement_input: AtomicCell<Vector3<f64>>,
    pub equipment_slots: Arc<FxHashMap<usize, EquipmentSlot>>,

    pub jumping: AtomicBool,

    pub jumping_cooldown: AtomicU8,

    pub climbing: AtomicBool,

    /// 实体最后攀爬的位置，用于死亡消息
    pub climbing_pos: AtomicCell<Option<BlockPos>>,

    /// 最近攻击此生物实体的实体的实体 ID。
    pub last_attacker_id: AtomicI32,
    /// 此实体最后一次被攻击的刻（以实体年龄计）。
    pub last_attacked_time: AtomicI32,
    last_damage_type: std::sync::Mutex<Option<ResolvedDamageType>>,
    last_damage_stamp: std::sync::atomic::AtomicI64,

    /// 此生物实体最近攻击的实体的实体 ID。
    pub last_attacking_id: AtomicI32,
    /// 此实体最后一次攻击某物的刻（以实体年龄计）。
    pub last_attack_time: AtomicI32,

    /// 跟踪战斗条目、协助摔落、击杀归属和死亡消息。
    pub combat_tracker: std::sync::Mutex<CombatTracker>,

    /// 最后伤害此实体的玩家的 ID。
    pub last_hurt_by_player_id: AtomicI32,
    /// 此实体最后一次被玩家伤害的刻。
    pub last_hurt_by_player_time: AtomicI64,
    /// 最后伤害此实体的生物/实体 ID。
    pub last_hurt_by_mob_id: AtomicI32,
    /// 此实体最后一次被生物/实体伤害的刻。
    pub last_hurt_by_mob_time: AtomicI64,

    water_movement_speed_multiplier: f32,
    livings_flags: AtomicU8,

    /// 实体最后所在的方块位置，用于触发位置变更效果。
    pub last_block_pos: AtomicCell<Option<BlockPos>>,

    /// 实体的属性
    pub attributes: RwLock<FxHashMap<u8, AttributeInstance>>,
    /// 从每个装备栏位的当前物品应用的修改器 ID。
    /// 用于在卸下装备时（没有先前物品堆的情况下）移除它们。
    equipment_attribute_modifier_ids: std::sync::Mutex<FxHashMap<EquipmentSlot, Vec<(u8, String)>>>,
}

#[derive(Clone)]
struct EffectParticle {
    particle_id: VarInt,
    color: i32,
}

#[derive(Clone)]
struct EffectParticles(Vec<EffectParticle>);

impl MetadataSerializer for EffectParticles {
    fn write_metadata(
        &self,
        writer: &mut impl std::io::Write,
        _version: &papokin_util::version::JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        let count = i32::try_from(self.0.len())
            .map_err(|_| WritingError::Message("Too many effect particles".into()))?;
        writer.write_var_int(&VarInt(count))?;
        for particle in &self.0 {
            writer.write_var_int(&particle.particle_id)?;
            writer.write_i32(particle.color)?;
        }
        Ok(())
    }
}

impl EffectParticle {
    const fn from_effect(effect: &Effect) -> Self {
        Self {
            particle_id: VarInt(Particle::EntityEffect as i32),
            color: (((if effect.ambient { 38 } else { 255 }) as u32) << 24
                | effect.effect_type.color as u32) as i32,
        }
    }
}

fn is_allowed_by_team_rules(
    own_team: Option<&crate::world::scoreboard::Team>,
    their_team: Option<&crate::world::scoreboard::Team>,
) -> bool {
    use crate::world::scoreboard::CollisionRule;

    let own_rule = own_team.map_or(CollisionRule::Always, |team| team.collision_rule);
    let their_rule = their_team.map_or(CollisionRule::Always, |team| team.collision_rule);

    if own_rule == CollisionRule::Never || their_rule == CollisionRule::Never {
        return false;
    }

    let same_team = own_team
        .zip(their_team)
        .is_some_and(|(own, their)| own.name == their.name);

    if (own_rule == CollisionRule::PushOwnTeam || their_rule == CollisionRule::PushOwnTeam)
        && same_team
    {
        return false;
    }

    (own_rule != CollisionRule::PushOtherTeams && their_rule != CollisionRule::PushOtherTeams)
        || same_team
}

impl LivingEntity {
    const USING_ITEM_FLAG: u8 = 1;
    const OFF_HAND_ACTIVE_FLAG: u8 = 2;
    const RANDOM_TELEPORT_ATTEMPTS: usize = 16;
    #[expect(dead_code)]
    const USING_RIPTIDE_FLAG: u8 = 4;

    const PREVENT_AREA_FALL_DAMAGE_BLOCKS: [&'static Block; 4] = [
        &Block::COBWEB,
        &Block::LADDER,
        &Block::POWDER_SNOW,
        &Block::SLIME_BLOCK,
    ];

    fn hurt_sound_for_entity(entity_type: &'static EntityType) -> Sound {
        entity_type.hurt_sound.unwrap_or(Sound::EntityGenericHurt)
    }

    fn death_sound_for_entity(entity_type: &'static EntityType) -> Sound {
        entity_type.death_sound.unwrap_or(Sound::EntityGenericDeath)
    }

    fn get_pitch(&self) -> f32 {
        let is_baby = self
            .get_mob()
            .and_then(|x| x.as_ageable())
            .is_some_and(AgeableMob::is_baby);

        let mut rng = rand::rng();
        if is_baby {
            (rng.random::<f32>() - rng.random::<f32>()) * 0.2 + 1.5
        } else {
            (rng.random::<f32>() - rng.random::<f32>()) * 0.2 + 1.0
        }
    }

    pub fn new(entity: Entity) -> Self {
        let water_movement_speed_multiplier = if entity.entity_type == &EntityType::POLAR_BEAR {
            0.98
        } else if entity.entity_type == &EntityType::SKELETON_HORSE {
            0.96
        } else {
            0.8
        };
        let mut max_health: f32 = 20.0; // 会被下方属性的基础值覆盖
        Self {
            // 从默认注册表填充本地属性实例并获取初始变量
            attributes: {
                let mut m = FxHashMap::default();

                for (attr, base) in entity.entity_type.attributes {
                    if attr.id == Attributes::MAX_HEALTH.id {
                        max_health = *base as f32;
                    }
                    m.insert(attr.id, AttributeInstance::new(*base));
                }
                std::sync::RwLock::new(m)
            },
            health: AtomicCell::new(max_health), // 来自属性的初始生命值
            entity,
            hurt_cooldown: AtomicI32::new(0),
            last_damage_taken: AtomicCell::new(0.0),
            absorption: AtomicCell::new(0.0),
            fall_distance: AtomicCell::new(0.0),
            death_time: AtomicU8::new(0),
            dead: AtomicBool::new(false),
            item_use_time: AtomicI32::new(0),
            item_in_use: std::sync::Mutex::new(None),
            active_hand: std::sync::Mutex::new(None),
            recent_kinetic_enemies: std::sync::Mutex::new(FxHashMap::default()),
            livings_flags: AtomicU8::new(0),
            active_effects: std::sync::Mutex::new(FxHashMap::default()),
            entity_equipment: Arc::new(std::sync::Mutex::new(EntityEquipment::new())),
            equipment_drop_chances: Arc::new(std::sync::Mutex::new(FxHashMap::default())),
            equipment_slots: Arc::new(build_equipment_slots()),
            jumping: AtomicBool::new(false),
            jumping_cooldown: AtomicU8::new(0),
            climbing: AtomicBool::new(false),
            climbing_pos: AtomicCell::new(None),
            last_attacker_id: AtomicI32::new(0),
            last_attacked_time: AtomicI32::new(0),
            last_damage_type: std::sync::Mutex::new(None),
            last_damage_stamp: std::sync::atomic::AtomicI64::new(0),
            last_attacking_id: AtomicI32::new(0),
            last_attack_time: AtomicI32::new(0),
            combat_tracker: std::sync::Mutex::new(CombatTracker::new()),
            last_hurt_by_player_id: AtomicI32::new(0),
            last_hurt_by_player_time: AtomicI64::new(0),
            last_hurt_by_mob_id: AtomicI32::new(0),
            last_hurt_by_mob_time: AtomicI64::new(0),
            movement_input: AtomicCell::new(Vector3::default()),
            water_movement_speed_multiplier,
            last_block_pos: AtomicCell::new(None),
            equipment_attribute_modifier_ids: std::sync::Mutex::new(FxHashMap::default()),
        }
    }

    /// 返回应为此实体的死亡获得击杀归属的实体。
    /// 遵循原版 Java 逻辑（`LivingEntity.getKillCredit`）：
    /// 1. 如果在最近 100 刻（5 秒）内受过伤害，优先使用 `last_hurt_by_player`。
    /// 2. 其次，若在最近 100 刻内受过伤害，则使用 `last_hurt_by_mob`。
    /// 3. 若可用，则回退到战斗追踪器的击杀者条目。
    pub fn get_kill_credit(&self) -> Option<Arc<dyn EntityBase>> {
        let world = self.entity.world.load();
        let current_tick = world.level_info.load().day_time;

        let player_id = self.last_hurt_by_player_id.load(Relaxed);
        let player_time = self.last_hurt_by_player_time.load(Relaxed);
        if player_id != 0
            && (current_tick - player_time).abs() <= 100
            && let Some(player) = world.get_entity_by_id(player_id)
        {
            return Some(player);
        }

        let mob_id = self.last_hurt_by_mob_id.load(Relaxed);
        let mob_time = self.last_hurt_by_mob_time.load(Relaxed);
        if mob_id != 0
            && (current_tick - mob_time).abs() <= 100
            && let Some(mob) = world.get_entity_by_id(mob_id)
        {
            return Some(mob);
        }

        let tracker = self
            .combat_tracker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(killer) = tracker.get_killer_entry()
            && let Some(killer_id) = killer.attacker_id
        {
            return world.get_entity_by_id(killer_id);
        }

        None
    }

    /// 当实体的方块位置变化时，触发基于位置的附魔效果（例如冰霜行者）。
    pub fn on_changed_block(&self, caller: &dyn EntityBase, _pos: BlockPos) {
        let pos_f64 = self.entity.pos.load();
        if let Some(player) = caller.get_player() {
            let boots = player.inventory.get_slot(36);
            if !boots.is_empty() {
                crate::enchantment::EnchantmentHelper::on_location_changed(
                    &self.entity,
                    &boots,
                    pos_f64,
                );
            }
        } else {
            let boots = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(&EquipmentSlot::FEET);
            if !boots.is_empty() {
                crate::enchantment::EnchantmentHelper::on_location_changed(
                    &self.entity,
                    &boots,
                    pos_f64,
                );
            }
        }
    }

    pub fn send_equipment_changes(&self, equipment: &[(EquipmentSlot, ItemStack)]) {
        if equipment.is_empty() {
            return;
        }

        // 通知插件每个发生变化的槽位。旧物品已不再
        // 此处可用（调用方已更新装备映射表），因此
        // `old_item` 被报告为 `None`。
        if let Some(server) = self.entity.world.load().server.upgrade() {
            for (slot, stack) in equipment {
                let mut event = crate::plugin::api::events::entity::entity_equipment_changed::EntityEquipmentChangedEvent::new(
                    self.entity.entity_id,
                    slot.to_name().to_string(),
                    None,
                    if stack.is_empty() {
                        None
                    } else {
                        Some(stack.clone())
                    },
                );
                server.plugin_manager.fire_blocking(&server, &mut event);
            }
        }

        self.apply_and_send_equipment_attribute_modifiers(equipment);

        if equipment
            .iter()
            .any(|(slot, _)| *slot == EquipmentSlot::FEET)
        {
            let pos_f64 = self.entity.pos.load();
            for (slot, stack) in equipment {
                if *slot == EquipmentSlot::FEET && !stack.is_empty() {
                    crate::enchantment::EnchantmentHelper::on_location_changed(
                        &self.entity,
                        stack,
                        pos_f64,
                    );
                }
            }
        }

        let equipment_java: Vec<(i8, ItemStackSerializer)> = equipment
            .iter()
            .map(|(slot, stack)| {
                (
                    slot.discriminant(),
                    ItemStackSerializer::from(stack.clone()),
                )
            })
            .collect();
        let je_packet = CSetEquipment::new(self.entity_id().into(), equipment_java);

        for (slot, stack) in equipment {
            if *slot == EquipmentSlot::MAIN_HAND {
                self.update_weapon_attributes(stack);
            }
        }

        self.entity
            .world
            .load()
            .send_to_tracking_players(&self.entity, &je_packet);
    }

    /// 将手持物品的攻击属性修饰符应用到该实体的攻击属性上
    /// 属性映射，并将更改后的属性发送给客户端。没有它
    /// 客户端看不到被降低的攻击速度，也不会显示
    /// 准星攻击指示器。
    fn update_weapon_attributes(&self, stack: &ItemStack) {
        let component = stack.get_data_component::<AttributeModifiersImpl>();

        // 对物品的属性修饰符做单次遍历，按属性拆分。
        let mut speed_modifiers: Vec<Modifier> = Vec::new();
        let mut damage_modifiers: Vec<Modifier> = Vec::new();
        for modifier in component
            .into_iter()
            .flat_map(|c| c.attribute_modifiers.iter())
        {
            let target = if modifier.r#type == &Attributes::ATTACK_SPEED {
                &mut speed_modifiers
            } else if modifier.r#type == &Attributes::ATTACK_DAMAGE {
                &mut damage_modifiers
            } else {
                continue;
            };
            target.push(Modifier {
                id: modifier.id.to_string(),
                amount: modifier.amount,
                operation: match modifier.operation {
                    Operation::AddValue => ModifierOperation::Add,
                    Operation::AddMultipliedBase => ModifierOperation::MultiplyBase,
                    Operation::AddMultipliedTotal => ModifierOperation::MultiplyTotal,
                },
            });
        }

        let mut changed: Vec<Attributes> = Vec::new();
        {
            let mut attributes = self
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for (attribute, modifiers) in [
                (Attributes::ATTACK_SPEED, speed_modifiers),
                (Attributes::ATTACK_DAMAGE, damage_modifiers),
            ] {
                let instance = attributes
                    .entry(attribute.id)
                    .or_insert_with(|| AttributeInstance::new(attribute.default_value));
                if instance.modifiers == modifiers {
                    continue;
                }
                instance.modifiers = modifiers;
                instance.dirty.store(true, Ordering::Relaxed);
                changed.push(attribute);
            }
        }
        if !changed.is_empty() {
            crate::entity::attributes::send_attribute_updates_for_living(self, changed);
        }
    }

    /// 为给定槽位应用物品的 `attribute_modifiers`，并通知客户端。
    ///
    /// 本地 HUD 盔甲条由 `minecraft:armor` / `minecraft:armor_toughness` 驱动
    /// 通过 `UPDATE_ATTRIBUTES` 完成，而不是 `SET_EQUIPMENT`。
    pub fn apply_and_send_equipment_attribute_modifiers(
        &self,
        equipment: &[(EquipmentSlot, ItemStack)],
    ) {
        let mut touched = Vec::new();
        for (slot, stack) in equipment {
            self.apply_equipment_slot_attribute_modifiers(slot, stack, &mut touched);
        }
        if !touched.is_empty() {
            crate::entity::attributes::send_attribute_updates_for_living(self, touched);
        }
    }

    /// 重新应用当前所有已装备物品堆的修饰符，但不通知客户端。
    pub fn apply_current_equipment_attribute_modifiers(&self) {
        let equipment = self.snapshot_equipped_stacks();
        let mut touched = Vec::new();
        for (slot, stack) in &equipment {
            self.apply_equipment_slot_attribute_modifiers(slot, stack, &mut touched);
        }
    }

    /// 重新应用当前所有已装备物品堆的修饰符，并发送更新。
    pub fn send_current_equipment_attribute_modifiers(&self) {
        self.apply_and_send_equipment_attribute_modifiers(&self.snapshot_equipped_stacks());
    }

    fn snapshot_equipped_stacks(&self) -> Vec<(EquipmentSlot, ItemStack)> {
        let guard = self
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard
            .equipment
            .iter()
            .map(|(slot, stack)| (slot.clone(), stack.clone()))
            .collect()
    }

    fn apply_equipment_slot_attribute_modifiers(
        &self,
        slot: &EquipmentSlot,
        stack: &ItemStack,
        touched: &mut Vec<Attributes>,
    ) {
        let previous = {
            let mut map = self
                .equipment_attribute_modifier_ids
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            map.remove(slot).unwrap_or_default()
        };
        for (attr_id, modifier_id) in previous {
            if let Some(attr) = attributes_by_id(attr_id) {
                self.update_attribute(attr, |inst| inst.remove_modifier(&modifier_id));
                push_unique_attribute(touched, attr);
            }
        }

        if stack.is_empty() {
            return;
        }
        let Some(modifiers) = stack.get_data_component::<AttributeModifiersImpl>() else {
            return;
        };

        let mut applied = Vec::new();
        for item_mod in modifiers.attribute_modifiers.iter() {
            if !attribute_modifier_slot_matches(&item_mod.slot, slot) {
                continue;
            }
            let operation = match item_mod.operation {
                Operation::AddValue => ModifierOperation::Add,
                Operation::AddMultipliedBase => ModifierOperation::MultiplyBase,
                Operation::AddMultipliedTotal => ModifierOperation::MultiplyTotal,
            };
            self.update_attribute(item_mod.r#type, |inst| {
                inst.add_or_replace_modifier(Modifier {
                    id: item_mod.id.to_string(),
                    amount: item_mod.amount,
                    operation,
                });
            });
            applied.push((item_mod.r#type.id, item_mod.id.to_string()));
            push_unique_attribute(touched, item_mod.r#type);
        }

        if !applied.is_empty() {
            let mut map = self
                .equipment_attribute_modifier_ids
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            map.insert(slot.clone(), applied);
        }
    }

    /// 拾取物品实体或经验球
    pub fn pickup(&self, item: &Entity, stack_amount: u32) -> bool {
        let mut pickup_event =
            crate::plugin::api::events::entity::entity_pickup_item::EntityPickupItemEvent::new(
                self.entity.entity_id,
                item.entity_type.id.to_string(),
                stack_amount as u8,
            );
        if let Some(server) = self.entity.world.load().server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut pickup_event);
            if pickup_event.cancelled {
                return false;
            }
        }

        let chunk_pos = self.entity.chunk_pos.load();
        self.entity.world.load().broadcast_to_chunk(
            chunk_pos,
            &CTakeItemEntity::new(
                item.entity_id.into(),
                self.entity.entity_id.into(),
                VarInt(stack_amount as i32),
            ),
        );
        true
    }

    /// 将手部动画发送给所有其他玩家，例如进食时会使用
    pub fn set_active_hand(&self, hand: Hand, stack: ItemStack, duration: i32) {
        self.item_use_time.store(duration, Ordering::Relaxed);
        *self
            .item_in_use
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(stack);
        *self
            .active_hand
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(hand);
        self.recent_kinetic_enemies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
        self.set_living_flag(Self::USING_ITEM_FLAG, true);
        self.set_living_flag(Self::OFF_HAND_ACTIVE_FLAG, hand == Hand::Left);
    }

    fn set_living_flag(&self, flag: u8, value: bool) {
        let index = flag;
        let mut b = self.livings_flags.load(Ordering::Relaxed);
        if value {
            b |= index;
        } else {
            b &= !index;
        }
        self.livings_flags.store(b, Ordering::Relaxed);

        self.entity
            .set_synced_data(tracked_data::living_entity::DATA_LIVING_ENTITY_FLAGS, b);
    }

    pub fn clear_active_hand(&self) {
        *self
            .item_in_use
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        *self
            .active_hand
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        self.recent_kinetic_enemies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
        self.item_use_time.store(0, Ordering::Relaxed);

        self.set_living_flag(Self::USING_ITEM_FLAG, false);
    }

    pub fn was_recently_stabbed(&self, target_id: i32, now: i32, allowed_ticks: i32) -> bool {
        self.recent_kinetic_enemies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&target_id)
            .is_some_and(|stabbed_at| now - stabbed_at < allowed_ticks)
    }

    pub fn remember_stabbed_entity(&self, target_id: i32, now: i32) {
        self.recent_kinetic_enemies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(target_id, now);
    }

    pub fn is_blocking(&self) -> bool {
        let item_in_use = self
            .item_in_use
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(item) = item_in_use.as_ref()
            && item.get_data_component::<BlocksAttacksImpl>().is_some()
        {
            let use_time = self.item_use_time.load(Ordering::Relaxed);
            let required_time = 5;
            return item.get_max_use_time() - use_time >= required_time;
        }
        false
    }

    pub fn heal(&self, additional_health: f32) {
        assert!(additional_health > 0.0);
        let mut event =
            crate::plugin::api::events::entity::entity_regain_health::EntityRegainHealthEvent::new(
                self.entity.entity_id,
                additional_health,
            );
        if let Some(server) = self.entity.world.load().server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return;
            }
        }
        self.set_health(self.health.load() + additional_health);
    }

    pub fn set_health(&self, health: f32) {
        // 钳制到 [0, max_health]
        let max_health = self.get_max_health();
        let clamped = health.max(0.0).min(max_health);
        self.health.store(clamped);
        // 告知所有人实体生命值已改变
        self.entity
            .set_synced_data(tracked_data::living_entity::DATA_HEALTH_ID, clamped);
    }

    ///返回此实体当前的最大生命值
    pub fn get_max_health(&self) -> f32 {
        self.get_attribute_value(&Attributes::MAX_HEALTH) as f32
    }

    /// 设置此实体的最大生命值
    pub fn set_max_health(&self, max_health: f32) {
        // 更新基础属性
        self.set_attribute_base(&Attributes::MAX_HEALTH, max_health as f64);

        // 广播属性变更
        crate::entity::attributes::send_attribute_updates_for_living(
            self,
            vec![Attributes::MAX_HEALTH],
        );

        // 必要时将当前生命值钳制到新的上限，并发送元数据更新
        let current_health = self.health.load();
        if current_health > max_health {
            self.set_health(max_health);
        }
    }

    ///返回此实体当前的生命值吸收量（黄色爱心）
    pub fn get_absorption(&self) -> f32 {
        self.absorption.load()
    }

    /// 设置此实体当前的伤害吸收量（黄心）
    pub fn set_absorption(&self, new_abs: f32) {
        // 必须至少为 0
        let new_abs = new_abs.max(0.0);

        // 设置本地状态
        self.absorption.store(new_abs);

        // 广播 max_absorption 的属性更新，使客户端收到
        // 通过属性数据包发送更新后的伤害吸收值。
        crate::entity::attributes::send_attribute_updates_for_living(
            self,
            vec![Attributes::MAX_ABSORPTION],
        );

        // 为玩家发送吸收元数据（视觉上显示为黄心）
        if self.entity.entity_type == &EntityType::PLAYER {
            self.entity
                .set_synced_data(tracked_data::player::DATA_PLAYER_ABSORPTION_ID, new_abs);
        }
    }

    /// 修改属性实例的便捷辅助方法。会自动插入
    /// 一个在需要时从注册表基础数据填充的新实例。
    pub fn update_attribute<F: FnOnce(&mut AttributeInstance)>(
        &self,
        attribute: &Attributes,
        f: F,
    ) {
        let mut map = self
            .attributes
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let inst = map.entry(attribute.id).or_insert_with(|| {
            let base = self
                .entity
                .entity_type
                .attributes
                .iter()
                .find(|a| a.0.id == attribute.id)
                .map_or_else(
                    || {
                        tracing::warn!(
                            "实体类型 {:?} 没有属性 {:?} 的基础值；回退到默认值 {}",
                            self.entity.entity_type,
                            attribute.id,
                            attribute.default_value,
                        );
                        attribute.default_value
                    },
                    |a| a.1,
                );
            AttributeInstance::new(base)
        });

        f(inst);
        inst.dirty.store(true, Ordering::Relaxed);
    }

    ///使用本地实例返回 `attribute` 的计算值，不可用时回退到注册表
    /// 回退到 `attribute.default_value`（如果不存在本地实例）。
    pub fn get_attribute_value(&self, attribute: &Attributes) -> f64 {
        let map = self
            .attributes
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.get(&attribute.id)
            .map_or(attribute.default_value, AttributeInstance::value)
    }

    ///返回该实体类型在 `attribute` 上的基础属性值。
    pub fn get_attribute_base(&self, attribute: &Attributes) -> f64 {
        // 先检查本地基础值（可能已被修改）
        let map = self
            .attributes
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(instance) = map.get(&attribute.id) {
            return instance.base_value;
        }

        // 若不存在本地实例，则回退到注册表基准值
        self.entity
            .entity_type
            .attributes
            .iter()
            .find(|a| a.0.id == attribute.id)
            .map_or(attribute.default_value, |a| a.1)
    }

    /// 更新或插入此实体上某属性的基础值。
    /// 如果该属性在本地尚不存在，则会被插入。
    pub fn set_attribute_base(&self, attribute: &Attributes, new_base: f64) {
        let mut map = self
            .attributes
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(inst) = map.get_mut(&attribute.id) {
            inst.base_value = new_base;
            inst.dirty.store(true, Ordering::Relaxed);
        } else {
            let ai = AttributeInstance::new(new_base);
            ai.dirty.store(true, Ordering::Relaxed);
            map.insert(attribute.id, ai);
        }
    }

    pub fn reset_effects_and_attributes(&self) {
        // 清除激活的效果并重置被修改的属性
        let effects_to_remove: Vec<_> = {
            let lock = self
                .active_effects
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            lock.keys().copied().collect()
        };

        for effect_type in effects_to_remove {
            self.remove_effect(effect_type);
        }
    }

    pub const fn entity_id(&self) -> i32 {
        self.entity.entity_id
    }

    #[expect(clippy::too_many_lines)]
    pub fn add_effect(&self, effect: Effect) {
        let mut effect_event =
            crate::plugin::api::events::entity::entity_potion_effect::EntityPotionEffectEvent::new(
                self.entity.entity_id,
                effect.effect_type.translation_key.to_string(),
                effect.duration,
                effect.amplifier,
            );
        if let Some(server) = self.entity.world.load().server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut effect_event);
        }
        if effect_event.cancelled {
            return;
        }

        // 在存储前立即应用即时效果
        if effect.effect_type == &StatusEffect::INSTANT_HEALTH {
            let heal_amount = 4.0 * (1 << effect.amplifier) as f32;
            self.heal(heal_amount);
            // 与原版一样，即时效果从不作为活跃效果发送或存储。
            return;
        } else if effect.effect_type == &StatusEffect::INSTANT_DAMAGE {
            let damage_amount = 6.0 * (1 << effect.amplifier) as f32;
            let dyn_self = self
                .entity
                .world
                .load()
                .get_entity_by_id(self.entity.entity_id);
            if let Some(dyn_self) = dyn_self {
                let _ = dyn_self.damage(&*dyn_self, damage_amount, DamageType::MAGIC);
            }
            return;
        }

        // 应用非即时效果

        // 修改属性（如速度）的效果也应更新
        // 实体的属性实例（服务器端），然后通知客户端。
        if !effect.effect_type.attribute_modifiers.is_empty() {
            // 将每个属性修饰符应用到本地 AttributeInstance 中
            for m in effect.effect_type.attribute_modifiers {
                let id = m.id.to_string();
                let op = match m.operation {
                    Operation::AddValue => ModifierOperation::Add,
                    Operation::AddMultipliedBase => ModifierOperation::MultiplyBase,
                    Operation::AddMultipliedTotal => ModifierOperation::MultiplyTotal,
                };
                let scaled_amount = m.base_value * (f64::from(effect.amplifier) + 1.);
                let mod_inst = Modifier {
                    id,
                    amount: scaled_amount,
                    operation: op,
                };

                self.update_attribute(m.attribute, |inst| {
                    inst.add_or_replace_modifier(mod_inst.clone());
                });
            }

            // 根据每个受影响属性的激活效果重新计算数据包修饰符
            let mut touched_attrs: Vec<papokin_data::attributes::Attributes> = Vec::new();
            for m in effect.effect_type.attribute_modifiers {
                if !touched_attrs.iter().any(|a| a.id == m.attribute.id) {
                    touched_attrs.push(m.attribute.clone());
                }
            }

            if !touched_attrs.is_empty() {
                crate::entity::attributes::send_attribute_updates_for_living(self, touched_attrs);
            }
        }

        // 应用吸收效果（每级 +4 吸收值）
        if effect.effect_type == &StatusEffect::ABSORPTION {
            let added = 4.0 * (effect.amplifier as f32 + 1.0);
            let max_abs = self.get_attribute_value(&Attributes::MAX_ABSORPTION) as f32;
            let new_abs = (self.absorption.load() + added).min(max_abs);
            self.set_absorption(new_abs);
        }

        // 应用隐身效果
        if effect.effect_type == &StatusEffect::INVISIBILITY {
            self.entity.set_invisible(true);
        }

        // 应用发光效果
        if effect.effect_type == &StatusEffect::GLOWING {
            self.entity.set_glowing(true);
        }

        // 向附近玩家广播效果
        let mut flag: i8 = 0;
        if effect.ambient {
            flag |= 1;
        }
        if effect.show_particles {
            flag |= 2;
        }
        if effect.show_icon {
            flag |= 4;
        }
        if effect.blend {
            flag |= 8;
        }

        let je_packet = CUpdateMobEffect::new(
            self.entity.entity_id.into(),
            VarInt(i32::from(effect.effect_type.id)),
            effect.amplifier.into(),
            effect.duration.into(),
            flag,
        );

        let chunk_pos = self.entity.chunk_pos.load();
        self.entity
            .world
            .load()
            .broadcast_to_chunk(chunk_pos, &je_packet);
        if effect.effect_type != &StatusEffect::INSTANT_HEALTH
            && effect.effect_type != &StatusEffect::INSTANT_DAMAGE
        {
            self.active_effects
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(effect.effect_type, effect);
        }
        self.sync_effect_particles();
    }

    fn sync_effect_particles(&self) {
        let effects = self
            .active_effects
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let has_effects = !effects.is_empty();
        let particles = EffectParticles(
            effects
                .values()
                .filter(|effect| effect.show_particles)
                .map(EffectParticle::from_effect)
                .collect(),
        );
        let ambient = effects
            .values()
            .filter(|effect| effect.show_particles)
            .all(|effect| effect.ambient);
        drop(effects);

        self.entity
            .set_synced_data(tracked_data::living_entity::EFFECT_PARTICLES, particles);
        if has_effects {
            self.entity
                .set_synced_data(tracked_data::living_entity::EFFECT_AMBIENCE_ID, ambient);
        }
    }

    pub fn remove_effect(&self, effect_type: &'static StatusEffect) -> bool {
        // 移除该效果
        let succeeded = self
            .active_effects
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&effect_type)
            .is_some();

        // 广播效果移除
        self.entity
            .world
            .load()
            .send_remove_mob_effect(&self.entity, effect_type);

        // 移除属性修饰符（如有）
        if !effect_type.attribute_modifiers.is_empty() {
            let mut touched_attrs = Vec::new();

            for m in effect_type.attribute_modifiers {
                let id = m.id.to_string();

                // 清理本地服务器状态
                self.update_attribute(m.attribute, |inst| {
                    inst.remove_modifier(&id);
                });

                // 为数据包更新跟踪不重复的属性
                if !touched_attrs
                    .iter()
                    .any(|a: &Attributes| a.id == m.attribute.id)
                {
                    touched_attrs.push(m.attribute.clone());
                }
            }

            // 将干净状态同步到客户端
            if !touched_attrs.is_empty() {
                crate::entity::attributes::send_attribute_updates_for_living(self, touched_attrs);
            }
        }

        // 如果吸收效果被移除，清空当前吸收值并通知客户端
        if effect_type == &StatusEffect::ABSORPTION {
            self.set_absorption(0.0);
        }

        // 如果生命提升效果被移除，则将当前生命值钳制到新的上限并通知客户端
        if effect_type == &StatusEffect::HEALTH_BOOST {
            let new_max = self.get_max_health();
            if self.health.load() > new_max {
                // 更新本地生命值，并把生命值与伤害吸收元数据一并发送
                self.set_health(new_max.max(0.0));
            }
        }

        // 如果隐身效果被移除，则禁用隐身
        if effect_type == &StatusEffect::INVISIBILITY {
            self.entity.set_invisible(false);
        }

        // 如果发光效果被移除，则禁用发光
        if effect_type == &StatusEffect::GLOWING {
            self.entity.set_glowing(false);
        }

        if succeeded {
            self.sync_effect_particles();
        }

        succeeded
    }

    pub fn has_effect(&self, effect: &'static StatusEffect) -> bool {
        self.active_effects
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains_key(&effect)
    }

    pub fn get_effect(&self, effect: &'static StatusEffect) -> Option<Effect> {
        self.active_effects
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&effect)
            .cloned()
    }

    pub fn is_in_fall_damage_resetting(&self) -> (bool, &Block) {
        let block_pos = self.entity.block_pos.load();
        let block = self.entity.world.load().get_block(&block_pos);
        (
            block.has_tag(&tag::Block::MINECRAFT_FALL_DAMAGE_RESETTING),
            block,
        )
    }

    // 检查实体是否在水中
    pub fn is_in_water(&self) -> bool {
        self.entity.touching_water.load(Ordering::Relaxed)
    }

    // 检查实体是否处于细雪中
    pub fn is_in_powder_snow(&self) -> bool {
        self.entity.is_in_powder_snow.load(Ordering::Relaxed)
    }

    pub fn should_prevent_fall_damage(&self) -> bool {
        let (prevents, block) = self.is_in_fall_damage_resetting();

        if block == &Block::SCAFFOLDING && !self.entity.is_sneaking() {
            return false;
        }

        if block == &Block::WATER {
            return true;
        }

        if self.entity.entity_type == &EntityType::PLAYER {
            if block == &Block::END_GATEWAY || block == &Block::END_PORTAL {
                return true;
            }

            if block == &Block::NETHER_PORTAL {
                let world = self.entity.world.load();
                let level_info = world.level_info.load();

                return level_info.game_rules.players_nether_portal_default_delay == 0;
            }
        }

        prevents
    }

    pub fn should_prevent_fall_damage_in_area(&self) -> bool {
        let world = self.entity.world.load();
        let block_pos = self.entity.block_pos.load().down();
        let entity_pos = self.entity.pos.load();

        let min = BlockPos(Vector3::new(
            block_pos.0.x - 1,
            block_pos.0.y,
            block_pos.0.z - 1,
        ));
        let max = BlockPos(Vector3::new(
            block_pos.0.x + 1,
            block_pos.0.y,
            block_pos.0.z + 1,
        ));
        let pos_iter = BlockPos::iterate(min, max);

        // FIXME: 似乎 Java 服务端会用射线检测检查周围所有方块并判断命中或未命中，
        // 然后添加到碰撞检查器，在刻处理程序中处理
        for pos in pos_iter {
            let block = world.get_block(&pos);

            if Self::PREVENT_AREA_FALL_DAMAGE_BLOCKS.contains(&block) {
                let block_center = Vector3::new(
                    f64::from(pos.0.x) + 0.5,
                    f64::from(pos.0.y) + 0.5,
                    f64::from(pos.0.z) + 0.5,
                );
                let distance = entity_pos.squared_distance_to_vec(&block_center);

                // 从属性获取安全摔落距离
                let safe_distance = self.get_attribute_value(&Attributes::SAFE_FALL_DISTANCE);
                return distance.sqrt() <= safe_distance * safe_distance;
            }
        }

        false
    }

    pub fn is_immune_to_fall_damage(&self) -> bool {
        self.entity
            .entity_type
            .has_tag(&tag::EntityType::MINECRAFT_FALL_DAMAGE_IMMUNE)
    }

    fn get_effective_gravity(&self, caller: &dyn EntityBase) -> f64 {
        let final_gravity = caller.get_gravity();

        if self.entity.velocity.load().y <= 0.0 && self.has_effect(&StatusEffect::SLOW_FALLING) {
            final_gravity.min(0.01)
        } else {
            final_gravity
        }
    }

    pub fn swing_hand(&self) {
        let world = self.entity.world.load();
        let entity_id = self.entity_id();

        let je_packet =
            papokin_protocol::java::client::play::CSwingArm::new(entity_id.into(), false);

        world.broadcast_packet_all(&je_packet);
    }

    pub fn swing_off_hand(&self) {
        let world = self.entity.world.load();
        let entity_id = self.entity_id();

        let je_packet = papokin_protocol::java::client::play::CEntityAnimation::new(
            entity_id.into(),
            papokin_protocol::java::client::play::Animation::SwingOffhand,
        );

        world.broadcast_packet_all(&je_packet);
    }

    fn tick_movement(&self, caller: &dyn EntityBase) {
        if self.jumping_cooldown.load(Relaxed) != 0 {
            self.jumping_cooldown.fetch_sub(1, Relaxed);
        }

        let should_swim_in_fluids = caller.get_player().is_none_or(|player| !player.is_flying());

        self.entity.check_zero_velo();

        let mut movement_input = self.movement_input.load();

        movement_input.x *= 0.98;

        movement_input.z *= 0.98;

        self.movement_input.store(movement_input);

        // TODO: 每刻运行 AI

        if self.jumping.load(SeqCst) && should_swim_in_fluids {
            let in_lava = self.entity.touching_lava.load(SeqCst);

            let in_water = self.entity.touching_water.load(SeqCst);

            let fluid_height = if in_lava {
                self.entity.lava_height.load()
            } else {
                self.entity.water_height.load()
            };

            let swim_height = self.get_swim_height();

            let on_ground = self.entity.on_ground.load(SeqCst);

            if (in_water || in_lava) && (!on_ground || fluid_height > swim_height) {
                // 向上游动

                let mut velo = self.entity.velocity.load();

                velo.y += 0.04;

                self.entity.velocity.store(velo);
            } else if (on_ground || in_water && fluid_height <= swim_height)
                && self.jumping_cooldown.load(SeqCst) == 0
            {
                self.jump();

                self.jumping_cooldown.store(10, SeqCst);
            }
        } else {
            self.jumping_cooldown.store(0, SeqCst);
        }

        if self.has_effect(&StatusEffect::SLOW_FALLING)
            || self.has_effect(&StatusEffect::LEVITATION)
        {
            self.fall_distance.store(0.0);
        }

        let touching_water = self.entity.touching_water.load(SeqCst);

        // 炽足兽是唯一 canWalkOnFluid = false 的实体

        if (touching_water || self.entity.touching_lava.load(SeqCst))
            && should_swim_in_fluids
            && self.entity.entity_type != &EntityType::STRIDER
        {
            self.travel_in_fluid(caller, touching_water);
        } else {
            // TODO: 滑翔

            self.travel_in_air(caller);
        }

        let suffocating = self.entity.tick_block_collisions(caller);

        if suffocating {
            caller.damage(caller, 1.0, DamageType::IN_WALL);
        }

        self.tick_frost_walker();

        self.push_entities(caller);
    }

    /// 冰霜行者：穿着该附魔靴子且站在地面时，把脚下的水源冻成霜冰。
    /// 语义对齐原版 `FrostWalkerEnchantment#onEntityMove`：
    /// 半径为 `2 + 附魔等级`（上限 16）的圆形区域，仅冻结上方为空气的水源方块，
    /// 冻结出的霜冰在 60-120 刻后由方块自身的计划刻负责融化。
    fn tick_frost_walker(&self) {
        let level = {
            let equipment = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            equipment
                .equipment
                .get(&EquipmentSlot::FEET)
                .map_or(0, |boots| {
                    boots.get_enchantment_level(&Enchantment::FROST_WALKER)
                })
        };
        if level <= 0 || !self.entity.on_ground.load(SeqCst) {
            return;
        }

        let entity = &self.entity;
        let world = entity.world.load();
        let pos = entity.pos.load();
        let center = BlockPos::new(
            pos.x.floor() as i32,
            pos.y.floor() as i32,
            pos.z.floor() as i32,
        );
        let radius = (2 + level).min(16);
        let radius_sq = f64::from(radius * radius);
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                if f64::from(dx * dx + dz * dz) > radius_sq {
                    continue;
                }
                let target = BlockPos::new(center.0.x + dx, center.0.y, center.0.z + dz);
                // 上方必须为空气
                if !world.get_block_state(&target.up()).is_air() {
                    continue;
                }
                let (block, state_id) = world.get_block_and_state_id(&target);
                // 仅冻结水源（level=0 的水方块）
                if block != &Block::WATER || state_id != Block::WATER.default_state.id {
                    continue;
                }
                let new_state_id = Block::FROSTED_ICE.default_state.id;
                // 实体域方块形成事件（冰霜行者），取消则跳过该格
                let Some(entity_arc) = world.get_entity_by_id(entity.entity_id) else {
                    return;
                };
                let mut event =
                    crate::plugin::api::events::block::entity_block_form::EntityBlockFormEvent::new(
                        entity_arc,
                        target,
                        world.clone(),
                        new_state_id,
                    );
                if let Some(server) = world.server.upgrade() {
                    server.plugin_manager.fire_blocking(&server, &mut event);
                    if event.cancelled {
                        continue;
                    }
                }
                world.set_block_state(&target, new_state_id, BlockFlags::NOTIFY_ALL);
                world.schedule_block_tick(
                    &Block::FROSTED_ICE,
                    target,
                    rand::rng().random_range(60..=120),
                    TickPriority::Normal,
                );
            }
        }
    }

    fn push_entities(&self, dyn_self: &dyn EntityBase) {
        let world = self.entity.world.load();
        let entity_bb = self.entity.bounding_box.load();
        let own_team = dyn_self.get_team();

        let pushable: Vec<Arc<dyn EntityBase>> = world
            .get_all_at_box(&entity_bb)
            .into_iter()
            .filter(|entity| {
                let entity_ref = entity.get_entity();
                entity_ref.entity_id != self.entity.entity_id
                    && !entity.is_spectator()
                    && entity.is_pushable()
                    && is_allowed_by_team_rules(own_team.as_ref(), entity.get_team().as_ref())
            })
            .collect();

        if pushable.is_empty() {
            return;
        }

        // 实体挤压检查
        let max_cramming = match world.get_game_rule(&GameRule::MaxEntityCramming) {
            GameRuleValue::Int(value) => value,
            GameRuleValue::Bool(_) => 0,
        };
        if max_cramming > 0
            && pushable.len() as i64 > max_cramming - 1
            && rand::random::<u32>().is_multiple_of(4)
        {
            let count = pushable
                .iter()
                .filter(|entity| !entity.is_passenger())
                .count();
            if count as i64 > max_cramming - 1 {
                dyn_self.damage(dyn_self, 6.0, DamageType::CRAMMING);
            }
        }

        for entity in pushable {
            entity.push(dyn_self);
        }
    }

    /// 像原版 `travelInAir` 的摩擦力一样衰减玩家速度。
    fn apply_travel_friction(&self) {
        let mut velo = self.entity.velocity.load();
        if velo.x == 0.0 && velo.z == 0.0 {
            return;
        }

        let friction = if self.entity.on_ground.load(Relaxed) {
            f64::from(
                self.entity
                    .get_block_with_y_offset(0.500_001)
                    .1
                    .slipperiness,
            ) * 0.91
        } else {
            0.91
        };

        velo.x *= friction;

        velo.z *= friction;

        self.entity.velocity.store(velo);
    }

    fn travel_in_air(&self, caller: &dyn EntityBase) {
        // applyMovementInput

        let effective_speed = self.get_attribute_value(&Attributes::MOVEMENT_SPEED);

        let (speed, friction) = if self.entity.on_ground.load(Relaxed) {
            // getVelocityAffectingPos

            let slipperiness = f64::from(
                self.entity
                    .get_block_with_y_offset(0.500_001)
                    .1
                    .slipperiness,
            );

            let speed =
                effective_speed * 0.216_000_02 / (slipperiness * slipperiness * slipperiness);

            (speed, slipperiness * 0.91)
        } else {
            let speed = caller
                .get_player()
                .map_or(0.02, super::player::Player::get_off_ground_speed);

            (speed, 0.91)
        };

        self.entity
            .update_velocity_from_input(self.movement_input.load(), speed);

        self.apply_climbing_speed();

        self.make_move(caller);

        let mut velo = self.entity.velocity.load();

        let can_powder_snow_climb = if self.entity.was_in_powder_snow.load(Relaxed) {
            crate::block::blocks::powder_snow::can_entity_walk_on_powder_snow(caller)
        } else {
            false
        };

        if (self.entity.horizontal_collision.load(SeqCst) || self.jumping.load(SeqCst))
            && (self.climbing.load(Relaxed) || can_powder_snow_climb)
        {
            velo.y = 0.2;
        }

        let levitation = self.get_effect(&StatusEffect::LEVITATION);

        if let Some(lev) = levitation {
            velo.y += 0.05f64.mul_add(f64::from(lev.amplifier + 1), -velo.y) * 0.2;
        } else {
            velo.y -= self.get_effective_gravity(caller);

            // TODO: 若世界未加载：将有效重力替换为：

            // 若低于世界底部 y 则为 -0.1，否则为 0.0
        }

        // 如果实体没有阻力：存储 velo 并返回

        velo.x *= friction;

        velo.z *= friction;

        velo.y *= caller.get_y_velocity_drag().unwrap_or_else(|| {
            if caller.is_flutterer() {
                friction
            } else {
                0.98
            }
        });

        self.entity.velocity.store(velo);
    }

    fn travel_in_fluid(&self, caller: &dyn EntityBase, water: bool) {
        let movement_input = self.movement_input.load();

        let falling = self.entity.velocity.load().y <= 0.0;
        let gravity = self.get_effective_gravity(caller);
        let effective_speed = self.get_attribute_value(&Attributes::MOVEMENT_SPEED);

        if water {
            let mut friction = if self.entity.sprinting.load(Relaxed) {
                0.9
            } else {
                f64::from(self.water_movement_speed_multiplier)
            };

            let mut speed = 0.02;

            // 应用水中移动效率属性
            let mut water_movement_efficiency =
                self.get_attribute_value(&Attributes::WATER_MOVEMENT_EFFICIENCY);

            if water_movement_efficiency > 0.0 {
                if !self.entity.on_ground.load(SeqCst) {
                    water_movement_efficiency *= 0.5;
                }

                friction += (0.546_000_06 - friction) * water_movement_efficiency;
                speed += (effective_speed - speed) * water_movement_efficiency;
            }

            if self.has_effect(&StatusEffect::DOLPHINS_GRACE) {
                friction = 0.96;
            }

            self.entity
                .update_velocity_from_input(movement_input, speed);

            self.make_move(caller);

            let mut velo = self.entity.velocity.load();
            if self.entity.horizontal_collision.load(SeqCst) && self.climbing.load(Relaxed) {
                velo.y = 0.2;
            }

            velo = velo.multiply(friction, 0.8, friction);

            self.apply_fluid_moving_speed(&mut velo.y, gravity, falling);
            self.entity.velocity.store(velo);
        } else {
            self.entity.update_velocity_from_input(movement_input, 0.02);

            self.make_move(caller);

            let mut velo = self.entity.velocity.load();

            if self.entity.lava_height.load() <= self.get_swim_height() {
                velo.x *= 0.5;
                velo.z *= 0.5;
                velo.y *= 0.8;

                self.apply_fluid_moving_speed(&mut velo.y, gravity, falling);
            } else {
                velo = velo * 0.5;
            }

            if gravity != 0.0 {
                velo.y -= gravity / 4.0; // 负重力 = 浮力
            }

            self.entity.velocity.store(velo);
        }

        let mut velo = self.entity.velocity.load();

        if self.entity.horizontal_collision.load(SeqCst)
            && !self
                .entity
                .world
                .load()
                .check_fluid_collision(self.entity.bounding_box.load().shift(velo))
        {
            velo.y = 0.3;

            self.entity.velocity.store(velo);
        }
    }

    fn apply_fluid_moving_speed(&self, dy: &mut f64, gravity: f64, falling: bool) {
        if gravity != 0.0 && !self.entity.sprinting.load(Relaxed) {
            if falling && (*dy - 0.005).abs() >= 0.003 && (*dy - gravity / 16.0).abs() < 0.003 {
                *dy = -0.003;
            } else {
                *dy -= gravity / 16.0;
            }
        }
    }

    fn make_move(&self, caller: &dyn EntityBase) {
        self.entity.move_entity(caller, self.entity.velocity.load());

        self.check_climbing();
    }

    fn check_climbing(&self) {
        // 如果为旁观模式：返回 false

        // TODO
        // let mut pos = self.entity.block_pos.load();

        // let world = self.entity.world.read().await;

        // let (block, state) = world.get_block_and_state(&pos);

        // let name = block.properties(state.id).map(|props| props.name());

        // if let Some(name) = name {
        //     if name == "LadderLikeProperties"
        //         || name == "ScaffoldingLikeProperties"
        //         || name == "CaveVinesLikeProperties"
        //         || name == "CaveVinesPlantLikeProperties"
        //     {
        //         self.climbing.store(true, Relaxed);

        //         self.climbing_pos.store(Some(pos));

        //         return;
        //     }

        //     if name == "OakTrapdoorLikeProperties" {
        //         let trapdoor = OakTrapdoorLikeProperties::from_state_id(state.id);

        //         pos.0.y -= 1;

        //         let (down_block, down_state) = world.get_block_and_state(&pos);

        //         let is_ladder = down_block
        //             .properties(down_state.id)
        //             .is_some_and(|down_props| down_props.name() == "LadderLikeProperties");

        //         if is_ladder {
        //             let ladder = LadderLikeProperties::from_state_id(down_state.id);

        //             if trapdoor.r#facing == ladder.r#facing {
        //                 self.climbing.store(true, Relaxed);

        //                 self.climbing_pos.store(Some(pos));

        //                 return;
        //             }
        //         }
        //     }
        // }

        self.climbing.store(false, Relaxed);

        if self.entity.on_ground.load(SeqCst) {
            self.climbing_pos.store(None);
        }
    }

    fn apply_climbing_speed(&self) {
        if self.climbing.load(Relaxed) {
            self.fall_distance.store(0.0);

            let mut velo = self.entity.velocity.load();

            let pos = 0.15;

            let neg = -0.15;

            if velo.x < neg {
                velo.x = neg;
            } else if velo.x > pos {
                velo.x = pos;
            }

            if velo.z < neg {
                velo.z = neg;
            } else if velo.z > pos {
                velo.z = pos;
            }

            velo.y = velo.y.max(neg);

            // TODO
            // if velo.y < 0.0
            //     && self.entity.entity_type == &EntityType::PLAYER
            //     && self.entity.sneaking.load(Relaxed)
            // {
            //     let block = self
            //         .entity
            //         .world
            //         .read()
            //         .await
            //         .get_block(&self.entity.block_pos.load())
            //         .await;

            //     if let Some(props) = block.properties(block.default_state.id) {
            //         if props.name() == "ScaffoldingLikeProperties" {
            //             velo.y = 0.0;
            //         }
            //     }
            // }

            self.entity.velocity.store(velo);
        }
    }

    pub fn get_swim_height(&self) -> f64 {
        let eye_height = self.entity.get_eye_height();

        if self.entity.entity_type == &EntityType::BREEZE {
            eye_height
        } else if eye_height < 0.4 {
            0.0
        } else {
            0.4
        }
    }

    fn jump(&self) {
        let jump = self.get_jump_velocity(1.0);

        if jump <= 1.0e-5 {
            return;
        }

        let mut velo = self.entity.velocity.load();

        velo.y = jump.max(velo.y);

        if self.entity.sprinting.load(Relaxed) {
            let yaw = f64::from(self.entity.yaw.load()).to_radians();

            velo.x -= yaw.sin() * 0.2;
            velo.z += yaw.cos() * 0.2;
        }

        self.entity.velocity.store(velo);

        self.entity.velocity_dirty.store(true, SeqCst);

        if let Some(server) = self.entity.world.load().server.upgrade() {
            let mut event = crate::plugin::api::events::entity::entity_jump::EntityJumpEvent::new(
                self.entity.entity_id,
            );
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
    }

    fn get_jump_velocity(&self, mut strength: f64) -> f64 {
        strength *= self.get_attribute_value(&Attributes::JUMP_STRENGTH);
        strength *= f64::from(self.entity.get_jump_velocity_multiplier());
        if let Some(effect) = self.get_effect(&StatusEffect::JUMP_BOOST) {
            strength += 0.1 * f64::from(effect.amplifier + 1);
        }
        strength
    }

    pub fn fall(
        &self,
        caller: &dyn EntityBase,
        height_difference: f64,
        ground: bool,
        dont_damage: bool,
    ) {
        if ground {
            let fall_distance = self.fall_distance.swap(0.0);
            if fall_distance > 0.0 {
                self.on_changed_block(caller, self.entity.block_pos.load());
            }
            if fall_distance <= 0.0
                || dont_damage
                || self.should_prevent_fall_damage()
                || self.should_prevent_fall_damage_in_area()
                || self.is_immune_to_fall_damage()
            {
                return;
            }
            let world = self.entity.world.load();
            let block = world.get_block(&self.entity.get_pos_with_y_offset(0.2).0);
            let pumpkin_block = world.block_registry.get_pumpkin_block(block.id);
            if let Some(pumpkin_block) = pumpkin_block {
                pumpkin_block.on_landed_upon(OnLandedUponArgs {
                    world: &world,
                    fall_distance,
                    entity: caller,
                });
            } else {
                self.handle_fall_damage(caller, fall_distance, 1.0);
            }
        } else if height_difference < 0.0 {
            let new_fall_distance = if !self.should_prevent_fall_damage()
                && !self.should_prevent_fall_damage_in_area()
            {
                let distance = self.fall_distance.load();
                distance - (height_difference as f32)
            } else {
                0f32
            };
            self.fall_distance.store(new_fall_distance);
        }
    }

    pub fn handle_fall_damage(
        &self,
        caller: &dyn EntityBase,
        fall_distance: f32,
        damage_per_distance: f32,
    ) {
        let may_fly = caller.get_player().is_some_and(|player| {
            player
                .abilities
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .allow_flying
        });
        if may_fly || self.is_immune_to_fall_damage() {
            return;
        }

        // 原版对齐：fall_damage 游戏规则只影响玩家。
        if caller.get_player().is_some()
            && !self
                .entity
                .world
                .load()
                .level_info
                .load()
                .game_rules
                .fall_damage
        {
            return;
        }

        if fall_distance >= 2.0
            && let Some(player) = caller.get_player()
        {
            player.increment_stat(
                StatisticCategory::Custom,
                CustomStatistic::FallOneCm as i32,
                (fall_distance * 100.0).round() as i32,
            );
        }

        let safe_fall_distance = self.get_attribute_value(&Attributes::SAFE_FALL_DISTANCE) as f32;
        let unsafe_fall_distance = fall_distance + 1.0E-6 - safe_fall_distance;

        let damage = (unsafe_fall_distance * damage_per_distance).floor();
        if damage > 0.0 {
            let check_damage = self.damage(caller, damage, DamageType::FALL); // 摔落
            if check_damage {
                self.entity
                    .play_sound(Self::get_fall_sound(fall_distance as i32));
            }
        }
    }

    const fn get_fall_sound(distance: i32) -> Sound {
        if distance > 4 {
            Sound::EntityGenericBigFall
        } else {
            Sound::EntityGenericSmallFall
        }
    }

    #[allow(clippy::redundant_closure_for_method_calls)]
    /// 为此实体构建聊天死亡消息：选取
    /// `death.attack.<id>`（存在击杀实体时使用 `.player` 变体，
    /// 已知，或当击杀者离线/被移除时的击杀认定名称）
    /// 翻译键，并填入受害者与击杀者的显示名称。
    pub fn get_death_message(
        dyn_self: &dyn EntityBase,
        damage_type: &ResolvedDamageType,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) -> TextComponent {
        let kill_credit = dyn_self.get_living_entity().and_then(Self::get_kill_credit);
        let kill_credit_name = kill_credit.as_ref().map(|c| c.get_display_name());

        if let Some(living) = dyn_self.get_living_entity() {
            let tracker = living
                .combat_tracker
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            return tracker.get_death_message(dyn_self.get_display_name(), kill_credit_name);
        }

        if let Some(cause) = cause
            && source.is_some()
        {
            TextComponent::translate(
                format!("death.attack.{}", damage_type.message_id()),
                [dyn_self.get_display_name(), cause.get_display_name()],
            )
        } else if let Some(killer) = cause
            .or(source)
            .map(|c| c.get_display_name())
            .or(kill_credit_name)
        {
            TextComponent::translate(
                format!("death.attack.{}.player", damage_type.message_id()),
                [dyn_self.get_display_name(), killer],
            )
        } else {
            TextComponent::translate(
                format!("death.attack.{}", damage_type.message_id()),
                [dyn_self.get_display_name()],
            )
        }
    }

    /// 确保只将实体标记为死亡一次，并运行服务器端的死亡处理逻辑
    /// 流程：停止移动输入、认定击杀归属、掉落战利品、广播
    /// `Death`（3）实体事件，并发放经验值。可安全地在每次致命
    /// 伤害事件；只有第一次调用会生效。
    #[allow(clippy::too_many_lines)]
    pub fn on_death(
        &self,
        damage_type: &ResolvedDamageType,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) {
        let world = self.entity.world.load();
        let Some(dyn_self) = world.get_entity_by_id(self.entity.entity_id) else {
            return;
        };
        if self
            .dead
            .compare_exchange(false, true, Relaxed, Relaxed)
            .is_ok()
        {
            self.movement_input.store(Vector3::default());
            self.jumping.store(false, Relaxed);

            let kill_credit = self.get_kill_credit();
            let killer = cause.or(source).or(kill_credit.as_deref());

            self.update_death_stats(&*dyn_self, killer);

            // 播放死亡音效
            world.play_sound_fine(
                self.death_sound(&*dyn_self),
                SoundCategory::Players,
                &self.entity.pos.load(),
                1.0,
                self.get_pitch(),
            );
            world.send_entity_status(&self.entity, EntityStatus::Death);
            let looting_level;
            let tool = if let Some(cause_ent) = cause {
                if let Some(player) = cause_ent
                    .cast_any()
                    .downcast_ref::<crate::entity::player::Player>()
                {
                    let hand_stack = player
                        .inventory()
                        .get_stack_in_hand(papokin_util::Hand::Right);
                    looting_level = hand_stack
                        .get_enchantment_level(&Enchantment::LOOTING)
                        .max(0) as u32;
                    (!hand_stack.is_empty()).then(|| hand_stack.clone())
                } else {
                    looting_level = 0;
                    None
                }
            } else {
                looting_level = 0;
                None
            };

            let is_raining = world.is_raining();
            let is_thundering = world.is_thundering();

            let has_player_kill =
                killer.is_some_and(|c| c.get_entity().entity_type == &EntityType::PLAYER) || {
                    let tracker = self
                        .combat_tracker
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    tracker.has_player_attacker()
                };

            let params = LootContextParameters {
                killed_by_player: Some(has_player_kill),
                this_entity: Some(self.entity.entity_type),
                killer_entity: killer.map(|c| c.get_entity().entity_type),
                direct_killer_entity: source.map(|s| s.get_entity().entity_type),
                position: Some(self.entity.pos.load()),
                world_time: world.level_info.load().day_time as u64,
                // 战利品条件只认识原版伤害类型；自定义类型
                // 不匹配任何依赖伤害类型的谓词。
                damage_type: damage_type.vanilla(),
                tool,
                is_raining: Some(is_raining),
                is_thundering: Some(is_thundering),
                is_on_fire: Some(
                    self.entity
                        .fire_ticks
                        .load(std::sync::atomic::Ordering::Relaxed)
                        > 0,
                ),
                ..Default::default()
            };

            // 掉落战利品
            self.drop_loot(&params);

            // 给予经验
            if params.killed_by_player.unwrap_or(false)
                && world.level_info.load().game_rules.mob_drops
            {
                let amount = dyn_self.get_experience_reward(killer);
                if amount > 0 {
                    // 幽匿催发：死亡位置 8 格内有催化体时，经验被吸收用于催发幽匿，
                    // 不再生成经验球（原版幽匿催发体语义）
                    let death_pos = BlockPos::new(
                        self.entity.pos.load().x.floor() as i32,
                        self.entity.pos.load().y.floor() as i32,
                        self.entity.pos.load().z.floor() as i32,
                    );
                    let catalyst_absorbed = crate::block::blocks::sculk::sculk_catalyst::SculkCatalystBlock::find_nearby_catalyst(&world, &death_pos)
                        .is_some_and(|catalyst| {
                            crate::block::blocks::sculk::sculk_catalyst::SculkCatalystBlock::absorb_death(&world, catalyst, amount)
                        });
                    if !catalyst_absorbed {
                        ExperienceOrbEntity::spawn(&world, self.entity.pos.load(), amount);
                    }
                }
            }
            self.entity.pose.store(EntityPose::Dying);

            self.drop_equipment(looting_level);

            // 若为玩家且游戏规则已启用，则广播死亡消息
            self.broadcast_death_message(&*dyn_self, damage_type, source, cause);

            // 为已激活的状态效果触发 on_mob_death
            let active_effects_vec: Vec<_> = {
                let effects = self
                    .active_effects
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                effects
                    .values()
                    .map(|e| (e.effect_type, e.amplifier))
                    .collect()
            };
            // 效果钩子接受一个静态伤害类型，但没有当前的
            // 实现会读取它；自定义类型退化为 GENERIC。
            let effect_damage_type = damage_type.vanilla_or(DamageType::GENERIC);
            for (effect_type, amplifier) in active_effects_vec {
                if let Some(mob_effect) = crate::entity::effect::get_mob_effect(effect_type) {
                    mob_effect.on_mob_death(self, amplifier, &effect_damage_type);
                }
            }

            self.reset_effects_and_attributes();
        }
    }

    fn drop_equipment(&self, looting_level: u32) {
        let world = self.entity.world.load();
        let block_pos = self.entity.block_pos.load();

        let drop_chances = self
            .equipment_drop_chances
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let slots_to_drop: Vec<EquipmentSlot> = {
            let mut slots: Vec<_> = self.equipment_slots.values().cloned().collect();
            slots.push(EquipmentSlot::MAIN_HAND);
            slots
        };

        for slot in &slots_to_drop {
            let mut chance = drop_chances
                .get(slot)
                .copied()
                .unwrap_or(DEFAULT_EQUIPMENT_DROP_CHANCE);
            // 高于 1.0 的概率表示必定掉落且完好无损。
            let preserved = chance > 1.0;
            // 原版近似：EnchantmentHelper.processEquipmentDropChance
            // 为每槽位的装备掉落概率加上 lootingLevel * 0.01。
            chance += looting_level as f32 * 0.01;
            chance = chance.min(1.0);
            if !preserved && rand::random::<f32>() >= chance {
                continue;
            }
            let mut item = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .equipment
                .remove(slot)
                .unwrap_or_else(|| ItemStack::EMPTY.clone());
            if item.is_empty() {
                continue;
            }
            // 原版近似：Mob.dropCustomDeathLoot 应用随机
            // 对掉落装备造成损伤，使用两次链式随机调用：
            // setDamageValue(maxDamage - random.nextInt(1 + random.nextInt(max(maxDamage - 3, 1))))
            if !preserved && let Some(max_damage) = item.get_max_damage() {
                let mut rng = rand::rng();
                let inner = rng.random_range(0..(max_damage - 3).max(1));
                let outer = rng.random_range(0..=inner);
                item.set_damage((max_damage - outer).max(0));
            }
            world.drop_stack(&block_pos, item);
        }
    }

    fn broadcast_death_message(
        &self,
        dyn_self: &dyn EntityBase,
        damage_type: &ResolvedDamageType,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) {
        let world = self.entity.world.load();
        let show_death_messages = { world.level_info.load().game_rules.show_death_messages };
        if self.entity.entity_type == &EntityType::PLAYER {
            let death_message = Self::get_death_message(dyn_self, damage_type, source, cause);
            let mut final_death_message = death_message;
            if let Some(player) = dyn_self.get_player() {
                if let Some(player_arc) = world.get_player_by_uuid(player.gameprofile.id)
                    && let Some(server) = world.server.upgrade()
                {
                    let mut event =
                        crate::plugin::api::events::entity::entity_death::PlayerDeathEvent::new(
                            player_arc,
                            final_death_message.clone(),
                            0,
                        );
                    server.plugin_manager.fire_blocking(&server, &mut event);
                    if event.cancelled {
                        return;
                    }
                    final_death_message = event.death_message;
                }

                player.handle_killed(&final_death_message);
            }

            if show_death_messages && let Some(server) = world.server.upgrade() {
                for player in server.get_all_players() {
                    player.send_system_message(&final_death_message);
                }
            }
        } else {
            // 原版对齐：已驯服实体的死亡消息会发给其主人。
            let tameable = dyn_self
                .get_mob()
                .and_then(|mob| mob.as_tamable())
                .filter(|tamable| tamable.is_tame());
            if let Some(tameable) = tameable {
                let death_message = Self::get_death_message(dyn_self, damage_type, source, cause);
                let mut event = crate::plugin::api::events::entity::tameable_death_message::TameableDeathMessageEvent::new(
                    self.entity.entity_id,
                    death_message,
                );
                if let Some(server) = world.server.upgrade() {
                    server.plugin_manager.fire_blocking(&server, &mut event);
                }
                if show_death_messages
                    && let Some(owner_uuid) = tameable.get_owner()
                    && let Some(owner) = world.get_player_by_uuid(owner_uuid)
                {
                    owner.send_system_message(&event.death_message);
                }
            }

            if self.entity.custom_name.load().is_some() {
                let death_message = Self::get_death_message(dyn_self, damage_type, source, cause);
                tracing::info!(
                    "被命名的实体 {} 死亡：{}",
                    dyn_self.get_display_name().to_pretty_console(),
                    death_message.to_pretty_console()
                );
            }
        }
    }

    fn update_death_stats(&self, dyn_self: &dyn EntityBase, cause: Option<&dyn EntityBase>) {
        if let Some(victim_player) = dyn_self.get_player() {
            victim_player.increment_custom_stat(CustomStatistic::Deaths, 1);
            victim_player.set_stat(
                StatisticCategory::Custom,
                CustomStatistic::TimeSinceDeath as i32,
                0,
            );
            victim_player.set_stat(
                StatisticCategory::Custom,
                CustomStatistic::TimeSinceRest as i32,
                0,
            );
            if let Some(killer_entity) = cause.map(EntityBase::get_entity) {
                victim_player.increment_stat(
                    StatisticCategory::KilledBy,
                    killer_entity.entity_type.id as i32,
                    1,
                );
            }
        }

        if let Some(killer_player) = cause.and_then(|c| c.get_player()) {
            killer_player.increment_stat(
                StatisticCategory::Killed,
                self.entity.entity_type.id as i32,
                1,
            );
            if dyn_self.get_player().is_some() {
                killer_player.increment_stat(
                    StatisticCategory::Custom,
                    CustomStatistic::PlayerKills as i32,
                    1,
                );
            } else {
                killer_player.increment_stat(
                    StatisticCategory::Custom,
                    CustomStatistic::MobKills as i32,
                    1,
                );

                let resource_name = self.entity.entity_type.resource_name;
                let criterion_key = format!("minecraft:{resource_name}");
                killer_player.trigger_advancement(
                    crate::entity::player::advancement::trigger::AdvancementTrigger::PlayerKilledEntity {
                        entity_type_resource: criterion_key,
                    },
                );

                if resource_name == "skeleton" {
                    let distance_sq = killer_player
                        .position()
                        .squared_distance_to_vec(&self.entity.pos.load());
                    if distance_sq >= 2500.0 {
                        killer_player.trigger_advancement(crate::entity::player::advancement::trigger::AdvancementTrigger::SniperDuel);
                    }
                }

                if resource_name == "phantom" {
                    killer_player.trigger_advancement(crate::entity::player::advancement::trigger::AdvancementTrigger::TwoBirdsOneArrow);
                }

                let held_item = killer_player.inventory().held_item();
                let is_crossbow = held_item.item.registry_key == "crossbow";
                if is_crossbow {
                    killer_player.trigger_advancement(
                        crate::entity::player::advancement::trigger::AdvancementTrigger::Arbalistic,
                    );
                }
            }
        }
    }

    fn drop_loot(&self, params: &LootContextParameters) {
        let resource_name = self.get_entity().entity_type.resource_name;
        let key = format!("minecraft:entities/{resource_name}");
        if let Some(loot_table) = papokin_data::loot_table::get_loot_table(&key) {
            let seed: i64 = rand::random();
            let pos = self.entity.block_pos.load();
            for stack in crate::world::loot::generate_loot_with_context(loot_table, seed, params) {
                self.entity.world.load().drop_stack(&pos, stack);
            }
        }
    }

    fn tick_effects(&self) {
        let mut effects_to_remove = Vec::new();
        let mut effects_to_apply = Vec::new();

        // 极热路径：仅在插件实际监听时才构建事件
        let effect_tick_server = self.entity.world.load().server.upgrade().filter(|server| {
            server
                .plugin_manager
                .has_handlers::<crate::plugin::api::events::entity::entity_effect_tick::EntityEffectTickEvent>()
        });

        {
            let Ok(mut effects) = self.active_effects.try_lock() else {
                return;
            };
            let entity_age = self.entity.age.load(Relaxed);
            for effect in effects.values_mut() {
                if let Some(server) = &effect_tick_server {
                    let mut event = crate::plugin::api::events::entity::entity_effect_tick::EntityEffectTickEvent::new(
                        self.entity.entity_id,
                        effect.effect_type.minecraft_name.to_string(),
                        i32::from(effect.amplifier),
                        effect.duration,
                    );
                    // 有意忽略取消：这是一个纯粹的
                    // 通知每刻对每个活跃效果只触发一次。
                    server.plugin_manager.fire_blocking(server, &mut event);
                }

                if effect.duration == 0 {
                    effects_to_remove.push(effect.effect_type);
                    continue;
                }

                let tick_duration = if effect.duration == -1 {
                    entity_age
                } else {
                    effect.duration
                };

                if let Some(mob_effect) = crate::entity::effect::get_mob_effect(effect.effect_type)
                    && mob_effect.should_apply_effect_tick(tick_duration, effect.amplifier)
                {
                    effects_to_apply.push((mob_effect, effect.amplifier));
                }

                if effect.duration != -1 {
                    effect.duration -= 1;
                }
            }
        }

        // 为每个已过期的效果调用中央移除函数
        for effect_type in effects_to_remove {
            self.remove_effect(effect_type);
        }

        for (mob_effect, amplifier) in effects_to_apply {
            mob_effect.apply_effect_tick(self, amplifier);
        }
    }

    /// 尝试使用实体手中的不死图腾。成功时会应用图腾效果并返回 true。
    #[allow(dead_code)]
    async fn try_use_death_protector(&self, caller: &dyn EntityBase) -> bool {
        for hand in Hand::all() {
            let mut stack = self.get_stack_in_hand(caller, hand);

            // 清空物品堆并使用不死图腾
            if stack.get_data_component::<DeathProtectionImpl>().is_some() {
                let mut resurrect_event =
                    crate::plugin::api::events::entity::entity_resurrect::EntityResurrectEvent::new(
                        self.entity.entity_id,
                    );
                if let Some(server) = self.entity.world.load().server.upgrade() {
                    server
                        .plugin_manager
                        .fire(&server, &mut resurrect_event)
                        .await;
                }
                if resurrect_event.cancelled {
                    return false;
                }

                stack.clear();
                let slot = match hand {
                    Hand::Right => EquipmentSlot::MAIN_HAND,
                    Hand::Left => EquipmentSlot::OFF_HAND,
                };
                if let Some(player) = caller.get_player() {
                    player
                        .inventory()
                        .entity_equipment
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .equipment
                        .insert(slot, stack);
                } else {
                    self.entity_equipment
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .equipment
                        .insert(slot, stack);
                }
                self.set_health(1.0);
                self.entity
                    .world
                    .load()
                    .send_entity_status(&self.entity, EntityStatus::ProtectedFromDeath);

                // 设置吸收、再生和抗火效果
                self.add_effect(Effect {
                    effect_type: &StatusEffect::ABSORPTION,
                    duration: 100,
                    amplifier: 1,
                    ambient: false,
                    show_particles: true,
                    show_icon: true,
                    blend: false,
                });
                self.add_effect(Effect {
                    effect_type: &StatusEffect::REGENERATION,
                    duration: 900,
                    amplifier: 1,
                    ambient: false,
                    show_particles: true,
                    show_icon: true,
                    blend: false,
                });
                self.add_effect(Effect {
                    effect_type: &StatusEffect::FIRE_RESISTANCE,
                    duration: 800,
                    amplifier: 0,
                    ambient: false,
                    show_particles: true,
                    show_icon: true,
                    blend: false,
                });

                return true;
            }
        }

        false
    }

    #[allow(dead_code)]
    fn damage_armor_items(&self, caller: &dyn EntityBase, damage_amount: f32) {
        // 公式：护甲损失 floor(incoming_damage / 4) 点耐久，最少 1 点。
        let armor_damage = (damage_amount / 4.0).floor().max(1.0) as i32;
        let mut equipment_updates = Vec::new();

        // TODO: 落下的铁砧/钟乳石应只对头盔槽位造成伤害。
        // TODO: 实现 DAMAGE_RESISTANT 组件检查（例如下界合金防火）。

        let armor_slots: Vec<(usize, ItemStack, EquipmentSlot)> = {
            let equipment_lock = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            self.equipment_slots
                .iter()
                .filter(|(_, slot)| slot.is_armor_slot())
                .filter_map(|(index, slot)| {
                    equipment_lock
                        .equipment
                        .get(slot)
                        .cloned()
                        .map(|stack| (*index, stack, slot.clone()))
                })
                .collect()
        };

        for (slot_index, mut stack, slot) in armor_slots {
            if stack.is_empty() {
                continue;
            }

            let takes_damage = stack
                .get_data_component::<EquippableImpl>()
                .is_none_or(|equippable| equippable.damage_on_hurt);

            if takes_damage {
                let item_id = stack.item.id;

                if let Some(server) = self.entity.world.load().server.upgrade() {
                    let mut event = crate::plugin::api::events::entity::entity_damage_item::EntityDamageItemEvent::new(
                        self.entity.entity_id,
                        stack.clone(),
                        armor_damage,
                    );
                    server.plugin_manager.fire_blocking(&server, &mut event);
                    if event.cancelled {
                        continue;
                    }
                }

                let slot_result = stack.damage_item(armor_damage);
                if slot_result != papokin_data::item_stack::DamageResult::Untouched {
                    if slot_result == papokin_data::item_stack::DamageResult::Broken {
                        if let Some(player) = caller.get_player() {
                            player.increment_stat(
                                papokin_data::statistic::StatisticCategory::Broken,
                                item_id as i32,
                                1,
                            );
                        }
                        let world = self.entity.world.load();
                        world
                            .send_entity_status(&self.entity, super::equipment_break_status(&slot));
                    }
                    equipment_updates.push((slot.clone(), stack.clone()));
                    if let Some(player) = caller.get_player() {
                        player.enqueue_slot_set_packet(&CSetPlayerInventory::new(
                            (slot_index as i32).into(),
                            &ItemStackSerializer::from(stack),
                        ));
                    }
                }
            }
        }

        if !equipment_updates.is_empty() {
            self.send_equipment_changes(&equipment_updates);
        }
    }

    pub fn held_item(&self, caller: &dyn EntityBase) -> ItemStack {
        if let Some(player) = caller.get_player() {
            return player.inventory.held_item();
        }
        let equipment = self
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        equipment
            .equipment
            .get(&EquipmentSlot::MAIN_HAND)
            .cloned()
            .unwrap_or_else(|| ItemStack::EMPTY.clone())
    }

    pub fn get_stack_in_hand(&self, caller: &dyn EntityBase, hand: Hand) -> ItemStack {
        match hand {
            Hand::Left => self.off_hand_item(caller),
            Hand::Right => self.held_item(caller),
        }
    }

    /// 对应源码中的 getOffHandStack
    pub fn off_hand_item(&self, caller: &dyn EntityBase) -> ItemStack {
        if let Some(player) = caller.get_player() {
            return player.inventory.off_hand_item();
        }
        let Some(slot) = self.equipment_slots.get(&PlayerInventory::OFF_HAND_SLOT) else {
            return ItemStack::EMPTY.clone();
        };
        let equipment = self
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        equipment
            .equipment
            .get(slot)
            .cloned()
            .unwrap_or_else(|| ItemStack::EMPTY.clone())
    }

    /// 40 刻后遗忘。自定义伤害类型无法通过
    /// 此访问器；这类情况请使用 [`Self::get_last_resolved_damage_type`]。
    pub fn get_last_damage_type(&self) -> Option<DamageType> {
        self.get_last_resolved_damage_type()
            .and_then(|damage_type| damage_type.vanilla())
    }

    /// 最近一次确认命中的伤害类型，原版或自定义。
    /// 40 刻后遗忘。
    pub fn get_last_resolved_damage_type(&self) -> Option<ResolvedDamageType> {
        let stamp = self.last_damage_stamp.load(Ordering::Relaxed);
        let mut last = self
            .last_damage_type
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if self.entity.world.load().get_world_age() - stamp > 40 {
            *last = None;
        }
        last.clone()
    }

    pub fn can_take_damage(&self) -> bool {
        !self.entity.invulnerable.load(Ordering::Relaxed) && self.is_part_of_game()
    }

    pub fn is_part_of_game(&self) -> bool {
        !self.is_spectator() && self.entity.is_alive()
    }

    pub fn can_attack(&self, target: &Self) -> bool {
        if target.entity.entity_type == &EntityType::PLAYER
            && self.entity.world.load().level_info.load().difficulty == Difficulty::Peaceful
        {
            return false;
        }
        target.can_take_damage()
    }

    pub fn reset_state(&self) {
        self.entity.reset_state();

        // 恢复为此实体类型的最大生命值
        let max_health = self.get_max_health();
        self.set_health(max_health);
        // 清除所有吸收值
        self.absorption.store(0.0);
        // 发送生命值元数据
        self.entity
            .set_synced_data(tracked_data::living_entity::DATA_HEALTH_ID, max_health);

        self.reset_effects_and_attributes();

        // 重生后给予短暂的无敌宽限期
        self.hurt_cooldown.store(20, Relaxed);
        self.last_damage_taken.store(0f32);

        self.entity.portal_cooldown.store(0, Relaxed);
        *self
            .entity
            .portal_manager
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;

        // 清除跌落/着火状态
        self.fall_distance.store(0f32);
        self.death_time.store(0, Relaxed);
        self.entity.extinguish();
        self.entity.fire_ticks.store(0, Relaxed);

        // 清除速度与移动输入，以消除残留的动量
        self.entity.velocity.store(Vector3::default());
        self.entity.velocity_dirty.store(true, SeqCst);
        self.movement_input.store(Vector3::default());
        self.jumping.store(false, Relaxed);

        // 如果此 LivingEntity 对应一个 Player，重置其饥饿管理器
        let world = self.entity.world.load();
        if let Some(player) = world.get_player_by_id(self.entity.entity_id) {
            player.hunger_manager.restart();
        }

        self.dead.store(false, Relaxed);
    }

    pub fn is_player(&self) -> bool {
        let world = self.entity.world.load();
        world.get_player_by_id(self.entity.entity_id).is_some()
    }

    pub fn get_movement(&self) -> Vector3<f64> {
        self.entity.movement.load()
    }

    fn death_sound(&self, entity: &dyn EntityBase) -> Sound {
        if let Some(sound_source) = entity.get_mob().and_then(|x| x.as_custom_sound())
            && let Some(audio) = sound_source.death_sound()
        {
            return audio;
        }

        Self::death_sound_for_entity(self.entity.entity_type)
    }

    fn hurt_sound(&self, entity: &dyn EntityBase) -> Sound {
        if let Some(sound_source) = entity.get_mob().and_then(|x| x.as_custom_sound())
            && let Some(audio) = sound_source.hurt_sound()
        {
            return audio;
        }

        Self::hurt_sound_for_entity(self.entity.entity_type)
    }
}

impl LivingEntity {
    pub fn write_living_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put("Health", NbtTag::Float(self.health.load()));
        // 实体死亡时避免保存致命的坠落距离，以防死亡循环
        let fall_distance = if self.dead.load(Relaxed) {
            0.0
        } else {
            self.fall_distance.load()
        };
        // 持久化当前伤害吸收量
        nbt.put("AbsorptionAmount", NbtTag::Float(self.absorption.load()));
        nbt.put("FallDistance", NbtTag::Float(fall_distance));
        nbt.put_short("HurtTime", self.hurt_cooldown.load(Relaxed).max(0) as i16);
        nbt.put_short("DeathTime", i16::from(self.death_time.load(Relaxed)));
        nbt.put_bool("FallFlying", self.entity.is_fall_flying());
        {
            let effects_vec: Vec<papokin_data::potion::Effect> = {
                let effects = self
                    .active_effects
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                effects.values().cloned().collect()
            };
            if !effects_vec.is_empty() {
                // 迭代效果并创建 Box<[NbtTag]>
                let mut effects_list = Vec::with_capacity(effects_vec.len());
                for effect in effects_vec {
                    let mut effect_nbt = papokin_nbt::compound::NbtCompound::new();
                    effect.write_nbt(&mut effect_nbt);
                    effects_list.push(NbtTag::Compound(effect_nbt));
                }
                nbt.put("active_effects", NbtTag::List(effects_list));
            }
        }
        let equipment = {
            let guard = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut compound = NbtCompound::new();
            for (slot, stack) in &guard.equipment {
                if !stack.is_empty() {
                    let mut item_nbt = NbtCompound::new();
                    stack.write_item_stack(&mut item_nbt);
                    compound.put(slot.to_name(), NbtTag::Compound(item_nbt));
                }
            }
            compound
        };
        if !equipment.child_tags.is_empty() {
            nbt.put("equipment", NbtTag::Compound(equipment));
        }
    }

    pub fn read_living_nbt_non_mut(&self, nbt: &NbtCompound) {
        self.health.store(nbt.get_float("Health").unwrap_or(20.0));

        if let Some(equipment) = nbt.get_compound("equipment") {
            let mut guard = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for (name, tag) in &equipment.child_tags {
                if let Some(slot) = EquipmentSlot::get_from_name(name)
                    && let Some(compound) = tag.extract_compound()
                    && let Some(stack) = ItemStack::read_item_stack(compound)
                {
                    guard.put(slot, stack);
                }
            }
        }

        // 将任何已持久化的吸收值钳制到实体配置的最大值
        let raw_abs = nbt.get_float("AbsorptionAmount").unwrap_or(0.0);
        let max_abs = self.get_attribute_value(&Attributes::MAX_ABSORPTION) as f32;
        let clamped_abs = raw_abs.max(0.0).min(max_abs);
        self.absorption.store(clamped_abs);

        // 加载摔落距离，但如果该实体当前被标记为死亡，确保不恢复
        // 一个会在生成时立即再次致死的致命坠落距离。
        let fd = nbt
            .get_float("FallDistance")
            .or_else(|| nbt.get_float("fall_distance"))
            .unwrap_or(0.0);
        if self.dead.load(Relaxed) {
            self.fall_distance.store(0.0);
        } else {
            self.fall_distance.store(fd);
        }
        if let Some(hurt_time) = nbt.get_short("HurtTime") {
            self.hurt_cooldown.store(i32::from(hurt_time), Relaxed);
        }
        if let Some(death_time) = nbt.get_short("DeathTime") {
            self.death_time.store(death_time as u8, Relaxed);
        }
        self.entity
            .fall_flying
            .store(nbt.get_bool("FallFlying").unwrap_or(false), Relaxed);
        {
            let nbt_effects = nbt.get_list("active_effects");
            if let Some(nbt_effects) = nbt_effects {
                let mut read_effects = Vec::new();
                for effect in nbt_effects {
                    if let NbtTag::Compound(effect_nbt) = effect {
                        if let Some(mut effect) = Effect::create_from_nbt(&mut effect_nbt.clone()) {
                            effect.blend = true; // TODO: 待更改，取自 effect give 命令
                            read_effects.push(effect);
                        } else {
                            warn!("无法从 NBT 读取效果");
                        }
                    }
                }
                if !read_effects.is_empty() {
                    let mut active_effects = self
                        .active_effects
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    for effect in read_effects {
                        active_effects.insert(effect.effect_type, effect);
                    }
                }
            }
        }
        // todo 更多...
    }

    /// 计算盔甲减免后的伤害，对应原版 `LivingEntity.getDamageAfterArmorAbsorb`。
    pub fn get_damage_after_armor_absorb(
        &self,
        damage: f32,
        damage_type: &ResolvedDamageType,
        attacker: Option<&dyn EntityBase>,
    ) -> f32 {
        if damage_type.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_ARMOR) {
            return damage;
        }

        let mut armor = 0.0f32;
        let mut toughness = 0.0f32;
        {
            let equipment_lock = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for slot in [
                EquipmentSlot::HEAD,
                EquipmentSlot::CHEST,
                EquipmentSlot::LEGS,
                EquipmentSlot::FEET,
            ] {
                if let Some(stack) = equipment_lock.equipment.get(&slot)
                    && !stack.is_empty()
                    && let Some(modifiers) = stack.get_data_component::<AttributeModifiersImpl>()
                {
                    for modifier in modifiers.attribute_modifiers.iter() {
                        if modifier.r#type == &Attributes::ARMOR {
                            armor += modifier.amount as f32;
                        } else if modifier.r#type == &Attributes::ARMOR_TOUGHNESS {
                            toughness += modifier.amount as f32;
                        }
                    }
                }
            }
        }

        let breach_level = attacker
            .and_then(|att| {
                let player = att.get_player()?;
                let hand_stack = player
                    .inventory()
                    .get_stack_in_hand(papokin_util::Hand::Right);
                let level = hand_stack.get_enchantment_level(&Enchantment::BREACH);
                (level > 0).then_some(level as u32)
            })
            .unwrap_or(0);

        CombatRules::get_damage_after_absorb(damage, armor, toughness, breach_level)
    }

    /// 计算魔法/抗性/附魔减免后的伤害，对应原版 `LivingEntity.getDamageAfterMagicAbsorb`。
    pub fn get_damage_after_magic_absorb(
        &self,
        mut damage: f32,
        damage_type: &ResolvedDamageType,
        caller: &dyn EntityBase,
        cause: Option<&dyn EntityBase>,
    ) -> f32 {
        if damage_type.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_EFFECTS) {
            return damage;
        }

        // 1. 抗性效果（在附魔之前评估）
        if !damage_type.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_RESISTANCE)
            && let Some(effect) = self.get_effect(&StatusEffect::RESISTANCE)
        {
            let absorb_value = (effect.amplifier + 1) * 5;
            let absorb = 25 - absorb_value;
            let v = damage * absorb as f32;
            let old_damage = damage;
            damage = (v / 25.0).max(0.0);
            let damage_resisted = old_damage - damage;
            if damage_resisted > 0.0 {
                if let Some(victim_player) = caller.get_player() {
                    victim_player.increment_stat(
                        StatisticCategory::Custom,
                        CustomStatistic::DamageResisted as i32,
                        (damage_resisted * 10.0).round() as i32,
                    );
                } else if let Some(attacker_player) = cause.and_then(|c| c.get_player()) {
                    attacker_player.increment_stat(
                        StatisticCategory::Custom,
                        CustomStatistic::DamageDealtResisted as i32,
                        (damage_resisted * 10.0).round() as i32,
                    );
                }
            }
        }

        if damage <= 0.0 {
            return 0.0;
        }

        // 2. 附魔保护
        if damage_type.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_ENCHANTMENTS) {
            return damage;
        }

        let is_fire_damage = damage_type.has_tag(&tag::DamageType::MINECRAFT_IS_FIRE);
        let mut epf = 0.0f32;
        {
            let equipment_lock = self
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for slot in [
                EquipmentSlot::HEAD,
                EquipmentSlot::CHEST,
                EquipmentSlot::LEGS,
                EquipmentSlot::FEET,
            ] {
                if let Some(stack) = equipment_lock.equipment.get(&slot)
                    && !stack.is_empty()
                    && let Some(enchantments) = stack.get_data_component::<EnchantmentsImpl>()
                {
                    for (enchantment, level) in enchantments.enchantment.iter() {
                        let enc = *enchantment;
                        let lvl = *level as f32;
                        if enc == &Enchantment::PROTECTION {
                            if !damage_type
                                .has_tag(&tag::DamageType::MINECRAFT_BYPASSES_INVULNERABILITY)
                                && !damage_type.is(DamageType::STARVE)
                                && !damage_type.is(DamageType::GENERIC_KILL)
                                && !damage_type.is(DamageType::OUT_OF_WORLD)
                            {
                                epf += lvl;
                            }
                        } else if enc == &Enchantment::FIRE_PROTECTION {
                            if is_fire_damage {
                                epf += lvl * 2.0;
                            }
                        } else if enc == &Enchantment::BLAST_PROTECTION {
                            if damage_type.has_tag(&tag::DamageType::MINECRAFT_IS_EXPLOSION) {
                                epf += lvl * 2.0;
                            }
                        } else if enc == &Enchantment::PROJECTILE_PROTECTION {
                            if damage_type.has_tag(&tag::DamageType::MINECRAFT_IS_PROJECTILE) {
                                epf += lvl * 2.0;
                            }
                        } else if enc == &Enchantment::FEATHER_FALLING
                            && damage_type.has_tag(&tag::DamageType::MINECRAFT_IS_FALL)
                        {
                            epf += lvl * 3.0;
                        }
                    }
                }
            }
        }

        if epf > 0.0 {
            damage = CombatRules::get_damage_after_magic_absorb(damage, epf);
        }

        damage
    }

    #[allow(clippy::too_many_lines)]
    pub fn damage_with_resolved_context(
        &self,
        caller: &dyn EntityBase,
        amount: f32,
        damage_type: &ResolvedDamageType,
        position: Option<Vector3<f64>>,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) -> bool {
        let mut amount = amount;

        // 在应用伤害前检查无敌状态
        if self.entity.is_invulnerable_to_resolved(damage_type) {
            return false;
        }

        if self.health.load() <= 0.0 || self.dead.load(Relaxed) {
            return false; // 垂死或已死
        }

        if amount < 0.0 {
            return false;
        }

        let mut damage_event =
            crate::plugin::api::events::entity::entity_damage::EntityDamageEvent::new_resolved(
                self.entity.entity_id,
                damage_type,
                amount,
            );
        if let Some(server) = self.entity.world.load().server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut damage_event);
        }
        if damage_event.cancelled {
            return false;
        }
        amount = damage_event.damage;

        if let Some(damager) = source.or(cause) {
            let mut by_entity_event =
                crate::plugin::api::events::entity::entity_damage_by_entity::EntityDamageByEntityEvent {
                    entity_id: self.entity.entity_id,
                    damager_id: damager.get_entity().entity_id,
                    damage: amount,
                    cause: damage_type_debug_name(damage_type),
                    cancelled: false,
                };
            if let Some(server) = self.entity.world.load().server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut by_entity_event);
            }
            if by_entity_event.cancelled {
                return false;
            }
            amount = by_entity_event.damage;
        } else if position.is_some()
            || matches!(
                damage_type,
                ResolvedDamageType::Vanilla(
                    DamageType::CACTUS
                        | DamageType::SWEET_BERRY_BUSH
                        | DamageType::CAMPFIRE
                        | DamageType::HOT_FLOOR
                        | DamageType::STALAGMITE
                )
            )
        {
            let damager_pos = position.map(|p| {
                BlockPos(Vector3::new(
                    p.x.floor() as i32,
                    p.y.floor() as i32,
                    p.z.floor() as i32,
                ))
            });
            let mut by_block_event =
                crate::plugin::api::events::entity::entity_damage_by_block::EntityDamageByBlockEvent {
                    entity_id: self.entity.entity_id,
                    damager_pos,
                    damage: amount,
                    cause: damage_type_debug_name(damage_type),
                    cancelled: false,
                };
            if let Some(server) = self.entity.world.load().server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut by_block_event);
            }
            if by_block_event.cancelled {
                return false;
            }
            amount = by_block_event.damage;
        }

        let world = self.entity.world.load();
        let is_fire_damage = damage_type.has_tag(&tag::DamageType::MINECRAFT_IS_FIRE);

        // 游戏规则或抗火效果均可防止火焰伤害
        if is_fire_damage {
            // 检查火焰伤害的游戏规则（仅对玩家）
            if self.entity.entity_type == &EntityType::PLAYER
                && !world.level_info.load().game_rules.fire_damage
            {
                return false;
            }

            // 检查抗火效果
            if self.has_effect(&StatusEffect::FIRE_RESISTANCE)
                && !damage_type.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_EFFECTS)
            {
                return false;
            }
        }

        // 原版对齐：FREEZE_HURTS_EXTRA_TYPES 中的实体受到 5 倍冰冻伤害。
        if damage_type.is(DamageType::FREEZE)
            && self
                .entity
                .entity_type
                .has_tag(&tag::EntityType::MINECRAFT_FREEZE_HURTS_EXTRA_TYPES)
        {
            amount *= 5.0;
        }

        // 在护甲/魔法/冷却之前检查盾牌格挡
        if self.is_blocking()
            && !damage_type.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_SHIELD)
            && let Some(pos) = position
        {
            let player_pos = self.entity.pos.load();
            let look_vec = Vector3::rotation_vector(0.0, self.entity.yaw.load() as f64);
            let mut source_to_player = (player_pos - pos).normalize();
            source_to_player.y = 0.0;

            if source_to_player.dot(&look_vec) < 0.0 {
                world.play_sound(Sound::ItemShieldBlock, SoundCategory::Players, &player_pos);

                if let Some(player) = caller.get_player() {
                    player.increment_stat(
                        StatisticCategory::Custom,
                        CustomStatistic::DamageBlockedByShield as i32,
                        (amount * 10.0).round() as i32,
                    );
                }

                let active_hand = self
                    .active_hand
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if let Some(hand) = *active_hand {
                    let slot = if hand == Hand::Left {
                        EquipmentSlot::MAIN_HAND
                    } else {
                        EquipmentSlot::OFF_HAND
                    };

                    let durability_damage = (amount / 1.0).floor().max(1.0) as i32;
                    if let Some(player) = caller.get_player() {
                        let held = player.inventory.get_stack_in_hand(match &slot {
                            EquipmentSlot::OffHand(_) => Hand::Left,
                            _ => Hand::Right,
                        });
                        let mut event = crate::plugin::api::events::entity::entity_damage_item::EntityDamageItemEvent::new(
                            self.entity.entity_id,
                            held,
                            durability_damage,
                        );
                        if let Some(server) = world.server.upgrade() {
                            server.plugin_manager.fire_blocking(&server, &mut event);
                        }
                        if !event.cancelled {
                            let broke = player.damage_item_in_slot(&slot, durability_damage);
                            let empty = player
                                .inventory
                                .get_stack_in_hand(match &slot {
                                    EquipmentSlot::OffHand(_) => Hand::Left,
                                    _ => Hand::Right,
                                })
                                .is_empty();
                            if broke && empty {
                                self.clear_active_hand();
                            }
                        }
                    } else {
                        let mut equipment_guard = self
                            .entity_equipment
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        if let Some(stack) = equipment_guard.equipment.get_mut(&slot) {
                            let mut event = crate::plugin::api::events::entity::entity_damage_item::EntityDamageItemEvent::new(
                                self.entity.entity_id,
                                stack.clone(),
                                durability_damage,
                            );
                            if let Some(server) = world.server.upgrade() {
                                server.plugin_manager.fire_blocking(&server, &mut event);
                            }
                            if !event.cancelled
                                && stack.damage_item(durability_damage) == DamageResult::Broken
                            {
                                world.send_entity_status(
                                    &self.entity,
                                    crate::entity::equipment_break_status(&slot),
                                );
                                *stack = ItemStack::EMPTY.clone();
                                let broken_stack = stack.clone();
                                drop(equipment_guard);

                                self.send_equipment_changes(&[(slot, broken_stack)]);
                                self.clear_active_hand();
                            }
                        }
                    }
                }

                return false;
            }
        }

        // 原版对齐：1. 盔甲吸收
        let damage_after_armor =
            self.get_damage_after_armor_absorb(amount, damage_type, cause.or(source));

        let effective_amount = self.get_damage_after_magic_absorb(
            damage_after_armor,
            damage_type,
            caller,
            cause.or(source),
        );

        // 这些伤害类型会绕过受伤冷却和死亡保护
        let bypasses_cooldown_protection =
            damage_type.is(DamageType::GENERIC_KILL) || damage_type.is(DamageType::OUT_OF_WORLD);

        // 应用受伤冷却逻辑
        let last_damage = self.last_damage_taken.load();
        let (damage_amount, play_sound) =
            if self.hurt_cooldown.load(Relaxed) > 10 && !bypasses_cooldown_protection {
                if effective_amount <= last_damage {
                    return false;
                }
                (effective_amount - last_damage, false)
            } else {
                self.hurt_cooldown.store(20, Relaxed);
                (effective_amount, self.health.load() > effective_amount)
            };

        // 完成状态
        self.last_damage_taken.store(amount);
        let damage_amount = damage_amount.max(0.0);

        // 命中确认后记录来源。
        *self
            .last_damage_type
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(damage_type.clone());
        self.last_damage_stamp.store(world.get_world_age(), Relaxed);

        let Some(server) = world.server.upgrade() else {
            return false;
        };
        let config = &server.advanced_config.pvp;

        if config.hurt_animation {
            let entity_id = self.entity.entity_id;
            let hurt_yaw = source.map_or(0.0, |source| {
                let src = source.get_entity().pos.load();
                let tgt = self.entity.pos.load();
                (src.z - tgt.z).atan2(src.x - tgt.x).to_degrees() as f32 - self.entity.yaw.load()
            });
            let hurt_animation = CHurtAnimation::new(entity_id.into(), hurt_yaw);
            world.send_to_tracking_players_and_self(&self.entity, &hurt_animation);
        }

        world.broadcast_damage_event(
            &self.entity,
            i32::from(damage_type.network_id()),
            source.map(|e| e.get_entity().entity_id),
            cause.map(|e| e.get_entity().entity_id),
            position,
        );

        if play_sound {
            world.play_sound_fine(
                self.hurt_sound(caller),
                SoundCategory::Players,
                &self.entity.pos.load(),
                1.0,
                self.get_pitch(),
            );

            if let Some(source) = source {
                let source_pos = source.get_entity().pos.load();
                let target_pos = self.entity.pos.load();
                let dx = source_pos.x - target_pos.x;
                let dz = source_pos.z - target_pos.z;
                let resistance = self.get_attribute_value(&Attributes::KNOCKBACK_RESISTANCE);
                let strength = knockback_after_resistance(0.4, resistance);

                // 镜像 `apply_knockback` 即将进行的速度变化，使
                // 事件便能在其发生前报告它。
                let old_vel = self.entity.velocity.load();
                let push = Vector3::new(dx, 0.0, dz).normalize() * strength;
                let delta_x = old_vel.x / 2.0 - push.x;
                let delta_z = old_vel.z / 2.0 - push.z;

                let mut by_entity_event =
                    crate::plugin::api::events::entity::entity_knockback_by_entity::EntityKnockbackByEntityEvent::new(
                        self.entity.entity_id,
                        source.get_entity().entity_id,
                        strength,
                        delta_x,
                        delta_z,
                    );
                if let Some(server) = self.entity.world.load().server.upgrade() {
                    server
                        .plugin_manager
                        .fire_blocking(&server, &mut by_entity_event);
                }
                if !by_entity_event.cancelled {
                    self.entity.apply_knockback(strength, dx, dz);
                }
            }
        }

        // 原版对齐：actuallyHurt
        let original_damage = damage_amount;
        let current_abs = self.absorption.load();
        let dmg_to_health = (original_damage - current_abs).max(0.0);
        let absorbed_damage = original_damage - dmg_to_health;

        if absorbed_damage > 0.0 {
            let new_abs = (current_abs - absorbed_damage).max(0.0);
            self.set_absorption(new_abs);

            if let Some(player) = caller.get_player() {
                player.increment_stat(
                    StatisticCategory::Custom,
                    CustomStatistic::DamageAbsorbed as i32,
                    (absorbed_damage * 10.0).round() as i32,
                );
            }

            if let Some(attacker_player) = cause.or(source).and_then(|c| c.get_player()) {
                attacker_player.increment_stat(
                    StatisticCategory::Custom,
                    CustomStatistic::DamageDealtAbsorbed as i32,
                    (absorbed_damage * 10.0).round() as i32,
                );
            }

            if let Some(attacker) = cause.or(source) {
                self.last_attacker_id
                    .store(attacker.get_entity().entity_id, Relaxed);
                self.last_attacked_time
                    .store(self.entity.age.load(Relaxed), Relaxed);
            }
        }

        let max_h = self.get_max_health();
        let new_health = (self.health.load() - dmg_to_health).clamp(0.0, max_h);

        if dmg_to_health > 0.0 {
            if let Some(player) = caller.get_player() {
                if damage_type.exhaustion() > 0.0 {
                    player.add_exhaustion(damage_type.exhaustion());
                }
                player.increment_stat(
                    StatisticCategory::Custom,
                    CustomStatistic::DamageTaken as i32,
                    (dmg_to_health * 10.0).round() as i32,
                );
            }

            self.set_health(new_health);

            if let Some(attacker_player) = cause.or(source).and_then(|c| c.get_player()) {
                attacker_player.increment_stat(
                    StatisticCategory::Custom,
                    CustomStatistic::DamageDealt as i32,
                    (dmg_to_health * 10.0).round() as i32,
                );
            }

            if let Some(attacker) = cause.or(source) {
                let attacker_id = attacker.get_entity().entity_id;
                self.last_attacker_id.store(attacker_id, Relaxed);
                self.last_attacked_time
                    .store(self.entity.age.load(Relaxed), Relaxed);

                let current_tick = world.level_info.load().day_time;
                if attacker.get_player().is_some() {
                    self.last_hurt_by_player_id.store(attacker_id, Relaxed);
                    self.last_hurt_by_player_time.store(current_tick, Relaxed);
                } else if attacker.get_living_entity().is_some() {
                    self.last_hurt_by_mob_id.store(attacker_id, Relaxed);
                    self.last_hurt_by_mob_time.store(current_tick, Relaxed);
                }
            }
        }

        if dmg_to_health > 0.0 || absorbed_damage > 0.0 {
            let current_tick = world.level_info.load().day_time;
            let fall_location = FallLocation::get_current_fall_location(self, &world);
            let fall_distance = self.fall_distance.load();

            {
                let mut tracker = self
                    .combat_tracker
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                tracker.record_damage(
                    current_tick,
                    self.health.load() > 0.0 && !self.dead.load(Relaxed),
                    fall_distance,
                    fall_location,
                    damage_type.clone(),
                    effective_amount,
                    source,
                    cause,
                );
            }
        }

        if new_health <= 0.0 {
            let mut death_event =
                crate::plugin::api::events::entity::entity_death::EntityDeathEvent::new(
                    self.entity.entity_id,
                    0,
                );
            if let Some(server) = world.server.upgrade() {
                server
                    .plugin_manager
                    .fire_blocking(&server, &mut death_event);
            }
            self.on_death(damage_type, source, cause);
        }

        true
    }

    /// 使用原版伤害类型造成伤害。这是对……的轻量封装
    /// [`Self::damage_with_resolved_context`]；该已解析入口点还会
    /// 接受插件注册的自定义伤害类型。
    pub fn damage_with_context(
        &self,
        caller: &dyn EntityBase,
        amount: f32,
        damage_type: DamageType,
        position: Option<Vector3<f64>>,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) -> bool {
        self.damage_with_resolved_context(
            caller,
            amount,
            &ResolvedDamageType::Vanilla(damage_type),
            position,
            source,
            cause,
        )
    }

    /// 使用已解析的（原版或插件注册的自定义）伤害类型造成伤害
    /// 伤害类型。
    pub fn damage_resolved(
        &self,
        caller: &dyn EntityBase,
        amount: f32,
        damage_type: &ResolvedDamageType,
    ) -> bool {
        self.damage_with_resolved_context(caller, amount, damage_type, None, None, None)
    }

    pub fn damage(&self, caller: &dyn EntityBase, amount: f32, damage_type: DamageType) -> bool {
        self.damage_with_context(caller, amount, damage_type, None, None, None)
    }
}

impl EntityBase for LivingEntity {
    fn damage_with_context(
        &self,
        caller: &dyn EntityBase,
        amount: f32,
        damage_type: DamageType,
        position: Option<Vector3<f64>>,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) -> bool {
        self.damage_with_context(caller, amount, damage_type, position, source, cause)
    }

    fn damage_with_resolved_context(
        &self,
        caller: &dyn EntityBase,
        amount: f32,
        damage_type: &ResolvedDamageType,
        position: Option<Vector3<f64>>,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) -> bool {
        self.damage_with_resolved_context(caller, amount, damage_type, position, source, cause)
    }

    fn tick_in_void(&self, dyn_self: &dyn EntityBase) {
        dyn_self.damage(dyn_self, 4.0, DamageType::OUT_OF_WORLD);
    }

    fn get_gravity(&self) -> f64 {
        self.get_attribute_value(&Attributes::GRAVITY)
    }

    /// 推进生物实体一刻：基础实体刻、移动以及
    /// 存活期间的物理（在 20 刻死亡动画期间仍然应用，因此
    /// 击退落点）、速度合并、状态效果、虚空伤害以及
    /// 死亡动画完成。
    #[allow(clippy::too_many_lines)]
    fn tick(&self, caller: &dyn EntityBase, server: &Server) {
        self.entity.tick(caller, server);

        // 仅在实体存活时才进行移动刻计算。这可防止死亡的“尸体”
        // 继续被模拟（不断累积 fall_distance/velocity）。
        // 我们允许在死亡动画期间（20 刻）移动，以便应用击退。
        let is_alive = !self.dead.load(Relaxed) && self.health.load() > 0.0;
        let in_death_animation = self.health.load() <= 0.0 && self.death_time.load(Relaxed) < 20;
        let is_player = self.entity.entity_type == &EntityType::PLAYER;
        if (is_alive || in_death_animation) && !is_player {
            self.tick_movement(caller);
            // 类原版顺序：冰冻逻辑在移动/碰撞之后运行。
            self.entity.tick_frozen(caller);
        } else if is_alive {
            // 客户端权威的玩家跳过 `travel`，因此推送速度的衰减如同
            // 原版行为，以防止其累积并将玩家弹飞。
            self.apply_travel_friction();

            let suffocating = self.entity.tick_block_collisions(caller);
            if suffocating {
                caller.damage(caller, 1.0, DamageType::IN_WALL);
            }

            self.tick_frost_walker();

            // 玩家会像任何生物实体一样推挤其他实体。
            self.push_entities(caller);

            self.entity.tick_frozen(caller);
        }

        // 将速度发送合并为每刻一次。
        if self.entity.velocity_dirty.swap(false, Ordering::SeqCst) {
            self.entity.send_velocity();
        }

        // TODO
        let player = caller.get_player();
        let is_player = player.is_some();

        if !is_player {
            self.entity.send_pos_rot();
        }

        // 获取玩家或其他实体的支撑方块
        let supporting_pos = caller.get_player().map_or_else(
            || self.entity.get_supporting_block_pos(),
            super::player::Player::get_supporting_block_pos,
        );

        // 若找到支撑方块位置，则每刻通知实体下方的方块
        if self.entity.is_affected_by_blocks()
            && let Some(supporting) = supporting_pos
        {
            let world = self.entity.world.load_full();
            let (block, state) = world.get_block_and_state(&supporting);

            world
                .block_registry
                .on_entity_step(block, &world, caller, &supporting, state, false);

            // 在 supporting_pos 略下方检查额外的支撑方块（地毯等下方的方块）
            if !block.is_solid() {
                let below_supporting = supporting.down();
                let (below_block, below_state) = world.get_block_and_state(&below_supporting);

                // 如果方块不是空气，同样通知它
                world.block_registry.on_entity_step(
                    below_block,
                    &world,
                    caller,
                    &below_supporting,
                    below_state,
                    true, // 支撑方块下方
                );
            }
        }

        let current_block_pos = self.entity.block_pos.load();
        if is_alive && self.last_block_pos.load() != Some(current_block_pos) {
            self.last_block_pos.store(Some(current_block_pos));
            self.on_changed_block(caller, current_block_pos);
        }

        self.tick_effects();

        if let Some(player) = caller.get_player() {
            let remaining_use_ticks = self.item_use_time.load(Ordering::Relaxed);
            if remaining_use_ticks > 0 {
                let item_in_use = self
                    .item_in_use
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone();
                if let Some(item) = item_in_use.as_ref() {
                    server
                        .item_registry
                        .on_use_tick(item, player, remaining_use_ticks);
                }
            }
        }

        // 当前激活物品
        if self.item_use_time.load(Ordering::Relaxed) > 0
            && self.item_use_time.fetch_sub(1, Ordering::Relaxed) <= 1
        {
            let item_in_use = self
                .item_in_use
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            if let Some(item) = item_in_use.as_ref() {
                // 消耗物品
                let mut is_potion = false;
                if let Some(food) = item.get_data_component::<FoodImpl>()
                    && let Some(player) = caller.get_player()
                {
                    player
                        .hunger_manager
                        .eat(player, food.nutrition as u8, food.saturation);
                    self.entity.world.load().play_sound(
                        Sound::EntityPlayerBurp,
                        SoundCategory::Players,
                        &self.entity.pos.load(),
                    );
                }

                self.apply_consumable_effects(caller, item);

                // 处理药水消耗
                if item
                    .get_data_component::<papokin_data::data_component_impl::PotionContentsImpl>()
                    .is_some()
                {
                    let effects = crate::item::potion::PotionContents::read_potion_effects(item);
                    crate::item::potion::PotionContents::apply_effects_to(
                        self,
                        effects,
                        1.0,
                        crate::item::potion::PotionApplicationSource::Normal,
                    );
                    is_potion = true;
                }

                if let Some(player) = caller.get_player() {
                    player.trigger_advancement(
                        crate::entity::player::advancement::trigger::AdvancementTrigger::ConsumeItem {
                            item_id: format!("minecraft:{}", item.item.registry_key),
                        },
                    );

                    // 优先修改与被消耗物品匹配的确切物品堆：
                    // 1）选中的快捷栏（held_item）
                    // 2）副手
                    // 3）若上述均未匹配，则回退到 active_hand
                    let mut handled = false;

                    // 检查主手（快捷栏选中项）
                    let mut held = player.inventory.held_item();
                    if held.are_items_and_components_equal(item) {
                        if is_potion {
                            if player.gamemode.load() != GameMode::Creative {
                                held.decrement(1);
                                if held.is_empty() {
                                    held = ItemStack::new(1, &Item::GLASS_BOTTLE);
                                }
                            }
                        } else {
                            held.decrement_unless_creative(player.gamemode.load(), 1);
                        }
                        player.inventory.set_held_item(held);
                        handled = true;
                    }

                    if !handled {
                        // 检查副手
                        let mut off_hand = player.inventory.off_hand_item();
                        if off_hand.are_items_and_components_equal(item) {
                            if is_potion {
                                if player.gamemode.load() != GameMode::Creative {
                                    off_hand.decrement(1);
                                    if off_hand.is_empty() {
                                        off_hand = ItemStack::new(1, &Item::GLASS_BOTTLE);
                                    }
                                }
                            } else {
                                off_hand.decrement_unless_creative(player.gamemode.load(), 1);
                            }
                            player.inventory.set_stack_in_hand(Hand::Left, off_hand);
                            handled = true;
                        }
                    }

                    if !handled {
                        // 使用已存储的 active_hand（作为回退）
                        let active_hand = *self
                            .active_hand
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        let hand_to_modify = active_hand.unwrap_or(Hand::Right);
                        let mut item_stack = self.get_stack_in_hand(caller, hand_to_modify);

                        if is_potion {
                            if player.gamemode.load() != GameMode::Creative {
                                item_stack.decrement(1);
                                if item_stack.is_empty() {
                                    item_stack = ItemStack::new(1, &Item::GLASS_BOTTLE);
                                }
                            }
                        } else {
                            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                        }
                        player
                            .inventory
                            .set_stack_in_hand(hand_to_modify, item_stack);
                    }

                    if let Some(cooldown) = item.get_use_cooldown() {
                        let group = cooldown
                            .cooldown_group
                            .clone()
                            .unwrap_or_else(|| item.item.registry_key.to_string());
                        player.start_cooldown(group, (cooldown.seconds * 20.0) as i32);
                    }
                }

                self.clear_active_hand();
            }
        }

        if self.hurt_cooldown.load(Relaxed) > 0 {
            self.hurt_cooldown.fetch_sub(1, Relaxed);
        }
        if self.health.load() <= 0.0 {
            let time = self
                .death_time
                .fetch_update(Relaxed, Relaxed, |time| Some(time.saturating_add(1)))
                .unwrap_or_else(|time| time)
                .saturating_add(1);
            if self.entity.entity_type == &EntityType::PLAYER {
                // 玩家保留在世界中，直到其客户端请求
                // 重生。在此移除一个会破坏死亡状态下的重新连接。
                return;
            }
            // 原版 `LivingEntity.tickDeath` 发送 POOF (60) 粒子事件
            // 死亡动画结束后；死亡事件（3）之前已经
            // 已在 `on_death` 中广播。在此再次发送会重启
            // 客户端侧死亡动画。
            if time >= 20 && !self.entity.removed.swap(true, Ordering::Relaxed) {
                self.entity
                    .world
                    .load()
                    .send_entity_status(&self.entity, EntityStatus::Poof);
                self.entity.remove();
            }
        }
    }

    fn get_entity(&self) -> &Entity {
        &self.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        Some(self)
    }

    fn is_pushable(&self) -> bool {
        self.health.load() > 0.0 && !self.dead.load(Relaxed)
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub const SPEED_MODIFIER_SPRINTING_ID: &str = "minecraft:sprinting";
pub const SPEED_MODIFIER_SPRINTING_AMOUNT: f64 = 0.300_000_011_920_928_96;

impl LivingEntity {
    pub fn set_sprinting(&self, is_sprinting: bool) {
        self.entity.set_sprinting(is_sprinting);
        self.update_attribute(&Attributes::MOVEMENT_SPEED, |speed| {
            speed.remove_modifier(SPEED_MODIFIER_SPRINTING_ID);
            if is_sprinting {
                speed.add_or_replace_modifier(Modifier {
                    id: SPEED_MODIFIER_SPRINTING_ID.to_string(),
                    amount: SPEED_MODIFIER_SPRINTING_AMOUNT,
                    operation: ModifierOperation::MultiplyTotal,
                });
            }
        });
        crate::entity::attributes::send_attribute_updates_for_living(
            self,
            vec![Attributes::MOVEMENT_SPEED],
        );
    }

    #[must_use]
    pub fn get_block_speed_factor(&self) -> f32 {
        let efficiency = self.get_attribute_value(&Attributes::MOVEMENT_EFFICIENCY) as f32;
        let super_factor = self.entity.get_block_speed_factor();
        super_factor + efficiency * (1.0 - super_factor)
    }

    /// 在物品完成使用后，应用数据驱动的 `apply_effects` 消耗效果。
    /// 原版：`Consumable.onConsume` 在服务器端调用每个已配置的效果。
    fn apply_consumable_effects(&self, caller: &dyn EntityBase, item: &ItemStack) {
        let Some(consumable) = item.get_data_component::<ConsumableImpl>() else {
            return;
        };

        for consume_effect in consumable.effects.iter() {
            match consume_effect {
                ConsumeEffect::ApplyEffects((effects, probability)) => {
                    if !consume_effect_probability_applies(*probability, rand::random()) {
                        continue;
                    }

                    for effect in effects.iter() {
                        let Some(effect_type) =
                            StatusEffect::from_minecraft_name(&effect.effect_id)
                        else {
                            continue;
                        };
                        let Ok(amplifier) = u8::try_from(effect.amplifier) else {
                            continue;
                        };

                        self.add_effect(Effect {
                            effect_type,
                            duration: effect.duration,
                            amplifier,
                            ambient: effect.ambient,
                            show_particles: effect.show_particles,
                            show_icon: effect.show_icon,
                            blend: false,
                        });
                    }
                }
                ConsumeEffect::ClearAllEffects => {
                    self.reset_effects_and_attributes();
                }
                ConsumeEffect::RemoveEffects(idset) => {
                    if let papokin_data::data_component_impl::IDSet::IDs(ids) = idset {
                        for effect_type in ids.iter() {
                            self.remove_effect(effect_type);
                        }
                    }
                }
                ConsumeEffect::TeleportRandomly(diameter) => {
                    // Java 版在随机传送尝试前让乘坐者下车。
                    let vehicle = caller
                        .get_entity()
                        .vehicle
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .clone();
                    if let Some(vehicle) = vehicle {
                        vehicle
                            .get_entity()
                            .remove_passenger_sync(caller.get_entity().entity_id);
                        if caller.get_entity().has_vehicle() {
                            continue;
                        }
                    }

                    let center = self.entity.pos.load();
                    let Some(pos) = self.find_random_teleport_target(*diameter) else {
                        continue;
                    };
                    let (yaw, pitch) = (self.entity.yaw.load(), self.entity.pitch.load());
                    let world = self.entity.world.load_full();
                    caller.teleport(pos, Some(yaw), Some(pitch), world.clone());

                    let destination = self.entity.pos.load();
                    if destination != center {
                        self.fall_distance.store(0.0);
                        // 原版在成功时广播实体事件 46（传送粒子）。
                        world.send_entity_status(&self.entity, EntityStatus::Teleport);
                        world.emit_game_event("teleport", center);
                        world.play_sound(
                            Sound::ItemChorusFruitTeleport,
                            SoundCategory::Players,
                            &destination,
                        );
                    }
                }
                ConsumeEffect::PlaySound(_) => {}
            }
        }
    }

    fn find_random_teleport_target(&self, diameter: f32) -> Option<Vector3<f64>> {
        let center = self.entity.pos.load();
        let world = self.entity.world.load();
        let bottom_y = world.get_bottom_y();
        let top_y = world.get_top_y();
        let dimensions = self.entity.entity_dimension.load();
        let mut rng = rand::rng();

        'attempts: for _ in 0..Self::RANDOM_TELEPORT_ATTEMPTS {
            let target_x = random_teleport_coordinate(center.x, diameter, rng.random());
            let target_z = random_teleport_coordinate(center.z, diameter, rng.random());
            let sampled_y = random_teleport_coordinate(center.y, diameter, rng.random())
                .clamp(f64::from(bottom_y + 1), f64::from(top_y));
            let mut block_y = sampled_y.floor() as i32;
            let block_x = target_x.floor() as i32;
            let block_z = target_z.floor() as i32;

            loop {
                if block_y <= bottom_y {
                    continue 'attempts;
                }

                let below = BlockPos::new(block_x, block_y - 1, block_z);
                let Some(below_state) = world.get_block_state_if_loaded(&below) else {
                    continue 'attempts;
                };
                if below_state.is_solid() {
                    break;
                }
                block_y -= 1;
            }

            let target = Vector3::new(target_x, f64::from(block_y), target_z);
            let bounding_box = BoundingBox::new_from_pos(target.x, target.y, target.z, &dimensions);

            for block_pos in
                BlockPos::iterate(bounding_box.min_block_pos(), bounding_box.max_block_pos())
            {
                if world.get_block_state_if_loaded(&block_pos).is_none()
                    || world.get_fluid(&block_pos).id != Fluid::EMPTY.id
                {
                    continue 'attempts;
                }
            }

            if world.is_space_empty(bounding_box) {
                return Some(target);
            }
        }

        None
    }
}

/// 用于插件事件 "cause" 字符串的伤害类型调试名称：
/// 原版类型的静态结构调试输出（与引入自定义伤害
/// 类型存在），自定义条目则使用命名空间 ID。
fn damage_type_debug_name(damage_type: &ResolvedDamageType) -> String {
    match damage_type {
        ResolvedDamageType::Vanilla(vanilla) => format!("{vanilla:?}"),
        ResolvedDamageType::Custom(custom) => format!("Custom({})", custom.name),
    }
}

fn random_teleport_coordinate(center: f64, diameter: f32, random: f64) -> f64 {
    center + (random - 0.5) * f64::from(diameter)
}

fn attributes_by_id(id: u8) -> Option<&'static Attributes> {
    Attributes::ALL.iter().find(|attr| attr.id == id)
}

fn push_unique_attribute(touched: &mut Vec<Attributes>, attr: &Attributes) {
    if !touched.iter().any(|existing| existing.id == attr.id) {
        touched.push(attr.clone());
    }
}

const fn attribute_modifier_slot_matches(
    modifier_slot: &AttributeModifierSlot,
    equipment_slot: &EquipmentSlot,
) -> bool {
    match modifier_slot {
        AttributeModifierSlot::Any => true,
        AttributeModifierSlot::MainHand => matches!(equipment_slot, EquipmentSlot::MainHand(_)),
        AttributeModifierSlot::OffHand => matches!(equipment_slot, EquipmentSlot::OffHand(_)),
        AttributeModifierSlot::Hand => {
            matches!(
                equipment_slot,
                EquipmentSlot::MainHand(_) | EquipmentSlot::OffHand(_)
            )
        }
        AttributeModifierSlot::Feet => matches!(equipment_slot, EquipmentSlot::Feet(_)),
        AttributeModifierSlot::Legs => matches!(equipment_slot, EquipmentSlot::Legs(_)),
        AttributeModifierSlot::Chest => matches!(equipment_slot, EquipmentSlot::Chest(_)),
        AttributeModifierSlot::Head => matches!(equipment_slot, EquipmentSlot::Head(_)),
        AttributeModifierSlot::Armor => matches!(
            equipment_slot,
            EquipmentSlot::Feet(_)
                | EquipmentSlot::Legs(_)
                | EquipmentSlot::Chest(_)
                | EquipmentSlot::Head(_)
        ),
        AttributeModifierSlot::Body => matches!(equipment_slot, EquipmentSlot::Body(_)),
        AttributeModifierSlot::Saddle => matches!(equipment_slot, EquipmentSlot::Saddle(_)),
    }
}

/// 对应原版严格的 `random < probability` 消耗效果门槛。
const fn consume_effect_probability_applies(probability: f32, random: f32) -> bool {
    random < probability
}

#[cfg(test)]
mod consumable_effect_tests {
    use super::{consume_effect_probability_applies, random_teleport_coordinate};

    #[test]
    fn consumable_effect_probability_matches_vanilla_strict_threshold() {
        assert!(!consume_effect_probability_applies(0.0, 0.0));
        assert!(consume_effect_probability_applies(1.0, 0.999));
        assert!(consume_effect_probability_applies(0.5, 0.499));
        assert!(!consume_effect_probability_applies(0.5, 0.5));
    }

    #[test]
    fn random_teleport_coordinate_uses_full_diameter() {
        assert_eq!(random_teleport_coordinate(10.0, 16.0, 0.0), 2.0);
        assert_eq!(random_teleport_coordinate(10.0, 16.0, 0.5), 10.0);
        assert_eq!(random_teleport_coordinate(10.0, 16.0, 1.0), 18.0);
    }
}
///若 `damage_type` 属于 `#minecraft:bypasses_armor`（1.21.11），则返回 `true`。
/// 这些伤害来源完全无视护甲（摔落、溺水、冰冻等）。
pub(crate) const fn bypasses_armor_durability(damage_type: &DamageType) -> bool {
    // 位掩码查找：两条指令（移位 + AND）即可 O(1)，无需扫描数组。
    // DamageType ID 可能超过 31；使用 u64 以获得足够范围。
    // TODO: 待数据包系统能够在不出现性能退化的前提下处理后再改为数据驱动。
    // 编译期断言：确保所有 bypassing 类型都能放入 u64 位掩码。
    const _: () = assert!(
        DamageType::FALL.id < 64
            && DamageType::FLY_INTO_WALL.id < 64
            && DamageType::ON_FIRE.id < 64
            && DamageType::IN_WALL.id < 64
            && DamageType::CRAMMING.id < 64
            && DamageType::DROWN.id < 64
            && DamageType::GENERIC.id < 64
            && DamageType::WITHER.id < 64
            && DamageType::DRAGON_BREATH.id < 64
            && DamageType::STARVE.id < 64
            && DamageType::ENDER_PEARL.id < 64
            && DamageType::FREEZE.id < 64
            && DamageType::STALAGMITE.id < 64
            && DamageType::MAGIC.id < 64
            && DamageType::INDIRECT_MAGIC.id < 64
            && DamageType::OUT_OF_WORLD.id < 64
            && DamageType::GENERIC_KILL.id < 64
            && DamageType::SONIC_BOOM.id < 64
            && DamageType::OUTSIDE_BORDER.id < 64,
        "One or more bypass DamageType IDs exceed u64 bitmask width (>= 64)"
    );
    const BYPASS_MASK: u64 = (1u64 << DamageType::FALL.id)
        | (1u64 << DamageType::FLY_INTO_WALL.id)
        | (1u64 << DamageType::ON_FIRE.id)
        | (1u64 << DamageType::IN_WALL.id)
        | (1u64 << DamageType::CRAMMING.id)
        | (1u64 << DamageType::DROWN.id)
        | (1u64 << DamageType::GENERIC.id)
        | (1u64 << DamageType::WITHER.id)
        | (1u64 << DamageType::DRAGON_BREATH.id)
        | (1u64 << DamageType::STARVE.id)
        | (1u64 << DamageType::ENDER_PEARL.id)
        | (1u64 << DamageType::FREEZE.id)
        | (1u64 << DamageType::STALAGMITE.id)
        | (1u64 << DamageType::MAGIC.id)
        | (1u64 << DamageType::INDIRECT_MAGIC.id)
        | (1u64 << DamageType::OUT_OF_WORLD.id)
        | (1u64 << DamageType::GENERIC_KILL.id)
        | (1u64 << DamageType::SONIC_BOOM.id)
        | (1u64 << DamageType::OUTSIDE_BORDER.id);
    (damage_type.id < 64) && ((BYPASS_MASK >> damage_type.id) & 1 == 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── bypasses_armor_durability ─────────────────────────────────────

    /// `minecraft:bypasses_armor`（1.21.11）的每个成员都必须返回 `true`。
    #[test]
    fn bypasses_armor_durability_returns_true_for_tag_members() {
        // 1.21.11 中 minecraft:bypasses_armor 标签的完整内容
        let bypassing: &[DamageType] = &[
            DamageType::ON_FIRE,
            DamageType::IN_WALL,
            DamageType::CRAMMING,
            DamageType::DROWN,
            DamageType::FLY_INTO_WALL,
            DamageType::GENERIC,
            DamageType::WITHER,
            DamageType::DRAGON_BREATH,
            DamageType::STARVE,
            DamageType::FALL,
            DamageType::ENDER_PEARL,
            DamageType::FREEZE,
            DamageType::STALAGMITE,
            DamageType::MAGIC,
            DamageType::INDIRECT_MAGIC,
            DamageType::OUT_OF_WORLD,
            DamageType::GENERIC_KILL,
            DamageType::SONIC_BOOM,
            DamageType::OUTSIDE_BORDER,
        ];
        for dt in bypassing {
            assert!(
                bypasses_armor_durability(dt),
                "{} should bypass armor durability",
                dt.message_id
            );
        }
    }

    /// 物理/战斗伤害类型绝不能绕过盔甲耐久。
    #[test]
    fn bypasses_armor_durability_returns_false_for_physical_sources() {
        let physical: &[DamageType] = &[
            DamageType::MOB_ATTACK,
            DamageType::PLAYER_ATTACK,
            DamageType::ARROW,
            DamageType::CACTUS,
            DamageType::SWEET_BERRY_BUSH,
            DamageType::LAVA,
            DamageType::EXPLOSION,
            DamageType::PLAYER_EXPLOSION,
            DamageType::LIGHTNING_BOLT,
            DamageType::FIREBALL,
            DamageType::THORNS,
            DamageType::TRIDENT,
        ];
        for dt in physical {
            assert!(
                !bypasses_armor_durability(dt),
                "{} should NOT bypass armor durability",
                dt.message_id
            );
        }
    }

    #[test]
    fn hurt_sound_for_entity_uses_zombie_family_sounds() {
        let cases = [
            (&EntityType::ZOMBIE, Sound::EntityZombieHurt),
            (&EntityType::DROWNED, Sound::EntityDrownedHurt),
            (&EntityType::HUSK, Sound::EntityHuskHurt),
            (
                &EntityType::ZOMBIE_VILLAGER,
                Sound::EntityZombieVillagerHurt,
            ),
        ];

        for (entity_type, expected) in cases {
            assert_eq!(LivingEntity::hurt_sound_for_entity(entity_type), expected);
        }
    }

    #[test]
    fn hurt_sound_for_entity_uses_enderman_hurt_sound() {
        assert_eq!(
            LivingEntity::hurt_sound_for_entity(&EntityType::ENDERMAN),
            Sound::EntityEndermanHurt
        );
    }

    #[test]
    fn hurt_sound_for_entity_uses_skeleton_family_sounds() {
        let cases = [
            (&EntityType::SKELETON, Sound::EntitySkeletonHurt),
            (&EntityType::BOGGED, Sound::EntityBoggedHurt),
            (&EntityType::PARCHED, Sound::EntityParchedHurt),
            (
                &EntityType::WITHER_SKELETON,
                Sound::EntityWitherSkeletonHurt,
            ),
            (&EntityType::STRAY, Sound::EntityStrayHurt),
        ];

        for (entity_type, expected) in cases {
            assert_eq!(LivingEntity::hurt_sound_for_entity(entity_type), expected);
        }
    }

    #[test]
    fn hurt_sound_for_entity_defaults_to_generic_hurt() {
        assert_eq!(
            LivingEntity::hurt_sound_for_entity(&EntityType::ITEM),
            Sound::EntityGenericHurt
        );
    }

    #[test]
    fn regeneration_particle_metadata_uses_vanilla_argb_color() {
        let effect = Effect {
            effect_type: &StatusEffect::REGENERATION,
            duration: 200,
            amplifier: 0,
            ambient: false,
            show_particles: true,
            show_icon: true,
            blend: false,
        };
        let metadata = papokin_protocol::java::client::play::Metadata::new(
            tracked_data::living_entity::EFFECT_PARTICLES,
            EffectParticles(vec![EffectParticle::from_effect(&effect)]),
        );
        let mut bytes = Vec::new();

        metadata
            .write(
                &mut bytes,
                &papokin_util::version::JavaMinecraftVersion::V_26_3,
            )
            .unwrap();

        assert_eq!(bytes, [10, 17, 1, 28, 0xff, 0xcd, 0x5c, 0xab]);
    }
}
