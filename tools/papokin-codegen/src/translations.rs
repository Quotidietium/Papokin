use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use std::{collections::BTreeMap, fs};

pub fn build() -> TokenStream {
    let java_json: BTreeMap<String, String> = serde_json::from_str(
        &fs::read_to_string("../../assets/en_us_java.json").expect("缺少 en_us_java"),
    )
    .unwrap();

    let mut java_constants = TokenStream::new();
    let mut java_match_arms = TokenStream::new();
    for (name, value) in &java_json {
        let ident = to_valid_ident(name);
        let ident_str = ident.to_string();
        let doc = if !value.is_empty() {
            quote!(#[doc = #value])
        } else {
            quote!()
        };
        java_constants.extend(quote! {
            #doc
            pub const #ident: &str = #name;
        });
        java_match_arms.extend(quote! {
            #ident_str => Some(#ident),
        });
    }

    let mut java_value_match_arms = TokenStream::new();
    for (name, value) in &java_json {
        java_value_match_arms.extend(quote! {
            #name => Some(#value),
        });
    }

    // --- 最终组装 ---
    quote! {
        #![allow(clippy::doc_markdown)]
        pub mod java {
            #java_constants
            pub fn get(const_name: &str) -> Option<&'static str> {
                match const_name {
                    #java_match_arms
                    _ => None,
                }
            }
            pub fn get_value(key: &str) -> Option<&'static str> {
                match key {
                    #java_value_match_arms
                    _ => None,
                }
            }
        }
    }
}

fn to_valid_ident(name: &str) -> Ident {
    let mut clean = name.to_uppercase().replace(['.', ':', '-'], "_");

    if clean.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        clean.insert(0, '_');
    }

    format_ident!("{}", clean)
}
