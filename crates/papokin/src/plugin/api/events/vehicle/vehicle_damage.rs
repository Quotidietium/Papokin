use papokin_macros::{Event, cancellable};

/// 载具受到伤害时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct VehicleDamageEvent {
    /// 载具实体的 ID。
    pub vehicle_id: i32,
    /// 伤害数值。
    pub damage: f32,
    /// 攻击者的 ID（如适用）。
    pub attacker_id: Option<i32>,
}

impl VehicleDamageEvent {
    #[must_use]
    pub const fn new(vehicle_id: i32, damage: f32, attacker_id: Option<i32>) -> Self {
        Self {
            vehicle_id,
            damage,
            attacker_id,
            cancelled: false,
        }
    }
}
