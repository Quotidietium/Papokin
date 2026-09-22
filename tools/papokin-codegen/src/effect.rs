use std::{collections::BTreeMap, fs};

use heck::{ToPascalCase, ToShoutySnakeCase};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use serde::Deserialize;

/// `effect.json` 中单个生物效果条目的原始反序列化结构。
#[derive(Deserialize)]
struct Effect {
    /// 此生物效果的数字注册表 ID。
    id: u8,
    /// 控制 UI 呈现与总体行为的类别。
    category: MobEffectCategory,
    /// 此效果的粒子与图标颜色，以打包的 RGB 整数表示。
    color: i32,
    /// 用于在 UI 中显示效果名称的翻译键。
    translation_key: String,
    /// 应用于受此效果影响的实体的属性修饰符。
    attribute_modifiers: Vec<Modifiers>,
}

/// 生物效果的分类，决定其 UI 呈现方式和总体用途。
#[expect(clippy::upper_case_acronyms)]
#[derive(Deserialize)]
pub enum MobEffectCategory {
    BENEFICIAL,
    HARMFUL,
    NEUTRAL,
}

/// 由生物效果施加的单个属性修饰符，定义于 `effect.json`。
#[derive(Deserialize)]
pub struct Modifiers {
    /// 带命名空间的属性资源位置（例如 `"minecraft:generic.attack_damage"`）。
    attribute: String,
    /// 此修改器的唯一资源位置 ID（用于防止叠加）。
    id: String,
    /// 此修改器所应用的基础数值。
    #[serde(rename = "baseValue")]
    base_value: f64,
    /// 将此修饰符与属性基础值合并时所用的运算。
    operation: String,
}

impl Modifiers {
    /// 将此修饰器条目转换为 `TokenStream`，供生成的代码使用。
    pub fn get_tokens(self) -> TokenStream {
        let attribute = format_ident!("{}", self.attribute.to_uppercase());
        let id = self.id;
        let base_value = self.base_value;
        let operation = format_ident!("{}", self.operation.to_pascal_case());
        quote! {
            Modifiers {
                attribute: &Attributes::#attribute,
                id: #id,
                base_value: #base_value,
                operation: Operation::#operation,
            }
        }
    }
}

impl MobEffectCategory {
    /// 将此类别变体转换为 `TokenStream`，供生成的代码使用。
    pub fn to_tokens(&self) -> TokenStream {
        match self {
            Self::BENEFICIAL => quote! { MobEffectCategory::Beneficial },
            Self::HARMFUL => quote! { MobEffectCategory::Harmful },
            Self::NEUTRAL => quote! { MobEffectCategory::Neutral },
        }
    }
}

/// 生成 `StatusEffect` 结构体、`MobEffectCategory`、`Modifiers` 的 `TokenStream`，
/// 以及 `from_name`/`from_minecraft_name` 查找方法。
pub fn build() -> TokenStream {
    let effects: BTreeMap<String, Effect> =
        serde_json::from_str(&fs::read_to_string("../../assets/effect.json").unwrap())
            .expect("解析 effect.json 失败");

    let mut variants = TokenStream::new();
    let mut name_to_type = TokenStream::new();
    let mut minecraft_name_to_type = TokenStream::new();
    let mut id_to_type = TokenStream::new();

    for (name, effect) in effects {
        let format_name = format_ident!("{}", name.to_shouty_snake_case());
        let id = effect.id;
        let u16_id = effect.id as u16;
        let color = effect.color;
        let translation_key = effect.translation_key;
        let category = effect.category.to_tokens();
        let slots = effect.attribute_modifiers;
        let slots = slots.into_iter().map(Modifiers::get_tokens);

        let minecraft_name = "minecraft:".to_string() + &name;
        variants.extend([quote! {
            pub const #format_name: Self = Self {
                minecraft_name: #minecraft_name,
                id: #id,
                category: #category,
                color: #color,
                translation_key: #translation_key,
                attribute_modifiers: &[#(#slots),*],
            };
        }]);

        name_to_type.extend(quote! { #name => Some(&Self::#format_name), });

        minecraft_name_to_type.extend(quote! { #minecraft_name => Some(&Self::#format_name), });
        id_to_type.extend(quote! { #u16_id => Some(&Self::#format_name), })
    }

    quote! {
        use std::hash::{Hash, Hasher};
        use crate::{attributes::Attributes, data_component_impl::{Operation, IDSetContent}};
        #[derive(Clone, Debug)]
        pub struct StatusEffect {
            pub minecraft_name: &'static str,
            pub id: u8,
            pub category: MobEffectCategory,
            pub color: i32,
            pub translation_key: &'static str,
            pub attribute_modifiers: &'static [Modifiers],
        }

        impl PartialEq for StatusEffect {
            fn eq(&self, other: &Self) -> bool {
                self.id == other.id
            }
        }

        impl Eq for StatusEffect {}

        impl Hash for StatusEffect {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.id.hash(state);
            }
        }

        #[derive(Debug, Clone, Hash)]
        pub enum MobEffectCategory {
            Beneficial,
            Harmful,
            Neutral,
        }

        #[derive(Debug)]
        pub struct Modifiers {
            pub attribute: &'static Attributes,
            pub id: &'static str,
            pub base_value: f64,
            pub operation: Operation,
        }

        impl StatusEffect {
            #variants

            #[must_use]
            pub fn from_name(name: &str) -> Option<&'static Self> {
                match name {
                    #name_to_type
                    _ => None
                }
            }

            #[must_use]
            pub fn from_minecraft_name(name: &str) -> Option<&'static Self> {
                match name {
                    #minecraft_name_to_type
                    _ => None
                }
            }
        }
        impl IDSetContent for StatusEffect {
            fn registry_id(&self) -> u16 {
                self.id as u16
            }

            fn from_id(id: u16) -> Option<&'static Self> {
                match id {
                    #id_to_type
                    _ => None
                }
            }

            fn from_str(name: &str) -> Option<&'static Self> {
                Self::from_minecraft_name(name)
            }

            fn to_string(&self) -> String {
                self.minecraft_name.to_string()
            }
        }
    }
}
