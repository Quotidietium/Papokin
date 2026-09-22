use std::fs;

use proc_macro2::TokenStream;
use quote::quote;

use crate::array_to_tokenstream;

/// 生成 `ScoreboardDisplaySlot` 枚举的 `TokenStream`。
pub fn build() -> TokenStream {
    let sound_categories: Vec<String> = serde_json::from_str(
        &fs::read_to_string("../../assets/scoreboard_display_slot.json").unwrap(),
    )
    .expect("解析 scoreboard_display_slot.json 失败");
    let variants = array_to_tokenstream(&sound_categories);

    quote! {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum ScoreboardDisplaySlot {
            #variants
        }
    }
}
