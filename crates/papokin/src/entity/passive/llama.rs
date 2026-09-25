use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering},
};

use crossbeam::atomic::AtomicCell;
use papokin_data::data_component_impl::EquipmentSlot;
use papokin_data::entity::EntityType;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_data::sound::{Sound, SoundCategory};
use papokin_data::tag::{self, Taggable};
use papokin_inventory::inventory::{EquipmentSlotInventory, Inventory, SimpleInventory};
use papokin_inventory::llama_screen_handler::LlamaScreenHandler;
use papokin_nbt::compound::NbtCompound;
use papokin_protocol::codec::var_int::VarInt;
use rand::RngExt;
use uuid::Uuid;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        active_target::ActiveTargetGoal, breed::BreedGoal, escape_danger::EscapeDangerGoal,
        follow_parent::FollowParentGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, ranged_attack::RangedAttackGoal, revenge::RevengeGoal,
        swim::SwimGoal, tempt::TemptGoal, wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity, RangedAttackMob},
    passive::animal::{Animal, get_carpet_color_from_item},
    player::Player,
    projectile::llama_spit::LlamaSpitEntity,
};

const TEMPT_ITEMS: &[&Item] = &[&Item::HAY_BLOCK];

pub const FLAG_TAME: u8 = 2;
pub const FLAG_BRED: u8 = 8;
pub const FLAG_EATING: u8 = 16;
pub const FLAG_STANDING: u8 = 32;
pub const FLAG_OPEN_MOUTH: u8 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum LlamaVariant {
    #[default]
    Creamy = 0,
    White = 1,
    Brown = 2,
    Gray = 3,
}

impl LlamaVariant {
    #[must_use]
    pub const fn from_id(id: i32) -> Self {
        match id {
            1 => Self::White,
            2 => Self::Brown,
            3 => Self::Gray,
            _ => Self::Creamy,
        }
    }

    #[must_use]
    pub const fn id(self) -> i32 {
        self as i32
    }

    #[must_use]
    pub fn random_variant() -> Self {
        let mut rng = rand::rng();
        match rng.random_range(0..4) {
            1 => Self::White,
            2 => Self::Brown,
            3 => Self::Gray,
            _ => Self::Creamy,
        }
    }
}

pub struct LlamaEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub variant: AtomicI32,
    pub strength: AtomicI32,
    /// 驮箱内容（原版带箱羊驼按 strength 3-15 格；15 格上限常驻，
    /// 界面按 strength 截断暴露，避免 strength 变化时迁移数据）。
    /// 驮物（地毯）不在此处：存 `BODY` 装备槽（通用 `ArmorItem` NBT）。
    pub chest_inventory: Arc<SimpleInventory>,
    pub flags: AtomicU8,
    pub has_chest: AtomicBool,
    pub temper: AtomicI32,
    pub owner: AtomicCell<Option<Uuid>>,
}

