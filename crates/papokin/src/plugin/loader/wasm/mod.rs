use std::{any::Any, path::Path, sync::Arc};

use wasm_host::{
    PluginRuntime, WasmPlugin,
    concurrent_store::{LegacySyncReentry, TokioSpawner},
};

use crate::plugin::{
    Context, Plugin, PluginFuture,
    loader::{PluginLoadFuture, PluginLoader, PluginUnloadFuture},
};

pub mod wasm_host;

impl Plugin for WasmPlugin {
    fn on_load(&self, context: Arc<Context>) -> PluginFuture<'_, Result<(), String>> {
        Box::pin(async move {
            // 使用更明确的限定语法，避免递归调用当前的 on_load 函数，改为调用
            // WasmPlugin::on_load
            Self::on_load(self, context)
                .await
                .map_err(|err| err.to_string())
                .flatten()
        })
    }

    fn on_enable(&self, context: Arc<Context>) -> PluginFuture<'_, Result<(), String>> {
        Box::pin(async move {
            Self::on_enable(self, context)
                .await
                .map_err(|err| err.to_string())
                .flatten()
        })
    }

    fn on_disable(&self, context: Arc<Context>) -> PluginFuture<'_, Result<(), String>> {
        Box::pin(async move {
            Self::on_disable(self, context)
                .await
                .map_err(|err| err.to_string())
                .flatten()
        })
    }

    fn on_unload(&self, context: Arc<Context>) -> PluginFuture<'_, Result<(), String>> {
        Box::pin(async move {
            Self::on_unload(self, context)
                .await
                .map_err(|err| err.to_string())
                .flatten()
        })
    }

    fn on_ipc_message(
        &self,
        sender: &str,
        message: &[u8],
    ) -> PluginFuture<'_, Result<Vec<u8>, String>> {
        let sender_own = sender.to_owned();
        let message_own = message.to_owned();
        Box::pin(async move {
            self.handle_ipc_message(&sender_own, &message_own)
                .await
                .map_err(|err| err.to_string())
                .flatten()
        })
    }

    fn on_plugin_message(
        &self,
        player_uuid: uuid::Uuid,
        channel: &str,
        data: &[u8],
    ) -> PluginFuture<'_, Result<(), String>> {
        let channel_own = channel.to_owned();
        let data_own = data.to_vec();
        Box::pin(async move {
            self.handle_plugin_message(player_uuid, channel_own, data_own)
                .await
                .map_err(|err| err.to_string())
        })
    }
}

pub struct WasmPluginLoader {
    verify_signatures: bool,
    legacy_sync_reentry: LegacySyncReentry,
}

impl WasmPluginLoader {
    #[must_use]
    pub fn new(verify_signatures: bool) -> Self {
        Self {
            verify_signatures,
            legacy_sync_reentry: LegacySyncReentry::new(),
        }
    }
}

impl PluginLoader for WasmPluginLoader {
    fn load<'a>(&'a self, path: &'a Path) -> PluginLoadFuture<'a> {
        Box::pin(async {
            let path = path.to_owned();

            let spawner = Arc::new(TokioSpawner::new(tokio::runtime::Handle::current()));
            let mut runtime = PluginRuntime::new(&path, self.legacy_sync_reentry.clone(), spawner)?;
            let (plugin, metadata) = runtime.init_plugin(&path, self.verify_signatures).await?;
            // epoch ticker 必须随插件存活：PluginRuntime 在 load 返回后
            // 即被 drop，ticker 若随之停止，epoch 不再递增，
            // `set_epoch_budget` 的调用超时便永不触发。
            *plugin
                .epoch_ticker
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = runtime.take_epoch_ticker();

            Ok((
                plugin as Arc<dyn Plugin>,
                metadata,
                Box::new(()) as Box<dyn Any + Send + Sync>,
            ))
        })
    }

    fn can_load(&self, path: &Path) -> bool {
        let ext = path.extension().unwrap_or_default();

        ext.eq_ignore_ascii_case("wasm")
    }

    fn unload(&self, _data: Box<dyn Any + Send + Sync>) -> PluginUnloadFuture<'_> {
        Box::pin(async { Ok(()) })
    }

    fn can_unload(&self) -> bool {
        true
    }

    fn reentry_policy(&self) -> Option<LegacySyncReentry> {
        Some(self.legacy_sync_reentry.clone())
    }
}
