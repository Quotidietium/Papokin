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
        entity.set_item_stack(item.clone());
        world.spawn_entity(Arc::new(entity));
        item.decrement_unless_creative(player.gamemode.load(), 1);
        BlockActionResult::Success
    }

    fn normal_use_in_hand(
        &self,
        stack: &ItemStack,
        player: &Player,
        hand: papokin_util::Hand,
        _yaw: f32,
        _pitch: f32,
    ) {
        if !player.get_entity().is_fall_flying() {
            return;
        }
        // 使用实际操作的手；扣减成功后才生成，避免副手误扣主手和空手生成。
        let rocket_stack = player.inventory().update_held(hand, |mut current| {
            if current.is_empty() || !current.are_items_and_components_equal(stack) {
                return (current, None);
            }
            let rocket_stack = current.clone();
            current.decrement_unless_creative(player.gamemode.load(), 1);
            (current, Some(rocket_stack))
        });
        let Some(rocket_stack) = rocket_stack else {
            return;
        };
        let world = player.world();
        let entity = Entity::new(
            world.clone(),
            player.get_entity().pos.load(),
            &EntityType::FIREWORK_ROCKET,
        );
        let rocket = FireworkRocketEntity::new_shot(entity, player.get_entity());
        rocket.set_item_stack(rocket_stack);
        world.spawn_entity(Arc::new(rocket));
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
