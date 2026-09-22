use std::sync::Arc;
use std::sync::Mutex;

use crate::block::{GetComparatorOutputArgs, GetScreenHandlerFactoryArgs, OnPlaceArgs, PlacedArgs};
use crate::block::{
    registry::BlockActionResult,
    {BlockBehaviour, NormalUseArgs},
};

use crate::block::entities::barrel::BarrelBlockEntity;
use crate::entity::EntityBase;
use papokin_data::BlockStateId;
use papokin_data::block_properties::BarrelLikeProperties;
use papokin_data::translation;
use papokin_inventory::Inventory;
use papokin_inventory::generic_container_screen_handler::create_generic_9x3;
use papokin_inventory::player::player_inventory::PlayerInventory;
use papokin_inventory::screen_handler::{
    InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler,
};
use papokin_macros::pumpkin_block;
use papokin_util::text::TextComponent;

struct BarrelScreenFactory(Arc<dyn Inventory>);

impl ScreenHandlerFactory for BarrelScreenFactory {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        player: &dyn InventoryPlayer,
    ) -> Option<SharedScreenHandler> {
        let handler = create_generic_9x3(sync_id, player_inventory, self.0.clone(), player);
        let concrete_arc = Arc::new(Mutex::new(handler));

        Some(concrete_arc as SharedScreenHandler)
    }

    fn get_display_name(&self) -> TextComponent {
        papokin_macros::translate!(translation::java::CONTAINER_BARREL)
    }
}

#[pumpkin_block("minecraft:barrel")]
pub struct BarrelBlock;

impl BlockBehaviour for BarrelBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = BarrelLikeProperties::default(args.block);
        props.facing = args.player.get_entity().get_facing().opposite();
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
                papokin_data::statistic::CustomStatistic::OpenBarrel as i32,
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
        let block_entity = args.world.get_block_entity(args.position)?;
        let inventory = block_entity.get_inventory()?;
        Some(Box::new(BarrelScreenFactory(inventory)))
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        let barrel_block_entity = BarrelBlockEntity::new(*args.position);
        args.world.add_block_entity(Arc::new(barrel_block_entity));
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        crate::block::container_comparator_output(&args)
    }
}
