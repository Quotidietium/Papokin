use crate::block::registry::BlockActionResult;
use crate::block::{BlockBehaviour, GetScreenHandlerFactoryArgs, NormalUseArgs, OnPlaceArgs};
use crate::entity::EntityBase;

use papokin_data::block_properties::WallTorchLikeProperties;
use papokin_data::translation;
use papokin_data::{BlockStateId, FacingExt};
use papokin_inventory::loom_screen_handler::LoomScreenHandler;
use papokin_inventory::player::player_inventory::PlayerInventory;
use papokin_inventory::screen_handler::{
    InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler,
};
use papokin_macros::pumpkin_block;
use papokin_util::text::TextComponent;
use std::sync::Arc;
use std::sync::Mutex;

#[pumpkin_block("minecraft:loom")]
pub struct LoomBlock;

impl BlockBehaviour for LoomBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = WallTorchLikeProperties::default(args.block);
        if let Some(facing) = args
            .player
            .get_entity()
            .get_facing()
            .opposite()
            .to_horizontal_facing()
        {
            props.facing = facing;
        }
        props.to_state_id(args.block)
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        if let Some(factory) = self.get_screen_handler_factory(GetScreenHandlerFactoryArgs {
            server: args.server,
            world: args.world,
            block: args.block,
            position: args.position,
            player: args.player,
        }) {
            args.player.increment_stat(
                papokin_data::statistic::StatisticCategory::Custom,
                papokin_data::statistic::CustomStatistic::InteractWithLoom as i32,
                1,
            );
            args.player
                .open_handled_screen(factory.as_ref(), Some(*args.position));
        }

        BlockActionResult::Success
    }

    fn get_screen_handler_factory(
        &self,
        _args: GetScreenHandlerFactoryArgs<'_>,
    ) -> Option<Box<dyn ScreenHandlerFactory>> {
        Some(Box::new(LoomScreenFactory))
    }
}

struct LoomScreenFactory;

impl ScreenHandlerFactory for LoomScreenFactory {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        _player: &dyn InventoryPlayer,
    ) -> Option<SharedScreenHandler> {
        let handler: SharedScreenHandler = Arc::new(Mutex::new(LoomScreenHandler::new(
            sync_id,
            player_inventory,
        )));
        Some(handler)
    }

    fn get_display_name(&self) -> TextComponent {
        TextComponent::translate(translation::java::CONTAINER_LOOM, [])
    }
}
