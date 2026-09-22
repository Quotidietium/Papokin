use heck::ToShoutySnakeCase;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use syn::LitInt;

/// `attributes.json` 中单个属性条目的原始反序列化结构。
#[derive(Deserialize)]
struct Attributes {
    /// 此属性的数字注册表 ID。
    id: u8,
    /// 应用于未覆盖此属性的实体的默认数值。
    default_value: f64,
}

/// 生成 `Attributes` 结构体及其关联常量的 `TokenStream`。
pub fn build() -> TokenStream {
    let attributes: BTreeMap<String, Attributes> =
        serde_json::from_str(&fs::read_to_string("../../assets/attributes.json").unwrap())
            .expect("解析 attributes.json 失败");

    let mut sorted_attributes: Vec<(String, Attributes)> = attributes.into_iter().collect();
    sorted_attributes.sort_by_key(|(_, raw)| raw.id);

    let mut constant_defs = Vec::new();
    let mut constant_idents = Vec::new();

    for (raw_name, raw_value) in sorted_attributes {
        let constant_ident = format_ident!("{}", raw_name.to_shouty_snake_case());
        constant_idents.push(constant_ident.clone());

        let id_lit = LitInt::new(&raw_value.id.to_string(), Span::call_site());
        let default_value_lit = raw_value.default_value;
        let name_str = format!("minecraft:{raw_name}");

        constant_defs.push(quote!(
            pub const #constant_ident: Self = Self {
                id: #id_lit,
                default_value: #default_value_lit,
                name: #name_str,
            };
        ));
    }

    quote! {
        use std::hash::Hash;

        #[derive(Clone, Debug)]
        pub struct Attributes {
            pub id: u8,
            pub default_value: f64,
            pub name: &'static str,
        }
        impl PartialEq for Attributes {
            fn eq(&self, other: &Self) -> bool {
                self.id == other.id
            }
        }
        impl Eq for Attributes {}
        impl Hash for Attributes {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.id.hash(state);
            }
        }
        impl Attributes {
            #(#constant_defs)*

            pub const ALL: &'static [Self] = &[
                #(Self::#constant_idents),*
            ];
        }
    }
}
