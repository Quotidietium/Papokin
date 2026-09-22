use papokin_macros::{Event, cancellable};

/// 实体受到来自另一个实体的伤害时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityDamageByEntityEvent {
    /// 受到伤害的实体 ID。
    pub entity_id: i32,

    /// 造成伤害的实体 ID。
    pub damager_id: i32,

    /// 造成的伤害量。
    pub damage: f32,

    /// 伤害原因。
    pub cause: String,
}
