use std::sync::Arc;
use std::sync::atomic::{AtomicI32, AtomicI64, AtomicU8, Ordering};

use crate::entity::mob::equipment::get_equipment_slot_for_item;
use crate::entity::{Entity, EntityBase, living::LivingEntity, player::Player};
use crossbeam::atomic::AtomicCell;
use papokin_data::item_stack::ItemStack;
use papokin_data::{
    damage::DamageType,
    data_component_impl::{EquipmentSlot, EquipmentType},
    entity::EntityStatus,
    item::Item,
    particle::Particle,
    sound::{Sound, SoundCategory},
};
use papokin_nbt::{compound::NbtCompound, tag::NbtTag};
use papokin_util::math::{euler_angle::EulerAngle, vector3::Vector3};

#[derive(Debug, Clone, Copy)]
pub struct PackedRotation {
    pub head: EulerAngle,
    pub body: EulerAngle,
    pub left_arm: EulerAngle,
    pub right_arm: EulerAngle,
    pub left_leg: EulerAngle,
    pub right_leg: EulerAngle,
}

impl Default for PackedRotation {
    fn default() -> Self {
        Self {
            head: EulerAngle::new(0.0, 0.0, 0.0),
            body: EulerAngle::new(0.0, 0.0, 0.0),
            left_arm: EulerAngle::new(-10.0, 0.0, -10.0),
            right_arm: EulerAngle::new(-15.0, 0.0, 10.0),
            left_leg: EulerAngle::new(-1.0, 0.0, -1.0),
            right_leg: EulerAngle::new(1.0, 0.0, 1.0),
        }
    }
}

impl From<PackedRotation> for NbtTag {
    fn from(val: PackedRotation) -> Self {
        let mut compound = NbtCompound::new();
        compound.put("Head", val.head);
        compound.put("Body", val.body);
        compound.put("LeftArm", val.left_arm);
        compound.put("RightArm", val.right_arm);
        compound.put("LeftLeg", val.left_leg);
        compound.put("RightLeg", val.right_leg);
        Self::Compound(compound)
    }
}

impl From<NbtTag> for PackedRotation {
    #[expect(clippy::unnecessary_fallible_conversions)]
    fn from(tag: NbtTag) -> Self {
        if let NbtTag::Compound(compound) = tag {
            fn get_rotation(
                compound: &NbtCompound,
                key: &'static str,
                default: EulerAngle,
            ) -> EulerAngle {
                compound
                    .get(key)
                    .and_then(|tag| tag.clone().try_into().ok())
                    .unwrap_or(default)
            }

            let default = Self::default();

            Self {
                head: get_rotation(&compound, "Head", default.head),
                body: get_rotation(&compound, "Body", default.body),
                left_arm: get_rotation(&compound, "LeftArm", default.left_arm),
                right_arm: get_rotation(&compound, "RightArm", default.right_arm),
                left_leg: get_rotation(&compound, "LeftLeg", default.left_leg),
                right_leg: get_rotation(&compound, "RightLeg", default.right_leg),
            }
        } else {
            Self::default()
        }
    }
}

pub struct ArmorStandEntity {
    living_entity: LivingEntity,

    armor_stand_flags: AtomicU8,
    last_hit_time: AtomicI64,
    disabled_slots: AtomicI32,

    rotation: AtomicCell<PackedRotation>,
}

impl ArmorStandEntity {
    pub fn new(entity: Entity) -> Self {
        let living_entity = LivingEntity::new(entity);
        let packed_rotation = PackedRotation::default();

        Self {
            living_entity,
            armor_stand_flags: AtomicU8::new(0),
            last_hit_time: AtomicI64::new(0),
            disabled_slots: AtomicI32::new(0),
            rotation: AtomicCell::new(packed_rotation),
        }
    }

    pub fn set_small(&self, small: bool) {
        self.set_bit_field(ArmorStandFlags::Small, small);
    }

    pub fn is_small(&self) -> bool {
        (self.armor_stand_flags.load(Ordering::Relaxed) & ArmorStandFlags::Small as u8) != 0
    }

