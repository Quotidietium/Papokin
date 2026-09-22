use papokin_macros::Event;

/// 白名单被开启或关闭时发生的事件。
#[derive(Event, Clone)]
pub struct WhitelistToggleEvent {
    /// 白名单现在是否已启用。
    pub enabled: bool,
}

impl WhitelistToggleEvent {
    #[must_use]
    pub const fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}
