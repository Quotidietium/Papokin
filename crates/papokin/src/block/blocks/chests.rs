use std::sync::Arc;

use crate::block::entities::BlockEntity;
use crate::block::entities::chest::ChestBlockEntity;
use papokin_data::BlockStateId;
use papokin_data::block_properties::{ChestLikeProperties, ChestType, HorizontalFacing};
use papokin_data::entity::EntityPose;
use papokin_data::loot_table::get_loot_table;
use papokin_data::{Block, BlockDirection, translation};
use papokin_inventory::Inventory;
use papokin_inventory::double::DoubleInventory;
use papokin_inventory::generic_container_screen_handler::{create_generic_9x3, create_generic_9x6};
use papokin_inventory::player::player_inventory::PlayerInventory;
use papokin_inventory::screen_handler::{
    InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler,
};
use papokin_macros::{pumpkin_block, pumpkin_block_from_tag};
use papokin_util::GameMode;
use papokin_util::math::position::BlockPos;
use papokin_util::text::TextComponent;
use papokin_world::world::BlockFlags;
use std::sync::Mutex;

use crate::block::{
    BlockBehaviour, BrokenArgs, EmitsRedstonePowerArgs, GetComparatorOutputArgs,
    GetRedstonePowerArgs, GetScreenHandlerFactoryArgs, NormalUseArgs, OnPlaceArgs,
    OnStateReplacedArgs, OnSyncedBlockEventArgs, PathComputationType, PlacedArgs, RandomTickArgs,
    registry::BlockActionResult,
};
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::world::World;
use crate::world::loot::fill_chest_inventory;
use papokin_data::BlockState;

struct ChestScreenFactory(Arc<dyn Inventory>);

impl ScreenHandlerFactory for ChestScreenFactory {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        player: &dyn InventoryPlayer,
    ) -> Option<SharedScreenHandler> {
        let concrete_handler = if self.0.size() > 27 {
            create_generic_9x6(sync_id, player_inventory, self.0.clone(), player)
        } else {
            create_generic_9x3(sync_id, player_inventory, self.0.clone(), player)
        };

        let concrete_arc = Arc::new(Mutex::new(concrete_handler));

        Some(concrete_arc as SharedScreenHandler)
    }

    fn get_display_name(&self) -> TextComponent {
        if self.0.size() > 27 {
            papokin_macros::translate!(translation::java::CONTAINER_CHESTDOUBLE)
        } else {
            papokin_macros::translate!(translation::java::CONTAINER_CHEST)
        }
    }
}

// 共享的箱子行为实现
const LID_ANIMATION_EVENT_TYPE: u8 = 1;

fn on_place_chest_impl(args: &OnPlaceArgs<'_>) -> BlockStateId {
    let mut chest_props = ChestLikeProperties::default(args.block);
    chest_props.waterlogged = args.replacing.water_source();

    let (r#type, facing) = compute_chest_props(
        args.world,
        args.player,
        args.block,
        args.position,
        args.direction,
    );
    chest_props.facing = facing;
    chest_props.r#type = r#type;

    chest_props.to_state_id(args.block)
}

fn placed_chest_impl<E: BlockEntity + 'static>(
    args: &PlacedArgs<'_>,
    create_entity: impl FnOnce(BlockPos) -> E,
) {
    let chest = create_entity(*args.position);
    args.world.add_block_entity(Arc::new(chest));

    let chest_props = ChestLikeProperties::from_state_id(args.state_id);
    let connected_towards = match chest_props.r#type {
        ChestType::Single => return,
        ChestType::Left => chest_props.facing.rotate_clockwise(),
        ChestType::Right => chest_props.facing.rotate_counter_clockwise(),
    };

    if let Some(mut neighbor_props) = get_chest_properties_if_can_connect(
        args.world,
        args.block,
        args.position,
        chest_props.facing,
        connected_towards,
        ChestType::Single,
    ) {
        neighbor_props.r#type = chest_props.r#type.opposite();

        args.world.set_block_state(
            &args.position.offset(connected_towards.to_offset()),
            neighbor_props.to_state_id(args.block),
            BlockFlags::NOTIFY_LISTENERS,
        );
    }
}

