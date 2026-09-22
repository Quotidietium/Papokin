use std::{collections::BTreeMap, fs};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use serde::Deserialize;

/// `message_type.json` 中单个聊天类型条目的原始反序列化结构。
#[derive(Deserialize)]
pub struct RawChatType {
    /// 在原版注册表中分配给此聊天类型的数字 ID。
    id: u32,
    //    components: ChatType,
}

// #[derive(Deserialize)]
// pub struct ChatType {
//     chat: Decoration,
//     narration: Decoration,
// }

// #[derive(Deserialize)]
// pub struct Decoration {
//     translation_key: String,
//     #[serde(default, skip_serializing_if = "Option::is_none")]
//     style: Option<Style>,
//     parameters: Vec<String>,
// }

/// 从 26.3 数据包生成消息类型 `u8` 常量的 `TokenStream`，包括一个合成的 `RAW` 变体。
pub fn build() -> TokenStream {
    let dir = std::path::Path::new("../../assets/datapacks/26_3/data/minecraft/chat_type");
    let mut entries: Vec<_> = fs::read_dir(dir)
        .expect("缺少 chat_type 目录")
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .collect();
    entries.sort_by_key(|e| e.path());

    let mut variants = TokenStream::new();

    for (i, entry) in entries.iter().enumerate() {
        let stem = entry
            .path()
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let i = i as u8;
        let name = format_ident!("{}", stem.to_uppercase());
        variants.extend([quote! {
            pub const #name: u8 = #i;
        }]);
    }

    let raw_id = entries.len() as u8;
    variants.extend([quote! {
        pub const RAW: u8 = #raw_id; // 比原版最高 ID 大 1
    }]);

    quote! {
        #variants
    }
}
