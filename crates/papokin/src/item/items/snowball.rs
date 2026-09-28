use std::sync::Arc;

use crate::entity::Entity;
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::entity::projectile::snowball::SnowballEntity;
use crate::item::{ItemBehaviour, ItemMetadata};
use papokin_data::entity::EntityType;
use papokin_data::item::Item;
use papokin_data::sound::Sound;

pub struct SnowBallItem;

impl ItemMetadata for SnowBallItem {
    fn ids() -> Box<[u16]> {
        [Item::SNOWBALL.id].into()
    }
}

const POWER: f32 = 1.5;

impl ItemBehaviour for SnowBallItem {
    fn normal_use(&self, _block: &Item, player: &Player) {
        let position = player.position();
        let world = player.world();
        world.play_sound(
            Sound::EntitySnowballThrow,
            papokin_data::sound::SoundCategory::Neutral,
            &position,
        );
        let entity = Entity::new(world.clone(), position, &EntityType::SNOWBALL);
        let snowball = SnowballEntity::new_shot(entity, player.get_entity());
        let (yaw, pitch) = player.rotation();
        snowball
            .thrown
            .set_velocity_from(pitch, yaw, 0.0, POWER, 1.0);
        world.spawn_entity(Arc::new(snowball));

        // 消耗物品：读取-校验-扣减-写回在写锁内原子完成
        let gamemode = player.gamemode.load();
        let consumed = player
            .inventory
            .update_held(papokin_util::Hand::Right, |mut s| {
                let ok = !s.is_empty() && s.item.id == Item::SNOWBALL.id;
                if ok {
                    s.decrement_unless_creative(gamemode, 1);
                }
                (s, ok)
            });

        if !consumed {
            player
                .inventory
                .update_held(papokin_util::Hand::Left, |mut s| {
                    let ok = !s.is_empty() && s.item.id == Item::SNOWBALL.id;
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
