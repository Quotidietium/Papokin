use std::{collections::BTreeMap, fs};

use heck::{ToPascalCase, ToSnakeCase};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use serde_json::Value;

/// 生成 `GameRule` 枚举、`GameRuleRegistry` 结构体以及它们
/// 访问器方法，并为每条规则提供合适的默认值。
pub fn build() -> TokenStream {
    let game_rules: BTreeMap<String, Value> =
        serde_json::from_str(&fs::read_to_string("../../assets/game_rules.json").unwrap())
            .expect("解析 game_rules.json 失败");

    let mut enum_variants = TokenStream::new();
    let mut enum_variants_list = TokenStream::new();
    let mut enum_to_string = TokenStream::new();
    let mut struct_fields = TokenStream::new();
    let mut default_values = TokenStream::new();
    let mut getter_match = TokenStream::new();
    let mut mut_getter_match = TokenStream::new();
    let mut default_functions = TokenStream::new();

    for (raw_name, raw_value) in &game_rules {
        let (variant_type, field_type, default_value) = match raw_value {
            Value::Bool(b) => (quote! { Bool }, quote! { bool }, quote! { #b }),
            Value::Number(n) if n.is_i64() => {
                let i = n.as_i64().unwrap();
                (quote! { Int }, quote! { i64 }, quote! { #i })
            }
            Value::Object(obj) => {
                let default_val = obj.get("default").expect("对象型游戏规则缺少默认值");
                match default_val {
                    Value::Number(n) if n.is_i64() => {
                        let i = n.as_i64().unwrap();
                        (quote! { Int }, quote! { i64 }, quote! { #i })
                    }
                    _ => panic!("对象中键 '{raw_name}' 的默认值类型不受支持"),
                }
            }
            _ => panic!("键 '{raw_name}' 的值类型不受支持"),
        };

        let snake_case = format_ident!("{}", raw_name.to_snake_case());
        let pascal_case = format_ident!("{}", raw_name.to_pascal_case());
        let default_fn_name = format!("default_{snake_case}");
        let default_fn_ident = format_ident!("default_{snake_case}");

        // 结构体字段
        struct_fields.extend(quote! {
            #[serde(rename = #raw_name)]
            #[serde(default = #default_fn_name)]
            #[serde(with = "as_string")]
            pub #snake_case: #field_type,
        });

        // 枚举变体
        enum_variants.extend(quote! {
            #pascal_case,
        });

        // Enum::all()
        enum_variants_list.extend(quote! {
            Self::#pascal_case,
        });

        // 枚举 -> &str
        enum_to_string.extend(quote! {
            Self::#pascal_case => write!(f, #raw_name),
        });

        // 默认值
        default_values.extend(quote! {
            #snake_case: #default_value,
        });

        // Getter 的 match 分支
        getter_match.extend(quote! {
            GameRule::#pascal_case => GameRuleValue::#variant_type(&self.#snake_case),
        });

        mut_getter_match.extend(quote! {
            GameRule::#pascal_case => GameRuleValue::#variant_type(&mut self.#snake_case),
        });

        // 默认 fn
        default_functions.extend(quote! {
            fn #default_fn_ident() -> #field_type {
                GameRuleRegistry::default().#snake_case
            }
        });
    }

    quote! {
        use std::fmt;
        use serde::{Deserialize, Serialize};

        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum GameRule {
            #enum_variants
        }

        impl GameRule {
            pub const fn all() -> &'static [Self] {
                &[
                    #enum_variants_list
                ]
            }
        }

        impl fmt::Display for GameRule {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                match self {
                    #enum_to_string
                }
            }
        }

        #[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
        pub struct GameRuleRegistry {
            #struct_fields
        }

        pub enum GameRuleValue<I, B> {
            Int(I),
            Bool(B),
        }

        impl<I: fmt::Display, B: fmt::Display> fmt::Display for GameRuleValue<I, B> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                match self {
                    Self::Int(v) => write!(f, "{v}"),
                    Self::Bool(v) => write!(f, "{v}"),
                }
            }
        }

        impl GameRuleRegistry {
            pub fn get(&self, rule: &GameRule) -> GameRuleValue<&i64, &bool> {
                match rule {
                    #getter_match
                }
            }

            pub fn get_mut(&mut self, rule: &GameRule) -> GameRuleValue<&mut i64, &mut bool> {
                match rule {
                    #mut_getter_match
                }
            }
        }

        impl Default for GameRuleRegistry {
            fn default() -> Self {
                Self {
                    #default_values
                }
            }
        }

        #default_functions

        mod as_string {
            use serde::{Serialize, Deserialize, Serializer, Deserializer};
            use std::{fmt::Display, str::FromStr};

            pub fn serialize<T: Display, S: Serializer>(value: &T, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&value.to_string())
            }

            pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
            where
                T: FromStr,
                D: Deserializer<'de>,
                <T as FromStr>::Err: Display,
            {
                let s = String::deserialize(deserializer)?;
                s.parse::<T>().map_err(serde::de::Error::custom)
            }
        }
    }
}
