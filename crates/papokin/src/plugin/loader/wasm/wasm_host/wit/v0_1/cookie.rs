use wasmtime::component::Resource;

use crate::plugin::loader::wasm::wasm_host::{
    state::PluginHostState,
    wit::v0_1::{
        papokin::plugin::{cookie, player::Player},
        player::player_from_resource,
    },
};

/// 原版 cookie 负载上限（5 KiB）；对应于
/// `crate::net::java::cookie::MAX_COOKIE_PAYLOAD`。
const MAX_COOKIE_PAYLOAD: usize = crate::net::java::cookie::MAX_COOKIE_PAYLOAD;

/// 检查 `key` 是否为有效的原版资源标识符：
/// `namespace:path`，或纯路径（原版将缺失的命名空间视为
/// `minecraft`）。命名空间允许 `a-z 0-9 _ . -`，路径额外允许 `/`。
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
    format!("cookie 键 `{key}` 不是有效的资源标识符")
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
                "{} 字节的 cookie 负载超出原版 {MAX_COOKIE_PAYLOAD} 字节上限",
                payload.len()
            )));
        }
        let player = player_from_resource(self, &player)?;
        player.client.store_cookie(&key, &payload).await;
        Ok(Ok(()))
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
        player.client.request_cookie(&key).await;
        Ok(Ok(()))
    }

    async fn get_cookie(
        &mut self,
        player: Resource<Player>,
        key: String,
    ) -> wasmtime::Result<Option<Vec<u8>>> {
        let player = player_from_resource(self, &player)?;
        Ok(player.client.get_cached_cookie(&key))
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
        player.client.clear_cookie(&key).await;
        Ok(Ok(()))
    }
}
