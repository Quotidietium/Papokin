use std::{future::Future, pin::Pin};

use thiserror::Error;

pub type SpawnFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

#[derive(Clone, Debug, Error)]
#[error("{message}")]
pub struct SpawnError {
    message: String,
}

impl SpawnError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// 生成运行时工作，而无需选择或构造异步运行时。
pub trait RuntimeSpawner: Send + Sync + 'static {
    /// 转移一个最终必须被轮询或……的 future 的所有权
    /// 生成成功时即被丢弃。
    fn spawn(&self, task: SpawnFuture) -> Result<(), SpawnError>;

    fn spawn_blocking(&self, task: Box<dyn FnOnce() + Send + 'static>) -> Result<(), SpawnError>;
}
