use std::any::Any;

use crate::block::registry::BlockActionResult;
use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use crate::server::Server;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_data::world::WorldEvent;
use papokin_data::{Block, BlockDirection};
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;

pub struct BoneMealItem;

impl ItemMetadata for BoneMealItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::BONE_MEAL.id])
    }
}

impl ItemBehaviour for BoneMealItem {
    #[allow(clippy::too_many_lines)]
    fn use_on_block(
        &self,
        item: &mut ItemStack,
        player: &Player,
        location: BlockPos,
        _face: BlockDirection,
        _cursor_pos: Vector3<f32>,
        block: &Block,
        server: &Server,
    ) -> BlockActionResult {
        let world = player.world();
        let state_id = world.get_block_state_id(&location);
        if server
            .block_registry
            .bone_meal(block, &world, &location, state_id)
        {
            world.sync_world_event(WorldEvent::ParticlesAndSoundPlantGrowth, location, 15);
            item.decrement_unless_creative(player.gamemode.load(), 1);
            BlockActionResult::Success
        } else {
            BlockActionResult::Pass
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
