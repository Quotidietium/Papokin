use std::sync::atomic::{AtomicU64, Ordering};

use tracing_serde_structured::AsSerde;

use crate::wit;

/// 通过 WIT 将事件转发给宿主服务器的 [`tracing::Subscriber`]。
///
/// 插件加载时自动安装为全局 subscriber。
pub(crate) struct WitSubscriber {
    next_id: AtomicU64,
}

impl WitSubscriber {
    /// 创建 span ID 计数器从 `1` 开始的新 `WitSubscriber`。
    pub const fn new() -> Self {
        Self {
            next_id: AtomicU64::new(1),
        }
    }
}

impl tracing::Subscriber for WitSubscriber {
    /// 总是返回 `true`——所有日志级别都会转发给宿主。
    fn enabled(&self, _metadata: &tracing::Metadata<'_>) -> bool {
        true
    }

    /// 分配一个新的单调递增 span ID。
    fn new_span(&self, _attrs: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        tracing::span::Id::from_u64(id)
    }

    /// 空操作——span 字段记录不转发给宿主。
    fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}

    /// 空操作——因果链接不转发给宿主。
    fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {}

    /// 使用 `postcard` 序列化跟踪事件，并通过 WIT 将其发送到宿主。
    fn event(&self, event: &tracing::Event<'_>) {
        if let Ok(serialized) = postcard::to_allocvec(&event.as_serde()) {
            wit::papokin::plugin::logging::log_tracing(&serialized);
        }
    }

    /// 空操作——不跟踪 span 进入。
    fn enter(&self, _span: &tracing::span::Id) {}

    /// 空操作——不跟踪 span 退出。
    fn exit(&self, _span: &tracing::span::Id) {}
}

/// 与 [`log`] 配合使用的日志严重级别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// 非常细粒度的诊断信息。
    Trace,
    /// 开发期间有用的诊断信息。
    Debug,
    /// 一般信息性消息。
    Info,
    /// 有潜在危害但不中断执行的情况。
    Warn,
    /// 可能需要关注的错误。
    Error,
}

impl LogLevel {
    /// 将该级别转换为宿主期望的 WIT 生成类型 `Level`。
    const fn to_wit(self) -> wit::papokin::plugin::logging::Level {
        match self {
            Self::Trace => wit::papokin::plugin::logging::Level::Trace,
            Self::Debug => wit::papokin::plugin::logging::Level::Debug,
            Self::Info => wit::papokin::plugin::logging::Level::Info,
            Self::Warn => wit::papokin::plugin::logging::Level::Warn,
            Self::Error => wit::papokin::plugin::logging::Level::Error,
        }
    }
}

/// 以给定的严重级别向服务器发送日志消息。
///
/// 推荐使用标准 `tracing` 宏（`tracing::info!`、`tracing::warn!` 等）
/// 用于结构化日志；当需要直接的、低层级的日志调用时使用此函数。
pub fn log(level: LogLevel, message: &str) {
    wit::papokin::plugin::logging::log(level.to_wit(), message);
}
