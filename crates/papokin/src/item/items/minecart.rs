use std::sync::Arc;

use crate::block::registry::BlockActionResult;
use crate::entity::Entity;
use crate::entity::player::Player;
use crate::entity::vehicle::minecart::MinecartEntity;
use crate::item::{ItemBehaviour, ItemMetadata};
use crate::server::Server;
use papokin_data::BlockDirection;
use papokin_data::block_properties::{
    BlockProperties, PoweredRailLikeProperties, RailLikeProperties,
};
use papokin_data::entity::EntityType;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_data::tag::Taggable;
use papokin_data::{Block, tag};
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;

pub struct MinecartItem;

impl MinecartItem {
    pub(crate) fn item_to_entity(item: &Item) -> &'static EntityType {
        match item.id {
            val if val == Item::MINECART.id => &EntityType::MINECART,
            val if val == Item::TNT_MINECART.id => &EntityType::TNT_MINECART,
            val if val == Item::CHEST_MINECART.id => &EntityType::CHEST_MINECART,
            val if val == Item::HOPPER_MINECART.id => &EntityType::HOPPER_MINECART,
            val if val == Item::FURNACE_MINECART.id => &EntityType::FURNACE_MINECART,
            val if val == Item::COMMAND_BLOCK_MINECART.id => &EntityType::COMMAND_BLOCK_MINECART,
            _ => {
                tracing::error!("未知的矿车物品 ID：{}", item.id);
                &EntityType::MINECART
            }
        }
    }
}

impl ItemMetadata for MinecartItem {
    fn ids() -> Box<[u16]> {
        [
            Item::MINECART.id,
            Item::TNT_MINECART.id,
            Item::CHEST_MINECART.id,
            Item::HOPPER_MINECART.id,
            Item::FURNACE_MINECART.id,
            Item::COMMAND_BLOCK_MINECART.id,
        ]
        .into()
    }
}

impl ItemBehaviour for MinecartItem {
    fn use_on_block(
        &self,
        item: &mut ItemStack,
        player: &Player,
        location: BlockPos,
        _face: BlockDirection,
        _cursor_pos: Vector3<f32>,
        block: &Block,
        _server: &Server,
    ) -> BlockActionResult {
        let world = player.world();

        if !block.has_tag(&tag::Block::MINECRAFT_RAILS) {
            return BlockActionResult::Fail;
        }
        let state_id = world.get_block_state_id(&location);
        let is_ascending = if PoweredRailLikeProperties::handles_block_id(block.id) {
            PoweredRailLikeProperties::from_state_id(state_id)
                .shape
                .is_ascending()
        } else {
            RailLikeProperties::from_state_id(state_id)
                .shape
                .is_ascending()
        };
        let height = if is_ascending { 0.5 } else { 0.0 };
        let entity_type = Self::item_to_entity(item.item);
        let pos = location.to_f64();
        let entity = Entity::new(
            world.clone(),
            Vector3::new(pos.x + 0.5, pos.y + 0.0625 + height, pos.z + 0.5),
            entity_type,
        );
        let minecart_entity = Arc::new(MinecartEntity::new(entity));
        world.spawn_entity(minecart_entity);
        item.decrement_unless_creative(player.gamemode.load(), 1);
        BlockActionResult::Success
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