impl LlamaEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let mut rng = rand::rng();
        let variant = LlamaVariant::random_variant();
        let strength = rng.random_range(1..=5);

        let llama = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            variant: AtomicI32::new(variant.id()),
            strength: AtomicI32::new(strength),
            chest_inventory: Arc::new(SimpleInventory::new(15)),
            flags: AtomicU8::new(0),
            has_chest: AtomicBool::new(false),
            temper: AtomicI32::new(0),
            owner: AtomicCell::new(None),
        };
        let mob_arc = Arc::new(llama);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };
        let ranged_weak: Weak<dyn RangedAttackMob> = {
            let ranged_arc: Arc<dyn RangedAttackMob> = mob_arc.clone();
            Arc::downgrade(&ranged_arc)
        };

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, EscapeDangerGoal::new(1.2));
            goal_selector.add_goal(2, BreedGoal::new(1.0));
            goal_selector.add_goal(
                3,
                Box::new(RangedAttackGoal::new(ranged_weak, 1.25, 40, 20.0)),
            );
            goal_selector.add_goal(4, Box::new(TemptGoal::new(1.25, TEMPT_ITEMS, false)));
            goal_selector.add_goal(5, Box::new(FollowParentGoal::new(1.0)));
            goal_selector.add_goal(7, Box::new(WanderAroundGoal::new(0.7)));
            goal_selector.add_goal(
                8,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(9, Box::new(RandomLookAroundGoal::default()));

            target_selector.add_goal(1, Box::new(RevengeGoal::new(true)));
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::WOLF, true),
            );
        };

        mob_arc
    }

    #[must_use]
    pub fn get_variant(&self) -> LlamaVariant {
        LlamaVariant::from_id(self.variant.load(Ordering::Relaxed))
    }

    pub fn set_variant(&self, variant: LlamaVariant) {
        self.variant.store(variant.id(), Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            papokin_data::tracked_data::llama::DATA_VARIANT_ID,
            VarInt(variant.id()),
        );
    }

    #[must_use]
    pub fn get_strength(&self) -> i32 {
        self.strength.load(Ordering::Relaxed)
    }

    pub fn set_strength(&self, strength: i32) {
        self.strength.store(strength, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            papokin_data::tracked_data::llama::DATA_STRENGTH_ID,
            VarInt(strength),
        );
    }

    #[must_use]
    pub fn has_flag(&self, flag: u8) -> bool {
        (self.flags.load(Ordering::Relaxed) & flag) != 0
    }

    pub fn set_flag(&self, flag: u8, val: bool) {
        let current = self.flags.load(Ordering::Relaxed);
        let new_flags = if val { current | flag } else { current & !flag };
        self.flags.store(new_flags, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            papokin_data::tracked_data::llama::DATA_ID_FLAGS,
            new_flags as i8,
        );
    }

    #[must_use]
    pub fn is_tame(&self) -> bool {
        self.has_flag(FLAG_TAME)
    }

    pub fn set_tame(&self, val: bool) {
        self.set_flag(FLAG_TAME, val);
    }

    #[must_use]
    pub fn has_chest(&self) -> bool {
        self.has_chest.load(Ordering::Relaxed)
    }

    pub fn set_has_chest(&self, val: bool) {
        self.has_chest.store(val, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(papokin_data::tracked_data::llama::DATA_ID_CHEST, val);
    }

    pub fn spit(&self, target: &Arc<dyn EntityBase>) {
        let entity = self.get_entity();
        let world = entity.world.load_full();

        let spit_entity = Entity::new(world.clone(), entity.pos.load(), &EntityType::LLAMA_SPIT);
        let spit = LlamaSpitEntity::new_shot(spit_entity, entity);

        let mob_pos = entity.pos.load();
        let target_entity = target.get_entity();
        let target_pos = target_entity.pos.load();
        let target_height = f64::from(target_entity.entity_dimension.load().height);

        let dx = target_pos.x - mob_pos.x;
        let dy = (target_pos.y + target_height / 3.0) - spit.get_entity().pos.load().y;
        let dz = target_pos.z - mob_pos.z;
        let horizontal_distance = dx.hypot(dz);
        let yo = horizontal_distance * 0.2;

        spit.thrown.set_velocity(dx, dy + yo, dz, 1.5, 10.0);

        if !entity.silent.load(Ordering::Relaxed) {
            world.play_sound(Sound::EntityLlamaSpit, SoundCategory::Neutral, &mob_pos);
        }

        let spit_arc: Arc<dyn EntityBase> = Arc::new(spit);
        world.spawn_entity(spit_arc);
    }
}

/// 打开羊驼坐骑界面（驼物地毯槽 + 按 strength 3-15 格驮箱）。
/// 槽位序对齐原版 `LlamaScreenHandler`：槽 0 地毯（BODY 装备槽
/// 适配，装卸经装备同步渲染），其后驮箱格。`slot_count` 为马侧总
/// 槽数（1 + 驮箱格数，原版带箱羊驼 `inventory.size()` 语义）。
pub fn open_llama_screen(
    mount: &Arc<dyn EntityBase>,
    player: &Arc<Player>,
    chest_inventory: Arc<SimpleInventory>,
    chest_slots: usize,
) {
    let Some(entity_equipment) = mount
        .get_living_entity()
        .map(|living| living.entity_equipment.clone())
    else {
        return;
    };

    let carpet_mount = mount.clone();
    let carpet_inventory = EquipmentSlotInventory::with_callback(
        entity_equipment,
        EquipmentSlot::BODY,
        Arc::new(move |stack| {
            if let Some(living) = carpet_mount.get_living_entity() {
                living.send_equipment_changes(&[(EquipmentSlot::BODY, stack.clone())]);
            }
        }),
    );

    player.increment_screen_handler_sync_id();
    let handler = Arc::new(std::sync::Mutex::new(LlamaScreenHandler::new(
        player.screen_handler_sync_id.load(Ordering::Relaxed),
        &player.inventory,
        carpet_inventory,
        chest_inventory,
        chest_slots,
    )));
    player.open_mount_screen(
        handler,
        LlamaScreenHandler::packet_slot_count(chest_slots),
        mount.get_entity().entity_id,
    );
}

impl AgeableMob for LlamaEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for LlamaEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_LLAMA_FOOD)
            || item_stack
                .item
                .has_tag(&tag::Item::MINECRAFT_LLAMA_TEMPT_ITEMS)
            || item_stack.item == &Item::WHEAT
            || item_stack.item == &Item::HAY_BLOCK
    }
}

