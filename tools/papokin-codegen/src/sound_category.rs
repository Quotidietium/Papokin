use std::fs;

use heck::ToPascalCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::array_to_tokenstream;

/// 生成 `SoundCategory` 枚举及其 `from_name`/`to_name` 方法的 `TokenStream`。
pub fn build() -> TokenStream {
    let sound_categories: Vec<String> =
        serde_json::from_str(&fs::read_to_string("../../assets/sound_category.json").unwrap())
            .expect("解析 sound_category.json 失败");
    let variants = array_to_tokenstream(&sound_categories);
    let type_from_name = &sound_categories
        .iter()
        .map(|sound| {
            let id = &sound.to_lowercase();
            let name = format_ident!("{}", sound.to_pascal_case());

            quote! {
                #id => Some(Self::#name),
            }
        })
        .collect::<TokenStream>();

    let type_to_name = &sound_categories
        .iter()
        .map(|sound| {
            let id = &sound.to_lowercase();
            let name = format_ident!("{}", sound.to_pascal_case());

            quote! {
                Self::#name => #id,
            }
        })
        .collect::<TokenStream>();
    quote! {
        #[derive(Clone, Copy)]
        pub enum SoundCategory {
            #variants
        }

        impl SoundCategory {
            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    #type_from_name
                    _ => None
                }
            }

            pub const fn to_name(&self) -> &'static str {
                match self {
                    #type_to_name
                }
            }
        }
    }
}
