use proc_macro2::{Span, TokenStream};
use quote::quote;
use std::{collections::BTreeMap, fs};
use syn::LitInt;
/// 生成将物品 ID 映射到盆栽方块 ID 的 `get_potted_item` 函数的 `TokenStream`。
pub fn build() -> TokenStream {
    let flower_pot_transformation: BTreeMap<u16, u16> = serde_json::from_str(
        &fs::read_to_string("../../assets/flower_pot_transformations.json").unwrap(),
    )
    .expect("解析 flower_pot_transformations.json 失败");
    let match_arms: Vec<TokenStream> = flower_pot_transformation
        .into_iter()
        .map(|(item_id, potted_block_id)| {
            let item_id_lit = LitInt::new(&item_id.to_string(), Span::call_site());
            let potted_id_lit = LitInt::new(&potted_block_id.to_string(), Span::call_site());
            quote! {
                #item_id_lit => #potted_id_lit,
            }
        })
        .collect();
    quote! {
        use crate::BlockId;

        #[must_use]
        pub const fn get_potted_item(item_id: u16) -> BlockId {
            let raw_id = match item_id {
                #(#match_arms)*
                _ => 0,
            };
            BlockId::new_or_air(raw_id)
        }
    }
}
