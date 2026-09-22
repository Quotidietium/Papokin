use std::sync::Arc;

use papokin_data::Block;
use papokin_data::BlockDirection;
use papokin_data::BlockStateId;
use papokin_data::HorizontalFacingExt;
use papokin_data::block_properties::AttachFace;
use papokin_data::sound::{Sound, SoundCategory};
use papokin_macros::pumpkin_block_from_tag;
use papokin_util::math::position::BlockPos;
use papokin_world::tick::TickPriority;
use papokin_world::world::BlockFlags;

type ButtonLikeProperties = papokin_data::block_properties::LeverLikeProperties;

use crate::block::CanPlaceAtArgs;
use crate::block::EmitsRedstonePowerArgs;
use crate::block::GetRedstonePowerArgs;
use crate::block::GetStateForNeighborUpdateArgs;
use crate::block::OnPlaceArgs;
use crate::block::OnScheduledTickArgs;
use crate::block::OnStateReplacedArgs;
use crate::block::blocks::abstract_wall_mounting::WallMountedBlock;
use crate::block::blocks::redstone::lever::LeverLikePropertiesExt;
use crate::block::registry::BlockActionResult;
use crate::block::{BlockBehaviour, NormalUseArgs};
use crate::world::World;

fn get_sound(block: &Block, on: bool) -> Sound {
    if block == &Block::STONE_BUTTON || block == &Block::POLISHED_BLACKSTONE_BUTTON {
        if on {
            Sound::BlockStoneButtonClickOn
        } else {
            Sound::BlockStoneButtonClickOff
        }
    } else if on {
        Sound::BlockWoodenButtonClickOn
    } else {
        Sound::BlockWoodenButtonClickOff
    }
}

/// 按下按钮（除非它已被按下）。返回它是否
/// 被按下，因此调用方可以像原版那样区分这两种情况，
/// `ButtonBlock::useWithoutItem` 所做的事。
fn click_button(world: &Arc<World>, block_pos: &BlockPos) -> bool {
    let (block, state) = world.get_block_and_state_id(block_pos);

    let mut button_props = ButtonLikeProperties::from_state_id(state);
    if button_props.powered {
        return false;
    }

    button_props.powered = true;
    world.set_block_state(
        block_pos,
        button_props.to_state_id(block),
        BlockFlags::NOTIFY_ALL,
    );
    let delay = if block == &Block::STONE_BUTTON {
        20
    } else {
        30
    };
    world.schedule_block_tick(block, *block_pos, delay, TickPriority::Normal);
    ButtonBlock::update_neighbors(world, block_pos, button_props);
    world.play_block_sound(get_sound(block, true), SoundCategory::Blocks, *block_pos);

    true
}

#[pumpkin_block_from_tag("minecraft:buttons")]
pub struct ButtonBlock;

impl BlockBehaviour for ButtonBlock {
    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        if click_button(args.world, args.position) {
            BlockActionResult::Success
        } else {
            BlockActionResult::Consume
        }
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        let mut props = ButtonLikeProperties::from_state_id(state.id);
        if props.powered {
            props.powered = false;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );
            Self::update_neighbors(args.world, args.position, props);
            args.world.play_block_sound(
                get_sound(args.block, false),
                SoundCategory::Blocks,
                *args.position,
            );
        }
    }

    fn emits_redstone_power(&self, _args: EmitsRedstonePowerArgs<'_>) -> bool {
        true
    }

    fn get_weak_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        let button_props = ButtonLikeProperties::from_state_id(args.state.id);
        if button_props.powered { 15 } else { 0 }
    }

    fn get_strong_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        let button_props = ButtonLikeProperties::from_state_id(args.state.id);
        if button_props.powered && button_props.get_direction() == args.direction {
            15
        } else {
            0
        }
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        if !args.moved {
            let button_props = ButtonLikeProperties::from_state_id(args.old_state_id);
            if button_props.powered {
                Self::update_neighbors(args.world, args.position, button_props);
            }
        }
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = ButtonLikeProperties::from_state_id(args.block.default_state.id);
        (props.face, props.facing) =
            WallMountedBlock::get_placement_face(self, args.player, args.direction);

        props.to_state_id(args.block)
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        // 使用给定的方向；若缺失，则回退到当前状态的方向
        let direction = args
            .direction
            .unwrap_or_else(|| self.get_direction(args.state.id, args.block));

        WallMountedBlock::can_place_at(self, args.block_accessor, args.position, direction)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        WallMountedBlock::get_state_for_neighbor_update(self, args)
    }
}

impl WallMountedBlock for ButtonBlock {
    fn get_direction(&self, state_id: BlockStateId, _block: &Block) -> BlockDirection {
        let props = ButtonLikeProperties::from_state_id(state_id);
        match props.face {
            AttachFace::Floor => BlockDirection::Up,
            AttachFace::Ceiling => BlockDirection::Down,
            AttachFace::Wall => props.facing.to_block_direction(),
        }
    }
}

impl ButtonBlock {
    fn update_neighbors(world: &Arc<World>, block_pos: &BlockPos, props: ButtonLikeProperties) {
        let direction = props.get_direction().opposite();
        world.update_neighbors(block_pos, None);
        world.update_neighbors(&block_pos.offset(direction.to_offset()), None);
    }
}
