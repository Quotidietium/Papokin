use crate::block::registry::BlockActionResult;
use crate::block::{BlockBehaviour, GetScreenHandlerFactoryArgs, NormalUseArgs};

use papokin_data::translation;
use papokin_inventory::player::player_inventory::PlayerInventory;
use papokin_inventory::screen_handler::{
    InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler,
};
use papokin_inventory::smithing_table_screen_handler::SmithingTableScreenHandler;
use papokin_macros::pumpkin_block;
use papokin_util::text::TextComponent;
use std::sync::Arc;
use std::sync::Mutex;

#[pumpkin_block("minecraft:smithing_table")]
pub struct SmithingTableBlock;

impl BlockBehaviour for SmithingTableBlock {
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
                papokin_data::statistic::CustomStatistic::InteractWithSmithingTable as i32,
                1,
            );
            args.player
                .open_handled_screen(factory.as_ref(), Some(*args.position));
        }

        BlockActionResult::Success
    }

    fn get_screen_handler_factory(
        &self,
        args: GetScreenHandlerFactoryArgs<'_>,
    ) -> Option<Box<dyn ScreenHandlerFactory>> {
        let recipe_manager = args.server.recipe_manager.clone();
        Some(Box::new(SmithingTableScreenFactory(recipe_manager)))
    }
}

struct SmithingTableScreenFactory(Arc<crate::server::RecipeManager>);

impl ScreenHandlerFactory for SmithingTableScreenFactory {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        _player: &dyn InventoryPlayer,
    ) -> Option<SharedScreenHandler> {
        let handler: SharedScreenHandler = Arc::new(Mutex::new(
            SmithingTableScreenHandler::with_dynamic_recipe_provider(
                sync_id,
                player_inventory,
                Some(self.0.clone()),
            ),
        ));
        Some(handler)
    }

    fn get_display_name(&self) -> TextComponent {
        TextComponent::translate(translation::java::CONTAINER_UPGRADE, [])
    }
}
