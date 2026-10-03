use std::sync::{
    Arc, Weak,
    atomic::{AtomicI32, AtomicU8, Ordering},
};

use crossbeam::atomic::AtomicCell;
use papokin_data::entity::EntityType;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_data::sound::{Sound, SoundCategory};
use papokin_nbt::compound::NbtCompound;
use uuid::Uuid;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        escape_danger::EscapeDangerGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, run_around_like_crazy::RunAroundLikeCrazyGoal,
        swim::SwimGoal, wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    passive::equine::EquineTaming,
    player::Player,
};

pub const FLAG_TAME: u8 = 2;
pub const FLAG_SADDLE: u8 = 4;
pub const FLAG_EATING: u8 = 16;
pub const FLAG_STANDING: u8 = 32;
pub const FLAG_OPEN_MOUTH: u8 = 64;

pub struct ZombieHorseEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub flags: AtomicU8,
    pub temper: AtomicI32,
    pub owner: AtomicCell<Option<Uuid>>,
    /// 驯化状态机（见 `EquineTaming`），不入 NBT。
    pub taming_timer: AtomicI32,
}

impl ZombieHorseEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let horse = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            flags: AtomicU8::new(0),
            temper: AtomicI32::new(0),
            owner: AtomicCell::new(None),
            taming_timer: AtomicI32::new(0),
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
            goal_selector.add_goal(5, Box::new(RunAroundLikeCrazyGoal::new(1.2)));
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
            papokin_data::tracked_data::zombie_horse::DATA_ID_FLAGS,
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

impl AgeableMob for ZombieHorseEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for ZombieHorseEntity {
    fn is_food(&self, _item_stack: &ItemStack) -> bool {
        false
    }
}

impl EquineTaming for ZombieHorseEntity {
    fn equine_temper(&self) -> &AtomicI32 {
        &self.temper
    }

    fn equine_owner(&self) -> &AtomicCell<Option<Uuid>> {
        &self.owner
    }

    fn equine_taming_timer(&self) -> &AtomicI32 {
        &self.taming_timer
    }

    fn is_equine_tamed(&self) -> bool {
        self.is_tame()
    }

    fn equine_set_tame(&self, tame: bool) {
        self.set_tame(tame);
    }

    fn equine_set_standing(&self, standing: bool) {
        self.set_flag(FLAG_STANDING, standing);
    }

    fn equine_angry_sound(&self) -> Sound {
        Sound::EntityZombieHorseAngry
    }
}

impl Mob for ZombieHorseEntity {
    fn open_rider_inventory(&self, player: &Arc<Player>) {
        let world = player.world();
        if let Some(vehicle) = world.get_entity_by_id(self.get_entity().entity_id) {
            super::horse::open_equipment_screen(&vehicle, player, None, false);
        }
    }

    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn is_tamed(&self) -> bool {
        self.is_tame()
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        nbt.put_bool("Tame", self.is_tame());
        nbt.put_int("Temper", self.temper.load(Ordering::Relaxed));
        if let Some(owner) = self.owner.load() {
            nbt.put_uuid("Owner", owner);
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
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

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.equine_taming_tick();
        self.ageable_ai_step();
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(papokin_data::tracked_data::zombie_horse::DATA_BABY_ID, true);
        }
        entity.set_synced_data(
            papokin_data::tracked_data::zombie_horse::DATA_ID_FLAGS,
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

        if !self.is_baby() {
            // 原版交互分流：已驯服潜行打开装备界面（装/卸鞍），未驯服
            // 空手上马发起驯化尝试，持物品则被发怒拒绝。
            let world = player.world();
            let ent = &self.mob_entity.living_entity.entity;
            if self.is_tame() && player.get_entity().is_sneaking() {
                if let Some(vehicle) = world.get_entity_by_id(ent.entity_id) {
                    super::horse::open_equipment_screen(&vehicle, player, None, false);
                }
                return true;
            }
            // 已被骑乘时不再触发上马/界面（对齐原版 isVehicle 早退）
            if ent.has_passengers() {
                return true;
            }
            if !self.is_tame() && !item_stack.is_empty() {
                self.equine_make_mad();
                return true;
            }
            if let (Some(vehicle), Some(passenger)) = (
                world.get_entity_by_id(ent.entity_id),
                world.get_player_by_id(player.entity_id()),
            ) {
                ent.add_passenger(vehicle, passenger as Arc<dyn EntityBase>);
                if !self.is_tame() {
                    self.start_equine_taming_attempt();
                }
            }
            return true;
        }

        self.animal_interact(player, item_stack, Sound::EntityZombieHorseAmbient)
    }
}
