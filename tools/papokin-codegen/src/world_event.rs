use std::{collections::BTreeMap, fs};

use heck::ToPascalCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// 生成带有 `u16` 判别值的 `WorldEvent` 枚举的 `TokenStream`。
pub fn build() -> TokenStream {
    let events: BTreeMap<String, u16> =
        serde_json::from_str(&fs::read_to_string("../../assets/world_event.json").unwrap())
            .expect("解析 world_event.json 失败");
    let variants: Vec<TokenStream> = events
        .into_iter()
        .map(|(event_name, id)| {
            let name = format_ident!("{}", event_name.to_pascal_case());

            quote! {
                #name = #id,
            }
        })
        .collect();

    quote! {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[repr(u16)]
        pub enum WorldEvent {
            #(#variants)*
        }
    }
}
