use std::sync::Arc;
use wasmtime::component::{Access, HasSelf, Resource};

use crate::plugin::loader::wasm::wasm_host::{
    state::{LivingEntityResource, PluginHostState},
    wit::v0_1::papokin::plugin::{
        attributes::{
            Attribute, AttributeModifier as WitAttributeModifier,
            ModifierOperation as WitModifierOperation,
        },
        combat::CombatEntry as WitCombatEntry,
        damage_types::DamageType as WitDamageType,
        item_stack::ItemStack as WitHostItemStack,
        text::TextComponent,
        world::{
            Entity, EquipmentSlot as WitEquipmentSlot, HostLivingEntity,
            LivingEntity as WitLivingEntity, Mob as WitMob,
        },
    },
};

pub fn living_entity_from_resource(
    state: &PluginHostState,
    entity: &Resource<WitLivingEntity>,
) -> wasmtime::Result<std::sync::Arc<dyn crate::entity::EntityBase>> {
    state
        .resource_table
        .get::<LivingEntityResource>(&Resource::new_own(entity.rep()))
        .map_err(|_| wasmtime::Error::msg("无效的生物实体资源句柄"))
        .map(|resource| resource.provider.clone())
}

fn active_plugin(
    state: &PluginHostState,
) -> wasmtime::Result<Arc<crate::plugin::loader::wasm::wasm_host::WasmPlugin>> {
    state
        .plugin
        .as_ref()
        .and_then(std::sync::Weak::upgrade)
        .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))
}

#[must_use]
pub const fn from_wit_attribute(attr: Attribute) -> &'static papokin_data::attributes::Attributes {
    use papokin_data::attributes::Attributes;
    match attr {
        Attribute::AirDragModifier => &Attributes::AIR_DRAG_MODIFIER,
        Attribute::Armor => &Attributes::ARMOR,
        Attribute::ArmorToughness => &Attributes::ARMOR_TOUGHNESS,
        Attribute::AttackDamage => &Attributes::ATTACK_DAMAGE,
        Attribute::AttackKnockback => &Attributes::ATTACK_KNOCKBACK,
        Attribute::AttackSpeed => &Attributes::ATTACK_SPEED,
        Attribute::BelowNameDistance => &Attributes::BELOW_NAME_DISTANCE,
        Attribute::BlockBreakSpeed => &Attributes::BLOCK_BREAK_SPEED,
        Attribute::BlockInteractionRange => &Attributes::BLOCK_INTERACTION_RANGE,
        Attribute::Bounciness => &Attributes::BOUNCINESS,
        Attribute::BurningTime => &Attributes::BURNING_TIME,
        Attribute::CameraDistance => &Attributes::CAMERA_DISTANCE,
        Attribute::ExplosionKnockbackResistance => &Attributes::EXPLOSION_KNOCKBACK_RESISTANCE,
        Attribute::EntityInteractionRange => &Attributes::ENTITY_INTERACTION_RANGE,
        Attribute::FallDamageMultiplier => &Attributes::FALL_DAMAGE_MULTIPLIER,
        Attribute::FlyingSpeed => &Attributes::FLYING_SPEED,
        Attribute::FollowRange => &Attributes::FOLLOW_RANGE,
        Attribute::FrictionModifier => &Attributes::FRICTION_MODIFIER,
        Attribute::Gravity => &Attributes::GRAVITY,
        Attribute::JumpStrength => &Attributes::JUMP_STRENGTH,
        Attribute::KnockbackResistance => &Attributes::KNOCKBACK_RESISTANCE,
        Attribute::Luck => &Attributes::LUCK,
        Attribute::MaxAbsorption => &Attributes::MAX_ABSORPTION,
        Attribute::MaxHealth => &Attributes::MAX_HEALTH,
        Attribute::MiningEfficiency => &Attributes::MINING_EFFICIENCY,
        Attribute::MovementEfficiency => &Attributes::MOVEMENT_EFFICIENCY,
        Attribute::MovementSpeed => &Attributes::MOVEMENT_SPEED,
        Attribute::NameTagDistance => &Attributes::NAME_TAG_DISTANCE,
        Attribute::OxygenBonus => &Attributes::OXYGEN_BONUS,
        Attribute::SafeFallDistance => &Attributes::SAFE_FALL_DISTANCE,
        Attribute::Scale => &Attributes::SCALE,
        Attribute::SneakingSpeed => &Attributes::SNEAKING_SPEED,
        Attribute::SpawnReinforcements => &Attributes::SPAWN_REINFORCEMENTS,
        Attribute::StepHeight => &Attributes::STEP_HEIGHT,
        Attribute::SubmergedMiningSpeed => &Attributes::SUBMERGED_MINING_SPEED,
        Attribute::SweepingDamageRatio => &Attributes::SWEEPING_DAMAGE_RATIO,
        Attribute::TemptRange => &Attributes::TEMPT_RANGE,
        Attribute::WaterMovementEfficiency => &Attributes::WATER_MOVEMENT_EFFICIENCY,
        Attribute::WaypointTransmitRange => &Attributes::WAYPOINT_TRANSMIT_RANGE,
        Attribute::WaypointReceiveRange => &Attributes::WAYPOINT_RECEIVE_RANGE,
    }
}

