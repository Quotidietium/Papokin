use papokin_data::BlockState;
use papokin_data::tag::Taggable;
use papokin_data::{Block, fluid::Fluid, tag};

/// 检查特定方块能否被流体替换（基于方块属性）
#[must_use]
pub fn can_be_replaced(block_state: &BlockState, block: &Block, fluid: &Fluid) -> bool {
    // 含水方块不应被水替换
    if block.is_waterlogged(block_state.id) {
        return false;
    }

    // 流体逻辑
    if let Some(other_fluid) = Fluid::from_state_id(block_state.id) {
        if !fluid.matches_type(other_fluid) {
            return true;
        }
        // 如果当前流体是下落水源则替换它
        if other_fluid.is_source(block_state.id) && other_fluid.is_falling(block_state.id) {
            return true;
        }
    }

    let id = block.id;

    // 流体绝不应替换的方块
    if block.has_tag(&tag::Block::MINECRAFT_DOORS)
        || block.has_tag(&tag::Block::MINECRAFT_BEDS)
        || block.has_tag(&tag::Block::MINECRAFT_LEAVES)
        || block.has_tag(&tag::Block::MINECRAFT_PRESSURE_PLATES)
        || block.has_tag(&tag::Block::C_CLUSTERS)
        || block.has_tag(&tag::Block::MINECRAFT_WALL_CORALS)
        || block.has_tag(&tag::Block::MINECRAFT_SHULKER_BOXES)
        || block.has_tag(&tag::Block::MINECRAFT_PORTALS)
        || id == Block::BELL.id
        || id == Block::BIG_DRIPLEAF.id
        || id == Block::BIG_DRIPLEAF_STEM.id
        || id == Block::SMALL_DRIPLEAF.id
        || id == Block::CAKE.id
        || id == Block::CONDUIT.id
        || id == Block::CAMPFIRE.id
        || id == Block::DRAGON_EGG.id
        || id == Block::KELP.id
        || id == Block::KELP_PLANT.id
        || id == Block::SEAGRASS.id
        || id == Block::TALL_SEAGRASS.id
        || id == Block::LADDER.id
        || id == Block::POINTED_DRIPSTONE.id
        || id == Block::SCAFFOLDING.id
    {
        return false;
    }

    // 只替换空气、明确可替换的方块或地毯
    block_state.replaceable()
        || id == Block::AIR.id
        || block.has_tag(&tag::Block::MINECRAFT_WOOL_CARPETS)
        // 仅在未通过上述检查时才使用 PistonBehavior::Destroy
        || block_state.piston_behavior == papokin_data::block_state::PistonBehavior::Destroy
}
