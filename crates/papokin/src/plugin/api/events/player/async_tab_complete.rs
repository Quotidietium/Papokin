use papokin_macros::{Event, cancellable};
use std::sync::Arc;

use crate::entity::player::Player;

/// 当异步计算 Tab 补全时发生的事件，计算对象为
/// 一个命令缓冲区。
///
/// 取消会抑制补全项；`completions` 可以被
/// 处理程序来添加或移除条目。当补全请求没有明确的发送者时，`sender` 为 `None`
/// 请求并非来自玩家（例如控制台），因此该事件
/// 未实现 `PlayerEvent`。
#[cancellable]
#[derive(Event, Clone)]
pub struct AsyncTabCompleteEvent {
    /// 请求补全的发送者（如果是玩家）。
    pub sender: Option<Arc<Player>>,

    /// 当前的命令缓冲区。
    pub buffer: String,

    /// 计算得到的补全项（可修改）。
    pub completions: Vec<String>,
}

impl AsyncTabCompleteEvent {
    /// 创建新的 `AsyncTabCompleteEvent` 实例。
    pub fn new(
        sender: Option<Arc<Player>>,
        buffer: impl Into<String>,
        completions: Vec<String>,
    ) -> Self {
        Self {
            sender,
            buffer: buffer.into(),
            completions,
            cancelled: false,
        }
    }
}
