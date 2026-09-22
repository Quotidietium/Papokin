use crate::block::blocks::fire::FireBlockBase;
use crate::block::blocks::fire::fire::FireBlock;
use crate::world::World;
use papokin_data::fluid::Fluid;
use papokin_data::tag::Taggable;
use papokin_data::{Block, BlockStateId, tag};
use papokin_util::math::position::BlockPos;
use std::sync::Arc;

pub struct Ignition;

impl Ignition {
    /// 若 `location` 处的 `block` 本身可被点燃（营火、蜡烛、蜡烛蛋糕等），则将其点亮
    /// 蛋糕），否则在 `fire_pos` 处放置火方块。
    pub fn ignite_block<F>(
        ignite_logic: F,
        world: &Arc<World>,
        location: BlockPos,
        fire_pos: BlockPos,
        block: &Block,
    ) -> bool
    where
        F: FnOnce(Arc<World>, BlockPos, BlockStateId),
    {
        if *world.get_fluid(&location) != Fluid::EMPTY {
            return false;
        }
        let fire_block = FireBlockBase::get_fire_type(world, &fire_pos);

        let state_id = world.get_block_state_id(&location);

        if let Some(new_state_id) = can_be_lit(block, state_id) {
            ignite_logic(world.clone(), location, new_state_id);
            return true;
        }

        let state_id = FireBlock.get_state_for_position(world, &fire_block, &fire_pos);
        if FireBlockBase::can_place_at(world, &fire_pos) {
            ignite_logic(world.clone(), fire_pos, state_id);
            return true;
        }

        false
    }
}

fn can_be_lit(block: &Block, state_id: BlockStateId) -> Option<BlockStateId> {
    // 原版对营火、蜡烛和蜡烛蛋糕只点燃被点击的方块本身。
    // 见 `CampfireBlock::canLight`、`CandleBlock::canLight` 和 `CandleCakeBlock::canLight`。
    // 其余仅携带 `lit` 属性的方块（熔炉、红石灯、铜
    // 鳞茎等）必须转而落到放置火焰方块的分支。
    if !block.has_tag(&tag::Block::MINECRAFT_CAMPFIRES)
        && !block.has_tag(&tag::Block::MINECRAFT_CANDLES)
        && !block.has_tag(&tag::Block::MINECRAFT_CANDLE_CAKES)
    {
        return None;
    }

    let mut props = block.properties(state_id)?.to_props();

    let (_, value) = props.iter_mut().find(|(k, _)| *k == "lit")?;
    if *value == "true" {
        return None;
    }

    *value = "true";
    Some(block.from_properties(&props).to_state_id(block))
}
