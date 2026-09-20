//! Task scheduling API for plugins.
//!
//! This module provides the ability for plugins to schedule closures to be
//! executed after a delay or repeatedly in the server's main tick loop.
//!
//! # Example
//!
//! ```rust,ignore
//! use pumpkin_plugin_api::scheduler::SchedulerExt;
//!
//! context.schedule_delayed_task(20, |server| {
//!     server.log("One second has passed!");
//! });
//! ```

use crate::wit::pumpkin::plugin::context::Server;
use crate::wit::pumpkin::plugin::scheduler;
use crate::wit::pumpkin::plugin::world::Entity;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// A type alias for a closure that can be scheduled as a task.
///
/// Scheduled callbacks can be re-entered, so mutable captured state must use thread-safe
/// interior mutability.
pub type TaskHandler = Arc<dyn Fn(Server) + Send + Sync>;

pub(crate) struct Task {
    handler: TaskHandler,
}

pub(crate) static TASK_HANDLERS: Mutex<LazyTaskHandlers> = Mutex::new(LazyTaskHandlers {
    handlers: BTreeMap::new(),
    next_id: 0,
});

pub(crate) struct LazyTaskHandlers {
    handlers: BTreeMap<u32, Task>,
    next_id: u32,
}

impl LazyTaskHandlers {
    /// Registers a new task handler and returns its unique ID.
    pub fn register(&mut self, handler: TaskHandler) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.insert(id, Task { handler });
        id
    }

    /// Returns the task handler for the given ID.
    #[must_use]
    pub fn get(&self, id: u32) -> Option<TaskHandler> {
        self.handlers.get(&id).map(|task| Arc::clone(&task.handler))
    }
}

/// Extension trait to provide ergonomic task scheduling on `Context` and `Server`.
pub trait SchedulerExt {
    /// Schedules a task to be executed once after the specified number of ticks.
    ///
    /// * `delay_ticks`: Number of game ticks to wait before execution.
    /// * `handler`: Closure to execute.
    ///
    /// Returns a unique task ID.
    fn schedule_delayed_task<F>(&self, delay_ticks: u64, handler: F) -> u32
    where
        F: Fn(Server) + Send + Sync + 'static;

    /// Schedules a task to be executed repeatedly.
    ///
    /// * `delay_ticks`: Number of game ticks to wait before the first execution.
    /// * `period_ticks`: Number of ticks between subsequent executions.
    /// * `handler`: Closure to execute.
    ///
    /// Returns a unique task ID.
    fn schedule_repeating_task<F>(&self, delay_ticks: u64, period_ticks: u64, handler: F) -> u32
    where
        F: Fn(Server) + Send + Sync + 'static;

    /// Schedules a task to run once on the async executor after a wall-clock
    /// delay, independent of the tick loop.
    ///
    /// * `delay_ms`: Wall-clock delay in milliseconds before execution.
    /// * `handler`: Closure to execute.
    ///
    /// Returns a unique task ID.
    fn schedule_async_delayed_task<F>(&self, delay_ms: u64, handler: F) -> u32
    where
        F: Fn(Server) + Send + Sync + 'static;

    /// Schedules a task to run repeatedly on the async executor with a
    /// wall-clock period, independent of the tick loop.
    ///
    /// * `delay_ms`: Wall-clock delay in milliseconds before the first execution.
    /// * `period_ms`: Wall-clock period in milliseconds between executions.
    /// * `handler`: Closure to execute.
    ///
    /// Returns a unique task ID.
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

/// Extension trait to provide ergonomic entity-lifecycle-bound task
/// scheduling on `Entity` (Bukkit's `EntityScheduler`). The task is silently
/// skipped — and a repeating task stops — once the entity is no longer
/// present on the server.
pub trait EntitySchedulerExt {
    /// Schedules a task to be executed once after the specified number of
    /// ticks, bound to this entity's lifetime.
    ///
    /// * `delay_ticks`: Number of game ticks to wait before execution.
    /// * `handler`: Closure to execute.
    ///
    /// Returns a unique task ID.
    fn schedule_entity_delayed_task<F>(&self, delay_ticks: u64, handler: F) -> u32
    where
        F: Fn(Server) + Send + Sync + 'static;

    /// Schedules a task to be executed repeatedly, bound to this entity's
    /// lifetime. The task stops permanently as soon as the entity is no
    /// longer present on the server.
    ///
    /// * `delay_ticks`: Number of game ticks to wait before the first execution.
    /// * `period_ticks`: Number of ticks between subsequent executions.
    /// * `handler`: Closure to execute.
    ///
    /// Returns a unique task ID.
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

/// Lower-level function to schedule a delayed task.
/// Prefer using [`SchedulerExt`] for a more ergonomic API.
pub fn schedule_delayed_task<F>(delay_ticks: u64, handler: F) -> u32
where
    F: Fn(Server) + Send + Sync + 'static,
{
    let handler_id = TASK_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .register(Arc::new(handler));
    scheduler::schedule_delayed_task(handler_id, delay_ticks)
}

/// Lower-level function to schedule a repeating task.
/// Prefer using [`SchedulerExt`] for a more ergonomic API.
pub fn schedule_repeating_task<F>(delay_ticks: u64, period_ticks: u64, handler: F) -> u32
where
    F: Fn(Server) + Send + Sync + 'static,
{
    let handler_id = TASK_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .register(Arc::new(handler));
    scheduler::schedule_repeating_task(handler_id, delay_ticks, period_ticks)
}

/// Schedules a task to run once on the async executor after a wall-clock delay.
/// Prefer using [`SchedulerExt`] for a more ergonomic API.
pub fn schedule_async_delayed_task<F>(delay_ms: u64, handler: F) -> u32
where
    F: Fn(Server) + Send + Sync + 'static,
{
    let handler_id = TASK_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .register(Arc::new(handler));
    scheduler::schedule_async_delayed_task(handler_id, delay_ms)
}

/// Schedules a task to run repeatedly on the async executor with a wall-clock period.
/// Prefer using [`SchedulerExt`] for a more ergonomic API.
pub fn schedule_async_repeating_task<F>(delay_ms: u64, period_ms: u64, handler: F) -> u32
where
    F: Fn(Server) + Send + Sync + 'static,
{
    let handler_id = TASK_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .register(Arc::new(handler));
    scheduler::schedule_async_repeating_task(handler_id, delay_ms, period_ms)
}

/// Schedules a task to run once after the given tick delay, bound to the
/// entity's lifetime. The task is silently skipped when the entity is no
/// longer present on the server.
/// Prefer using [`EntitySchedulerExt`] for a more ergonomic API.
pub fn schedule_entity_delayed_task<F>(entity: &Entity, delay_ticks: u64, handler: F) -> u32
where
    F: Fn(Server) + Send + Sync + 'static,
{
    let handler_id = TASK_HANDLERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .register(Arc::new(handler));
    scheduler::schedule_entity_delayed_task(handler_id, entity.get_id(), delay_ticks)
}

/// Schedules a task to run repeatedly, bound to the entity's lifetime. The
/// task stops permanently as soon as the entity is no longer present on the
/// server.
/// Prefer using [`EntitySchedulerExt`] for a more ergonomic API.
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
        .register(Arc::new(handler));
    scheduler::schedule_entity_repeating_task(
        handler_id,
        entity.get_id(),
        delay_ticks,
        period_ticks,
    )
}

/// Cancels a scheduled task.
pub fn cancel_task(task_id: u32) {
    scheduler::cancel_task(task_id);
}
