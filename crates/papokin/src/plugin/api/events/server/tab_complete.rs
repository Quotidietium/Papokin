use papokin_macros::{Event, cancellable};

/// 请求 Tab 补全时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct TabCompleteEvent {
    /// 正在补全的缓冲区/文本。
    pub buffer: String,
    /// 补全建议。
    pub completions: Vec<String>,
}

impl TabCompleteEvent {
    #[must_use]
    pub const fn new(buffer: String, completions: Vec<String>) -> Self {
        Self {
            buffer,
            completions,
            cancelled: false,
        }
    }
}
