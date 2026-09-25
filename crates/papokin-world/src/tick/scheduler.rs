use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

use papokin_util::math::position::BlockPos;
use rustc_hash::FxHashSet;

use crate::tick::{MAX_TICK_DELAY, OrderedTick, ScheduledTick};

pub struct ChunkTickScheduler<T> {
    inner: Mutex<Option<Box<ChunkTickSchedulerInner<T>>>>,
    offset: AtomicUsize,
}

struct ChunkTickSchedulerInner<T> {
    tick_queue: [Vec<OrderedTick<T>>; MAX_TICK_DELAY],
    queued_ticks: FxHashSet<(BlockPos, T)>,
}

impl<'a, T: std::hash::Hash + Eq> ChunkTickScheduler<&'a T> {
    pub fn step_tick(&self) -> Vec<OrderedTick<&'a T>> {
        // fetch_add 原子推进环形偏移；不得再显式 store 回写，
        // 否则并发调用会把偏移倒退、错位环形槽位。
        let current_offset = self.offset.fetch_add(1, Ordering::SeqCst) % MAX_TICK_DELAY;

        let mut inner_guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(inner) = inner_guard.as_mut() else {
            return Vec::new();
        };

        let mut res = std::mem::take(&mut inner.tick_queue[current_offset]);

        // 同一刻可能聚集不同优先级的刻（红石时序依赖优先级
        // 顺序）；按优先级与调度次序排序，与原版 LevelTicks 对齐。
        res.sort_unstable();

        if !res.is_empty() {
            for next_tick in &res {
                inner
                    .queued_ticks
                    .remove(&(next_tick.position, next_tick.value));
            }
            if inner.queued_ticks.is_empty() {
                *inner_guard = None;
            }
        }
        res
    }

    pub fn schedule_tick(&self, tick: &ScheduledTick<&'a T>, sub_tick_order: u64) {
        let offset = self.offset.load(Ordering::SeqCst);
        let mut inner_guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let inner = inner_guard.get_or_insert_with(|| {
            Box::new(ChunkTickSchedulerInner {
                tick_queue: std::array::from_fn(|_| Vec::new()),
                queued_ticks: FxHashSet::default(),
            })
        });

        if inner.queued_ticks.insert((tick.position, tick.value)) {
            let index = (offset + tick.delay as usize) % MAX_TICK_DELAY;

            inner.tick_queue[index].push(OrderedTick {
                priority: tick.priority,
                sub_tick_order,
                position: tick.position,
                value: tick.value,
            });
        }
    }

    pub fn is_scheduled(&self, pos: BlockPos, value: &T) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .is_some_and(|inner| inner.queued_ticks.contains(&(pos, value)))
    }

    pub fn clear_area(&self, min: &BlockPos, max: &BlockPos) {
        let mut inner_guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(inner) = inner_guard.as_mut() else {
            return;
        };

        let contains = |position: &BlockPos| {
            position.0.x >= min.0.x
                && position.0.x < max.0.x
                && position.0.y >= min.0.y
                && position.0.y < max.0.y
                && position.0.z >= min.0.z
                && position.0.z < max.0.z
        };

        for queue in &mut inner.tick_queue {
            queue.retain(|tick| !contains(&tick.position));
        }
        inner
            .queued_ticks
            .retain(|(position, _)| !contains(position));
        let became_empty = inner.queued_ticks.is_empty();

        if became_empty {
            *inner_guard = None;
        }
    }

    pub fn has_ticks(&self) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .is_some_and(|inner| !inner.queued_ticks.is_empty())
    }

    #[must_use]
    pub fn to_vec(&self) -> Vec<ScheduledTick<&'a T>> {
        let offset = self.offset.load(Ordering::SeqCst);
        let inner_guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(inner) = inner_guard.as_ref() else {
            return Vec::new();
        };

        let mut res = Vec::new();

        for i in 0..MAX_TICK_DELAY {
            let index = (offset + i) % MAX_TICK_DELAY;
            res.extend(inner.tick_queue[index].iter().map(|x| ScheduledTick {
                delay: i as u8,
                priority: x.priority,
                position: x.position,
                value: x.value,
            }));
        }
        res
    }
}

