#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_cookie_response(&self, packet: &SPCookieResponse<'_>) {
        debug!(
            "已收到 cookie_response[play]：key: \"{}\"，payload_length: \"{:?}\"",
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
