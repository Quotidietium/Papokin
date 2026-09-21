use pumpkin_macros::{Event, cancellable};
use pumpkin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// Well-known values for [`PlayerBedFailEnterEvent::fail_reason`].
///
/// Mirrors Paper's `PlayerBedFailEnterEvent.FailReason` names.
pub mod bed_fail_reasons {
    /// The bed cannot be used for sleeping in this dimension at all.
    pub const NOT_POSSIBLE_HERE: &str = "not_possible_here";
    /// The current time or weather does not allow sleeping.
    pub const NOT_POSSIBLE_NOW: &str = "not_possible_now";
    /// The player is too far away from the bed.
    pub const TOO_FAR_AWAY: &str = "too_far_away";
    /// The bed is obstructed.
    pub const OBSTRUCTED: &str = "obstructed";
    /// The bed is already occupied.
    pub const OCCUPIED: &str = "occupied";
    /// There are monsters nearby.
    pub const NOT_SAFE: &str = "not_safe";
}

/// An event that occurs when a player fails to enter a bed.
///
/// Cancelling the event suppresses the failure: no message is sent and the
/// player is allowed to proceed with entering the bed.
#[cancellable]
#[derive(Event, Clone)]
pub struct PlayerBedFailEnterEvent {
    /// The player failing to enter the bed.
    pub player: Arc<Player>,

    /// The position of the bed (head block).
    pub bed_pos: BlockPos,

    /// The reason entering the bed failed (see [`bed_fail_reasons`]).
    pub fail_reason: String,
}

impl PlayerBedFailEnterEvent {
    /// Creates a new instance of `PlayerBedFailEnterEvent`.
    pub fn new(player: Arc<Player>, bed_pos: BlockPos, fail_reason: impl Into<String>) -> Self {
        Self {
            player,
            bed_pos,
            fail_reason: fail_reason.into(),
            cancelled: false,
        }
    }
}

impl PlayerEvent for PlayerBedFailEnterEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