/// 计算箱子的比较器输出，对于大箱子会合并两部分的输出。
fn get_chest_comparator_output(args: &GetComparatorOutputArgs<'_>) -> Option<u8> {
    let state = args.world.get_block_state_id(args.position);
    let first_chest = args.world.get_block_entity(args.position);
    let first_inventory = first_chest.and_then(BlockEntity::get_inventory)?;

    let chest_props = ChestLikeProperties::from_state_id(state);
    let connected_towards = match chest_props.r#type {
        ChestType::Single => None,
        ChestType::Left => Some(chest_props.facing.rotate_clockwise()),
        ChestType::Right => Some(chest_props.facing.rotate_counter_clockwise()),
    };

    // 原版传入 `ignoreBeingBlocked = false`，因此被阻挡的一半读取为 0。
    if is_chest_blocked(args.world, args.position) {
        return Some(0);
    }

    if let Some(direction) = connected_towards
        && is_chest_blocked(args.world, &args.position.offset(direction.to_offset()))
    {
        return Some(0);
    }

    if let Some(direction) = connected_towards
        && let Some(second_inventory) = args
            .world
            .get_block_entity(&args.position.offset(direction.to_offset()))
            .and_then(BlockEntity::get_inventory)
    {
        let double_inventory = if matches!(chest_props.r#type, ChestType::Right) {
            DoubleInventory::new(first_inventory, second_inventory)
        } else {
            DoubleInventory::new(second_inventory, first_inventory)
        };
        Some(crate::block::calculate_comparator_output(
            double_inventory.as_ref(),
        ))
    } else {
        Some(crate::block::calculate_comparator_output(
            first_inventory.as_ref(),
        ))
    }
}

///返回一个用于打开箱子或大箱子物品栏的屏幕处理器工厂。
///
/// 在首次打开时解包延迟加载的战利品表（针对非旁观玩家），并合并
/// 若大箱子两半均未被阻挡，则将两者的容器连通。
fn get_chest_screen_handler_factory(
    args: GetScreenHandlerFactoryArgs<'_>,
) -> Option<Box<dyn ScreenHandlerFactory>> {
    let state = args.world.get_block_state_id(args.position);
    let first_chest = args.world.get_block_entity(args.position);

    let player_is_spectator = args.player.gamemode.load() == GameMode::Spectator;

    let chest_props = ChestLikeProperties::from_state_id(state);
    let connected_towards = match chest_props.r#type {
        ChestType::Single => None,
        ChestType::Left => Some(chest_props.facing.rotate_clockwise()),
        ChestType::Right => Some(chest_props.facing.rotate_counter_clockwise()),
    };

    let unpack = |entity: &Arc<dyn BlockEntity>| {
        if let Some((loot_key, seed)) = entity.take_loot_table() {
            // 战利品生成事件，取消则箱子保持为空（战利品表已消费）
            if !args.world.generate_loot(&loot_key) {
                return;
            }
            if let Some(table) = get_loot_table(&loot_key)
                && let Some(inv) = entity.clone().get_inventory()
            {
                fill_chest_inventory(&inv, table, seed);
                inv.mark_dirty();
            }
        }
    };

    // 首次打开时解包延迟加载的战利品表（仅限非旁观者）。
    if !player_is_spectator && let Some(ref entity) = first_chest {
        unpack(entity);
    }

    let first_inventory = first_chest.and_then(BlockEntity::get_inventory)?;

    if is_chest_blocked(args.world, args.position) {
        return None;
    }

    if let Some(direction) = connected_towards {
        let neighbor_pos = args.position.offset(direction.to_offset());
        if is_chest_blocked(args.world, &neighbor_pos) {
            return None;
        }
    }

    // 大箱子的两半会像原版的 CompoundContainer 一样同时解包。
    if !player_is_spectator
        && let Some(direction) = connected_towards
        && let Some(second) = args
            .world
            .get_block_entity(&args.position.offset(direction.to_offset()))
    {
        unpack(&second);
    }

    let inventory = if let Some(direction) = connected_towards
        && let Some(second_inventory) = args
            .world
            .get_block_entity(&args.position.offset(direction.to_offset()))
            .and_then(BlockEntity::get_inventory)
    {
        // 原版：chestType == ChestType.RIGHT ? DoubleBlockProperties.Type.FIRST : DoubleBlockProperties.Type.SECOND;
        if matches!(chest_props.r#type, ChestType::Right) {
            DoubleInventory::new(first_inventory, second_inventory)
        } else {
            DoubleInventory::new(second_inventory, first_inventory)
        }
    } else {
        first_inventory
    };

    Some(Box::new(ChestScreenFactory(inventory)))
}

