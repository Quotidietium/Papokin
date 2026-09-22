use papokin_macros::{Event, cancellable};

/// 烟花火箭爆炸时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct FireworkExplodeEvent {
    /// 烟花实体的 ID。
    pub entity_id: i32,
}