    pub fn set_show_arms(&self, show_arms: bool) {
        self.set_bit_field(ArmorStandFlags::ShowArms, show_arms);
    }

    pub fn should_show_arms(&self) -> bool {
        (self.armor_stand_flags.load(Ordering::Relaxed) & ArmorStandFlags::ShowArms as u8) != 0
    }

    pub fn set_hide_base_plate(&self, hide_base_plate: bool) {
        self.set_bit_field(ArmorStandFlags::HideBasePlate, hide_base_plate);
    }

    pub fn should_show_base_plate(&self) -> bool {
        (self.armor_stand_flags.load(Ordering::Relaxed) & ArmorStandFlags::HideBasePlate as u8) == 0
    }

    pub fn set_marker(&self, marker: bool) {
        self.set_bit_field(ArmorStandFlags::Marker, marker);
    }

    pub fn is_marker(&self) -> bool {
        (self.armor_stand_flags.load(Ordering::Relaxed) & ArmorStandFlags::Marker as u8) != 0
    }

    fn set_bit_field(&self, bit_field: ArmorStandFlags, set: bool) {
        let current = self.armor_stand_flags.load(Ordering::Relaxed);
        let new_value = if set {
            current | bit_field as u8
        } else {
            current & !(bit_field as u8)
        };
        self.armor_stand_flags.store(new_value, Ordering::Relaxed);
    }

    pub fn can_use_slot(&self, slot: &EquipmentSlot) -> bool {
        !matches!(slot, EquipmentSlot::Body(_) | EquipmentSlot::Saddle(_))
            && !self.is_slot_disabled(slot)
    }

    pub fn is_slot_disabled(&self, slot: &EquipmentSlot) -> bool {
        let disabled_slots = self.disabled_slots.load(Ordering::Relaxed);
        let slot_bit = 1 << slot.get_offset_entity_slot_id(0);

        (disabled_slots & slot_bit) != 0
            || (slot.slot_type() == EquipmentType::Hand && !self.should_show_arms())
    }

    pub fn set_slot_disabled(&self, slot: &EquipmentSlot, disabled: bool) {
        let slot_bit = 1 << slot.get_offset_entity_slot_id(0);
        let current = self.disabled_slots.load(Ordering::Relaxed);

        let new_val = if disabled {
            current | slot_bit
        } else {
            current & !slot_bit
        };

        self.disabled_slots.store(new_val, Ordering::Relaxed);
    }

    pub fn is_invisible(&self) -> bool {
        self.get_entity().invisible.load(Ordering::Relaxed)
    }

    pub fn pack_rotation(&self) -> PackedRotation {
        self.rotation.load()
    }

    pub fn unpack_rotation(&self, packed: &PackedRotation) {
        self.rotation.store(packed.to_owned());
    }

    /// 原版空手取下的槽位扫描顺序（`EquipmentSlot.values()` 中的手与护甲槽）。
    const INTERACT_SLOTS: &[EquipmentSlot] = &[
        EquipmentSlot::MAIN_HAND,
        EquipmentSlot::OFF_HAND,
        EquipmentSlot::FEET,
        EquipmentSlot::LEGS,
        EquipmentSlot::CHEST,
        EquipmentSlot::HEAD,
    ];

    /// 空手右键时取下：按原版顺序扫描第一个非空且可用的槽位。
    fn find_first_occupied_slot(&self) -> Option<EquipmentSlot> {
        let equipment = self
            .living_entity
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Self::INTERACT_SLOTS
            .iter()
            .find(|slot| self.can_use_slot(slot) && !equipment.get(slot).is_empty())
            .cloned()
    }

    /// 破坏时掉落存放的全部装备与手持物品。
    fn drop_all_equipment(&self) {
        let entity = self.get_entity();
        let world = entity.world.load();
        let mut equipment = self
            .living_entity
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for stack in equipment.equipment.values() {
            if !stack.is_empty() {
                world.drop_stack(&entity.block_pos.load(), stack.clone());
            }
        }
        equipment.equipment.clear();
    }

