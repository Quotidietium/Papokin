#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_config_cookie_response(&self, packet: &SConfigCookieResponse<'_>) {
        debug!(
            "收到 cookie_response[config]：key: \"{}\"，has_payload: \"{}\"，payload_length: \"{:?}\"",
            packet.key,
            packet.has_payload,
            packet.payload.as_ref().map(|p| p.len()),
        );
        super::super::cookie::apply_cookie_response(&self.cookies, packet.key, packet.payload);
    }
}
