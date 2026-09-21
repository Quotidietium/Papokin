//! Client cookie storage (the Java Edition `cookie` protocol feature, the
//! equivalent of Paper's 1.20.5+ `ClientCookie` API).
//!
//! Cookies are small blobs (at most [`MAX_COOKIE_PAYLOAD`] bytes) the server
//! stores on a Java client; the client persists them across server transfers
//! and reports them back when the server asks with a cookie request packet.
//!
//! The cache lives on the connection object: cookie response packets can
//! arrive during the login and configuration phases (handled by
//! [`super::pending::PendingConnection`]) as well as in play (handled by
//! [`super::JavaClient`]). `PendingConnection` owns the store while the
//! connection is pending and `JavaClient::from_pending` moves it across, so a
//! cookie reported during login or configuration is still visible to plugins
//! once the player is in game.

use std::collections::HashMap;

use pumpkin_protocol::java::client::play::{CPlayCookieRequest, CStoreCookie};

use super::JavaClient;

/// The vanilla cookie payload limit (5 KiB); mirrors `MAX_COOKIE_LENGTH` in
/// `pumpkin-protocol`'s cookie packet definitions.
pub const MAX_COOKIE_PAYLOAD: usize = 5120;

/// Server-side cache of the cookies a client has reported.
///
/// Keyed by the cookie's resource identifier (`namespace:path`). Matches the
/// lock-based field style of the connection structs (`std::sync::Mutex`,
/// like `JavaClient::pending_keep_alives`).
pub type CookieStore = std::sync::Mutex<HashMap<String, Vec<u8>>>;

/// Creates an empty cookie store.
#[must_use]
pub fn new_cookie_store() -> CookieStore {
    std::sync::Mutex::new(HashMap::new())
}

/// Applies a clientbound cookie response to the store with vanilla semantics.
///
/// `Some(payload)` inserts or replaces the entry, while `None` means the
/// client does not have the cookie and the entry is removed.
pub fn apply_cookie_response(store: &CookieStore, key: &str, payload: Option<&[u8]>) {
    let mut cookies = store
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match payload {
        Some(payload) => {
            cookies.insert(key.to_string(), payload.to_vec());
        }
        None => {
            cookies.remove(key);
        }
    }
}

impl JavaClient {
    /// Stores a cookie on the client (play-phase `CStoreCookie` packet).
    ///
    /// `key` must already be validated as a resource identifier and
    /// `payload` must not exceed [`MAX_COOKIE_PAYLOAD`] bytes.
    pub async fn store_cookie(&self, key: &str, payload: &[u8]) {
        let key = key.to_string();
        self.send_packet(&CStoreCookie::new(&key, payload)).await;
    }

    /// Asks the client for the cookie stored under `key` (play-phase
    /// `CPlayCookieRequest` packet). The client's response arrives
    /// asynchronously and is placed into the cookie store, readable with
    /// [`JavaClient::get_cached_cookie`].
    pub async fn request_cookie(&self, key: &str) {
        let key = key.to_string();
        self.send_packet(&CPlayCookieRequest::new(&key)).await;
    }

    /// Returns the cached value of the cookie `key`, or `None` when the
    /// client has not reported one. Never sends a packet.
    pub fn get_cached_cookie(&self, key: &str) -> Option<Vec<u8>> {
        self.cookies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(key)
            .cloned()
    }

    /// Clears the cookie `key`: drops it from the server-side cache and
    /// overwrites the client's copy with an empty payload (vanilla has no
    /// dedicated delete packet).
    pub async fn clear_cookie(&self, key: &str) {
        self.cookies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(key);
        self.store_cookie(key, &[]).await;
    }
}
