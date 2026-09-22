use crate::plugin::loader::wasm::wasm_host::{
    state::PluginHostState, wit::v0_1::papokin::plugin::scheduler,
};
use std::sync::atomic::Ordering;

impl scheduler::Host for PluginHostState {
    async fn schedule_delayed_task(
        &mut self,
        handler_id: u32,
        delay: u64,
    ) -> wasmtime::Result<u32> {
        let plugin = self
            .plugin
            .as_ref()
            .and_then(std::sync::Weak::upgrade)
            .ok_or_else(|| wasmtime::Error::msg("未找到插件"))?;
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("未找到服务器"))?;
        let tick_count = server.tick_count.load(Ordering::Relaxed) as u64;
        let task_id = server
            .task_scheduler
            .schedule_delayed_task(plugin, handler_id, delay, tick_count);
        Ok(task_id)
    }

    async fn schedule_repeating_task(
        &mut self,
        handler_id: u32,
        delay: u64,
        period: u64,
    ) -> wasmtime::Result<u32> {
        let plugin = self
            .plugin
            .as_ref()
            .and_then(std::sync::Weak::upgrade)
            .ok_or_else(|| wasmtime::Error::msg("未找到插件"))?;
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("未找到服务器"))?;
        let tick_count = server.tick_count.load(Ordering::Relaxed) as u64;
        let task_id = server
            .task_scheduler
            .schedule_repeating_task(plugin, handler_id, delay, period, tick_count);
        Ok(task_id)
    }

    async fn schedule_async_delayed_task(
        &mut self,
        handler_id: u32,
        delay_ms: u64,
    ) -> wasmtime::Result<u32> {
        let plugin = self
            .plugin
            .as_ref()
            .and_then(std::sync::Weak::upgrade)
            .ok_or_else(|| wasmtime::Error::msg("未找到插件"))?;
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("未找到服务器"))?;
        let task_id = server
            .task_scheduler
            .schedule_async_delayed_task(server, plugin, handler_id, delay_ms);
        Ok(task_id)
    }

    async fn schedule_async_repeating_task(
        &mut self,
        handler_id: u32,
        delay_ms: u64,
        period_ms: u64,
    ) -> wasmtime::Result<u32> {
        let plugin = self
            .plugin
            .as_ref()
            .and_then(std::sync::Weak::upgrade)
            .ok_or_else(|| wasmtime::Error::msg("未找到插件"))?;
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("未找到服务器"))?;
        let task_id = server
            .task_scheduler
            .schedule_async_repeating_task(server, plugin, handler_id, delay_ms, period_ms);
        Ok(task_id)
    }

    async fn schedule_entity_delayed_task(
        &mut self,
        handler_id: u32,
        entity_id: u32,
        delay: u64,
    ) -> wasmtime::Result<u32> {
        let plugin = self
            .plugin
            .as_ref()
            .and_then(std::sync::Weak::upgrade)
            .ok_or_else(|| wasmtime::Error::msg("未找到插件"))?;
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("未找到服务器"))?;
        let tick_count = server.tick_count.load(Ordering::Relaxed) as u64;
        let task_id = server.task_scheduler.schedule_entity_delayed_task(
            plugin,
            handler_id,
            entity_id as i32,
            delay,
            tick_count,
        );
        Ok(task_id)
    }

    async fn schedule_entity_repeating_task(
        &mut self,
        handler_id: u32,
        entity_id: u32,
        delay: u64,
        period: u64,
    ) -> wasmtime::Result<u32> {
        let plugin = self
            .plugin
            .as_ref()
            .and_then(std::sync::Weak::upgrade)
            .ok_or_else(|| wasmtime::Error::msg("未找到插件"))?;
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("未找到服务器"))?;
        let tick_count = server.tick_count.load(Ordering::Relaxed) as u64;
        let task_id = server.task_scheduler.schedule_entity_repeating_task(
            plugin,
            handler_id,
            entity_id as i32,
            delay,
            period,
            tick_count,
        );
        Ok(task_id)
    }

    async fn cancel_task(&mut self, task_id: u32) -> wasmtime::Result<()> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("未找到服务器"))?;
        server.task_scheduler.cancel_task(task_id);
        Ok(())
    }
}