    fn break_and_drop_items(&self) {
        let entity = self.get_entity();
        //let name = entity.custom_name.unwrap_or(entity.get_name());

        //TODO: 我真笨！let armor_stand_item = ItemStack::new_with_component(1, &Item::ARMOR_STAND, vec![(DataComponent::CustomName, self.get_custom_name())]);
        let armor_stand_item = ItemStack::new(1, &Item::ARMOR_STAND);
        self.drop_all_equipment();
        entity
            .world
            .load()
            .drop_stack(&entity.block_pos.load(), armor_stand_item);

        Self::on_break(entity);
    }

    fn on_break(entity: &Entity) {
        let world = entity.world.load();
        world.play_sound(
            Sound::EntityArmorStandBreak,
            SoundCategory::Neutral,
            &entity.pos.load(),
        );
    }

    /// 在盔甲架的位置生成破坏粒子。
    // TODO: 像原版那样使用橡木木板方块粒子（需要粒子系统中提供方块状态数据）
    fn spawn_break_particles(entity: &Entity) {
        let world = entity.world.load();
        let pos = entity.pos.load();
        let width = entity.width();
        let height = entity.height();

        // 生成与原版类似的粒子：10 个粒子，偏移量基于实体大小
        world.spawn_particle(
            Vector3::new(pos.x, pos.y + f64::from(height) * 0.6666, pos.z),
            Vector3::new(width / 4.0, height / 4.0, width / 4.0),
            0.05,
            10,
            Particle::Poof,
        );
    }
}

impl EntityBase for ArmorStandEntity {
    fn write_custom_nbt(&self, nbt: &mut NbtCompound) {
        let disabled_slots = self.disabled_slots.load(Ordering::Relaxed);
        // ...

        nbt.put_bool("Invisible", self.is_invisible());
        nbt.put_bool("Small", self.is_small());
        nbt.put_bool("ShowArms", self.should_show_arms());
        nbt.put_int("DisabledSlots", disabled_slots);
        nbt.put_bool("NoBasePlate", !self.should_show_base_plate());
        if self.is_marker() {
            nbt.put_bool("Marker", true);
        }

        nbt.put("Pose", self.pack_rotation());

        // 原版格式：ArmorItems=[靴、腿、胸、头]，HandItems=[主手、副手]；
        // 空槽位写空复合标签
        let equipment = self
            .living_entity
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let to_compound = |slot: &EquipmentSlot| {
            let mut compound = NbtCompound::new();
            equipment.get(slot).write_item_stack(&mut compound);
            NbtTag::Compound(compound)
        };
        nbt.put(
            "ArmorItems",
            NbtTag::List(
                [
                    EquipmentSlot::FEET,
                    EquipmentSlot::LEGS,
                    EquipmentSlot::CHEST,
                    EquipmentSlot::HEAD,
                ]
                .iter()
                .map(to_compound)
                .collect(),
            ),
        );
        nbt.put(
            "HandItems",
            NbtTag::List(
                [EquipmentSlot::MAIN_HAND, EquipmentSlot::OFF_HAND]
                    .iter()
                    .map(to_compound)
                    .collect(),
            ),
        );
    }

