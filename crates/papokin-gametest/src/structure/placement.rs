use papokin_data::Block;
use papokin_nbt::{NbtCompound, tag::NbtTag};
use papokin_util::math::{position::BlockPos, vector3::Vector3};
use papokin_world::world::BlockFlags;

use crate::error::GameTestResult;
use crate::model::GameTestRotation;
use crate::structure::template::GameTestStructureTemplate;
use crate::world::GameTestWorld;

const STRUCTURE_OFFSET: [i32; 3] = [0, 1, 1];

#[derive(Clone, Copy, Debug)]
pub struct GameTestPosition {
    x: i32,
    y: Option<i32>,
    z: i32,
}

impl GameTestPosition {
    #[must_use]
    pub const fn new(x: i32, y: Option<i32>, z: i32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Clone, Debug)]
pub struct TestStructureInstance {
    test_instance_pos: BlockPos,
    origin: BlockPos,
    source_size: [i32; 3],
    size: [i32; 3],
    rotation: GameTestRotation,
}

impl TestStructureInstance {
    #[must_use]
    pub const fn new(
        test_instance_pos: BlockPos,
        origin: BlockPos,
        source_size: [i32; 3],
        size: [i32; 3],
        rotation: GameTestRotation,
    ) -> Self {
        Self {
            test_instance_pos,
            origin,
            source_size,
            size,
            rotation,
        }
    }

    #[must_use]
    pub const fn test_instance_pos(&self) -> &BlockPos {
        &self.test_instance_pos
    }

    #[must_use]
    pub const fn origin(&self) -> &BlockPos {
        &self.origin
    }

    #[must_use]
    pub const fn size(&self) -> [i32; 3] {
        self.size
    }

    #[must_use]
    pub const fn rotation(&self) -> GameTestRotation {
        self.rotation
    }

    #[must_use]
    pub const fn transform(&self, relative: &BlockPos) -> BlockPos {
        let transformed = self.rotation.as_block_rotation().transform_pos(
            Vector3::new(relative.0.x, relative.0.y, relative.0.z),
            Vector3::new(
                self.source_size[0],
                self.source_size[1],
                self.source_size[2],
            ),
        );
        BlockPos::new(
            self.origin.0.x + transformed.x,
            self.origin.0.y + transformed.y,
            self.origin.0.z + transformed.z,
        )
    }
}

pub async fn place_structure(
    world: &dyn GameTestWorld,
    template: &GameTestStructureTemplate,
    test_id: &str,
    rotation: GameTestRotation,
    position: GameTestPosition,
    padding: i32,
) -> GameTestResult<TestStructureInstance> {
    place_structure_with_controller_rotation(
        world,
        template,
        test_id,
        rotation,
        GameTestRotation::None,
        position,
        padding,
    )
    .await
}