fn normal_use_chest_impl(args: &NormalUseArgs<'_>) -> BlockActionResult {
    let stat = if args.block.id == Block::TRAPPED_CHEST.id {
        papokin_data::statistic::CustomStatistic::TriggerTrappedChest
    } else {
        papokin_data::statistic::CustomStatistic::OpenChest
    };
    args.player.increment_stat(
        papokin_data::statistic::StatisticCategory::Custom,
        stat as i32,
        1,
    );

    if let Some(factory) = get_chest_screen_handler_factory(GetScreenHandlerFactoryArgs {
        server: args.server,
        world: args.world,
        block: args.block,
        position: args.position,
        player: args.player,
    }) {
        args.player
            .open_handled_screen(factory.as_ref(), Some(*args.position));
    }

    BlockActionResult::Success
}

fn broken_chest_impl(args: &BrokenArgs<'_>) {
    split_connected_chest(args.world, args.block, args.position, args.state.id);
}

fn on_state_replaced_chest_impl(args: &OnStateReplacedArgs<'_>) {
    // 活塞移动不拆除双箱关系（方块实体随移动保留）。
    if args.moved {
        return;
    }
    // 爆炸、指令 setblock 等非玩家替换路径也要重置另一半，
    // 否则残留半箱的 Left/Right 类型不再有效。
    split_connected_chest(args.world, args.block, args.position, args.old_state_id);
}

fn split_connected_chest(
    world: &Arc<World>,
    block: &Block,
    position: &BlockPos,
    state_id: BlockStateId,
) {
    let chest_props = ChestLikeProperties::from_state_id(state_id);
    let connected_towards = match chest_props.r#type {
        ChestType::Single => return,
        ChestType::Left => chest_props.facing.rotate_clockwise(),
        ChestType::Right => chest_props.facing.rotate_counter_clockwise(),
    };

    if let Some(mut neighbor_props) = get_chest_properties_if_can_connect(
        world,
        block,
        position,
        chest_props.facing,
        connected_towards,
        chest_props.r#type.opposite(),
    ) {
        neighbor_props.r#type = ChestType::Single;

        world.set_block_state(
            &position.offset(connected_towards.to_offset()),
            neighbor_props.to_state_id(block),
            BlockFlags::NOTIFY_LISTENERS,
        );
    }
}

#[pumpkin_block_from_tag("c:chests/wooden")]
pub struct ChestBlock;

