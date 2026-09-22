use papokin_macros::{Event, cancellable};

/// 施法实体施放法术时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntitySpellCastEvent {
    /// 施法者实体的 ID。
    pub entity_id: i32,

    /// 正在施放的法术名称。
    pub spell: String,
}
