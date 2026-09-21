//! Client cookie storage (the equivalent of Paper's 1.20.5+ `ClientCookie`
//! API).
//!
//! Cookies are small blobs (at most [`MAX_COOKIE_PAYLOAD`] bytes) the server
//! stores on a Java Edition client; the client persists them across server
//! transfers and hands them back when the server asks. Typical uses are
//! session tokens and per-player preferences that must survive a proxy
//! transfer.
//!
//! Cookies are a Java Edition protocol feature: for Bedrock players
//! [`PlayerCookieExt::store_cookie`], [`PlayerCookieExt::request_cookie`] and
//! [`PlayerCookieExt::clear_cookie`] return an error and
//! [`PlayerCookieExt::get_cookie`] returns `None`.
//!
//! [`PlayerCookieExt::store_cookie`] and [`PlayerCookieExt::request_cookie`]
//! send play-phase clientbound packets, so they only reach a fully connected
//! (in-game) player. [`PlayerCookieExt::get_cookie`] never sends a packet: it
//! reads the server-side cache of cookie responses the client has sent since
//! connecting, so a key only appears once a
//! [`PlayerCookieExt::request_cookie`] round trip has completed.
//!
//! # Examples
//!
//! ```rust,ignore
//! use pumpkin_plugin_api::cookie::PlayerCookieExt;
//!
//! fn on_join(player: &pumpkin_plugin_api::Player) {
//!     // Ask the client for the session token it kept from the lobby server.
//!     player.request_cookie("myplugin:session")?;
//!     // ...one the response round trip has completed:
//!     if let Some(token) = player.get_cookie("myplugin:session") {
//!         // validate the token...
//!     }
//!     // Persist a preference on the client for future sessions.
//!     player.store_cookie("myplugin:locale", b"en_us")?;
//! }
//! ```

pub use crate::wit::pumpkin::plugin::cookie::{
    clear_cookie, get_cookie, request_cookie, store_cookie,
};

use crate::wit::pumpkin::plugin::player::Player;

/// Maximum cookie payload size in bytes (the vanilla 5 KiB limit). Larger
/// payloads are rejected by the host instead of being truncated.
pub const MAX_COOKIE_PAYLOAD: usize = 5120;

/// Checks whether `key` is a valid cookie resource identifier:
/// `namespace:path`, or a plain path (vanilla treats a missing namespace as
/// `minecraft`). Namespaces allow `a-z 0-9 _ . -`, paths additionally `/`.
/// This mirrors the host-side validation, so keys passing here will not be
/// rejected by the host.
#[must_use]
pub fn is_valid_cookie_key(key: &str) -> bool {
    let (namespace, path) = key
        .split_once(':')
        .map_or(("minecraft", key), |(ns, p)| (ns, p));
    !path.is_empty()
        && namespace
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '.' | '-'))
        && path.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '.' | '-' | '/')
        })
}

/// Extension trait on [`Player`] for client cookie storage (Paper's
/// `ClientCookie` API equivalent).
pub trait PlayerCookieExt {
    /// Stores a cookie on the player's client.
    ///
    /// # Errors
    /// Returns an error when `key` is not a valid identifier, `payload`
    /// exceeds [`MAX_COOKIE_PAYLOAD`] bytes, or the player is a Bedrock
    /// client.
    fn store_cookie(&self, key: &str, payload: &[u8]) -> Result<(), String>;

    /// Asks the player's client for the cookie stored under `key`. The
    /// response arrives asynchronously and becomes readable through
    /// [`PlayerCookieExt::get_cookie`].
    ///
    /// # Errors
    /// Returns an error when `key` is not a valid identifier or the player is
    /// a Bedrock client.
    fn request_cookie(&self, key: &str) -> Result<(), String>;

    /// Returns the cached value of the cookie `key`, or `None` when the
    /// client has not reported one (yet). Never sends a packet; Bedrock
    /// players always yield `None`.
    fn get_cookie(&self, key: &str) -> Option<Vec<u8>>;

    /// Clears the cookie `key`: drops it from the server-side cache and
    /// overwrites the client's copy with an empty payload (vanilla has no
    /// dedicated delete packet).
    ///
    /// # Errors
    /// Returns an error when `key` is not a valid identifier or the player is
    /// a Bedrock client.
    fn clear_cookie(&self, key: &str) -> Result<(), String>;
}

impl PlayerCookieExt for Player {
    fn store_cookie(&self, key: &str, payload: &[u8]) -> Result<(), String> {
        store_cookie(self, key, payload)
    }

    fn request_cookie(&self, key: &str) -> Result<(), String> {
        request_cookie(self, key)
    }

    fn get_cookie(&self, key: &str) -> Option<Vec<u8>> {
        get_cookie(self, key)
    }

    fn clear_cookie(&self, key: &str) -> Result<(), String> {
        clear_cookie(self, key)
    }
}
