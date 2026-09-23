use std::any::Any;
use std::sync::Arc;

use crate::entity::projectile::experience_bottle::ExperienceBottleEntity;
use crate::entity::{Entity, EntityBase, player::Player};
use crate::item::{ItemBehaviour, ItemMetadata};
use papokin_data::entity::EntityType;
use papokin_data::item::Item;
use papokin_data::sound::{Sound, SoundCategory};

const POWER: f32 = 0.7;

pub struct ExperienceBottleItem;

impl ItemMetadata for ExperienceBottleItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::EXPERIENCE_BOTTLE.id])
    }
}

impl ItemBehaviour for ExperienceBottleItem {
    fn normal_use(&self, _item: &Item, player: &Player) {
        // 原版行为：掷出一枚可投掷的经验瓶实体，落地碎裂后释放经验；
        // 此前实现是立即在眼前生成经验球，飞行与碎裂体验全部缺失。
        let world = player.world();
        let position = player.position();
        world.play_sound(
            Sound::EntityExperienceBottleThrow,
            SoundCategory::Players,
            &position,
        );
        let entity = Entity::new(world.clone(), position, &EntityType::EXPERIENCE_BOTTLE);
        let bottle = ExperienceBottleEntity::new_shot(entity, player.get_entity());
        let (yaw, pitch) = player.rotation();
        bottle.thrown.set_velocity_from(pitch, yaw, 0.0, POWER, 1.0);
        world.spawn_entity(Arc::new(bottle));

        let mut held = player.inventory().held_item();
        held.decrement_unless_creative(player.gamemode.load(), 1);
        player.inventory().set_held_item(held);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
