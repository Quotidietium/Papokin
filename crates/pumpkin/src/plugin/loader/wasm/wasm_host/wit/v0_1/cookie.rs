use wasmtime::component::Resource;

use crate::net::ClientPlatform;
use crate::plugin::loader::wasm::wasm_host::{
    state::PluginHostState,
    wit::v0_1::{
        player::player_from_resource,
        pumpkin::plugin::{cookie, player::Player},
    },
};

/// Vanilla cookie payload limit (5 KiB); mirrors
/// `crate::net::java::cookie::MAX_COOKIE_PAYLOAD`.
const MAX_COOKIE_PAYLOAD: usize = crate::net::java::cookie::MAX_COOKIE_PAYLOAD;

/// Bedrock clients have no cookie protocol; every fallible cookie entry point
/// degrades to this error instead of panicking or fabricating bytes.
const BEDROCK_UNSUPPORTED: &str = "not supported on bedrock";

/// Checks that `key` is a valid vanilla resource identifier:
/// `namespace:path`, or a plain path (vanilla treats a missing namespace as
/// `minecraft`). Namespaces allow `a-z 0-9 _ . -`, paths additionally `/`.
fn is_valid_cookie_key(key: &str) -> bool {
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

fn invalid_key_error(key: &str) -> String {
    format!("cookie key `{key}` is not a valid resource identifier")
}

#[allow(clippy::unused_async_trait_impl)]
impl cookie::Host for PluginHostState {
    async fn store_cookie(
        &mut self,
        player: Resource<Player>,
        key: String,
        payload: Vec<u8>,
    ) -> wasmtime::Result<Result<(), String>> {
        if !is_valid_cookie_key(&key) {
            return Ok(Err(invalid_key_error(&key)));
        }
        if payload.len() > MAX_COOKIE_PAYLOAD {
            return Ok(Err(format!(
                "cookie payload of {} bytes exceeds the vanilla {MAX_COOKIE_PAYLOAD}-byte limit",
                payload.len()
            )));
        }
        let player = player_from_resource(self, &player)?;
        match player.client.as_ref() {
            ClientPlatform::Java(client) => {
                client.store_cookie(&key, &payload).await;
                Ok(Ok(()))
            }
            ClientPlatform::Bedrock(_) => Ok(Err(BEDROCK_UNSUPPORTED.to_string())),
        }
    }

    async fn request_cookie(
        &mut self,
        player: Resource<Player>,
        key: String,
    ) -> wasmtime::Result<Result<(), String>> {
        if !is_valid_cookie_key(&key) {
            return Ok(Err(invalid_key_error(&key)));
        }
        let player = player_from_resource(self, &player)?;
        match player.client.as_ref() {
            ClientPlatform::Java(client) => {
                client.request_cookie(&key).await;
                Ok(Ok(()))
            }
            ClientPlatform::Bedrock(_) => Ok(Err(BEDROCK_UNSUPPORTED.to_string())),
        }
    }

    async fn get_cookie(
        &mut self,
        player: Resource<Player>,
        key: String,
    ) -> wasmtime::Result<Option<Vec<u8>>> {
        let player = player_from_resource(self, &player)?;
        match player.client.as_ref() {
            ClientPlatform::Java(client) => Ok(client.get_cached_cookie(&key)),
            // The WIT signature has no error channel; a Bedrock client can
            // never have reported a cookie, so `None` is the truthful answer.
            ClientPlatform::Bedrock(_) => Ok(None),
        }
    }

    async fn clear_cookie(
        &mut self,
        player: Resource<Player>,
        key: String,
    ) -> wasmtime::Result<Result<(), String>> {
        if !is_valid_cookie_key(&key) {
            return Ok(Err(invalid_key_error(&key)));
        }
        let player = player_from_resource(self, &player)?;
        match player.client.as_ref() {
            ClientPlatform::Java(client) => {
                client.clear_cookie(&key).await;
                Ok(Ok(()))
            }
            ClientPlatform::Bedrock(_) => Ok(Err(BEDROCK_UNSUPPORTED.to_string())),
        }
    }
}
