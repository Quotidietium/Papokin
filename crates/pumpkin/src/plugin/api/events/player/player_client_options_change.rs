use pumpkin_macros::Event;
use std::sync::Arc;

use crate::entity::player::Player;

use super::PlayerEvent;

/// An event that occurs when a player's client options change during play
/// (the play-phase client-information packet).
///
/// Pure notification carrying the new values. This is a Java-protocol event;
/// Bedrock clients update settings through a different path and do not fire
/// it. The configuration-phase (pre-login) client information cannot fire
/// this event because no [`Player`] exists yet.
#[derive(Event, Clone)]
pub struct PlayerClientOptionsChangeEvent {
    /// The player whose client options changed.
    pub player: Arc<Player>,

    /// The new client locale.
    pub locale: String,

    /// The new view distance in chunks.
    pub view_distance: i32,

    /// The new chat visibility setting (`enabled`, `commands`, `hidden`).
    pub chat_visibility: String,

    /// Whether chat colors are enabled.
    pub chat_colors: bool,

    /// The new main hand setting (`left` or `right`).
    pub main_hand: String,

    /// The new visible skin parts bitmask.
    pub skin_parts: u32,
}

impl PlayerClientOptionsChangeEvent {
    /// Creates a new instance of `PlayerClientOptionsChangeEvent`.
    #[expect(clippy::too_many_arguments)]
    pub const fn new(
        player: Arc<Player>,
        locale: String,
        view_distance: i32,
        chat_visibility: String,
        chat_colors: bool,
        main_hand: String,
        skin_parts: u32,
    ) -> Self {
        Self {
            player,
            locale,
            view_distance,
            chat_visibility,
            chat_colors,
            main_hand,
            skin_parts,
        }
    }
}

impl PlayerEvent for PlayerClientOptionsChangeEvent {
    fn get_player(&self) -> &Arc<Player> {
        &self.player
    }
}
