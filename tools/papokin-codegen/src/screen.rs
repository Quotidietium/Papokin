use std::fs;

use proc_macro2::TokenStream;
use quote::quote;

use crate::array_to_tokenstream;

/// 生成 `WindowType` 枚举的 `TokenStream`。
pub fn build() -> TokenStream {
    let screens: Vec<String> =
        serde_json::from_str(&fs::read_to_string("../../assets/screens.json").unwrap())
            .expect("解析 screens.json 失败");
    let variants = array_to_tokenstream(&screens);

    quote! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum WindowType {
            #variants
        }
    }
}
