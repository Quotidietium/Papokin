//! 插件任务调度 API。
//!
//! 本模块让插件能够调度闭包，使其
//! 在服务器主刻循环中延迟执行或重复执行。
//!
//! # Example
//!
//! ```rust,ignore
//! use papokin_plugin_api::scheduler::SchedulerExt;
//!
//! context.schedule_delayed_task(20, |server| {
//!     server.log("One second has passed!");
//! });
//! ```

use crate::wit::papokin::plugin::context::Server;
use crate::wit::papokin::plugin::scheduler;
use crate::wit::papokin::plugin::world::Entity;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// 可作为任务调度的闭包的类型别名。
///
/// 已调度的回调可能被重入，因此捕获的可变状态必须使用线程安全
/// 内部可变性。
pub type TaskHandler = Arc<dyn Fn(Server) + Send + Sync>;

pub(crate) struct Task {
    handler: TaskHandler,
    /// 一次性处理器在首次调用后即被丢弃，因此插件
    /// 即使调度大量延迟任务，表也不会无限制增长。
    one_shot: bool,
}

pub(crate) static TASK_HANDLERS: Mutex<LazyTaskHandlers> = Mutex::new(LazyTaskHandlers {
    handlers: BTreeMap::new(),
    task_ids: BTreeMap::new(),
    next_id: 0,
});

pub(crate) struct LazyTaskHandlers {
    handlers: BTreeMap<u32, Task>,
    /// 宿主任务 id -> 处理器 id，使 `cancel_task` 也能丢弃
    /// 重复任务的客侧处理程序。
    task_ids: BTreeMap<u32, u32>,
    next_id: u32,
}

impl LazyTaskHandlers {
    /// 注册一个新的任务处理器，并返回其唯一 ID。
    pub fn register(&mut self, handler: TaskHandler, one_shot: bool) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.insert(id, Task { handler, one_shot });
        id
    }

    /// 记录哪个宿主任务 id 分发到 `handler_id`。
    pub fn link_task(&mut self, task_id: u32, handler_id: u32) {
        self.task_ids.insert(task_id, handler_id);
    }

    /// 返回 `id` 对应的处理器，并从中取出一次性处理器。
    /// 表（它的首次调用也是最后一次调用）。
    #[must_use]
    pub fn get_for_invocation(&mut self, id: u32) -> Option<TaskHandler> {
        match self.handlers.get(&id) {
            Some(task) if task.one_shot => {
                let task = self.handlers.remove(&id);
                self.task_ids.retain(|_, handler_id| *handler_id != id);
                task.map(|task| task.handler)
            }
            Some(task) => Some(Arc::clone(&task.handler)),
            None => None,
        }
    }

    /// 丢弃绑定到宿主任务 `task_id` 的处理器（如果有）。
    pub fn remove_by_task_id(&mut self, task_id: u32) {
        if let Some(handler_id) = self.task_ids.remove(&task_id) {
            self.handlers.remove(&handler_id);
        }
    }
}

/// 在 `Context` 与 `Server` 上提供符合人体工程学的任务调度的扩展 trait。
pub trait SchedulerExt {
    /// 调度一个任务，在指定的刻数后执行一次。
    ///
    /// * `delay_ticks`：执行前等待的游戏刻数。
    /// * `handler`：要执行的闭包。
    ///
    ///返回唯一的任务 ID。
    fn schedule_delayed_task<F>(&self, delay_ticks: u64, handler: F) -> u32
    where
        F: Fn(Server) + Send + Sync + 'static;

    /// 调度一个重复执行的任务。
    ///
    /// * `delay_ticks`：首次执行前等待的游戏刻数。
    /// * `period_ticks`：后续两次执行之间相隔的刻数。
    /// * `handler`：要执行的闭包。
    ///
    ///返回唯一的任务 ID。
    fn schedule_repeating_task<F>(&self, delay_ticks: u64, period_ticks: u64, handler: F) -> u32
    where
        F: Fn(Server) + Send + Sync + 'static;

    /// 调度一个任务，经过真实时钟
    /// 延迟，独立于刻循环。
    ///
    /// * `delay_ms`：执行前的墙上时钟延迟（毫秒）。
    /// * `handler`：要执行的闭包。
    ///
    ///返回唯一的任务 ID。
    fn schedule_async_delayed_task<F>(&self, delay_ms: u64, handler: F) -> u32
    where
        F: Fn(Server) + Send + Sync + 'static;

