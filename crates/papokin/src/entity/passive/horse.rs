use std::sync::{
    Arc, Weak,
    atomic::{AtomicI32, AtomicU8, Ordering},
};

use crossbeam::atomic::AtomicCell;
use papokin_data::entity::EntityType;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_data::sound::{Sound, SoundCategory};
use papokin_data::tag::{self, Taggable};
use papokin_nbt::compound::NbtCompound;
use papokin_protocol::codec::var_int::VarInt;
use rand::RngExt;
use uuid::Uuid;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        breed::BreedGoal, escape_danger::EscapeDangerGoal, follow_parent::FollowParentGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal, swim::SwimGoal,
        tempt::TemptGoal, wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};
use papokin_data::data_component_impl::EquipmentSlot;
use papokin_inventory::inventory::{EquipmentSlotInventory, SimpleInventory};
use papokin_inventory::mount_screen_handler::MountScreenHandler;

const TEMPT_ITEMS: &[&Item] = &[
    &Item::GOLDEN_APPLE,
    &Item::ENCHANTED_GOLDEN_APPLE,
    &Item::GOLDEN_CARROT,
];

pub const FLAG_TAME: u8 = 2;
pub const FLAG_SADDLE: u8 = 4;
pub const FLAG_BRED: u8 = 8;
pub const FLAG_EATING: u8 = 16;
pub const FLAG_STANDING: u8 = 32;
pub const FLAG_OPEN_MOUTH: u8 = 64;

pub struct HorseEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub variant: AtomicI32,
    pub flags: AtomicU8,
    pub temper: AtomicI32,
    pub owner: AtomicCell<Option<Uuid>>,
}

impl HorseEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let mut rng = rand::rng();
        let color = rng.random_range(0..7);
        let style = rng.random_range(0..5);
        let variant = color | (style << 8);

        let horse = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            variant: AtomicI32::new(variant),
            flags: AtomicU8::new(0),
            temper: AtomicI32::new(0),
            owner: AtomicCell::new(None),
        };
        let mob_arc = Arc::new(horse);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, EscapeDangerGoal::new(1.2));
            goal_selector.add_goal(2, BreedGoal::new(1.0));
            goal_selector.add_goal(3, Box::new(TemptGoal::new(1.25, TEMPT_ITEMS, false)));
            goal_selector.add_goal(4, Box::new(FollowParentGoal::new(1.0)));
            goal_selector.add_goal(6, Box::new(WanderAroundGoal::new(0.7)));
            goal_selector.add_goal(
                7,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    #[must_use]
    pub fn get_variant(&self) -> i32 {
        self.variant.load(Ordering::Relaxed)
    }

    pub fn set_variant(&self, variant: i32) {
        self.variant.store(variant, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            papokin_data::tracked_data::horse::DATA_ID_TYPE_VARIANT,
            VarInt(variant),
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
            papokin_data::tracked_data::horse::DATA_ID_FLAGS,
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
    pub fn is_saddled(&self) -> bool {
        self.has_flag(FLAG_SADDLE)
    }

    pub fn set_saddled(&self, val: bool) {
        self.set_flag(FLAG_SADDLE, val);
    }
}

/// 是否为马铠物品（皮/铁/金/钻）。
#[must_use]
pub fn is_horse_armor(item: &Item) -> bool {
    item.registry_key.ends_with("_horse_armor")
}

/// 打开马系装备界面（鞍 + 马铠槽；普通马无箱子格）。
/// 鞍槽装卸时同步客户端装备渲染与 `FLAG_SADDLE`（经
/// `Mob::set_saddled_flag`，骑乘控制与交互判定依赖该标志，
/// 避免界面状态与实体状态分裂）。
pub fn open_equipment_screen(mount: &Arc<dyn EntityBase>, player: &Arc<Player>) {
    let Some(entity_equipment) = mount
        .get_living_entity()
        .map(|living| living.entity_equipment.clone())
    else {
        return;
    };

    let saddle_mount = mount.clone();
    let saddle_inventory = EquipmentSlotInventory::with_callback(
        entity_equipment.clone(),
        EquipmentSlot::SADDLE,
        Arc::new(move |stack| {
            if let Some(living) = saddle_mount.get_living_entity() {
                living.send_equipment_changes(&[(EquipmentSlot::SADDLE, stack.clone())]);
            }
            if let Some(mob) = saddle_mount.get_mob() {
                mob.set_saddled_flag(!stack.is_empty());
            }
        }),
    );

    let armor_mount = mount.clone();
    let armor_inventory = EquipmentSlotInventory::with_callback(
        entity_equipment,
        EquipmentSlot::BODY,
        Arc::new(move |stack| {
            if let Some(living) = armor_mount.get_living_entity() {
                living.send_equipment_changes(&[(EquipmentSlot::BODY, stack.clone())]);
            }
        }),
    );

    player.increment_screen_handler_sync_id();
    let handler = Arc::new(std::sync::Mutex::new(MountScreenHandler::new(
        player.screen_handler_sync_id.load(Ordering::Relaxed),
        &player.inventory,
        Arc::new(SimpleInventory::new(0)),
        saddle_inventory,
        armor_inventory,
        0,
    )));
    player.open_mount_screen(handler, 0, mount.get_entity().entity_id);
}

impl AgeableMob for HorseEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for HorseEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_HORSE_FOOD)
            || item_stack.item == &Item::WHEAT
            || item_stack.item == &Item::SUGAR
            || item_stack.item == &Item::HAY_BLOCK
            || item_stack.item == &Item::APPLE
            || item_stack.item == &Item::GOLDEN_CARROT
            || item_stack.item == &Item::GOLDEN_APPLE
            || item_stack.item == &Item::ENCHANTED_GOLDEN_APPLE
    }
}

