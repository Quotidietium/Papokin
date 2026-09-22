use crate::plugin::loader::wasm::wasm_host::{
    state::PluginHostState,
    wit::v0_1::papokin::plugin::{common::Locale as WitLocale, i18n::Host},
};
use papokin_util::translation::{Locale as UtilLocale, add_translation_file, get_translation};
use std::str::FromStr;

impl Host for PluginHostState {
    async fn translate(&mut self, key: String, locale: WitLocale) -> wasmtime::Result<String> {
        let util_locale = wit_to_util_locale(locale);
        Ok(get_translation(&key, util_locale))
    }

    async fn load_translations(
        &mut self,
        namespace: String,
        json: String,
        locale: WitLocale,
    ) -> wasmtime::Result<()> {
        let util_locale = wit_to_util_locale(locale);
        add_translation_file(namespace, json, util_locale);
        Ok(())
    }
}

/// 将 WIT Locale 转换为 papokin-util 的 Locale。
fn wit_to_util_locale(wit: WitLocale) -> UtilLocale {
    // 像 EnUs 这样的 WIT 变体调试输出通常就是 "EnUs"。
    // 我们转换为小写并处理可能的格式差异。
    let s = format!("{wit:?}").to_lowercase();

    // 大多数翻译系统期望使用下划线（en_us）而非空或连字符。
    // 如果 WIT Debug 格式为 "EnUs"，小写为 "enus"。
    // 如果你的工具明确要求 "en_us"，我们可能需要更聪明的映射。
    UtilLocale::from_str(&s).unwrap_or(UtilLocale::EnUs)
}
