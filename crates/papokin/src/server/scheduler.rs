use crate::plugin::loader::wasm::wasm_host::WasmPlugin;
use crate::server::Server;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::atomic::Ordering as AtomicOrdering;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

pub type TaskId = u32;

pub struct ScheduledTask {
    pub id: TaskId,
    pub plugin: Arc<WasmPlugin>,
    pub handler_id: u32,
    pub next_tick: u64,
    pub period: Option<u64>,
    /// 设置后，任务会绑定到实体的生命周期：它会被静默
    /// 一旦实体不再存在，便被跳过（且不再重新调度）
    /// 在任何世界中（Bukkit `EntityScheduler` 的退役语义）。
    pub entity_id: Option<i32>,
}

impl PartialEq for ScheduledTask {
    fn eq(&self, other: &Self) -> bool {
        self.next_tick == other.next_tick
    }
}

impl Eq for ScheduledTask {}

impl PartialOrd for ScheduledTask {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledTask {
    fn cmp(&self, other: &Self) -> Ordering {
        // 反转顺序使 BinaryHeap 成为最小堆
        other.next_tick.cmp(&self.next_tick)
    }
}

/// 通过调用插件的 guest `handle-task` 导出函数来运行插件任务处理器。
///
/// 由刻对齐调度器与墙钟异步调度器共用。
async fn run_task_handler(plugin: Arc<WasmPlugin>, server: Arc<Server>, handler_id: u32) {
    let function = match plugin.plugin_instance.as_ref() {
        crate::plugin::loader::wasm::wasm_host::PluginInstance::V0_1(instance) => {
            instance.func_handle_task()
        }
    };
    if let Err(error) =
        plugin
            .store
            .call_guest(move |mut guest| {
                Box::pin(async move {
                    let (server_resource, server_rep) = guest.with(|mut store| {
                        let resource = store.data_mut().add_server(server)?;
                        let rep = resource.rep();
                        Ok::<_, wasmtime::Error>((resource, rep))
                    })?;
                    let result = guest.call(function, (handler_id, server_resource)).await;
                    guest.with(|mut store| {
                        let _ = store.data_mut().resource_table.delete::<
                        crate::plugin::loader::wasm::wasm_host::state::ServerResource,
                    >(wasmtime::component::Resource::new_own(server_rep));
                    });
                    result
                })
            })
            .await
    {
        tracing::error!(handler_id, %error, "Wasm 调度任务失败");
    }
}

pub struct TaskScheduler {
    tasks: Mutex<BinaryHeap<ScheduledTask>>,
    cancelled_tasks: Mutex<HashSet<TaskId>>,
    disabled_plugins: Mutex<Vec<Weak<WasmPlugin>>>,
    /// 墙钟异步任务的取消句柄，以任务 ID 为键。
    async_tasks: Mutex<HashMap<TaskId, AsyncTaskEntry>>,
    next_task_id: std::sync::atomic::AtomicU32,
}

struct AsyncTaskEntry {
    cancel: tokio_util::sync::CancellationToken,
    plugin: Weak<WasmPlugin>,
}

impl Default for TaskScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskScheduler {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tasks: Mutex::new(BinaryHeap::new()),
            cancelled_tasks: Mutex::new(HashSet::new()),
            disabled_plugins: Mutex::new(Vec::new()),
            async_tasks: Mutex::new(HashMap::new()),
            next_task_id: std::sync::atomic::AtomicU32::new(0),
        }
    }

    pub fn schedule_delayed_task(
        &self,
        plugin: Arc<WasmPlugin>,
        handler_id: u32,
        delay: u64,
        current_tick: u64,
    ) -> TaskId {
        let id = self.next_task_id.fetch_add(1, AtomicOrdering::SeqCst);
        let mut disabled_plugins = self
            .disabled_plugins
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if Self::is_plugin_disabled(&mut disabled_plugins, &plugin) {
            return id;
        }
        let task = ScheduledTask {
            id,
            plugin,
            handler_id,
            next_tick: current_tick.saturating_add(delay),
            period: None,
            entity_id: None,
        };
        self.tasks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(task);
        id
    }

    pub fn schedule_repeating_task(
        &self,
        plugin: Arc<WasmPlugin>,
        handler_id: u32,
        delay: u64,
        period: u64,
        current_tick: u64,
    ) -> TaskId {
        let id = self.next_task_id.fetch_add(1, AtomicOrdering::SeqCst);
        let mut disabled_plugins = self
            .disabled_plugins
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if Self::is_plugin_disabled(&mut disabled_plugins, &plugin) {
            return id;
        }
        let task = ScheduledTask {
            id,
            plugin,
            handler_id,
            next_tick: current_tick.saturating_add(delay),
            period: Some(period),
            entity_id: None,
        };
        self.tasks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(task);
        id
    }

    /// 调度一个任务，在给定的刻延迟后运行一次，绑定到某个
    /// 实体生命周期。当任务到期而实体不再
    /// 存在于任何世界中，任务将被静默跳过（Bukkit
    /// `EntityScheduler`）。
    pub fn schedule_entity_delayed_task(
        &self,
        plugin: Arc<WasmPlugin>,
        handler_id: u32,
        entity_id: i32,
        delay: u64,
        current_tick: u64,
    ) -> TaskId {
        let id = self.next_task_id.fetch_add(1, AtomicOrdering::SeqCst);
        let mut disabled_plugins = self
            .disabled_plugins
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if Self::is_plugin_disabled(&mut disabled_plugins, &plugin) {
            return id;
        }
        let task = ScheduledTask {
            id,
            plugin,
            handler_id,
            next_tick: current_tick.saturating_add(delay),
            period: None,
            entity_id: Some(entity_id),
        };
        self.tasks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(task);
        id
    }

    /// 调度一个重复运行的任务，绑定到实体的生命周期。该
    /// 一旦实体不再存在于
    /// 当运行到期时，可在任意世界中执行。
    pub fn schedule_entity_repeating_task(
        &self,
        plugin: Arc<WasmPlugin>,
        handler_id: u32,
        entity_id: i32,
        delay: u64,
        period: u64,
        current_tick: u64,
    ) -> TaskId {
        let id = self.next_task_id.fetch_add(1, AtomicOrdering::SeqCst);
        let mut disabled_plugins = self
            .disabled_plugins
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if Self::is_plugin_disabled(&mut disabled_plugins, &plugin) {
            return id;
        }
        let task = ScheduledTask {
            id,
            plugin,
            handler_id,
            next_tick: current_tick.saturating_add(delay),
            period: Some(period),
            entity_id: Some(entity_id),
        };
        self.tasks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(task);
        id
    }

    /// 调度一个在真实时钟延迟后运行一次的任务，不依赖于
    /// 刻循环（Bukkit 的 `runTaskLaterAsynchronously`）。延迟为零时会立即运行
    /// 在异步执行器上立即执行该任务。
    pub fn schedule_async_delayed_task(
        &self,
        server: &Arc<Server>,
        plugin: Arc<WasmPlugin>,
        handler_id: u32,
        delay_ms: u64,
    ) -> TaskId {
        let id = self.next_task_id.fetch_add(1, AtomicOrdering::SeqCst);
        {
            let mut disabled_plugins = self
                .disabled_plugins
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if Self::is_plugin_disabled(&mut disabled_plugins, &plugin) {
                return id;
            }
        }

        let cancel = tokio_util::sync::CancellationToken::new();
        self.async_tasks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                id,
                AsyncTaskEntry {
                    cancel: cancel.clone(),
                    plugin: Arc::downgrade(&plugin),
                },
            );

        let task_server = server.clone();
        server.spawn_task(async move {
            if delay_ms > 0 {
                tokio::select! {
                    () = cancel.cancelled() => {
                        task_server.task_scheduler.finish_async_task(id);
                        return;
                    }
                    () = tokio::time::sleep(Duration::from_millis(delay_ms)) => {}
                }
            }
            if !cancel.is_cancelled() {
                run_task_handler(Arc::clone(&plugin), Arc::clone(&task_server), handler_id).await;
            }
            task_server.task_scheduler.finish_async_task(id);
        });
        id
    }

    /// 调度一个任务，在异步执行器上重复运行，周期为
    /// 真实时间周期，独立于刻循环（Bukkit 的
    /// `runTaskTimerAsynchronously`）。该任务会持续运行，直到被取消，
    /// 插件被禁用，或插件被卸载。
    pub fn schedule_async_repeating_task(
        &self,
        server: &Arc<Server>,
        plugin: Arc<WasmPlugin>,
        handler_id: u32,
        delay_ms: u64,
        period_ms: u64,
    ) -> TaskId {
        let id = self.next_task_id.fetch_add(1, AtomicOrdering::SeqCst);
        {
            let mut disabled_plugins = self
                .disabled_plugins
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if Self::is_plugin_disabled(&mut disabled_plugins, &plugin) {
                return id;
            }
        }

        let cancel = tokio_util::sync::CancellationToken::new();
        self.async_tasks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                id,
                AsyncTaskEntry {
                    cancel: cancel.clone(),
                    plugin: Arc::downgrade(&plugin),
                },
            );

        let task_server = server.clone();
        server.spawn_task(async move {
            if delay_ms > 0 {
                tokio::select! {
                    () = cancel.cancelled() => {
                        task_server.task_scheduler.finish_async_task(id);
                        return;
                    }
                    () = tokio::time::sleep(Duration::from_millis(delay_ms)) => {}
                }
            }
            loop {
                if cancel.is_cancelled() {
                    break;
                }
                run_task_handler(Arc::clone(&plugin), Arc::clone(&task_server), handler_id).await;
                if period_ms == 0 {
                    break;
                }
                tokio::select! {
                    () = cancel.cancelled() => break,
                    () = tokio::time::sleep(Duration::from_millis(period_ms)) => {}
                }
            }
            task_server.task_scheduler.finish_async_task(id);
        });
        id
    }

    /// 将异步任务标记为已完成，并丢弃其取消句柄。
    fn finish_async_task(&self, id: TaskId) {
        self.async_tasks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&id);
    }

    pub fn cancel_task(&self, id: TaskId) {
        let async_entry = self
            .async_tasks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&id);
        if let Some(entry) = async_entry {
            // 异步任务 ID 不会进入 `cancelled_tasks`：该集合仅
            // 当与刻对齐的任务从堆中弹出时被消耗。
            entry.cancel.cancel();
        } else {
            // 仅当堆中确实存在该任务时才登记取消：对已完成或
            // 伪造 id 的无条件登记会在 HashSet 中永久堆积（这些
            // id 永远不会从堆中弹出，集合条目无法被消费）。
            // 已弹出待重入队窗口内的取消会失效一轮，该窗口
            // 位于 tick 的同步段内，实际竞态窗口极小。
            let in_heap = self
                .tasks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .iter()
                .any(|task| task.id == id);
            if in_heap {
                self.cancelled_tasks
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(id);
            }
        }
    }

    pub fn disable_plugin(&self, plugin: &Arc<WasmPlugin>) {
        {
            let mut disabled_plugins = self
                .disabled_plugins
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !Self::is_plugin_disabled(&mut disabled_plugins, plugin) {
                disabled_plugins.push(Arc::downgrade(plugin));
            }
        }

        let mut removed_ids = Vec::new();
        let mut tasks = self
            .tasks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut kept = BinaryHeap::new();
        for task in tasks.drain() {
            if Arc::ptr_eq(&task.plugin, plugin) {
                removed_ids.push(task.id);
            } else {
                kept.push(task);
            }
        }
        *tasks = kept;
        drop(tasks);
        // 被移除的任务永远不会从堆中弹出，因此丢弃它们的 id
        // 也应从 `cancelled_tasks` 中一并移除，而不是任其泄漏。
        if !removed_ids.is_empty() {
            let mut cancelled = self
                .cancelled_tasks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for id in removed_ids {
                cancelled.remove(&id);
            }
        }

        // 唤醒该插件的墙钟任务，让访客调度的长时间休眠
        // 在禁用时释放插件（及其存储内存），而不是
        // 取决于睡眠恰好在何时结束。
        let disabled_plugin = Arc::downgrade(plugin);
        self.async_tasks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .retain(|_, entry| {
                let owned = Weak::ptr_eq(&entry.plugin, &disabled_plugin);
                if owned {
                    entry.cancel.cancel();
                }
                !owned
            });
    }

    fn is_plugin_disabled(
        disabled_plugins: &mut Vec<Weak<WasmPlugin>>,
        plugin: &Arc<WasmPlugin>,
    ) -> bool {
        disabled_plugins.retain(|entry| entry.strong_count() > 0);
        let plugin = Arc::downgrade(plugin);
        disabled_plugins
            .iter()
            .any(|entry| Weak::ptr_eq(entry, &plugin))
    }

    pub fn tick(&self, server: &Arc<Server>) {
        let current_tick = server.tick_count.load(AtomicOrdering::Relaxed) as u64;
        let mut tasks_to_run = Vec::new();

        {
            let mut tasks = self
                .tasks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut cancelled = self
                .cancelled_tasks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            while let Some(task) = tasks.peek() {
                if task.next_tick > current_tick {
                    break;
                }

                let Some(task) = tasks.pop() else {
                    break;
                };
                if cancelled.remove(&task.id) {
                    continue;
                }

                tasks_to_run.push(task);
            }
        }

        for mut task in tasks_to_run {
            let mut disabled_plugins = self
                .disabled_plugins
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if Self::is_plugin_disabled(&mut disabled_plugins, &task.plugin) {
                continue;
            }
            drop(disabled_plugins);

            // 绑定实体的任务会在其实体消失后退役；此
            // 同时终止重复的实体任务（下方不会重新调度）。
            if let Some(entity_id) = task.entity_id
                && !server
                    .worlds
                    .load()
                    .iter()
                    .any(|world| world.get_entity_by_id(entity_id).is_some())
            {
                continue;
            }

            // 运行任务
            let plugin = task.plugin.clone();
            let handler_id = task.handler_id;
            let server_clone = server.clone();

            server.spawn_task(run_task_handler(plugin, server_clone, handler_id));

            // 如果是重复执行，调度下一次运行
            if let Some(period) = task.period {
                task.next_tick = current_tick.saturating_add(period);
                let mut disabled_plugins = self
                    .disabled_plugins
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if !Self::is_plugin_disabled(&mut disabled_plugins, &task.plugin) {
                    self.tasks
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .push(task);
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledFunctionEvent {
    pub id: String,
    pub trigger_tick: u64,
    pub function_name: String,
    pub is_tag: bool,
}

impl PartialOrd for ScheduledFunctionEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledFunctionEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        other.trigger_tick.cmp(&self.trigger_tick)
    }
}

#[derive(Default)]
pub struct ScheduledFunctionQueue {
    queue: Mutex<BinaryHeap<ScheduledFunctionEvent>>,
}

impl ScheduledFunctionQueue {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            queue: Mutex::new(BinaryHeap::new()),
        }
    }

    pub fn schedule(
        &self,
        id: String,
        trigger_tick: u64,
        function_name: String,
        is_tag: bool,
        replace: bool,
    ) {
        let mut queue = self
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if replace {
            let mut retained = Vec::new();
            while let Some(event) = queue.pop() {
                if event.id != id {
                    retained.push(event);
                }
            }
            for event in retained {
                queue.push(event);
            }
        }
        queue.push(ScheduledFunctionEvent {
            id,
            trigger_tick,
            function_name,
            is_tag,
        });
    }

    pub fn remove(&self, id: &str) -> usize {
        let mut queue = self
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut count = 0;
        let mut retained = Vec::new();
        while let Some(event) = queue.pop() {
            if event.id == id {
                count += 1;
            } else {
                retained.push(event);
            }
        }
        for event in retained {
            queue.push(event);
        }
        count
    }

    #[must_use]
    pub fn get_event_ids(&self) -> Vec<String> {
        let queue = self
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut ids: Vec<String> = queue.iter().map(|e| e.id.clone()).collect();
        ids.sort();
        ids.dedup();
        ids
    }

    pub fn tick(&self, server: &Arc<Server>, current_tick: u64) {
        let mut to_run = Vec::new();
        {
            let mut queue = self
                .queue
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            while let Some(event) = queue.peek() {
                if event.trigger_tick > current_tick {
                    break;
                }
                if let Some(event) = queue.pop() {
                    to_run.push(event);
                }
            }
        }
        for event in to_run {
            // 失败必须可见：调度链路无玩家反馈通道，
            // 静默吞错会让坏掉的 datapack 函数无人察觉
            if let Err(err) = crate::data::datapack::DatapackManager::execute_function_from_console(
                server,
                &event.function_name,
            ) {
                tracing::warn!(
                    "调度函数 {}（id {}）执行失败：{err}",
                    event.function_name,
                    event.id
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ScheduledFunctionQueue;

    #[test]
    fn schedule_replace_supersedes_prior_same_id_events() {
        let queue = ScheduledFunctionQueue::new();
        queue.schedule("a".into(), 10, "mc:old".into(), false, false);
        // replace 只清除同 id 的事件，其他 id 不受影响
        queue.schedule("a".into(), 20, "mc:new".into(), false, true);
        queue.schedule("b".into(), 30, "mc:other".into(), false, true);
        assert_eq!(
            queue.get_event_ids(),
            vec!["a".to_string(), "b".to_string()]
        );

        // 同 id 多条事件（未 replace）应被 remove 一并清除并计数
        queue.schedule("b".into(), 40, "mc:other2".into(), false, false);
        assert_eq!(queue.remove("b"), 2);
        assert_eq!(queue.get_event_ids(), vec!["a".to_string()]);
    }
}
