use papokin_macros::{Event, cancellable};

/// 炽足兽的颤抖状态变化时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct StriderTemperatureChangeEvent {
    /// 炽足兽实体的 ID。
    pub entity_id: i32,

    /// 炽足兽现在是否在发抖。
    pub is_shivering: bool,
}
