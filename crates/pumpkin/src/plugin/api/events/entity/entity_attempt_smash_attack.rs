use pumpkin_macros::Event;

/// The result of an attempted smash attack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmashAttackResult {
    /// The smash attack is allowed to proceed.
    Allowed,
    /// The smash attack is denied.
    Denied,
}

/// An event that occurs when an entity (e.g. a mace-wielding player) attempts
/// a smash attack.
#[derive(Event, Clone)]
pub struct EntityAttemptSmashAttackEvent {
    /// The ID of the entity attempting the smash attack.
    pub entity_id: i32,

    /// The ID of the entity targeted by the smash attack.
    pub target_id: i32,

    /// The result of the attempt.
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