#[must_use]
pub const fn from_wit_modifier_op(
    op: WitModifierOperation,
) -> crate::entity::attributes::ModifierOperation {
    match op {
        WitModifierOperation::Add => crate::entity::attributes::ModifierOperation::Add,
        WitModifierOperation::MultiplyBase => {
            crate::entity::attributes::ModifierOperation::MultiplyBase
        }
        WitModifierOperation::MultiplyTotal => {
            crate::entity::attributes::ModifierOperation::MultiplyTotal
        }
    }
}

#[must_use]
pub const fn to_wit_modifier_op(
    op: crate::entity::attributes::ModifierOperation,
) -> WitModifierOperation {
    match op {
        crate::entity::attributes::ModifierOperation::Add => WitModifierOperation::Add,
        crate::entity::attributes::ModifierOperation::MultiplyBase => {
            WitModifierOperation::MultiplyBase
        }
        crate::entity::attributes::ModifierOperation::MultiplyTotal => {
            WitModifierOperation::MultiplyTotal
        }
    }
}

#[must_use]
pub const fn from_wit_equipment_slot(
    slot: WitEquipmentSlot,
) -> papokin_data::data_component_impl::EquipmentSlot {
    use papokin_data::data_component_impl::EquipmentSlot;
    match slot {
        WitEquipmentSlot::MainHand => EquipmentSlot::MAIN_HAND,
        WitEquipmentSlot::OffHand => EquipmentSlot::OFF_HAND,
        WitEquipmentSlot::Feet => EquipmentSlot::FEET,
        WitEquipmentSlot::Legs => EquipmentSlot::LEGS,
        WitEquipmentSlot::Chest => EquipmentSlot::CHEST,
        WitEquipmentSlot::Head => EquipmentSlot::HEAD,
        WitEquipmentSlot::Body => EquipmentSlot::BODY,
        WitEquipmentSlot::Saddle => EquipmentSlot::SADDLE,
    }
}

#[must_use]
pub const fn to_wit_damage_type(damage_type: &papokin_data::damage::DamageType) -> WitDamageType {
    // SAFETY: WIT 枚举按与内部枚举 / ID 相同的顺序生成
    unsafe { std::mem::transmute(damage_type.id) }
}

#[must_use]
pub fn from_wit_damage_type(wit: WitDamageType) -> papokin_data::damage::DamageType {
    papokin_data::damage::DamageType::from_id(wit as u8)
        .unwrap_or(papokin_data::damage::DamageType::GENERIC)
}

/// 为伤害执行路径解析插件提供的伤害类型名称。
///
/// 原版名称（带或不带 "minecraft:" 前缀）会映射到静态
/// 表；其他都必须是插件注册的自定义伤害类型。
pub fn resolve_damage_type_by_name(
    state: &PluginHostState,
    name: &str,
) -> wasmtime::Result<papokin_data::damage_ext::ResolvedDamageType> {
    let server = state
        .server
        .as_ref()
        .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?;
    server
        .damage_type_manager
        .resolve(name)
        .ok_or_else(|| wasmtime::Error::msg(format!("未知的伤害类型 '{name}'")))
}