/// 使用有效的结构旋转放置测试。
///
/// 单独的控制器旋转存储在 `TestInstanceBlockEntity` 数据中，因为
/// 原版对 `/test run ... rotationSteps` 的做法。
pub async fn place_structure_with_controller_rotation(
    world: &dyn GameTestWorld,
    template: &GameTestStructureTemplate,
    test_id: &str,
    rotation: GameTestRotation,
    controller_rotation: GameTestRotation,
    position: GameTestPosition,
    padding: i32,
) -> GameTestResult<TestStructureInstance> {
    // TestInstanceBlockEntity.getStructurePos 按内边距偏移控制器，并
    // 然后是 STRUCTURE_OFFSET。控制器本身位于结构框之外。
    // 重跑时保留原始控制器 Y，而不是再次查询高度图。
    let test_y = match position.y {
        Some(test_y) => test_y,
        None => world.surface_height(position.x, position.z).await + 1,
    };
    let test_instance_pos = BlockPos::new(position.x, test_y, position.z);
    let origin = BlockPos::new(
        test_instance_pos.0.x + padding + STRUCTURE_OFFSET[0],
        test_instance_pos.0.y + padding + STRUCTURE_OFFSET[1],
        test_instance_pos.0.z + padding + STRUCTURE_OFFSET[2],
    );

    let source_size = template.size();
    let rotated_size = rotation.as_block_rotation().transform_size(Vector3::new(
        source_size[0],
        source_size[1],
        source_size[2],
    ));
    let size = [rotated_size.x, rotated_size.y, rotated_size.z];

    // 原版 TestInstanceBlockEntity::placeStructure 会清除非玩家实体
    // 从 getTestBounds() 获取的范围后再放置新的尝试。这一点尤其
    // 对重试至关重要：前一次尝试生成的生物绝不能存活
    // 合并进下一个，并继续针对替换后的结构执行刻逻辑。
    let test_min = BlockPos::new(
        origin.0.x - padding,
        origin.0.y - padding,
        origin.0.z - padding,
    );
    let test_max = BlockPos::new(
        origin.0.x + size[0] + padding,
        origin.0.y + size[1] + padding,
        origin.0.z + size[2] + padding,
    );
    world
        .clear_non_player_entities(&test_min, &test_max)
        .await?;

    clear_test_area(world, &origin, size, padding).await?;

    world
        .set_block_state(
            &test_instance_pos,
            Block::TEST_INSTANCE_BLOCK.default_state.id,
            BlockFlags::NOTIFY_ALL,
        )
        .await?;

    // TestInstanceBlockEntity.Data 仅存储额外的控制器旋转。
    // 客户端将其与来自同步数据的测试定义基础旋转合并
    // minecraft:test_instance 注册表。
    let mut data = NbtCompound::new();
    data.put_string("test", test_id.to_string());
    data.put("size", NbtTag::IntArray(source_size.to_vec()));
    data.put_string(
        "rotation",
        controller_rotation.serialized_name().to_string(),
    );
    data.put_bool("ignore_entities", false);
    data.put_string("status", "cleared".to_string());

    let mut test_instance_nbt = NbtCompound::new();
    test_instance_nbt.put_string("id", "minecraft:test_instance_block".to_string());
    test_instance_nbt.put_compound("data", data);
    world
        .set_block_entity_nbt(&test_instance_pos, &test_instance_nbt)
        .await?;

    // StructureTemplate.placeInWorld 会同时旋转相对坐标与方块
    // 状态，然后再在变换后的绝对位置处加载方块实体 NBT。
    let place_flags = BlockFlags::NOTIFY_LISTENERS
        | BlockFlags::MOVED
        | BlockFlags::SKIP_REDSTONE_WIRE_STATE_REPLACEMENT
        | BlockFlags::SKIP_BLOCK_ADDED_CALLBACK;

    for block in template.blocks() {
        let transformed = rotation.as_block_rotation().transform_pos(
            Vector3::new(block.position[0], block.position[1], block.position[2]),
            Vector3::new(source_size[0], source_size[1], source_size[2]),
        );
        let position = BlockPos::new(
            origin.0.x + transformed.x,
            origin.0.y + transformed.y,
            origin.0.z + transformed.z,
        );
        let state = world.rotate_block_state(block.state, rotation).await?;
        world.set_block_state(&position, state, place_flags).await?;

        if let Some(nbt) = &block.nbt {
            world.set_block_entity_nbt(&position, nbt).await?;
        }
    }

    // GameTestInfo::placeStructure 清除计划中的方块刻与排队的
    // 替换后测试盒内的方块事件。这避免了延迟工作
    // 阻止第 N 次尝试的执行波及第 N+1 次尝试。
    world
        .clear_scheduled_block_ticks(&test_min, &test_max)
        .await?;
    world.clear_block_events(&test_min, &test_max).await?;

    Ok(TestStructureInstance::new(
        test_instance_pos,
        origin,
        source_size,
        size,
        rotation,
    ))
}

/// 使用与原版相同的单方块屏障外壳将结构包围起来。
///
/// 来自 `TestInstanceBlockEntity::encaseStructure` 的地板和四面墙始终
/// 始终存在；当测试要求可见天空时则省略天花板。
pub async fn encase_structure(
    world: &dyn GameTestWorld,
    placement: &TestStructureInstance,
    sky_access: bool,
) -> GameTestResult<()> {
    process_structure_boundary(placement, sky_access, |position| async move {
        if position == *placement.test_instance_pos() {
            return Ok(());
        }

        world
            .set_block_state(
                &position,
                Block::BARRIER.default_state.id,
                BlockFlags::NOTIFY_ALL,
            )
            .await
    })
    .await
}

