use papokin_macros::{Event, cancellable};

/// 马跳跃时触发的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct HorseJumpEvent {
    /// 马的实体 ID。
    pub entity_id: i32,
    /// 跳跃力度。
    pub power: f32,
}

impl HorseJumpEvent {
    #[must_use]
    pub const fn new(entity_id: i32, power: f32) -> Self {
        Self {
            entity_id,
            power,
            cancelled: false,
        }
    }
}
