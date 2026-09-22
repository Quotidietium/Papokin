use papokin_macros::{Event, cancellable};

/// 袭击结束时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct RaidFinishEvent {
    /// 胜利是否由玩家取得。
    pub victory: bool,
}

impl RaidFinishEvent {
    #[must_use]
    pub const fn new(victory: bool) -> Self {
        Self {
            victory,
            cancelled: false,
        }
    }
}