/// 每个服务器刻的毫秒数（20 TPS），用于暴露基于刻的指标
/// 战斗追踪器时钟，以毫秒形式跨过插件边界。
const MS_PER_TICK: i64 = 50;

/// 已解析伤害类型的显示名称：带命名空间的注册
/// 自定义类型使用其名称，否则使用原版消息 id。
fn damage_type_name(damage_type: &papokin_data::damage_ext::ResolvedDamageType) -> String {
    damage_type
        .custom_name()
        .map_or_else(|| damage_type.message_id().to_string(), str::to_string)
}

fn to_wit_combat_entry(entry: &crate::entity::combat::CombatEntry) -> WitCombatEntry {
    WitCombatEntry {
        damage_type: damage_type_name(&entry.damage_type),
        damage: entry.damage,
        fall_distance: entry.fall_distance,
        source_id: entry.source_id,
        attacker_id: entry.attacker_id,
        attacker_name: entry
            .attacker_name
            .clone()
            .map(papokin_util::text::TextComponent::to_pretty_console),
        attacker_item_name: entry
            .attacker_item_name
            .clone()
            .map(papokin_util::text::TextComponent::to_pretty_console),
        attacker_is_player: entry.attacker_is_player,
        timestamp_ms: entry.timestamp.saturating_mul(MS_PER_TICK),
    }
}

impl HostLivingEntity for PluginHostState {
    async fn as_entity(
        &mut self,
        this: Resource<WitLivingEntity>,
    ) -> wasmtime::Result<Resource<Entity>> {
        let entity = living_entity_from_resource(self, &this)?;
        self.add_entity(entity)
    }

    async fn as_mob(
        &mut self,
        this: Resource<WitLivingEntity>,
    ) -> wasmtime::Result<Option<Resource<WitMob>>> {
        let entity = living_entity_from_resource(self, &this)?;
        if entity.get_mob().is_some() {
            Ok(Some(self.add_mob(entity)?))
        } else {
            Ok(None)
        }
    }

    async fn is_mob(&mut self, this: Resource<WitLivingEntity>) -> wasmtime::Result<bool> {
        let entity = living_entity_from_resource(self, &this)?;
        Ok(entity.get_mob().is_some())
    }

    async fn get_health(&mut self, this: Resource<WitLivingEntity>) -> wasmtime::Result<f32> {
        let entity = living_entity_from_resource(self, &this)?;
        Ok(entity
            .get_living_entity()
            .map_or(0.0, |living| living.health.load()))
    }

    async fn set_health(
        &mut self,
        this: Resource<WitLivingEntity>,
        health: f32,
    ) -> wasmtime::Result<()> {
        let entity = living_entity_from_resource(self, &this)?;
        if let Some(living) = entity.get_living_entity() {
            living.health.store(health);
        }
        Ok(())
    }

    async fn get_max_health(&mut self, this: Resource<WitLivingEntity>) -> wasmtime::Result<f32> {
        let entity = living_entity_from_resource(self, &this)?;
        Ok(entity
            .get_living_entity()
            .map_or(0.0, crate::entity::living::LivingEntity::get_max_health))
    }

    async fn set_max_health(
        &mut self,
        this: Resource<WitLivingEntity>,
        max_health: f32,
    ) -> wasmtime::Result<()> {
        let entity = living_entity_from_resource(self, &this)?;
        if let Some(living) = entity.get_living_entity() {
            living.set_max_health(max_health);
        }
        Ok(())
    }

    async fn is_dead(&mut self, this: Resource<WitLivingEntity>) -> wasmtime::Result<bool> {
        let entity = living_entity_from_resource(self, &this)?;
        Ok(entity.get_living_entity().map_or_else(
            || entity.get_entity().removal_reason.load().is_some(),
            |living| living.dead.load(std::sync::atomic::Ordering::Relaxed),
        ))
    }