impl BlockBehaviour for ChestBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        on_place_chest_impl(&args)
    }

    fn on_synced_block_event(&self, args: OnSyncedBlockEventArgs<'_>) -> bool {
        args.r#type == LID_ANIMATION_EVENT_TYPE
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        placed_chest_impl(&args, ChestBlockEntity::new);
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        normal_use_chest_impl(&args)
    }

    fn get_screen_handler_factory(
        &self,
        args: GetScreenHandlerFactoryArgs<'_>,
    ) -> Option<Box<dyn ScreenHandlerFactory>> {
        get_chest_screen_handler_factory(args)
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        broken_chest_impl(&args);
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        on_state_replaced_chest_impl(&args);
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        get_chest_comparator_output(&args)
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

/// 铜箱子的行为与木箱子相同，但会随时间氧化。
#[pumpkin_block_from_tag("minecraft:copper_chests")]
pub struct CopperChestBlock;

impl
    crate::block::blocks::weathering_copper::ChangeOverTimeBlock<
        crate::block::blocks::weathering_copper::WeatherState,
    > for CopperChestBlock
{
    fn get_age(
        &self,
        block: &Block,
    ) -> Option<crate::block::blocks::weathering_copper::WeatherState> {
        crate::block::blocks::weathering_copper::get_weather_state(block)
    }

    fn get_chance_modifier(
        &self,
        age: crate::block::blocks::weathering_copper::WeatherState,
    ) -> f32 {
        crate::block::blocks::weathering_copper::get_chance_modifier(age)
    }

    fn get_next(&self, block: &Block) -> Option<&'static Block> {
        crate::block::blocks::weathering_copper::get_next(block)
    }

    fn get_previous(&self, block: &Block) -> Option<&'static Block> {
        crate::block::blocks::weathering_copper::get_previous(block)
    }

    fn get_first(&self, block: &Block) -> Option<&'static Block> {
        crate::block::blocks::weathering_copper::get_first(block)
    }
}

impl crate::block::blocks::weathering_copper::WeatheringCopper for CopperChestBlock {}

impl BlockBehaviour for CopperChestBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        on_place_chest_impl(&args)
    }

    fn on_synced_block_event(&self, args: OnSyncedBlockEventArgs<'_>) -> bool {
        args.r#type == LID_ANIMATION_EVENT_TYPE
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        placed_chest_impl(&args, ChestBlockEntity::new);
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        normal_use_chest_impl(&args)
    }

    fn get_screen_handler_factory(
        &self,
        args: GetScreenHandlerFactoryArgs<'_>,
    ) -> Option<Box<dyn ScreenHandlerFactory>> {
        get_chest_screen_handler_factory(args)
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        broken_chest_impl(&args);
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        on_state_replaced_chest_impl(&args);
    }

    fn random_tick(&self, args: RandomTickArgs<'_>) {
        let current_state_id = args.world.get_block_state_id(args.position);
        let chest_props = ChestLikeProperties::from_state_id(current_state_id);

        // 只氧化 LEFT 或 SINGLE 箱子（不氧化 RIGHT）以防止双重氧化
        if chest_props.r#type == ChestType::Right {
            return;
        }

        // 仅当没有玩家正在查看箱子时才氧化
        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(chest_entity) = block_entity.as_any().downcast_ref::<ChestBlockEntity>()
            && chest_entity.get_viewer_count() > 0
        {
            return;
        }

        crate::block::blocks::weathering_copper::change_over_time(
            args.world,
            args.position,
            args.block,
        );
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        get_chest_comparator_output(&args)
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

/// 陷阱箱的行为与木箱相同，但还会根据查看者数量发出红石信号。
#[pumpkin_block("minecraft:trapped_chest")]
pub struct TrappedChestBlock;

