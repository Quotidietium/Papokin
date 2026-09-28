use crate::block::registry::BlockActionResult;
use std::sync::Arc;

use crate::entity::player::Player;
use crate::entity::projectile::firework_rocket::FireworkRocketEntity;
use crate::entity::{Entity, EntityBase};
use crate::item::{ItemBehaviour, ItemMetadata};
use crate::server::Server;
use papokin_data::Block;
use papokin_data::BlockDirection;
use papokin_data::entity::EntityType;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;

pub struct FireworkRocketItem;

impl ItemMetadata for FireworkRocketItem {
    fn ids() -> Box<[u16]> {
        [Item::FIREWORK_ROCKET.id].into()
    }
}

impl ItemBehaviour for FireworkRocketItem {
    fn use_on_block(
        &self,
        item: &mut ItemStack,
        player: &Player,
        location: BlockPos,
        _face: BlockDirection,
        cursor_pos: Vector3<f32>,
        _block: &Block,
        _server: &Server,
    ) -> BlockActionResult {
        let world = player.world();
        let entity = Entity::new(
            world.clone(),
            Vector3::new(
                f64::from(location.0.x) + f64::from(cursor_pos.x),
                f64::from(location.0.y) + f64::from(cursor_pos.y),
                f64::from(location.0.z) + f64::from(cursor_pos.z),
            ),
            &EntityType::FIREWORK_ROCKET,
        );
        let entity = FireworkRocketEntity::new(entity);
        world.spawn_entity(Arc::new(entity));
        item.decrement_unless_creative(player.gamemode.load(), 1);
        BlockActionResult::Success
    }

    fn normal_use(&self, _item: &Item, player: &Player) {
        if player.get_entity().is_fall_flying() {
            let world = player.world();
            let entity = Entity::new(
                world.clone(),
                player.get_entity().pos.load(),
                &EntityType::FIREWORK_ROCKET,
            );
            let entity = FireworkRocketEntity::new_shot(entity, player.get_entity());
            world.spawn_entity(Arc::new(entity));

            let mut held = player.inventory().held_item();
            let mut is_main = true;
            if held.is_empty() || held.item.id != Item::FIREWORK_ROCKET.id {
                held = player.inventory().off_hand_item();
                is_main = false;
                if held.is_empty() || held.item.id != Item::FIREWORK_ROCKET.id {
                    return;
                }
            }
            // 原子扣减：读取-校验-扣减-写回在写锁内完成，防止
            // 期间并入该槽位的物品被陈旧快照覆盖
            let hand = if is_main {
                papokin_util::Hand::Right
            } else {
                papokin_util::Hand::Left
            };
            player.inventory().update_held(hand, |mut s| {
                if !s.is_empty() && s.item.id == Item::FIREWORK_ROCKET.id {
                    s.decrement_unless_creative(player.gamemode.load(), 1);
                }
                (s, ())
            });
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
