use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use crate::block::BlockIsReplacing;
use crate::block::CanPlaceAtArgs;
use crate::block::EmitsRedstonePowerArgs;
use crate::block::GetRedstonePowerArgs;
use crate::block::GetStateForNeighborUpdateArgs;
use crate::block::OnNeighborUpdateArgs;
use crate::block::OnPlaceArgs;
use crate::block::OnScheduledTickArgs;
use crate::block::OnStateReplacedArgs;
use crate::block::PlacedArgs;
use crate::entity::EntityBase;
use papokin_data::Block;
use papokin_data::BlockDirection;
use papokin_data::BlockId;
use papokin_data::BlockStateId;
use papokin_data::FacingExt;
use papokin_data::HorizontalFacingExt;
use papokin_data::block_properties::Facing;
use papokin_util::math::position::BlockPos;
use papokin_world::tick::TickPriority;
use papokin_world::world::BlockAccessor;
use papokin_world::world::BlockFlags;
use uuid::Uuid;

type RWallTorchProps = papokin_data::block_properties::FurnaceLikeProperties;
type RTorchProps = papokin_data::block_properties::RedstoneOreLikeProperties;

use crate::block::{BlockBehaviour, BlockMetadata};
use crate::world::World;

use super::get_redstone_power;

pub struct RedstoneTorchBlock;

impl BlockMetadata for RedstoneTorchBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::REDSTONE_TORCH, BlockId::REDSTONE_WALL_TORCH].into()
    }
}

impl BlockBehaviour for RedstoneTorchBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let world = args.world;
        let block = args.block;
        let location = args.position;

        if args.direction == BlockDirection::Down {
            let support_block = world.get_block_state(&location.down());
            if support_block.is_center_solid(BlockDirection::Up) {
                return block.default_state.id;
            }
        }
        let mut directions = args.player.get_entity().get_entity_facing_order();

        if args.replacing == BlockIsReplacing::None {
            let face = args.direction.to_facing();
            let mut i = 0;
            while i < directions.len() && directions[i] != face {
                i += 1;
            }

            if i > 0 {
                directions.copy_within(0..i, 1);
                directions[0] = face;
            }
        } else if directions[0] == Facing::Down {
            let support_block = world.get_block_state(&location.down());
            if support_block.is_center_solid(BlockDirection::Up) {
                return block.default_state.id;
            }
        }

        for dir in directions {
            if dir != Facing::Up
                && dir != Facing::Down
                && can_place_at(world, location, dir.to_block_direction())
            {
                let mut torch_props = RWallTorchProps::default(&Block::REDSTONE_WALL_TORCH);
                if let Some(facing) = dir.opposite().to_horizontal_facing() {
                    torch_props.facing = facing;
                    return torch_props.to_state_id(&Block::REDSTONE_WALL_TORCH);
                }
            }
        }

        let support_block = world.get_block_state(&location.down());
        if support_block.is_center_solid(BlockDirection::Up) {
            block.default_state.id
        } else {
            BlockStateId::AIR
        }
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        let support_block = args.block_accessor.get_block_state(&args.position.down());
        if support_block.is_center_solid(BlockDirection::Up) {
            return true;
        }
        for dir in BlockDirection::horizontal() {
            if can_place_at(args.block_accessor, args.position, dir.to_block_direction()) {
                return true;
            }
        }
        false
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        if args.block == &Block::REDSTONE_WALL_TORCH {
            let props = RWallTorchProps::from_state_id(args.state_id);
            if props.facing.to_block_direction().opposite() == args.direction
                && !can_place_at(
                    args.world,
                    args.position,
                    props.facing.to_block_direction().opposite(),
                )
            {
                return BlockStateId::AIR;
            }
        } else if args.direction == BlockDirection::Down {
            let support_block = args.world.get_block_state(&args.position.down());
            if !support_block.is_center_solid(BlockDirection::Up) {
                return BlockStateId::AIR;
            }
        }
        args.state_id
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        {
            let state = args.world.get_block_state(args.position);

            if args
                .world
                .is_block_tick_scheduled(args.position, args.block)
            {
                return;
            }

            if args.block == &Block::REDSTONE_WALL_TORCH {
                let props = RWallTorchProps::from_state_id(state.id);
                if props.lit
                    != should_be_lit(
                        args.world,
                        args.position,
                        props.facing.to_block_direction().opposite(),
                    )
                {
                    args.world.schedule_block_tick(
                        args.block,
                        *args.position,
                        2,
                        TickPriority::Normal,
                    );
                }
            } else if args.block == &Block::REDSTONE_TORCH {
                let props = RTorchProps::from_state_id(state.id);
                if props.lit != should_be_lit(args.world, args.position, BlockDirection::Down) {
                    args.world.schedule_block_tick(
                        args.block,
                        *args.position,
                        2,
                        TickPriority::Normal,
                    );
                }
            }
        }
    }

    fn emits_redstone_power(&self, _args: EmitsRedstonePowerArgs<'_>) -> bool {
        true
    }

    fn get_weak_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        if args.block == &Block::REDSTONE_WALL_TORCH {
            let props = RWallTorchProps::from_state_id(args.state.id);
            if props.lit && args.direction != props.facing.to_block_direction() {
                return 15;
            }
        } else if args.block == &Block::REDSTONE_TORCH {
            let props = RTorchProps::from_state_id(args.state.id);
            if props.lit && args.direction != BlockDirection::Up {
                return 15;
            }
        }
        0
    }

    fn get_strong_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        if args.direction == BlockDirection::Down {
            if args.block == &Block::REDSTONE_WALL_TORCH {
                let props = RWallTorchProps::from_state_id(args.state.id);
                if props.lit {
                    return 15;
                }
            } else if args.block == &Block::REDSTONE_TORCH {
                let props = RTorchProps::from_state_id(args.state.id);
                if props.lit {
                    return 15;
                }
            }
        }
        0
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let world = args.world;
        let (block, state) = world.get_block_and_state(args.position);
        // 状态写入闭包：把新的 lit 值编回对应方块状态
        let state_for = |lit: bool| -> BlockStateId {
            if block == &Block::REDSTONE_WALL_TORCH {
                let mut props = RWallTorchProps::from_state_id(state.id);
                props.lit = lit;
                props.to_state_id(block)
            } else {
                let mut props = RTorchProps::from_state_id(state.id);
                props.lit = lit;
                props.to_state_id(block)
            }
        };

        if block == &Block::REDSTONE_WALL_TORCH {
            let props = RWallTorchProps::from_state_id(state.id);
            handle_torch_tick(
                world,
                args.position,
                block,
                props.lit,
                props.facing.to_block_direction().opposite(),
                state_for,
            );
        } else if block == &Block::REDSTONE_TORCH {
            let props = RTorchProps::from_state_id(state.id);
            handle_torch_tick(
                world,
                args.position,
                block,
                props.lit,
                BlockDirection::Down,
                state_for,
            );
        }
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        update_neighbors(args.world, args.position);
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        update_neighbors(args.world, args.position);
    }
}

