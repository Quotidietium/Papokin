use papokin_macros::{Event, cancellable};

/// 动物进入求爱模式以进行繁殖时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityEnterLoveModeEvent {
    /// 进入求偶模式的动物 ID。
    pub entity_id: i32,

    /// 喂食动物的玩家 ID（如有）。
    pub human_entity_id: Option<i32>,

    /// 恋爱模式的持续时间（以刻为单位）。
    pub ticks_in_love: i32,
}

impl EntityEnterLoveModeEvent {
    #[must_use]
    pub const fn new(entity_id: i32, human_entity_id: Option<i32>, ticks_in_love: i32) -> Self {
        Self {
            entity_id,
            human_entity_id,
            ticks_in_love,
            cancelled: false,
        }
    }
}