    async fn get_combat_entries(
        &mut self,
        this: Resource<WitLivingEntity>,
    ) -> wasmtime::Result<Vec<WitCombatEntry>> {
        let entity = living_entity_from_resource(self, &this)?;
        let Some(living) = entity.get_living_entity() else {
            return Ok(Vec::new());
        };
        let tracker = living
            .combat_tracker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(tracker.entries().iter().map(to_wit_combat_entry).collect())
    }

    async fn get_killer(
        &mut self,
        this: Resource<WitLivingEntity>,
    ) -> wasmtime::Result<Option<WitCombatEntry>> {
        let entity = living_entity_from_resource(self, &this)?;
        let Some(living) = entity.get_living_entity() else {
            return Ok(None);
        };
        let tracker = living
            .combat_tracker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(tracker.get_killer_entry().map(to_wit_combat_entry))
    }

    async fn is_in_combat(&mut self, this: Resource<WitLivingEntity>) -> wasmtime::Result<bool> {
        let entity = living_entity_from_resource(self, &this)?;
        Ok(entity.get_living_entity().is_some_and(|living| {
            living
                .combat_tracker
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_in_combat()
        }))
    }

    async fn get_combat_duration_ms(
        &mut self,
        this: Resource<WitLivingEntity>,
    ) -> wasmtime::Result<i64> {
        let entity = living_entity_from_resource(self, &this)?;
        let Some(living) = entity.get_living_entity() else {
            return Ok(0);
        };
        let current_tick = living.entity.world.load().level_info.load().day_time;
        let tracker = living
            .combat_tracker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(tracker
            .get_combat_duration(current_tick)
            .saturating_mul(MS_PER_TICK))
    }

    async fn get_last_damage_type_name(
        &mut self,
        this: Resource<WitLivingEntity>,
    ) -> wasmtime::Result<Option<String>> {
        let entity = living_entity_from_resource(self, &this)?;
        let Some(living) = entity.get_living_entity() else {
            return Ok(None);
        };
        Ok(living
            .get_last_resolved_damage_type()
            .map(|damage_type| damage_type_name(&damage_type)))
    }

    async fn has_player_attacker(
        &mut self,
        this: Resource<WitLivingEntity>,
    ) -> wasmtime::Result<bool> {
        let entity = living_entity_from_resource(self, &this)?;
        Ok(entity.get_living_entity().is_some_and(|living| {
            living
                .combat_tracker
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .has_player_attacker()
        }))
    }

    async fn get_absorption(&mut self, this: Resource<WitLivingEntity>) -> wasmtime::Result<f32> {
        let entity = living_entity_from_resource(self, &this)?;
        Ok(entity
            .get_living_entity()
            .map_or(0.0, |living| living.absorption.load()))
    }

    async fn set_absorption(
        &mut self,
        this: Resource<WitLivingEntity>,
        amount: f32,
    ) -> wasmtime::Result<()> {
        let entity = living_entity_from_resource(self, &this)?;
        if let Some(living) = entity.get_living_entity() {
            living.absorption.store(amount);
        }
        Ok(())
    }

    async fn get_attribute_value(
        &mut self,
        this: Resource<WitLivingEntity>,
        attr: Attribute,
    ) -> wasmtime::Result<f64> {
        let entity = living_entity_from_resource(self, &this)?;
        let attribute = from_wit_attribute(attr);
        Ok(entity
            .get_living_entity()
            .map_or(attribute.default_value, |living| {
                living.get_attribute_value(attribute)
            }))
    }

    async fn get_attribute_base(
        &mut self,
        this: Resource<WitLivingEntity>,
        attr: Attribute,
    ) -> wasmtime::Result<f64> {
        let entity = living_entity_from_resource(self, &this)?;
        let attribute = from_wit_attribute(attr);
        Ok(entity
            .get_living_entity()
            .map_or(attribute.default_value, |living| {
                living.get_attribute_base(attribute)
            }))
    }

    async fn set_attribute_base(
        &mut self,
        this: Resource<WitLivingEntity>,
        attr: Attribute,
        value: f64,
    ) -> wasmtime::Result<()> {
        let entity = living_entity_from_resource(self, &this)?;
        let attribute = from_wit_attribute(attr);
        if let Some(living) = entity.get_living_entity() {
            living.set_attribute_base(attribute, value);
            crate::entity::attributes::send_attribute_updates_for_living(
                living,
                vec![attribute.clone()],
            );
        }
        Ok(())
    }

