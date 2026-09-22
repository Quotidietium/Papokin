use papokin_macros::{Event, cancellable};

/// 苦力怕被充能时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct CreeperPowerEvent {
    /// 苦力怕实体 ID。
    pub entity_id: i32,
    /// 若由闪电引起，则为闪电实体的 ID。
    pub lightning_id: Option<i32>,
    /// 供能原因。
    pub cause: String,
}

impl CreeperPowerEvent {
    #[must_use]
    pub const fn new(entity_id: i32, lightning_id: Option<i32>, cause: String) -> Self {
        Self {
            entity_id,
            lightning_id,
            cause,
            cancelled: false,
        }
    }
}
