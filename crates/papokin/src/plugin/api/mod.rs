pub mod context;
pub mod events;
pub mod gui;
pub mod tab_list;
pub mod title;

use std::{pin::Pin, sync::Arc};

pub use context::*;
pub use events::*;
pub use tab_list::*;
pub use title::*;

/// 表示插件元数据的结构体。
///
/// 此结构体包含插件的关键信息，包括其名称、
/// 版本、作者和描述。它对生命周期 `'s` 泛型化，以允许
/// 返回在插件元数据生命周期内有效的字符串切片。
#[derive(Debug, Clone)]
pub struct PluginMetadata {
    /// 插件名称。
    pub name: String,
    /// 插件的版本。
    pub version: String,
    /// 插件的作者。
    pub authors: Vec<String>,
    /// 插件的描述。
    pub description: String,
    /// 硬依赖：缺少其中任何一项时插件将加载失败。
    pub dependencies: Vec<String>,
    /// 插件请求的权限。
    pub permissions: Vec<String>,
    /// 软排序边：当指定的插件存在时，将本插件排在这些插件之后加载
    /// 存在时才会包含。缺失的名称会被忽略。
    pub load_after: Vec<String>,
    /// 软排序边：当指定的插件存在时，将本插件排在这些插件之前加载
    /// 存在时才会包含。缺失的名称会被忽略。
    pub load_before: Vec<String>,
    /// 本插件满足的、供其他插件声明依赖的
    /// 边缘。
    pub provides: Vec<String>,
    /// 此插件加载时所处的启动阶段。
    pub load_order: LoadOrder,
}

/// 相对于服务器启动，插件应在何时加载。
///
/// 默认为 [`LoadOrder::PostWorld`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoadOrder {
    /// 在世界创建之前加载（引导阶段）。
    Startup,
    /// 在世界就绪后加载（默认）。
    #[default]
    PostWorld,
}

/// 此类型表示插件所用的 future。
pub type PluginFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// 表示具有异步生命周期方法的插件的 trait。
///
/// 此 trait 定义了插件所需的方法，包括相应事件发生时的钩子
/// 插件被加载和卸载。
pub trait Plugin: Send + Sync + 'static {
    /// 插件被加载时调用的异步方法。
    ///
    /// 此方法在服务器上下文中初始化插件。
    #[expect(unused)]
    fn on_load(&self, server: Arc<Context>) -> PluginFuture<'_, Result<(), String>> {
        Box::pin(async move { Ok(()) })
    }

    /// 插件成功加载后用于启用插件的异步方法。
    ///
    /// 启用失败不会卸载插件（与 [`Plugin::on_load`] 不同）：
    /// 插件保持已加载但处于非活动状态——其事件处理器与命令
    /// 会被注销——对应 Paper 的 `onEnable` 失败分级。
    #[expect(unused)]
    fn on_enable(&self, server: Arc<Context>) -> PluginFuture<'_, Result<(), String>> {
        Box::pin(async move { Ok(()) })
    }

    /// 用于停用已启用插件的异步方法。
    ///
    /// 在关停/卸载期间于 [`Plugin::on_unload`] 之前运行，并且在
    /// 插件已被显式禁用。
    #[expect(unused)]
    fn on_disable(&self, server: Arc<Context>) -> PluginFuture<'_, Result<(), String>> {
        Box::pin(async move { Ok(()) })
    }

    /// 插件被卸载时调用的异步方法。
    ///
    /// 当插件从服务器上下文中移除时，此方法会清理资源。
    #[expect(unused)]
    fn on_unload(&self, server: Arc<Context>) -> PluginFuture<'_, Result<(), String>> {
        Box::pin(async move { Ok(()) })
    }

    /// 插件收到 IPC 消息时调用的异步方法。
    ///
    /// 此方法处理消息，并可选择返回响应
    #[expect(unused)]
    fn on_ipc_message(
        &self,
        sender: &str,
        message: &[u8],
    ) -> PluginFuture<'_, Result<Vec<u8>, String>> {
        Box::pin(async move { Err("此插件无法接收消息。".to_string()) })
    }

    /// 异步方法，当玩家在某个通道上发送插件消息时调用
    /// 该插件注册的通道（Bukkit `PluginMessageListener`）。
    #[allow(unused_variables)]
    fn on_plugin_message(
        &self,
        _player_uuid: uuid::Uuid,
        _channel: &str,
        _data: &[u8],
    ) -> PluginFuture<'_, Result<(), String>> {
        Box::pin(async move { Ok(()) })
    }
}
