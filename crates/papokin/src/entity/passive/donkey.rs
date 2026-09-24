use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering},
};

use crossbeam::atomic::AtomicCell;
use papokin_data::entity::EntityType;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_data::sound::{Sound, SoundCategory};
use papokin_data::tag::{self, Taggable};
use papokin_nbt::compound::NbtCompound;
use uuid::Uuid;

use papokin_inventory::inventory::{Inventory, SimpleInventory};

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

pub struct DonkeyEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub flags: AtomicU8,
    pub has_chest: AtomicBool,
    /// 驮箱内容（原版带箱驴/骡 5 列 × 3 行 = 15 格）。物品栏常驻，
    /// 仅 `has_chest` 时暴露给界面，避免装箱/卸箱时迁移数据。
    pub chest_inventory: Arc<SimpleInventory>,
    pub temper: AtomicI32,
    pub owner: AtomicCell<Option<Uuid>>,
}

impl DonkeyEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let donkey = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            flags: AtomicU8::new(0),
            has_chest: AtomicBool::new(false),
            chest_inventory: Arc::new(SimpleInventory::new(15)),
            temper: AtomicI32::new(0),
            owner: AtomicCell::new(None),
        };
        let mob_arc = Arc::new(donkey);
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
    pub fn has_flag(&self, flag: u8) -> bool {
        (self.flags.load(Ordering::Relaxed) & flag) != 0
    }

    pub fn set_flag(&self, flag: u8, val: bool) {
        let current = self.flags.load(Ordering::Relaxed);
        let new_flags = if val { current | flag } else { current & !flag };
        self.flags.store(new_flags, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            papokin_data::tracked_data::donkey::DATA_ID_FLAGS,
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

    #[must_use]
    pub fn has_chest(&self) -> bool {
        self.has_chest.load(Ordering::Relaxed)
    }

    pub fn set_has_chest(&self, val: bool) {
        self.has_chest.store(val, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(papokin_data::tracked_data::donkey::DATA_ID_CHEST, val);
    }
}

impl AgeableMob for DonkeyEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for DonkeyEntity {
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

impl Mob for DonkeyEntity {
    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        nbt.put_bool("ChestedHorse", self.has_chest());
        // 驮箱内容走原版 Items 列表（Slot 字节 + 物品堆），空箱不写出
        self.chest_inventory.write_inventory_nbt(nbt, false);
        nbt.put_bool("Tame", self.is_tame());
        nbt.put_int("Temper", self.temper.load(Ordering::Relaxed));
        if let Some(owner) = self.owner.load() {
            nbt.put_uuid("Owner", owner);
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
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

    fn set_saddled_flag(&self, saddled: bool) {
        self.set_saddled(saddled);
    }

    fn drop_mount_chest(&self) {
        if !self.has_chest() {
            return;
        }
        // 箱子本体必掉（原版语义）；内容受 doMobLoot 游戏规则管控
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
            entity.set_synced_data(papokin_data::tracked_data::donkey::DATA_BABY_ID, true);
        }
        entity.set_synced_data(
            papokin_data::tracked_data::donkey::DATA_ID_CHEST,
            self.has_chest(),
        );
        entity.set_synced_data(
            papokin_data::tracked_data::donkey::DATA_ID_FLAGS,
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

        if !self.is_baby() && !self.is_food(item_stack) {
            let world = player.world();
            let ent = &self.mob_entity.living_entity.entity;
            if let Some(vehicle) = world.get_entity_by_id(ent.entity_id) {
                if self.is_tame() && !player.get_entity().is_sneaking() {
                    let chest_inventory = self
                        .has_chest()
                        .then(|| self.chest_inventory.clone())
                        .map(|inventory| inventory as Arc<dyn Inventory>);
                    super::horse::open_equipment_screen(&vehicle, player, chest_inventory);
                } else if let Some(passenger) = world.get_player_by_id(player.entity_id()) {
                    ent.add_passenger(vehicle, passenger as Arc<dyn EntityBase>);
                }
                return true;
            }
        }

        self.animal_interact(player, item_stack, Sound::EntityDonkeyAmbient)
    }
}
