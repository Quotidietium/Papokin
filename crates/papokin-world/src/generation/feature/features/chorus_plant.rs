use papokin_data::{
    Block, BlockDirection,
    block_properties::{
        BrownMushroomBlockLikeProperties, ChorusFlowerLikeProperties, HorizontalFacing,
    },
    tag,
};
use papokin_util::{
    math::position::BlockPos,
    random::{RandomGenerator, RandomImpl},
};

use crate::generation::proto_chunk::GenerationCache;

pub struct ChorusPlantFeature;

impl ChorusPlantFeature {
    pub fn generate<T: GenerationCache>(
        chunk: &mut T,
        _min_y: i8,
        _height: u16,
        _feature: papokin_data::placed_feature::PlacedFeature,
        random: &mut RandomGenerator,
        pos: BlockPos,
    ) -> bool {
        let origin = pos.0;

        // 必须是放置在 supports_chorus_plant 方块（end_stone）上的空方块
        if !chunk.is_air(&origin) {
            return false;
        }
        let below = pos.down().0;
        let below_id = GenerationCache::get_block_state(chunk, &below).to_block_id();
        if !below_id.has_tag(tag::Block::MINECRAFT_SUPPORTS_CHORUS_PLANT) {
            return false;
        }

        // 放置带连接的基部紫颂植株，然后生长树木
        set_chorus_plant(chunk, &pos);
        grow_tree_recursive(chunk, random, &pos, &pos, 8, 0);
        true
    }
}

///若 `pos` 的四个水平相邻方块均为空气，则返回 `true`，
/// 可选择忽略某个方向（`ignore`）。
fn all_neighbors_empty<T: GenerationCache>(
    chunk: &T,
    pos: &BlockPos,
    ignore: Option<HorizontalFacing>,
) -> bool {
    for dir in BlockDirection::horizontal() {
        if Some(dir) == ignore {
            continue;
        }
        if !chunk.is_air(&pos.offset(dir.to_offset()).0) {
            return false;
        }
    }
    true
}

/// 计算 `pos` 处紫颂植株的连接状态并写入区块。
fn set_chorus_plant<T: GenerationCache>(chunk: &mut T, pos: &BlockPos) {
    let down_id = GenerationCache::get_block_state(chunk, &pos.down().0).to_block_id();
    let up_id = GenerationCache::get_block_state(chunk, &pos.up().0).to_block_id();
    let north_id =
        GenerationCache::get_block_state(chunk, &pos.offset(BlockDirection::North.to_offset()).0)
            .to_block_id();
    let east_id =
        GenerationCache::get_block_state(chunk, &pos.offset(BlockDirection::East.to_offset()).0)
            .to_block_id();
    let south_id =
        GenerationCache::get_block_state(chunk, &pos.offset(BlockDirection::South.to_offset()).0)
            .to_block_id();
    let west_id =
        GenerationCache::get_block_state(chunk, &pos.offset(BlockDirection::West.to_offset()).0)
            .to_block_id();

    let plant_id = Block::CHORUS_PLANT.id;
    let flower_id = Block::CHORUS_FLOWER.id;
    let supports = tag::Block::MINECRAFT_SUPPORTS_CHORUS_PLANT;

    let props = BrownMushroomBlockLikeProperties {
        down: down_id == plant_id || down_id == flower_id || down_id.has_tag(supports),
        up: up_id == plant_id || up_id == flower_id,
        north: north_id == plant_id || north_id == flower_id,
        east: east_id == plant_id || east_id == flower_id,
        south: south_id == plant_id || south_id == flower_id,
        west: west_id == plant_id || west_id == flower_id,
    };

    let state_id = props.to_state_id(&Block::CHORUS_PLANT);
    chunk.set_block_state(&pos.0, papokin_data::BlockState::from_id(state_id));
}

/// 在 `pos` 处放置一朵枯萎的紫颂花（age 5）。
fn place_dead_flower<T: GenerationCache>(chunk: &mut T, pos: &BlockPos) {
    let props = ChorusFlowerLikeProperties { age: 5 };
    let state_id = props.to_state_id(&Block::CHORUS_FLOWER);
    chunk.set_block_state(&pos.0, papokin_data::BlockState::from_id(state_id));
}

/// 递归地使紫颂植株向上和横向生长。
fn grow_tree_recursive<T: GenerationCache>(
    chunk: &mut T,
    random: &mut RandomGenerator,
    current: &BlockPos,
    start_pos: &BlockPos,
    max_horizontal_spread: i32,
    depth: i32,
) {
    let mut height = random.next_bounded_i32(4) + 1;
    if depth == 0 {
        height += 1;
    }

    // 垂直生长：在当前位置上方放置 `height` 个紫颂植株方块
    for i in 0..height {
        let target = current.up_height(i + 1);
        if !all_neighbors_empty(chunk, &target, None) {
            return;
        }
        set_chorus_plant(chunk, &target);
        // 重新烘焙下方的方块，使其获得 UP 连接
        set_chorus_plant(chunk, &target.down());
    }

    let top = current.up_height(height);
    let mut placed_stem = false;

    if depth < 4 {
        let mut stems = random.next_bounded_i32(4);
        if depth == 0 {
            stems += 1;
        }

        for _ in 0..stems {
            // 随机选择一个水平方向
            let dir = BlockDirection::random_horizontal(random);
            let target = top.offset(dir.to_offset());
            let target_below = target.down();

            let dx = (target.0.x - start_pos.0.x).abs();
            let dz = (target.0.z - start_pos.0.z).abs();

            if dx < max_horizontal_spread
                && dz < max_horizontal_spread
                && chunk.is_air(&target.0)
                && chunk.is_air(&target_below.0)
                && all_neighbors_empty(chunk, &target, Some(dir.opposite()))
            {
                placed_stem = true;
                set_chorus_plant(chunk, &target);
                // 重新烘焙茎顶端，使其获得朝向新分支的连接面。
                set_chorus_plant(chunk, &top);
                grow_tree_recursive(
                    chunk,
                    random,
                    &target,
                    start_pos,
                    max_horizontal_spread,
                    depth + 1,
                );
            }
        }
    }

    if !placed_stem {
        // 用凋亡的紫颂花封住茎端
        place_dead_flower(chunk, &top);
    }
}
