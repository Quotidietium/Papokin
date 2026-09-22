use papokin_macros::{Event, cancellable};

/// 猪被闪电击中并转化时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct PigZapEvent {
    /// 猪的实体 ID。
    pub entity_id: i32,
    /// 闪电实体的 ID。
    pub lightning_id: i32,
    /// 生成的僵尸猪人实体 ID。
    pub pig_zombie_id: i32,
}

impl PigZapEvent {
    #[must_use]
    pub const fn new(entity_id: i32, lightning_id: i32, pig_zombie_id: i32) -> Self {
        Self {
            entity_id,
            lightning_id,
            pig_zombie_id,
            cancelled: false,
        }
    }
}
