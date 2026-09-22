use std::{collections::BTreeMap, fs};

use heck::ToPascalCase;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::LitInt;

/// 生成带有 `u8` 判别值的 `EntityStatus` 枚举的 `TokenStream`。
pub fn build() -> TokenStream {
    let events: BTreeMap<String, u8> =
        serde_json::from_str(&fs::read_to_string("../../assets/entity_statuses.json").unwrap())
            .expect("解析 entity_statuses.json 失败");
    let variants: Vec<TokenStream> = events
        .into_iter()
        .map(|(event_name, id)| {
            let name = format_ident!("{}", event_name.to_pascal_case());
            let id_lit = LitInt::new(&id.to_string(), Span::call_site());

            quote! {
                #name = #id_lit
            }
        })
        .collect();
    quote! {
        #[repr(u8)]
        pub enum EntityStatus {
            #(#variants),*
        }
    }
}
