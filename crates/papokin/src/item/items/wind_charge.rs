use std::sync::Arc;

use crate::entity::player::Player;
use papokin_data::entity::EntityType;
use papokin_data::item::Item;
use papokin_data::sound::Sound;

use crate::entity::Entity;
use crate::entity::EntityBase;
use crate::entity::projectile::ThrownItemEntity;
use crate::entity::projectile::wind_charge::{WIND_CHARGE_GRAVITY, WindChargeEntity};
use crate::item::{ItemBehaviour, ItemMetadata};

pub struct WindChargeItem;

impl ItemMetadata for WindChargeItem {
    fn ids() -> Box<[u16]> {
        [Item::WIND_CHARGE.id].into()
    }
}

const POWER: f32 = 1.5;

impl ItemBehaviour for WindChargeItem {
    fn normal_use(&self, _block: &Item, player: &Player) {
        let world = player.world();
        let position = player.position();

        world.play_sound(
            Sound::EntityWindChargeThrow,
            papokin_data::sound::SoundCategory::Neutral,
            &position,
        );

        let entity = Entity::new(world.clone(), position, &EntityType::WIND_CHARGE);

        let wind_charge = ThrownItemEntity::new(entity, player.get_entity(), WIND_CHARGE_GRAVITY);
        let (yaw, pitch) = player.rotation();
        wind_charge.set_velocity_from(pitch, yaw, 0.0, POWER, 1.0);

        world.spawn_entity(Arc::new(WindChargeEntity::new_normal(wind_charge)));

        let gamemode = player.gamemode.load();
        let consumed = player
            .inventory
            .update_held(papokin_util::Hand::Right, |mut s| {
                let ok = !s.is_empty() && s.item.id == Item::WIND_CHARGE.id;
                if ok {
                    s.decrement_unless_creative(gamemode, 1);
                }
                (s, ok)
            });

        if !consumed {
            player
                .inventory
                .update_held(papokin_util::Hand::Left, |mut s| {
                    let ok = !s.is_empty() && s.item.id == Item::WIND_CHARGE.id;
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