    fn read_custom_nbt(&self, nbt: &NbtCompound) {
        let mut flags = 0u8;
        // ...

        if let Some(invisible) = nbt.get_bool("Invisible")
            && invisible
        {
            self.get_entity().set_invisible(invisible);
        }

        if let Some(small) = nbt.get_bool("Small")
            && small
        {
            flags |= ArmorStandFlags::Small as u8;
        }

        if let Some(show_arms) = nbt.get_bool("ShowArms")
            && show_arms
        {
            flags |= ArmorStandFlags::ShowArms as u8;
        }

        if let Some(disabled_slots) = nbt.get_int("DisabledSlots") {
            self.disabled_slots.store(disabled_slots, Ordering::Relaxed);
        }

        if let Some(no_base_plate) = nbt.get_bool("NoBasePlate") {
            if !no_base_plate {
                flags |= ArmorStandFlags::HideBasePlate as u8;
            }
        } else {
            flags |= ArmorStandFlags::HideBasePlate as u8;
        }

        if let Some(marker) = nbt.get_bool("Marker")
            && marker
        {
            flags |= ArmorStandFlags::Marker as u8;
        }

        self.armor_stand_flags.store(flags, Ordering::Relaxed);

        if let Some(pose_tag) = nbt.get("Pose") {
            let packed: PackedRotation = pose_tag.clone().into();
            self.unpack_rotation(&packed);
        }

        // 原版格式：ArmorItems=[靴、腿、胸、头]，HandItems=[主手、副手]
        {
            let mut equipment = self
                .living_entity
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let armor_slots = [
                EquipmentSlot::FEET,
                EquipmentSlot::LEGS,
                EquipmentSlot::CHEST,
                EquipmentSlot::HEAD,
            ];
            if let Some(list) = nbt.get_list("ArmorItems") {
                for (index, tag) in list.iter().enumerate() {
                    let Some(slot) = armor_slots.get(index) else {
                        break;
                    };
                    if let Some(compound) = tag.extract_compound()
                        && let Some(stack) = ItemStack::read_item_stack(&compound.clone())
                        && !stack.is_empty()
                    {
                        equipment.put(slot, stack);
                    }
                }
            }
            let hand_slots = [EquipmentSlot::MAIN_HAND, EquipmentSlot::OFF_HAND];
            if let Some(list) = nbt.get_list("HandItems") {
                for (index, tag) in list.iter().enumerate() {
                    let Some(slot) = hand_slots.get(index) else {
                        break;
                    };
                    if let Some(compound) = tag.extract_compound()
                        && let Some(stack) = ItemStack::read_item_stack(&compound.clone())
                        && !stack.is_empty()
                    {
                        equipment.put(slot, stack);
                    }
                }
            }
        }
    }

    fn get_entity(&self) -> &Entity {
        &self.living_entity.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        Some(&self.living_entity)
    }

    /// 玩家右键盔甲架：手持装备则穿到对应槽位（原有装备回到手中），
    /// 空手则按原版顺序取下第一件装备。返回 true 表示交互已处理。
    fn interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        if self.is_marker() {
            return false;
        }

        // 目标槽位：空手取下第一个非空槽位；手持物品按其装备类型推断槽位
        let slot = if item_stack.is_empty() {
            self.find_first_occupied_slot()
        } else {
            let slot = get_equipment_slot_for_item(item_stack);
            self.can_use_slot(&slot).then_some(slot)
        };
        let Some(slot) = slot else {
            return false;
        };

