use papokin_macros::{Event, cancellable};

/// 实体被闪电击中时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityZapEvent {
    /// 被电击的实体 ID。
    pub entity_id: i32,

    /// 闪电实体的 ID。
    pub lightning_id: i32,

    /// 电击的原因。
    pub cause: String,
}

impl EntityZapEvent {
    #[must_use]
    pub const fn new(entity_id: i32, lightning_id: i32, cause: String) -> Self {
        Self {
            entity_id,
            lightning_id,
            cause,
            cancelled: false,
        }
    }
}
