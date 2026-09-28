use rand::{RngExt, rng};

use std::sync::Arc;

use crate::entity::Entity;
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::entity::projectile::ender_pearl::EnderPearlEntity;
use crate::item::{ItemBehaviour, ItemMetadata};
use papokin_data::entity::EntityType;
use papokin_data::item::Item;
use papokin_data::sound::Sound;

pub struct EnderPearlItem;

impl ItemMetadata for EnderPearlItem {
    fn ids() -> Box<[u16]> {
        [Item::ENDER_PEARL.id].into()
    }
}

const ROLL: f32 = 0.0;
const POWER: f32 = 1.5;
const DIVERGENCE: f32 = 1.0;
const THROW_SOUND_VOLUME: f32 = 0.5;

impl ItemBehaviour for EnderPearlItem {
    fn normal_use(&self, _item: &Item, player: &Player) {
        let position = player.position();
        let world = player.world();
        world.play_sound_fine(
            Sound::EntityEnderPearlThrow,
            papokin_data::sound::SoundCategory::Neutral,
            &position,
            THROW_SOUND_VOLUME,
            0.4 / (rng().random::<f32>() * 0.4 + 0.8),
        );

        let entity = Entity::new(world.clone(), position, &EntityType::ENDER_PEARL);
        let pearl = EnderPearlEntity::new_shot(entity, player.get_entity());
        let (yaw, pitch) = player.rotation();
        pearl
            .thrown
            .set_velocity_from(pitch, yaw, ROLL, POWER, DIVERGENCE);
        world.spawn_entity(Arc::new(pearl));

        // 消耗物品：读取-校验-扣减-写回在写锁内原子完成
        let gamemode = player.gamemode.load();
        let consumed = player
            .inventory
            .update_held(papokin_util::Hand::Right, |mut s| {
                let ok = !s.is_empty() && s.item.id == Item::ENDER_PEARL.id;
                if ok {
                    s.decrement_unless_creative(gamemode, 1);
                }
                (s, ok)
            });

        if !consumed {
            player
                .inventory
                .update_held(papokin_util::Hand::Left, |mut s| {
                    let ok = !s.is_empty() && s.item.id == Item::ENDER_PEARL.id;
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
