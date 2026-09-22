use std::{collections::BTreeMap, fs};

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::LitInt;

/// 生成 `get_composter_increase_chance_from_item_id` 函数的 `TokenStream`。
pub fn build() -> TokenStream {
    let composter_increase_chance: BTreeMap<u16, f32> = serde_json::from_str(
        &fs::read_to_string("../../assets/composter_increase_chance.json").unwrap(),
    )
    .expect("解析 composter_increase_chance.json 失败");
    let match_arms: Vec<TokenStream> = composter_increase_chance
        .into_iter() // 为提升效率而消耗掉该 map
        .map(|(item_id, chance)| {
            let item_id_lit = LitInt::new(&item_id.to_string(), Span::call_site());
            let chance_lit = chance;

            quote! {
                #item_id_lit => Some(#chance_lit),
            }
        })
        .collect();
    quote! {
        #[must_use]
        #[allow(clippy::too_many_lines, clippy::match_same_arms)]
        pub const fn get_composter_increase_chance_from_item_id(item_id: u16) -> Option<f32> {
            match item_id {
                #(#match_arms)*
                _ => None,
            }
        }
    }
}
