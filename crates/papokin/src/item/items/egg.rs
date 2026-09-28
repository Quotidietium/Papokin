use std::sync::Arc;

use crate::entity::Entity;
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::entity::projectile::egg::EggEntity;
use crate::item::{ItemBehaviour, ItemMetadata};
use papokin_data::entity::EntityType;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_data::sound::Sound;

pub struct EggItem;

impl ItemMetadata for EggItem {
    fn ids() -> Box<[u16]> {
        [Item::EGG.id, Item::BLUE_EGG.id, Item::BROWN_EGG.id].into()
    }
}

const POWER: f32 = 1.5;

impl ItemBehaviour for EggItem {
    fn normal_use(&self, _block: &Item, player: &Player) {
        let position = player.position();
        let world = player.world();
        world.play_sound(
            Sound::EntityEggThrow,
            papokin_data::sound::SoundCategory::Players,
            &position,
        );

        // 捕获手持物品堆并将其交给抛出的鸡蛋实体
        let item_stack: ItemStack = player.inventory.held_item();

        let entity = Entity::new(world.clone(), position, &EntityType::EGG);
        let egg = EggEntity::new_shot(entity, player.get_entity());

        // 传播物品堆，使客户端显示正确的变体
        egg.set_item_stack(item_stack);

        let (yaw, pitch) = player.rotation();
        egg.thrown.set_velocity_from(pitch, yaw, 0.0, POWER, 1.0);
        world.spawn_entity(Arc::new(egg));

        // 消耗物品：读取-校验-扣减-写回在写锁内原子完成
        let gamemode = player.gamemode.load();
        let consumed = player
            .inventory
            .update_held(papokin_util::Hand::Right, |mut s| {
                let ok = !s.is_empty() && Self::ids().contains(&s.item.id);
                if ok {
                    s.decrement_unless_creative(gamemode, 1);
                }
                (s, ok)
            });

        if !consumed {
            player
                .inventory
                .update_held(papokin_util::Hand::Left, |mut s| {
                    let ok = !s.is_empty() && Self::ids().contains(&s.item.id);
                    if ok {
                        s.decrement_unless_creative(gamemode, 1);
                    }
                    (s, ok)
                });
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
