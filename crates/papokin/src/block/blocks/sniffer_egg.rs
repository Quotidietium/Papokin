use papokin_data::block_properties::SnifferEggLikeProperties;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_data::sound::{Sound, SoundCategory};
use papokin_data::{BlockId, BlockState, BlockStateId};
use papokin_macros::pumpkin_block;
use papokin_world::tick::TickPriority;
use papokin_world::world::BlockFlags;

use crate::block::{
    BlockBehaviour, BrokenArgs, OnPlaceArgs, OnScheduledTickArgs, PathComputationType, PlacedArgs,
};

#[pumpkin_block("minecraft:sniffer_egg")]
pub struct SnifferEggBlock;

impl SnifferEggBlock {
    fn is_on_moss(
        world: &dyn papokin_world::world::BlockAccessor,
        pos: &papokin_util::math::position::BlockPos,
    ) -> bool {
        let below_pos = pos.down();
        let state = world.get_block_state(&below_pos);
        let block = BlockId::from_state_id(state.id);
        block == BlockId::MOSS_BLOCK
    }

    const fn get_hatch_delay(on_moss: bool) -> u8 {
        if on_moss { 100 } else { 200 }
    }
}

impl BlockBehaviour for SnifferEggBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let props = SnifferEggLikeProperties::default(args.block);
        props.to_state_id(args.block)
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        {
            args.world.play_sound(
                Sound::BlockSnifferEggPlop,
                SoundCategory::Blocks,
                &args.position.to_f64(),
            );

            let on_moss = Self::is_on_moss(args.world.as_ref(), args.position);
            let delay = Self::get_hatch_delay(on_moss);
            args.world
                .schedule_block_tick(args.block, *args.position, delay, TickPriority::Normal);
        }
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state_id = args.world.get_block_state_id(args.position);
        let mut props = SnifferEggLikeProperties::from_state_id(state_id);

        if props.hatch < 2 {
            props.hatch += 1;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );

            args.world.play_sound(
                Sound::BlockSnifferEggCrack,
                SoundCategory::Blocks,
                &args.position.to_f64(),
            );

            let on_moss = Self::is_on_moss(args.world.as_ref(), args.position);
            let delay = Self::get_hatch_delay(on_moss);
            args.world
                .schedule_block_tick(args.block, *args.position, delay, TickPriority::Normal);
        } else {
            args.world
                .break_block(args.position, None, BlockFlags::SKIP_DROPS);

            args.world.play_sound(
                Sound::BlockSnifferEggHatch,
                SoundCategory::Blocks,
                &args.position.to_f64(),
            );
        }
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        {
            args.world.play_sound(
                Sound::BlockSnifferEggCrack,
                SoundCategory::Blocks,
                &args.position.to_f64(),
            );
            args.world
                .drop_stack(args.position, ItemStack::new(1, &Item::SNIFFER_EGG));
        }
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