impl Mob for LlamaEntity {
    fn open_rider_inventory(&self, player: &Arc<Player>) {
        let world = player.world();
        if let Some(vehicle) = world.get_entity_by_id(self.get_entity().entity_id) {
            let chest_slots = if self.has_chest() {
                LlamaScreenHandler::get_chest_slot_count(self.get_strength())
            } else {
                0
            };
            open_llama_screen(&vehicle, player, self.chest_inventory.clone(), chest_slots);
        }
    }

    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        nbt.put_int("Variant", self.get_variant().id());
        nbt.put_int("Strength", self.get_strength());
        nbt.put_bool("ChestedHorse", self.has_chest());
        // 驮箱内容走原版 Items 列表（Slot 字节 + 物品堆），空箱不
        // 写出；驼物（地毯）由通用装备层存 BODY 槽（ArmorItem 键）。
        self.chest_inventory.write_inventory_nbt(nbt, false);
        nbt.put_bool("Tame", self.is_tame());
        nbt.put_int("Temper", self.temper.load(Ordering::Relaxed));
        if let Some(owner) = self.owner.load() {
            nbt.put_uuid("Owner", owner);
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
        if let Some(variant) = nbt.get_int("Variant") {
            self.set_variant(LlamaVariant::from_id(variant));
        }
        if let Some(strength) = nbt.get_int("Strength") {
            self.set_strength(strength);
        }
        if let Some(chested) = nbt.get_bool("ChestedHorse") {
            self.set_has_chest(chested);
        }
        // 恢复驮箱内容：越界 Slot 字节被忽略（防伪造存档越界写入）
        let mut stacks = vec![ItemStack::EMPTY.clone(); self.chest_inventory.size()];
        self.chest_inventory.read_data(nbt, &mut stacks);
        for (index, stack) in stacks.into_iter().enumerate() {
            if !stack.is_empty() {
                self.chest_inventory.set_stack(index, stack);
            }
        }
        if let Some(tame) = nbt.get_bool("Tame") {
            self.set_tame(tame);
        }
        if let Some(temper) = nbt.get_int("Temper") {
            self.temper.store(temper, Ordering::Relaxed);
        }
        if let Some(owner) = nbt.get_uuid("Owner") {
            self.owner.store(Some(owner));
        }
    }

    fn drop_mount_chest(&self) {
        if !self.has_chest() {
            return;
        }
        // 箱子本体必掉（原版语义）；内容受 doMobLoot 游戏规则管控。
        // 驼物（地毯）在 BODY 装备槽、必掉标记已设，走通用装备掉落。
        self.mob_entity
            .spawn_at_location(ItemStack::new(1, &Item::CHEST));
        let world = self.get_entity().world.load();
        if !world.level_info.load().game_rules.mob_drops {
            return;
        }
        for index in 0..self.chest_inventory.size() {
            let stack = self.chest_inventory.remove_stack(index);
            self.mob_entity.spawn_at_location(stack);
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.ageable_ai_step();
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(papokin_data::tracked_data::llama::DATA_BABY_ID, true);
        }
        entity.set_synced_data(
            papokin_data::tracked_data::llama::DATA_VARIANT_ID,
            VarInt(self.get_variant().id()),
        );
        entity.set_synced_data(
            papokin_data::tracked_data::llama::DATA_STRENGTH_ID,
            VarInt(self.get_strength()),
        );
        entity.set_synced_data(
            papokin_data::tracked_data::llama::DATA_ID_CHEST,
            self.has_chest(),
        );
        entity.set_synced_data(
            papokin_data::tracked_data::llama::DATA_ID_FLAGS,
            self.flags.load(Ordering::Relaxed) as i8,
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let item = item_stack.get_item();

        if self.is_tame() && item == &Item::CHEST && !self.has_chest() && !self.is_baby() {
            self.set_has_chest(true);
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            let entity = self.get_entity();
            let world = entity.world.load();
            world.play_sound(
                Sound::EntityDonkeyChest,
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
            return true;
        }

        if self.is_tame()
            && !self.is_baby()
            && get_carpet_color_from_item(item).is_some()
            && self
                .mob_entity
                .get_item_in_slot(&EquipmentSlot::BODY)
                .is_empty()
        {
            // 驼物（地毯）存入 BODY 装备槽并标记必掉：客户端经装备
            // 同步渲染地毯（1.21.2+ 语义），死亡/界面卸下物品不湮灭。
            self.mob_entity
                .set_item_slot_and_drop_when_killed(&EquipmentSlot::BODY, item_stack.clone());
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            let entity = self.get_entity();
            let world = entity.world.load();
            world.play_sound(
                Sound::EntityLlamaSwag,
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
            return true;
        }

        if !self.is_baby() && !self.is_food(item_stack) {
            // 原版交互分流：已驯服且非潜行打开驼物界面（装/卸地毯与
            // 驮箱存取），潜行或未驯服时上马（羊驼无需鞍即可骑）。
            let world = player.world();
            let ent = &self.mob_entity.living_entity.entity;
            if let Some(vehicle) = world.get_entity_by_id(ent.entity_id) {
                if self.is_tame() && !player.get_entity().is_sneaking() {
                    let chest_slots = if self.has_chest() {
                        LlamaScreenHandler::get_chest_slot_count(self.get_strength())
                    } else {
                        0
                    };
                    open_llama_screen(&vehicle, player, self.chest_inventory.clone(), chest_slots);
                } else if let Some(passenger) = world.get_player_by_id(player.entity_id()) {
                    ent.add_passenger(vehicle, passenger as Arc<dyn EntityBase>);
                }
                return true;
            }
        }

        self.animal_interact(player, item_stack, Sound::EntityLlamaAmbient)
    }
}

impl RangedAttackMob for LlamaEntity {
    fn perform_ranged_attack(&self, target: &Arc<dyn EntityBase>, _power: f32) {
        self.spit(target);
    }
}