/// 测试成功后移除单方块屏障外壳。
///
/// 这对应于 `GameTestRunner` 调用 `TestInstanceBlockEntity::removeBarriers` 的情况，位于
/// 与原版一致。仅移除屏障方块，因此边界上的测试方块会被保留。
pub async fn remove_barriers(
    world: &dyn GameTestWorld,
    placement: &TestStructureInstance,
    sky_access: bool,
) -> GameTestResult<()> {
    process_structure_boundary(placement, sky_access, |position| async move {
        if world.block_state_id(&position).await == Block::BARRIER.default_state.id {
            world
                .set_block_state(
                    &position,
                    Block::AIR.default_state.id,
                    BlockFlags::NOTIFY_ALL,
                )
                .await?;
        }
        Ok(())
    })
    .await
}

/// 原版 `GameTestInfo::succeed` 会丢弃位于……内的非玩家实体
/// 结构边界外扩一格，之后监听器才会调度任何重跑。
pub async fn clear_success_entities(
    world: &dyn GameTestWorld,
    placement: &TestStructureInstance,
) -> GameTestResult<()> {
    let origin = placement.origin();
    let size = placement.size();
    let min = BlockPos::new(origin.0.x - 1, origin.0.y - 1, origin.0.z - 1);
    let max = BlockPos::new(
        origin.0.x + size[0] + 1,
        origin.0.y + size[1] + 1,
        origin.0.z + size[2] + 1,
    );
    world.clear_non_player_entities(&min, &max).await
}

async fn process_structure_boundary<F, Fut>(
    placement: &TestStructureInstance,
    sky_access: bool,
    mut action: F,
) -> GameTestResult<()>
where
    F: FnMut(BlockPos) -> Fut,
    Fut: Future<Output = GameTestResult<()>>,
{
    let origin = placement.origin();
    let size = placement.size();
    let low = BlockPos::new(origin.0.x - 1, origin.0.y - 1, origin.0.z - 1);
    let high = BlockPos::new(
        origin.0.x + size[0],
        origin.0.y + size[1],
        origin.0.z + size[2],
    );

    for x in low.0.x..=high.0.x {
        for y in low.0.y..=high.0.y {
            for z in low.0.z..=high.0.z {
                let is_wall_or_floor =
                    x == low.0.x || x == high.0.x || z == low.0.z || z == high.0.z || y == low.0.y;
                let is_ceiling = y == high.0.y;
                if !is_wall_or_floor && (sky_access || !is_ceiling) {
                    continue;
                }

                action(BlockPos::new(x, y, z)).await?;
            }
        }
    }

    Ok(())
}

pub async fn clear_structure_area(
    world: &dyn GameTestWorld,
    origin: &BlockPos,
    size: [i32; 3],
) -> GameTestResult<()> {
    clear_box(world, origin, size).await
}

async fn clear_test_area(
    world: &dyn GameTestWorld,
    origin: &BlockPos,
    size: [i32; 3],
    padding: i32,
) -> GameTestResult<()> {
    let min = BlockPos::new(
        origin.0.x - padding,
        origin.0.y - padding,
        origin.0.z - padding,
    );
    let diameter = padding.saturating_mul(2);
    clear_box(
        world,
        &min,
        [size[0] + diameter, size[1] + diameter, size[2] + diameter],
    )
    .await
}

async fn clear_box(
    world: &dyn GameTestWorld,
    origin: &BlockPos,
    size: [i32; 3],
) -> GameTestResult<()> {
    let clear_flags = BlockFlags::NOTIFY_LISTENERS
        | BlockFlags::SKIP_DROPS
        | BlockFlags::SKIP_REDSTONE_WIRE_STATE_REPLACEMENT
        | BlockFlags::SKIP_BLOCK_ADDED_CALLBACK;

    for x in 0..size[0] {
        for y in 0..size[1] {
            for z in 0..size[2] {
                let position = BlockPos::new(origin.0.x + x, origin.0.y + y, origin.0.z + z);
                world
                    .set_block_state(&position, Block::AIR.default_state.id, clear_flags)
                    .await?;
            }
        }
    }

    Ok(())
}
