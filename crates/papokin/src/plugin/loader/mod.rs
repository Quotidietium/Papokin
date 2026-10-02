use crate::plugin::{PluginMetadata, api::Plugin, loader::wasm::wasm_host::PluginInitError};
use std::{any::Any, path::Path, pin::Pin, sync::Arc};
use thiserror::Error;

pub mod native;
pub mod wasm;

pub type PluginLoadFuture<'a> = Pin<
    Box<
        dyn Future<
                Output = Result<
                    (Arc<dyn Plugin>, PluginMetadata, Box<dyn Any + Send + Sync>),
                    LoaderError,
                >,
            > + Send
            + 'a,
    >,
>;

pub type PluginUnloadFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), LoaderError>> + Send + 'a>>;

pub trait PluginLoader: Send + Sync {
    /// 从指定路径加载插件
    fn load<'a>(&'a self, path: &'a Path) -> PluginLoadFuture<'a>;

    /// 检查此加载器能否处理给定文件
    fn can_load(&self, path: &Path) -> bool;

    fn unload(&self, data: Box<dyn Any + Send + Sync>) -> PluginUnloadFuture<'_>;

    /// 检查插件能否被安全卸载。
    fn can_unload(&self) -> bool;

    /// 该加载器创建的 Store 所属的重入准入策略（仅 Wasm 加载器有）。
    /// 插件管理器用它把派生的初始化任务包进调用方的因果链：
    /// 插件经宿主 API 动态加载新插件时，调用链正持有根准入，
    /// 初始化任务若不继承该链而另行申请根准入（信号量容量为 1），
    /// 双方会互相等待形成自锁。
    fn reentry_policy(&self) -> Option<papokin_plugin_runtime::LegacySyncReentry> {
        None
    }
}

/// 统一的加载器错误类型
#[derive(Error, Debug)]
pub enum LoaderError {
    #[error("加载库失败：{0}")]
    LibraryLoad(String),

    #[error("缺少插件元数据")]
    MetadataMissing,

    #[error("缺少插件入口点")]
    EntrypointMissing,

    #[error("插件初始化失败：{0}")]
    InitializationFailed(String),

    #[error("运行时错误：{0}")]
    RuntimeError(String),

    #[error("无效的加载器数据")]
    InvalidLoaderData,

    #[error("插件针对不兼容的 API 版本构建。请针对此 Pumpkin 版本重新构建。")]
    ApiVersionMissing,

    #[error(
        "插件 API 版本不匹配（插件 {plugin_version}，服务器 {server_version}）。请针对此 Pumpkin 版本重新构建。"
    )]
    ApiVersionMismatch {
        plugin_version: u32,
        server_version: u32,
    },

    #[error("Wasm 插件初始化错误：{0}")]
    WasmInitializationError(#[from] PluginInitError),
}