pub fn should_be_lit(world: &World, pos: &BlockPos, face: BlockDirection) -> bool {
    let other_pos = pos.offset(face.to_offset());
    let (block, state) = world.get_block_and_state(&other_pos);
    get_redstone_power(block, state, world, &other_pos, face) == 0
}

pub fn update_neighbors(world: &Arc<World>, pos: &BlockPos) {
    for dir in BlockDirection::all() {
        let other_pos = pos.offset(dir.to_offset());
        world.update_neighbors(&other_pos, None);
    }
}

fn can_place_at(world: &dyn BlockAccessor, block_pos: &BlockPos, facing: BlockDirection) -> bool {
    world
        .get_block_state(&block_pos.offset(facing.to_offset()))
        .is_side_solid(facing.opposite())
}

// ---------------------------------------------------------------------------
// 烧毁（burnout）防护
//
// 与原版一致：火把在 60 刻（3 秒）窗口内翻转 8 次即烧毁——强制熄灭并
// 锁定 1600 刻（80 秒）不响应输入。缺少该防护时，火把-方块反馈回路会以
// 每秒 10 次翻转永久运行，每次翻转都触发邻居级联与方块更新广播，可被
// 玩家低成本构造为持续的负载放大器。
// ---------------------------------------------------------------------------

/// 翻转计数的滑动窗口宽度（游戏刻）
const TORCH_TOGGLE_WINDOW_TICKS: i64 = 60;
/// 窗口内允许的最大翻转次数
const TORCH_MAX_TOGGLES_IN_WINDOW: usize = 8;
/// 烧毁后的锁定时长（游戏刻）
const TORCH_BURNOUT_LOCK_TICKS: i64 = 1600;

type TorchKey = (Uuid, BlockPos);

/// 单个火把位置的翻转追踪状态
#[derive(Default)]
struct TorchToggleTracker {
    /// 窗口内的翻转时间戳（游戏刻），单调递增
    recent_toggles: Vec<i64>,
    /// 烧毁锁定到期时间；None 表示未烧毁
    recheck_at: Option<i64>,
}

impl TorchToggleTracker {
    /// 丢弃窗口外的历史翻转
    fn prune(&mut self, now: i64) {
        self.recent_toggles
            .retain(|t| now - *t <= TORCH_TOGGLE_WINDOW_TICKS);
    }

    /// 记录一次翻转；窗口内次数达到上限则进入烧毁并返回 true
    fn record_toggle(&mut self, now: i64) -> bool {
        self.prune(now);
        self.recent_toggles.push(now);
        if self.recent_toggles.len() >= TORCH_MAX_TOGGLES_IN_WINDOW {
            self.recent_toggles.clear();
            self.recheck_at = Some(now + TORCH_BURNOUT_LOCK_TICKS);
            return true;
        }
        false
    }