    /// 调度一个任务，在异步执行器上重复运行，周期为
    /// 真实时间周期，独立于刻循环。
    ///
    /// * `delay_ms`：首次执行前的墙上时钟延迟（毫秒）。
    /// * `period_ms`：两次执行之间的墙上时钟周期（毫秒）。
    /// * `handler`：要执行的闭包。
    ///
    ///返回唯一的任务 ID。
    fn schedule_async_repeating_task<F>(&self, delay_ms: u64, period_ms: u64, handler: F) -> u32
    where
        F: Fn(Server) + Send + Sync + 'static;
}

impl SchedulerExt for crate::Context {
    fn schedule_delayed_task<F>(&self, delay_ticks: u64, handler: F) -> u32
    where
        F: Fn(Server) + Send + Sync + 'static,
    {
        schedule_delayed_task(delay_ticks, handler)
    }

    fn schedule_repeating_task<F>(&self, delay_ticks: u64, period_ticks: u64, handler: F) -> u32
    where
        F: Fn(Server) + Send + Sync + 'static,
    {
        schedule_repeating_task(delay_ticks, period_ticks, handler)
    }

    fn schedule_async_delayed_task<F>(&self, delay_ms: u64, handler: F) -> u32
    where
        F: Fn(Server) + Send + Sync + 'static,
    {
        schedule_async_delayed_task(delay_ms, handler)
    }

    fn schedule_async_repeating_task<F>(&self, delay_ms: u64, period_ms: u64, handler: F) -> u32
    where
        F: Fn(Server) + Send + Sync + 'static,
    {
        schedule_async_repeating_task(delay_ms, period_ms, handler)
    }
}

impl SchedulerExt for crate::Server {
    fn schedule_delayed_task<F>(&self, delay_ticks: u64, handler: F) -> u32
    where
        F: Fn(Self) + Send + Sync + 'static,
    {
        schedule_delayed_task(delay_ticks, handler)
    }

    fn schedule_repeating_task<F>(&self, delay_ticks: u64, period_ticks: u64, handler: F) -> u32
    where
        F: Fn(Self) + Send + Sync + 'static,
    {
        schedule_repeating_task(delay_ticks, period_ticks, handler)
    }

    fn schedule_async_delayed_task<F>(&self, delay_ms: u64, handler: F) -> u32
    where
        F: Fn(Self) + Send + Sync + 'static,
    {
        schedule_async_delayed_task(delay_ms, handler)
    }

    fn schedule_async_repeating_task<F>(&self, delay_ms: u64, period_ms: u64, handler: F) -> u32
    where
        F: Fn(Self) + Send + Sync + 'static,
    {
        schedule_async_repeating_task(delay_ms, period_ms, handler)
    }
}

/// 提供符合人体工程学的、绑定实体生命周期的任务
/// 实体上的调度（即 Bukkit 的 `EntityScheduler`）。任务会被静默
/// 一旦实体不再存在便会被跳过——重复任务也随之停止——
/// 服务器上存在的。
pub trait EntitySchedulerExt {
    /// 调度一个任务，在指定数量的
    /// 刻，绑定到此实体的生命周期。
    ///
    /// * `delay_ticks`：执行前等待的游戏刻数。
    /// * `handler`：要执行的闭包。
    ///
    ///返回唯一的任务 ID。
    fn schedule_entity_delayed_task<F>(&self, delay_ticks: u64, handler: F) -> u32
    where
        F: Fn(Server) + Send + Sync + 'static;

    /// 调度一个重复执行的任务，绑定到此实体的
    /// 生命周期。一旦实体不再
    /// 不再存在于服务器上。
    ///
    /// * `delay_ticks`：首次执行前等待的游戏刻数。
    /// * `period_ticks`：后续两次执行之间相隔的刻数。
    /// * `handler`：要执行的闭包。
    ///
    ///返回唯一的任务 ID。
    fn schedule_entity_repeating_task<F>(
        &self,
        delay_ticks: u64,
        period_ticks: u64,
        handler: F,
    ) -> u32
    where
        F: Fn(Server) + Send + Sync + 'static;
}

impl EntitySchedulerExt for Entity {
    fn schedule_entity_delayed_task<F>(&self, delay_ticks: u64, handler: F) -> u32
    where
        F: Fn(Server) + Send + Sync + 'static,
    {
        schedule_entity_delayed_task(self, delay_ticks, handler)
    }