    async fn add_attribute_modifier(
        &mut self,
        this: Resource<WitLivingEntity>,
        attr: Attribute,
        modifier: WitAttributeModifier,
    ) -> wasmtime::Result<()> {
        let entity = living_entity_from_resource(self, &this)?;
        let attribute = from_wit_attribute(attr);
        if let Some(living) = entity.get_living_entity() {
            let internal_mod = crate::entity::attributes::Modifier {
                id: modifier.id,
                amount: modifier.amount,
                operation: from_wit_modifier_op(modifier.operation),
            };
            living.update_attribute(attribute, |inst| inst.add_or_replace_modifier(internal_mod));
            crate::entity::attributes::send_attribute_updates_for_living(
                living,
                vec![attribute.clone()],
            );
        }
        Ok(())
    }

    async fn remove_attribute_modifier(
        &mut self,
        this: Resource<WitLivingEntity>,
        attr: Attribute,
        id: String,
    ) -> wasmtime::Result<()> {
        let entity = living_entity_from_resource(self, &this)?;
        let attribute = from_wit_attribute(attr);
        if let Some(living) = entity.get_living_entity() {
            living.update_attribute(attribute, |inst| inst.remove_modifier(&id));
            crate::entity::attributes::send_attribute_updates_for_living(
                living,
                vec![attribute.clone()],
            );
        }
        Ok(())
    }

    async fn get_attribute_modifiers(
        &mut self,
        this: Resource<WitLivingEntity>,
        attr: Attribute,
    ) -> wasmtime::Result<Vec<WitAttributeModifier>> {
        let entity = living_entity_from_resource(self, &this)?;
        let attribute = from_wit_attribute(attr);
        if let Some(living) = entity.get_living_entity() {
            let map = living
                .attributes
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(inst) = map.get(&attribute.id) {
                return Ok(inst
                    .modifiers
                    .iter()
                    .map(|m| WitAttributeModifier {
                        id: m.id.clone(),
                        amount: m.amount,
                        operation: to_wit_modifier_op(m.operation),
                    })
                    .collect());
            }
        }
        Ok(Vec::new())
    }

    async fn reset_attribute(
        &mut self,
        this: Resource<WitLivingEntity>,
        attr: Attribute,
    ) -> wasmtime::Result<()> {
        let entity = living_entity_from_resource(self, &this)?;
        let attribute = from_wit_attribute(attr);
        if let Some(living) = entity.get_living_entity() {
            {
                let mut map = living
                    .attributes
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                map.remove(&attribute.id);
            };
            crate::entity::attributes::send_attribute_updates_for_living(
                living,
                vec![attribute.clone()],
            );
        }
        Ok(())
    }

    async fn reset_all_attributes(
        &mut self,
        this: Resource<WitLivingEntity>,
    ) -> wasmtime::Result<()> {
        let entity = living_entity_from_resource(self, &this)?;
        if let Some(living) = entity.get_living_entity() {
            living.reset_effects_and_attributes();
        }
        Ok(())
    }

    async fn get_equipment(
        &mut self,
        this: Resource<WitLivingEntity>,
        slot: WitEquipmentSlot,
    ) -> wasmtime::Result<Option<Resource<WitHostItemStack>>> {
        let entity = living_entity_from_resource(self, &this)?;
        if let Some(living) = entity.get_living_entity() {
            let slot = from_wit_equipment_slot(slot);
            let equipment = living
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let stack = equipment.get(&slot);
            if !stack.is_empty() {
                return Ok(Some(
                    self.add_item_stack(Arc::new(tokio::sync::Mutex::new(stack)))?,
                ));
            }
        }
        Ok(None)
    }

