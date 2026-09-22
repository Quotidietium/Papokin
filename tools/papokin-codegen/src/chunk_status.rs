use std::fs;

use heck::ToPascalCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// 生成带有 serde 重命名属性的 `ChunkStatus` 枚举的 `TokenStream`。
pub fn build() -> TokenStream {
    let chunk_status: Vec<String> =
        serde_json::from_str(&fs::read_to_string("../../assets/chunk_status.json").unwrap())
            .expect("解析 chunk_status.json 失败");
    let variants: Vec<TokenStream> = chunk_status
        .into_iter()
        .map(|status| {
            let full_name = format!("minecraft:{status}");
            let name = format_ident!("{}", status.to_pascal_case());

            quote! {
                #[serde(rename = #full_name)]
                #name,
            }
        })
        .collect();
    quote! {
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy)]
        pub enum ChunkStatus {
            #(#variants)*
        }
    }
}
