#[allow(clippy::wildcard_imports)]
use super::*;

impl PendingConnection {
    pub fn handle_login_cookie_response(&self, packet: &SLoginCookieResponse<'_>) {
        debug!(
            "收到 cookie_response[login]：key: \"{}\"，payload_length: \"{:?}\"",
            packet.key,
            packet.payload.as_ref().map(|p| p.len())
        );
        super::super::cookie::apply_cookie_response(
            &self.cookies,
            &self.pending_cookie_requests,
            packet.key,
            packet.payload,
        );
    }
}