    async fn set_equipment(
        &mut self,
        this: Resource<WitLivingEntity>,
        slot: WitEquipmentSlot,
        stack: Option<Resource<WitHostItemStack>>,
    ) -> wasmtime::Result<()> {
        let entity = living_entity_from_resource(self, &this)?;
        if let Some(living) = entity.get_living_entity() {
            let slot = from_wit_equipment_slot(slot);
            let item_stack = if let Some(stack_res) = stack {
                self.get_item_stack(&stack_res)?.lock().await.clone()
            } else {
                papokin_data::item_stack::ItemStack::EMPTY.clone()
            };

            {
                let mut equipment = living
                    .entity_equipment
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                equipment.put(&slot, item_stack.clone());
            };

            living.send_equipment_changes(&[(slot, item_stack)]);
        }
        Ok(())
    }

    async fn clear_equipment(&mut self, this: Resource<WitLivingEntity>) -> wasmtime::Result<()> {
        let entity = living_entity_from_resource(self, &this)?;
        if let Some(living) = entity.get_living_entity() {
            let mut equipment = living
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let slots_to_clear: Vec<(
                papokin_data::data_component_impl::EquipmentSlot,
                papokin_data::item_stack::ItemStack,
            )> = equipment
                .equipment
                .drain()
                .map(|(slot, _)| (slot, papokin_data::item_stack::ItemStack::EMPTY.clone()))
                .collect();
            drop(equipment);

            living.send_equipment_changes(&slots_to_clear);
        }
        Ok(())
    }

    async fn get_age(&mut self, this: Resource<WitLivingEntity>) -> wasmtime::Result<i32> {
        let entity = living_entity_from_resource(self, &this)?;
        Ok(entity.get_living_entity().map_or(0, |living| {
            living.entity.age.load(std::sync::atomic::Ordering::Relaxed)
        }))
    }

    async fn set_age(&mut self, this: Resource<WitLivingEntity>, age: i32) -> wasmtime::Result<()> {
        let entity = living_entity_from_resource(self, &this)?;
        if let Some(living) = entity.get_living_entity() {
            living
                .entity
                .age
                .store(age, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(())
    }

    async fn send_system_message(
        &mut self,
        this: Resource<WitLivingEntity>,
        message: Resource<TextComponent>,
    ) -> wasmtime::Result<()> {
        let entity = living_entity_from_resource(self, &this)?;
        if let Some(player) = entity.get_player() {
            let text_res = self
                .resource_table
                .get::<crate::plugin::loader::wasm::wasm_host::state::TextComponentResource>(
                    &Resource::new_own(message.rep()),
                )
                .map_err(|_| wasmtime::Error::msg("无效的文本组件资源句柄"))?;
            player.send_system_message(&text_res.provider);
        }
        Ok(())
    }

    async fn drop(&mut self, rep: Resource<WitLivingEntity>) -> wasmtime::Result<()> {
        let _ = self
            .resource_table
            .delete::<LivingEntityResource>(Resource::new_own(rep.rep()));
        Ok(())
    }
}

impl crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::world::HostLivingEntityWithStore<
    PluginHostState,
> for HasSelf<PluginHostState>
{
    async fn damage(
        mut host: Access<'_, PluginHostState, Self>,
        this: Resource<WitLivingEntity>,
        amount: f32,
        damage_type: WitDamageType,
    ) -> wasmtime::Result<()> {
        let (entity, plugin) = {
            let state = host.get();
            (
                living_entity_from_resource(state, &this)?,
                active_plugin(state)?,
            )
        };
        let damage_type = from_wit_damage_type(damage_type);
        plugin
            .store
            .pump_blocking(&mut host, move || {
                entity.damage(&*entity, amount, damage_type);
            })
            .await
    }

    async fn damage_by_name(
        mut host: Access<'_, PluginHostState, Self>,
        this: Resource<WitLivingEntity>,
        amount: f32,
        damage_type_name: String,
    ) -> wasmtime::Result<()> {
        let (entity, plugin, damage_type) = {
            let state = host.get();
            (
                living_entity_from_resource(state, &this)?,
                active_plugin(state)?,
                resolve_damage_type_by_name(state, &damage_type_name)?,
            )
        };
        plugin
            .store
            .pump_blocking(&mut host, move || {
                entity.damage_resolved(&*entity, amount, &damage_type);
            })
            .await
    }
}
