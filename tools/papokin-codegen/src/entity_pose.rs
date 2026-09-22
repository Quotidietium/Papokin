use std::fs;

use proc_macro2::TokenStream;
use quote::quote;

use crate::array_to_tokenstream;

/// 生成 `EntityPose` 枚举的 `TokenStream`。
pub fn build() -> TokenStream {
    let poses: Vec<String> =
        serde_json::from_str(&fs::read_to_string("../../assets/entity_pose.json").unwrap())
            .expect("解析 entity_pose.json 失败");
    let variants = array_to_tokenstream(&poses);

    quote! {
        #[derive(PartialEq, Clone, Copy)]
        pub enum EntityPose {
            #variants
        }
    }
}