    /// 到期则解除烧毁并清空历史（重新点亮不计入新一轮计数，避免立即复烧）
    fn recover_if_due(&mut self, now: i64) {
        if self.recheck_at.is_some_and(|at| now >= at) {
            self.recheck_at = None;
            self.recent_toggles.clear();
        }
    }

    /// 烧毁锁定的剩余刻数；未锁定返回 None
    fn lock_remaining(&self, now: i64) -> Option<i64> {
        self.recheck_at.filter(|at| *at > now).map(|at| at - now)
    }

    /// 条目是否可整体回收
    const fn is_idle(&self) -> bool {
        self.recent_toggles.is_empty() && self.recheck_at.is_none()
    }
}

/// 进程级翻转追踪表；与原版一致仅存内存，重启后自然清零
static TORCH_TRACKERS: LazyLock<Mutex<HashMap<TorchKey, TorchToggleTracker>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 查询烧毁锁定的剩余刻数，顺带执行到期恢复与闲置条目回收
fn torch_lock_remaining(now: i64, key: &TorchKey) -> Option<i64> {
    let mut trackers = TORCH_TRACKERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let tracker = trackers.get_mut(key)?;
    tracker.recover_if_due(now);
    if tracker.is_idle() {
        trackers.remove(key);
        return None;
    }
    tracker.lock_remaining(now)
}

/// 记录一次实际翻转；返回 true 表示达到烧毁阈值
fn record_torch_toggle(now: i64, key: &TorchKey) -> bool {
    let mut trackers = TORCH_TRACKERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    trackers.entry(*key).or_default().record_toggle(now)
}

/// 火把 scheduled tick 的统一处理（含烧毁防护）。
/// `lit` 为当前是否点亮，`support_face` 为依附面方向，
/// `state_for` 把新的 lit 值编回对应方块状态。
fn handle_torch_tick(
    world: &Arc<World>,
    position: &BlockPos,
    block: &Block,
    lit: bool,
    support_face: BlockDirection,
    state_for: impl FnOnce(bool) -> BlockStateId,
) {
    let now = world.get_world_age();
    let key: TorchKey = (world.uuid, *position);

    if let Some(remaining) = torch_lock_remaining(now, &key) {
        // 烧毁锁定期：保持熄灭，并以不超过 u8 上限的链式计划刻保证到期复查
        if lit {
            world.set_block_state(position, state_for(false), BlockFlags::NOTIFY_ALL);
            update_neighbors(world, position);
        }
        world.schedule_block_tick(
            block,
            *position,
            remaining.min(i64::from(u8::MAX)) as u8,
            TickPriority::Normal,
        );
        return;
    }

    let should_be_lit_now = should_be_lit(world, position, support_face);
    if lit == should_be_lit_now {
        return;
    }

    if record_torch_toggle(now, &key) {
        // 窗口内翻转过频：强制熄灭并锁定，不响应本次输入
        if lit {
            world.set_block_state(position, state_for(false), BlockFlags::NOTIFY_ALL);
            update_neighbors(world, position);
        }
        world.schedule_block_tick(
            block,
            *position,
            TORCH_BURNOUT_LOCK_TICKS.min(i64::from(u8::MAX)) as u8,
            TickPriority::Normal,
        );
        return;
    }

    world.set_block_state(
        position,
        state_for(should_be_lit_now),
        BlockFlags::NOTIFY_ALL,
    );
    update_neighbors(world, position);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn torch_burns_out_after_eight_toggles_in_window() {
        let mut tracker = TorchToggleTracker::default();
        // 0,5,10,...,30：8 次翻转全部落在 60 刻窗口内
        for i in 0..7 {
            assert!(!tracker.record_toggle(i * 5));
        }
        assert_eq!(tracker.recent_toggles.len(), 7);
        // 第 8 次触发烧毁：历史清空、锁定 1600 刻
        assert!(tracker.record_toggle(35));
        assert_eq!(tracker.lock_remaining(36), Some(1599));
    }

    #[test]
    fn burnout_expires_and_clears_history() {
        let mut tracker = TorchToggleTracker::default();
        for i in 0..8 {
            tracker.record_toggle(i as i64);
        }
        let recheck_at = 7 + TORCH_BURNOUT_LOCK_TICKS;
        assert!(tracker.lock_remaining(recheck_at - 1).is_some());
        tracker.recover_if_due(recheck_at);
        assert!(tracker.lock_remaining(recheck_at).is_none());
        assert!(tracker.recent_toggles.is_empty());
        assert!(tracker.is_idle());
    }

    #[test]
    fn stale_toggles_outside_window_do_not_burn() {
        let mut tracker = TorchToggleTracker::default();
        for i in 0..7 {
            tracker.record_toggle(i as i64);
        }
        // 上次翻转为 6；61 刻之后的新翻转到来时，历史已全部滑出窗口
        assert!(!tracker.record_toggle(6 + 61));
        assert_eq!(tracker.recent_toggles.len(), 1);
    }
}