        // 盔甲架操作事件，取消则本次操作无效
        let mut event = crate::plugin::api::events::player::player_armor_stand_manipulate::PlayerArmorStandManipulateEvent {
            player: player.clone(),
            armor_stand_id: self.get_entity().entity_id,
            slot: slot.get_offset_entity_slot_id(0) as u8,
            cancelled: false,
        };
        let world = self.get_entity().world.load();
        if let Some(server) = world.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return true;
            }
        }

        let current = {
            let mut equipment = self
                .living_entity
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let current = equipment.get(&slot);
            if item_stack.is_empty() {
                // 取下：槽位清空
                equipment.put(&slot, ItemStack::EMPTY.clone());
            } else {
                // 放置：放入一份；数量大于 1 时仅消耗 1 个，否则与原有装备交换
                equipment.put(&slot, item_stack.copy_with_count(1));
            }
            current
        };

        // 非创造模式才更新玩家手持：取下/交换时装备进入手中，放置时消耗
        if !player.is_creative() {
            if item_stack.is_empty() || item_stack.item_count == 1 {
                *item_stack = current;
            } else {
                item_stack.set_count(item_stack.item_count - 1);
            }
        }

        // 同步装备变化到客户端
        let new_stack = {
            let equipment = self
                .living_entity
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            equipment.get(&slot)
        };
        self.living_entity
            .send_equipment_changes(&[(slot, new_stack)]);
        world.play_sound(
            Sound::ItemArmorEquipGeneric,
            SoundCategory::Neutral,
            &self.get_entity().pos.load(),
        );
        true
    }

    fn kill(&self, _caller: &dyn EntityBase) {
        // /kill 等路径同样掉落自身物品与全部装备（原版行为）
        self.drop_all_equipment();
        let entity = self.get_entity();
        let stand_item = ItemStack::new(1, &Item::ARMOR_STAND);
        entity
            .world
            .load()
            .drop_stack(&entity.block_pos.load(), stand_item);
        entity.remove();
        // TODO: 发出 GameEvent::ENTITY_DIE
    }

    fn damage_with_context(
        &self,
        _caller: &dyn EntityBase,
        _amount: f32,
        damage_type: DamageType,
        _position: Option<Vector3<f64>>,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) -> bool {
        let entity = self.get_entity();
        if entity.is_removed() {
            return false;
        }

        let world = entity.world.load();

        let mob_griefing_gamerule = {
            let game_rules = &world.level_info.load().game_rules;
            game_rules.mob_griefing
        };

        if !mob_griefing_gamerule && source.is_some_and(|source| source.get_player().is_none()) {
            return false;
        }

        let bypasses_invulnerability =
            damage_type == DamageType::OUT_OF_WORLD || damage_type == DamageType::GENERIC_KILL;

        if bypasses_invulnerability {
            entity.remove();
            return false;
        }

        if entity.is_invulnerable_to(&damage_type) || self.is_invisible() || self.is_marker() {
            return false;
        }

        let is_explosion = damage_type == DamageType::FIREWORKS
            || damage_type == DamageType::EXPLOSION
            || damage_type == DamageType::PLAYER_EXPLOSION
            || damage_type == DamageType::BAD_RESPAWN_POINT;

        if is_explosion {
            self.drop_all_equipment();
            Self::on_break(entity);
            entity.remove();
            return false;
        }

        // TODO: IGNITES_ARMOR_STANDS（in_fire、营火）- 使其着火
        // TODO: BURNS_ARMOR_STANDS（on_fire）- 减少生命值

        let can_break = damage_type == DamageType::PLAYER_EXPLOSION
            || damage_type == DamageType::PLAYER_ATTACK
            || damage_type == DamageType::SPEAR
            || damage_type == DamageType::MACE_SMASH;

        let always_kills = damage_type == DamageType::ARROW
            || damage_type == DamageType::TRIDENT
            || damage_type == DamageType::FIREBALL
            || damage_type == DamageType::WITHER_SKULL
            || damage_type == DamageType::WIND_CHARGE;

        if !can_break && !always_kills {
            return false;
        }

        let attacker = cause.or(source);
        if let Some(attacker) = attacker
            && let Some(player) = attacker.get_player()
        {
            if !player
                .abilities
                .try_lock()
                .is_ok_and(|a| a.allow_modify_world)
            {
                return false;
            } else if player.is_creative() {
                Self::spawn_break_particles(entity);
                entity.remove();
                return true;
            }
        }

        let time = world
            .level_time
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .query_gametime();

        if time - self.last_hit_time.load(Ordering::Relaxed) > 5 && !always_kills {
            world.send_entity_status(entity, EntityStatus::ArmorstandWobble);
            world.play_sound(
                Sound::EntityArmorStandHit,
                SoundCategory::Neutral,
                &entity.block_pos.load().to_f64(),
            );
            self.last_hit_time.store(time, Ordering::Relaxed);
        } else {
            Self::spawn_break_particles(entity);
            world.play_sound(
                Sound::EntityArmorStandBreak,
                SoundCategory::Neutral,
                &entity.block_pos.load().to_f64(),
            );
            self.break_and_drop_items();
            entity.remove();
        }

        true
    }

    fn get_gravity(&self) -> f64 {
        0.08
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub enum ArmorStandFlags {
    /// 小型盔甲架标志
    Small = 1,
    /// 显示手臂标志
    ShowArms = 4,
    /// 隐藏底板标志
    HideBasePlate = 8,
    /// 标记旗帜
    Marker = 16,
}
