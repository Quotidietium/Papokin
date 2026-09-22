use std::{collections::BTreeMap, fs};

use heck::ToShoutySnakeCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use serde::Deserialize;

#[derive(Deserialize)]
struct ItemEntry {
    id: u16,
    components: ItemComponents,
}

#[derive(Deserialize)]
struct ItemComponents {
    #[serde(rename = "minecraft:entity_data")]
    entity_data: Option<EntityDataComponent>,
}

#[derive(Deserialize)]
struct EntityDataComponent {
    id: String,
}

/// 生成 `entity_from_egg` 和 `spawn_egg_ids` 辅助函数的 `TokenStream`。
pub fn build() -> TokenStream {
    let items: BTreeMap<String, ItemEntry> =
        serde_json::from_str(&fs::read_to_string("../../assets/items.json").unwrap())
            .expect("解析 items.json 失败");

    let mut eggs: BTreeMap<u16, String> = BTreeMap::new();
    for (_, item) in items {
        if let Some(entity_data) = item.components.entity_data {
            let entity = entity_data
                .id
                .strip_prefix("minecraft:")
                .unwrap_or(&entity_data.id)
                .to_string();
            eggs.insert(item.id, entity);
        }
    }

    let mut names = TokenStream::new();
    let mut ids = TokenStream::new();

    for (egg, entity) in &eggs {
        let entity = entity.to_shouty_snake_case();
        let entity = format_ident!("{}", entity);
        ids.extend(quote! { #egg, });
        names.extend(quote! { #egg => Some(&EntityType::#entity), });
    }
    quote! {
        use crate::entity_type::EntityType;

        #[must_use]
        pub fn entity_from_egg(id: u16) -> Option<&'static EntityType> {
            match id {
                #names
                _ => None
            }
        }
        #[must_use]
        pub fn spawn_egg_ids() -> Box<[u16]> {
            [#ids].into()
        }
    }
}
