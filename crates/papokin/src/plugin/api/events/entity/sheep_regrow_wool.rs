use papokin_macros::{Event, cancellable};

/// 绵羊重新长出羊毛时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct SheepRegrowWoolEvent {
    /// 绵羊实体的 ID。
    pub entity_id: i32,
}
