use papokin_macros::Event;

/// 砸击攻击尝试的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmashAttackResult {
    /// 允许砸击攻击继续进行。
    Allowed,
    /// 砸击攻击被拒绝。
    Denied,
}

/// 实体（如手持重锤的玩家）尝试猛击时发生的事件
/// 猛击攻击。
#[derive(Event, Clone)]
pub struct EntityAttemptSmashAttackEvent {
    /// 尝试猛击攻击的实体 ID。
    pub entity_id: i32,

    /// 被猛击攻击瞄准的实体 ID。
    pub target_id: i32,

    /// 尝试的结果。
    pub result: SmashAttackResult,
}

impl EntityAttemptSmashAttackEvent {
    #[must_use]
    pub const fn new(entity_id: i32, target_id: i32, result: SmashAttackResult) -> Self {
        Self {
            entity_id,
            target_id,
            result,
        }
    }
}
