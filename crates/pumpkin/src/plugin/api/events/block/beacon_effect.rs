use pumpkin_macros::{Event, cancellable};
use pumpkin_util::math::position::BlockPos;
use std::sync::Arc;

use crate::entity::player::Player;

/// An event that occurs when a beacon applies a status effect to a player.
#[cancellable]
#[derive(Event, Clone)]
pub struct BeaconEffectEvent {
    /// The player receiving the effect.
    pub player: Arc<Player>,

    /// The identifier of the effect being applied (e.g. `minecraft:speed`).
    pub effect: String,

    /// Whether the effect is the primary effect of the beacon.
    pub primary: bool,

    /// Position of the beacon block.
    pub block_pos: BlockPos,
}

impl BeaconEffectEvent {
    #[must_use]
    pub const fn new(
        player: Arc<Player>,
        effect: String,
        primary: bool,
        block_pos: BlockPos,
    ) -> Self {
        Self {
            player,
            effect,
            primary,
            block_pos,
            cancelled: false,
        }
    }
}