    fn schedule_entity_repeating_task<F>(
        &self,
        delay_ticks: u64,
        period_ticks: u64,
        handler: F,
    ) -> u32
    where
        F: Fn(Server) + Send + Sync + 'static,
    {
        schedule_entity_repeating_task(self, delay_ticks, period_ticks, handler)
    }
}

/// 调度延迟任务的底层函数。
/// 推荐使用 [`SchedulerExt`] 以获得更符合人体工程学的 API。
pub fn schedule_delayed_task<F>(delay_ticks: u64, handler: F) -> u32
where
    F: Fn(Server) + Send + Sync + 'static,
{
    let handler_id = TASK_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .register(Arc::new(handler), true);
    let task_id = scheduler::schedule_delayed_task(handler_id, delay_ticks);
    TASK_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .link_task(task_id, handler_id);
    task_id
}

/// 调度重复任务的底层函数。
/// 推荐使用 [`SchedulerExt`] 以获得更符合人体工程学的 API。
pub fn schedule_repeating_task<F>(delay_ticks: u64, period_ticks: u64, handler: F) -> u32
where
    F: Fn(Server) + Send + Sync + 'static,
{
    let handler_id = TASK_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .register(Arc::new(handler), false);
    let task_id = scheduler::schedule_repeating_task(handler_id, delay_ticks, period_ticks);
    TASK_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .link_task(task_id, handler_id);
    task_id
}

/// 调度一个任务，在真实时钟延迟后在异步执行器上运行一次。
/// 推荐使用 [`SchedulerExt`] 以获得更符合人体工程学的 API。
pub fn schedule_async_delayed_task<F>(delay_ms: u64, handler: F) -> u32
where
    F: Fn(Server) + Send + Sync + 'static,
{
    let handler_id = TASK_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .register(Arc::new(handler), true);
    let task_id = scheduler::schedule_async_delayed_task(handler_id, delay_ms);
    TASK_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .link_task(task_id, handler_id);
    task_id
}

/// 调度一个任务，以真实时钟周期在异步执行器上重复运行。
/// 推荐使用 [`SchedulerExt`] 以获得更符合人体工程学的 API。
pub fn schedule_async_repeating_task<F>(delay_ms: u64, period_ms: u64, handler: F) -> u32
where
    F: Fn(Server) + Send + Sync + 'static,
{
    let handler_id = TASK_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .register(Arc::new(handler), false);
    let task_id = scheduler::schedule_async_repeating_task(handler_id, delay_ms, period_ms);
    TASK_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .link_task(task_id, handler_id);
    task_id
}

/// 调度一个任务，在给定的刻延迟后运行一次，绑定到该
/// 实体生命周期。当实体不再
/// 不再存在于服务器上。
/// 推荐使用 [`EntitySchedulerExt`] 以获得更符合人体工程学的 API。
pub fn schedule_entity_delayed_task<F>(entity: &Entity, delay_ticks: u64, handler: F) -> u32
where
    F: Fn(Server) + Send + Sync + 'static,
{
    let handler_id = TASK_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .register(Arc::new(handler), true);
    let task_id = scheduler::schedule_entity_delayed_task(handler_id, entity.get_id(), delay_ticks);
    TASK_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .link_task(task_id, handler_id);
    task_id
}

/// 调度一个重复运行的任务，绑定到该实体的生命周期。该
/// 一旦实体不再位于该
/// 服务器。
/// 推荐使用 [`EntitySchedulerExt`] 以获得更符合人体工程学的 API。
pub fn schedule_entity_repeating_task<F>(
    entity: &Entity,
    delay_ticks: u64,
    period_ticks: u64,
    handler: F,
) -> u32
where
    F: Fn(Server) + Send + Sync + 'static,
{
    let handler_id = TASK_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .register(Arc::new(handler), false);
    let task_id = scheduler::schedule_entity_repeating_task(
        handler_id,
        entity.get_id(),
        delay_ticks,
        period_ticks,
    );
    TASK_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .link_task(task_id, handler_id);
    task_id
}

/// 取消一个已调度的任务。
///
/// 对墙上时钟（异步）任务则立即生效：挂起的 sleep 会被中断
/// 而不是留待下一个应执行时间点再运行。访客侧处理器
/// 为该任务注册的资源也会释放；一次性处理器同样会
/// 在首次调用后自动释放。
pub fn cancel_task(task_id: u32) {
    TASK_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove_by_task_id(task_id);
    scheduler::cancel_task(task_id);
}
