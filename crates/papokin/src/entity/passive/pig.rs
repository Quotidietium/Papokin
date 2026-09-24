use std::sync::{Arc, Weak};

use papokin_data::item_stack::ItemStack;
use papokin_data::sound::Sound;
use papokin_data::{entity::EntityType, item::Item};

use crate::entity::{
    Entity,
    ageable::AgeableMob,
    ai::goal::{
        breed::BreedGoal, escape_danger::EscapeDangerGoal, follow_parent::FollowParentGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal, swim::SwimGoal,
        tempt::TemptGoal, wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};
use papokin_nbt::compound::NbtCompound;

const PIG_FOOD: &[&Item] = &[
    &Item::CARROT,
    &Item::POTATO,
    &Item::BEETROOT,
    &Item::CARROT_ON_A_STICK,
];

use crate::entity::EntityBase;
use crate::entity::item_steerable::{ItemBasedSteering, ItemSteerable};

/// 表示猪，一种常见的被动生物，提供生猪排。
///
/// Wiki: <https://minecraft.wiki/w/Pig>
pub struct PigEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: crate::entity::ageable::AgeableData,
    pub steering: ItemBasedSteering,
    pub saddled: std::sync::atomic::AtomicBool,
}

impl PigEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let pig = Self {
            mob_entity,
            ageable_data: crate::entity::ageable::AgeableData::default(),
            steering: ItemBasedSteering::default(),
            saddled: std::sync::atomic::AtomicBool::new(false),
        };
        let mob_arc = Arc::new(pig);
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
            goal_selector.add_goal(1, EscapeDangerGoal::new(1.25));
            goal_selector.add_goal(2, BreedGoal::new(1.0));
            goal_selector.add_goal(3, Box::new(TemptGoal::new(1.2, PIG_FOOD, false)));
            goal_selector.add_goal(4, Box::new(FollowParentGoal::new(1.1)));
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(7, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }
}

impl AgeableMob for PigEntity {
    fn get_ageable_data(&self) -> &crate::entity::ageable::AgeableData {
        &self.ageable_data
    }
}

impl Animal for PigEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        use papokin_data::tag::Taggable;
        item_stack
            .item
            .has_tag(&papokin_data::tag::Item::MINECRAFT_PIG_FOOD)
            || PIG_FOOD.iter().any(|i| i.id == item_stack.item.id)
    }
}

impl Mob for PigEntity {
    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_bool("Saddle", self.is_saddled());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(saddle) = nbt.get_byte("Saddle") {
            self.set_saddled(saddle == 1);
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn get_item_steerable(&self) -> Option<&dyn ItemSteerable> {
        Some(self)
    }

    fn mob_on_lightning_strike(
        &self,
        caller: &dyn EntityBase,
        lightning: &crate::entity::lightning::LightningBoltEntity,
    ) {
        let entity = &self.mob_entity.living_entity.entity;
        let world = entity.world.load_full();

        // 先创建用于替换的僵尸猪灵，以便插件可以检查
        // （并取消）所生成实体的 id。
        let zombie_pos = entity.pos.load();
        let zombie = crate::entity::r#type::from_type(
            &EntityType::ZOMBIFIED_PIGLIN,
            zombie_pos,
            &world,
            uuid::Uuid::new_v4(),
        );

        let mut event = crate::plugin::api::events::entity::pig_zap::PigZapEvent::new(
            entity.entity_id,
            lightning.get_entity().entity_id,
            zombie.get_entity().entity_id,
        );
        if let Some(server) = world.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }

        if event.cancelled {
            // 猪不会转化，但仍会承受基础打击（火焰 + 伤害）。
            self.mob_entity
                .living_entity
                .on_lightning_strike(caller, lightning);
            return;
        }

        entity.remove();
        world.spawn_entity(zombie);
    }

    fn is_saddled(&self) -> bool {
        self.saddled.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn can_be_saddled(&self) -> bool {
        use crate::entity::ageable::AgeableMob;
        self.mob_entity.living_entity.entity.is_alive() && !self.is_baby()
    }

    fn set_saddled(&self, saddled: bool) {
        self.saddled
            .store(saddled, std::sync::atomic::Ordering::Relaxed);
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        use super::animal::Animal;
        if item_stack.get_item() == &papokin_data::item::Item::SADDLE
            && self.can_be_saddled()
            && !self.is_saddled()
        {
            self.set_saddled(true);
            // 鞍具存入装备槽并标记必掉：死亡时完好掉落（原版行为），
            // 不再随 FLAG_SADDLE 标志一起湮灭。
            self.mob_entity.set_item_slot_and_drop_when_killed(
                &papokin_data::data_component_impl::EquipmentSlot::SADDLE,
                ItemStack::new(1, &papokin_data::item::Item::SADDLE),
            );
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            let entity = self.get_entity();
            let world = entity.world.load();
            let pos = entity.pos.load();
            world.play_sound(
                Sound::EntityPigSaddle,
                papokin_data::sound::SoundCategory::Neutral,
                &pos,
            );
            return true;
        }

        if self.is_saddled() && !self.is_food(item_stack) {
            let world = player.world();
            if let Some(vehicle) = world.get_entity_by_id(self.get_entity().entity_id)
                && let Some(passenger) = world.get_player_by_id(player.entity_id())
            {
                self.get_entity()
                    .add_passenger(vehicle, passenger as Arc<dyn EntityBase>);
                return true;
            }
        }
        self.animal_interact(player, item_stack, Sound::EntityPigAmbient)
    }
}

impl ItemSteerable for PigEntity {
    fn boost(&self) -> bool {
        self.steering.boost()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