impl BlockBehaviour for TrappedChestBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        on_place_chest_impl(&args)
    }

    fn on_synced_block_event(&self, args: OnSyncedBlockEventArgs<'_>) -> bool {
        args.r#type == LID_ANIMATION_EVENT_TYPE
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        use crate::block::entities::trapped_chest::TrappedChestBlockEntity;
        placed_chest_impl(&args, TrappedChestBlockEntity::new);
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        normal_use_chest_impl(&args)
    }

    fn get_screen_handler_factory(
        &self,
        args: GetScreenHandlerFactoryArgs<'_>,
    ) -> Option<Box<dyn ScreenHandlerFactory>> {
        get_chest_screen_handler_factory(args)
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        broken_chest_impl(&args);
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        on_state_replaced_chest_impl(&args);
    }

    fn emits_redstone_power(&self, _args: EmitsRedstonePowerArgs<'_>) -> bool {
        true
    }

    fn get_weak_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        use crate::block::entities::trapped_chest::TrappedChestBlockEntity;

        // 获取此箱子的查看者数量
        let viewer_count = if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(trapped_chest) = block_entity
                .as_any()
                .downcast_ref::<TrappedChestBlockEntity>()
        {
            trapped_chest.get_viewer_count()
        } else {
            0
        };

        viewer_count.min(15) as u8
    }

    fn get_strong_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        // 向陷阱箱下方的方块发出强充能
        // 下方方块以向上（Up）方向查询（从下方向上看着箱子）
        if args.direction == BlockDirection::Up {
            self.get_weak_redstone_power(args)
        } else {
            0
        }
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        get_chest_comparator_output(&args)
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

fn compute_chest_props(
    world: &World,
    player: &Player,
    block: &Block,
    block_pos: &BlockPos,
    face: BlockDirection,
) -> (ChestType, HorizontalFacing) {
    let player_facing = player.get_entity().get_horizontal_facing();
    let chest_facing = player_facing.opposite();

    if player.get_entity().pose.load() == EntityPose::Crouching {
        let Some(face) = face.to_horizontal_facing() else {
            return (ChestType::Single, chest_facing);
        };

        let (clicked_block, clicked_block_state) =
            world.get_block_and_state_id(&block_pos.offset(face.to_offset()));

        if clicked_block == block {
            let clicked_props = ChestLikeProperties::from_state_id(clicked_block_state);

            if clicked_props.r#type != ChestType::Single {
                return (ChestType::Single, chest_facing);
            }

            if clicked_props.facing.rotate_clockwise() == face {
                return (ChestType::Left, clicked_props.facing);
            } else if clicked_props.facing.rotate_counter_clockwise() == face {
                return (ChestType::Right, clicked_props.facing);
            }
        }

        return (ChestType::Single, chest_facing);
    }

    if get_chest_properties_if_can_connect(
        world,
        block,
        block_pos,
        chest_facing,
        chest_facing.rotate_clockwise(),
        ChestType::Single,
    )
    .is_some()
    {
        (ChestType::Left, chest_facing)
    } else if get_chest_properties_if_can_connect(
        world,
        block,
        block_pos,
        chest_facing,
        chest_facing.rotate_counter_clockwise(),
        ChestType::Single,
    )
    .is_some()
    {
        (ChestType::Right, chest_facing)
    } else {
        (ChestType::Single, chest_facing)
    }
}

fn get_chest_properties_if_can_connect(
    world: &World,
    block: &Block,
    block_pos: &BlockPos,
    facing: HorizontalFacing,
    direction: HorizontalFacing,
    wanted_type: ChestType,
) -> Option<ChestLikeProperties> {
    let (neighbor_block, neighbor_block_state) =
        world.get_block_and_state_id(&block_pos.offset(direction.to_offset()));

    if neighbor_block != block {
        return None;
    }

    let neighbor_props = ChestLikeProperties::from_state_id(neighbor_block_state);
    if neighbor_props.facing == facing && neighbor_props.r#type == wanted_type {
        return Some(neighbor_props);
    }

    None
}

fn is_chest_blocked(world: &World, block_pos: &BlockPos) -> bool {
    // TODO: 猫坐在上方时阻止打开。
    has_block_on_top(world, block_pos)
}
fn has_block_on_top(world: &World, block_pos: &BlockPos) -> bool {
    let above_pos = block_pos.up();
    let above_state = world.get_block_state(&above_pos);
    above_state.is_solid_block()
}

trait ChestTypeExt {
    fn opposite(&self) -> ChestType;
}

impl ChestTypeExt for ChestType {
    fn opposite(&self) -> Self {
        match self {
            Self::Single => Self::Single,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}