impl Mob for HorseEntity {
    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        nbt.put_int("Variant", self.get_variant());
        nbt.put_bool("Tame", self.is_tame());
        nbt.put_int("Temper", self.temper.load(Ordering::Relaxed));
        if let Some(owner) = self.owner.load() {
            nbt.put_uuid("Owner", owner);
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
        if let Some(variant) = nbt.get_int("Variant") {
            self.set_variant(variant);
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
        // 通用层已把 SaddleItem 读入 SADDLE 装备槽；据此恢复鞍具
        // 标志（客户端渲染与骑乘控制依赖 DATA_ID_FLAGS）。
        if !self
            .get_mob_entity()
            .get_item_in_slot(&papokin_data::data_component_impl::EquipmentSlot::SADDLE)
            .is_empty()
        {
            self.set_saddled(true);
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
            entity.set_synced_data(papokin_data::tracked_data::horse::DATA_BABY_ID, true);
        }
        entity.set_synced_data(
            papokin_data::tracked_data::horse::DATA_ID_TYPE_VARIANT,
            VarInt(self.get_variant()),
        );
        entity.set_synced_data(
            papokin_data::tracked_data::horse::DATA_ID_FLAGS,
            self.flags.load(Ordering::Relaxed) as i8,
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let item = item_stack.get_item();

        if self.is_tame() && item == &Item::SADDLE && !self.is_saddled() && !self.is_baby() {
            self.set_saddled(true);
            // 鞍具存入装备槽并标记必掉：死亡时完好掉落（原版行为），
            // 不再随 FLAG_SADDLE 标志一起湮灭。
            self.mob_entity.set_item_slot_and_drop_when_killed(
                &papokin_data::data_component_impl::EquipmentSlot::SADDLE,
                ItemStack::new(1, &Item::SADDLE),
            );
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            let entity = self.get_entity();
            let world = entity.world.load();
            world.play_sound(
                Sound::EntityHorseSaddle,
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
            return true;
        }

        // 手持马铠右键已驯服的马：直接穿上（原版行为），空槽才接受。
        if self.is_tame()
            && !self.is_baby()
            && is_horse_armor(item)
            && self
                .mob_entity
                .get_item_in_slot(&papokin_data::data_component_impl::EquipmentSlot::BODY)
                .is_empty()
        {
            self.mob_entity.set_item_slot_and_drop_when_killed(
                &papokin_data::data_component_impl::EquipmentSlot::BODY,
                item_stack.clone(),
            );
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            let entity = self.get_entity();
            let world = entity.world.load();
            world.play_sound(
                Sound::EntityHorseArmor,
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
            return true;
        }

        if !self.is_baby() && !self.is_food(item_stack) {
            // 原版交互分流：已驯服且非潜行打开装备界面（装/卸鞍与
            // 马铠），潜行或未驯服时上马（未驯服空手上马即驯服尝试）。
            let world = player.world();
            let ent = &self.mob_entity.living_entity.entity;
            if let Some(vehicle) = world.get_entity_by_id(ent.entity_id) {
                if self.is_tame() && !player.get_entity().is_sneaking() {
                    open_equipment_screen(&vehicle, player);
                } else if let Some(passenger) = world.get_player_by_id(player.entity_id()) {
                    ent.add_passenger(vehicle, passenger as Arc<dyn EntityBase>);
                }
            }
            return true;
        }

        self.animal_interact(player, item_stack, Sound::EntityHorseAmbient)
    }
}
