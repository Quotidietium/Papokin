use papokin_macros::{Event, cancellable};

/// 实体被点燃即将爆炸时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct ExplosionPrimeEvent {
    /// 实体的 ID。
    pub entity_id: i32,
    /// 爆炸半径。
    pub radius: f32,
    /// 它是否产生火焰。
    pub fire: bool,
}

impl ExplosionPrimeEvent {
    #[must_use]
    pub const fn new(entity_id: i32, radius: f32, fire: bool) -> Self {
        Self {
            entity_id,
            radius,
            fire,
            cancelled: false,
        }
    }
}