impl<'a, T: std::hash::Hash + Eq + 'static> FromIterator<ScheduledTick<&'a T>>
    for ChunkTickScheduler<&'a T>
{
    fn from_iter<I: IntoIterator<Item = ScheduledTick<&'a T>>>(iter: I) -> Self {
        let scheduler = Self::default();
        let iter = iter.into_iter();

        let (lower, _) = iter.size_hint();
        if lower > 0 {
            let mut inner_guard = scheduler
                .inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let inner = inner_guard.get_or_insert_with(|| {
                Box::new(ChunkTickSchedulerInner {
                    tick_queue: std::array::from_fn(|_| Vec::new()),
                    queued_ticks: FxHashSet::default(),
                })
            });
            inner.queued_ticks.reserve(lower);
        }

        for tick in iter {
            scheduler.schedule_tick(&tick, 0);
        }
        scheduler
    }
}

impl<T> Default for ChunkTickScheduler<T> {
    fn default() -> Self {
        Self {
            inner: Mutex::new(None),
            offset: AtomicUsize::new(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tick::{ScheduledTick, TickPriority};

    fn unit_tick(delay: u8, priority: TickPriority, pos: i32) -> ScheduledTick<&'static ()> {
        ScheduledTick {
            delay,
            priority,
            position: BlockPos::new(pos, 0, 0),
            value: &(),
        }
    }

    #[test]
    fn step_tick_orders_by_priority_then_schedule_order() {
        let scheduler = ChunkTickScheduler::<&()>::default();
        // 调度次序与优先级故意相反：排序必须以优先级为先。
        // 去重键是 (position, value)，各刻使用不同坐标。
        scheduler.schedule_tick(&unit_tick(0, TickPriority::Normal, 0), 1);
        scheduler.schedule_tick(&unit_tick(0, TickPriority::Normal, 1), 2);
        scheduler.schedule_tick(&unit_tick(0, TickPriority::High, 2), 3);
        scheduler.schedule_tick(&unit_tick(0, TickPriority::ExtremelyHigh, 3), 4);

        let batch = scheduler.step_tick();
        let priorities: Vec<TickPriority> = batch.iter().map(|t| t.priority).collect();
        assert_eq!(
            priorities,
            vec![
                TickPriority::ExtremelyHigh,
                TickPriority::High,
                TickPriority::Normal,
                TickPriority::Normal,
            ]
        );
        assert_eq!(batch[2].sub_tick_order, 1);
        assert_eq!(batch[3].sub_tick_order, 2);
    }

    #[test]
    fn scheduled_tick_fires_exactly_after_delay() {
        let scheduler = ChunkTickScheduler::<&()>::default();
        scheduler.schedule_tick(&unit_tick(3, TickPriority::Normal, 0), 0);

        assert!(scheduler.step_tick().is_empty(), "第 1 刻不应触发");
        assert!(scheduler.step_tick().is_empty(), "第 2 刻不应触发");
        assert!(scheduler.step_tick().is_empty(), "第 3 刻不应触发");
        let batch = scheduler.step_tick();
        assert_eq!(batch.len(), 1, "延迟 3 刻后应恰好触发一次");
        assert!(scheduler.step_tick().is_empty(), "触发后不得重复执行");
    }

    #[test]
    fn duplicate_scheduling_is_deduplicated() {
        let scheduler = ChunkTickScheduler::<&()>::default();
        // 同 (position, value) 的重复调度被去重，防止流体/红石
        // 重复登记导致队列膨胀。
        scheduler.schedule_tick(&unit_tick(1, TickPriority::Normal, 0), 0);
        scheduler.schedule_tick(&unit_tick(1, TickPriority::Normal, 0), 1);
        assert!(scheduler.step_tick().is_empty(), "第 1 刻（槽 0）应为空");
        let batch = scheduler.step_tick();
        assert_eq!(batch.len(), 1, "第 2 刻（槽 1）只应触发一次");
        assert!(scheduler.step_tick().is_empty());
    }
}
