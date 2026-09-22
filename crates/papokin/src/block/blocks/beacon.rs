use std::sync::Arc;
use std::sync::Mutex;

use papokin_data::translation;
use papokin_inventory::Inventory;
use papokin_inventory::beacon_screen_handler::create_beacon_handler;
use papokin_inventory::player::player_inventory::PlayerInventory;
use papokin_inventory::screen_handler::{
    InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler,
};
use papokin_macros::pumpkin_block;
use papokin_util::text::TextComponent;

use crate::block::registry::BlockActionResult;
use crate::block::{BlockBehaviour, GetScreenHandlerFactoryArgs, NormalUseArgs};

// 像 ChestScreenFactory 一样创建工厂
struct BeaconScreenFactory(Arc<dyn Inventory>);

impl ScreenHandlerFactory for BeaconScreenFactory {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        _player: &dyn InventoryPlayer,
    ) -> Option<SharedScreenHandler> {
        let concrete_handler = create_beacon_handler(sync_id, player_inventory, self.0.clone());
        let concrete_arc = Arc::new(Mutex::new(concrete_handler));

        Some(concrete_arc as SharedScreenHandler)
    }

    fn get_display_name(&self) -> TextComponent {
        papokin_macros::translate!(translation::java::CONTAINER_BEACON)
    }
}

#[pumpkin_block("minecraft:beacon")]
pub struct BeaconBlock;

impl BlockBehaviour for BeaconBlock {
    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        self.get_screen_handler_factory(GetScreenHandlerFactoryArgs {
            server: args.server,
            world: args.world,
            block: args.block,
            position: args.position,
            player: args.player,
        })
        .map_or(BlockActionResult::Fail, |factory| {
            args.player.increment_stat(
                papokin_data::statistic::StatisticCategory::Custom,
                papokin_data::statistic::CustomStatistic::InteractWithBeacon as i32,
                1,
            );

            // 使用工厂打开界面
            args.player
                .open_handled_screen(factory.as_ref(), Some(*args.position));

            BlockActionResult::Success
        })
    }

    fn get_screen_handler_factory(
        &self,
        args: GetScreenHandlerFactoryArgs<'_>,
    ) -> Option<Box<dyn ScreenHandlerFactory>> {
        let block_entity = args.world.get_block_entity(args.position)?;
        let inventory = block_entity.get_inventory()?;
        Some(Box::new(BeaconScreenFactory(inventory)))
    }
}
