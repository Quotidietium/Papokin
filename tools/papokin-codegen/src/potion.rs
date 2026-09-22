use std::{collections::BTreeMap, fs};

use heck::ToShoutySnakeCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use serde::Deserialize;

/// `potion.json` 中单个药水条目的原始反序列化结构。
#[derive(Deserialize)]
struct Potion {
    /// 此药水的数字注册表 ID。
    id: u8,
    /// 饮用或施加此药水时获得的状态效果。
    effects: Vec<Effect>,
}

/// 由药水施加的单个状态效果实例，定义于 `potion.json`。
#[derive(Deserialize)]
pub struct Effect {
    /// 带命名空间的效果资源位置（例如 `"minecraft:speed"`）。
    effect_type: String,
    /// 效果的持续时间（刻）。
    duration: i32,
    /// 放大等级（0 = 等级 I，1 = 等级 II，…）。
    amplifier: u8,
    /// 此效果是否为环境效果（由信标产生，降低粒子密度）。
    ambient: bool,
    /// 效果生效期间是否显示粒子。
    show_particles: bool,
    /// 效果图标是否应显示在 HUD 中。
    show_icon: bool,
}

impl Effect {
    /// 将此效果条目转换为 `TokenStream`，供生成的代码使用。
    pub fn to_tokens(&self) -> TokenStream {
        let effect_type = format_ident!(
            "{}",
            self.effect_type
                .strip_prefix("minecraft:")
                .unwrap()
                .to_uppercase()
        );
        let duration = self.duration;
        let amplifier = self.amplifier;
        let ambient = self.ambient;
        let show_particles = self.show_particles;
        let show_icon = self.show_icon;
        quote! {
            Effect {
                effect_type: &StatusEffect::#effect_type,
                duration: #duration,
                amplifier: #amplifier,
                ambient: #ambient,
                show_particles: #show_particles,
                show_icon: #show_icon,
                blend: false,
            }
        }
    }
}

/// 生成 `Potion` 结构体、`Effect` 结构体和 `from_name` 查找的 `TokenStream`。
pub fn build() -> TokenStream {
    let potions: BTreeMap<String, Potion> =
        serde_json::from_str(&fs::read_to_string("../../assets/potion.json").unwrap())
            .expect("解析药水 JSON 失败");

    let mut variants = TokenStream::new();
    let mut name_to_type = TokenStream::new();
    let mut id_to_type = TokenStream::new();

    for (name, potion) in potions {
        let format_name = format_ident!("{}", name.to_shouty_snake_case());
        let id = potion.id;
        let slots = potion.effects;
        let slots = slots.iter().map(Effect::to_tokens);

        variants.extend([quote! {
            pub const #format_name: Self = Self {
                name: #name,
                id: #id,
                effects: &[#(#slots),*],
            };
        }]);

        name_to_type.extend(quote! { #name => Some(&Self::#format_name), });
        id_to_type.extend(quote! { #id => Some(&Self::#format_name), });
    }

    quote! {
        use std::hash::Hash;
        use crate::effect::StatusEffect;

        pub struct Potion {
            pub id: u8,
            pub name: &'static str,
            pub effects: &'static [Effect],
        }

        #[derive(Clone)]
        pub struct Effect {
            pub effect_type: &'static StatusEffect,
            pub duration: i32,
            pub amplifier: u8,
            pub ambient: bool,
            pub show_particles: bool,
            pub show_icon: bool,
            pub blend: bool,
        }

        impl Potion {
            #variants

            #[must_use]
            pub fn from_name(name: &str) -> Option<&'static Self> {
                match name {
                    #name_to_type
                    _ => None
                }
            }

            #[must_use]
            pub fn from_id(id: u8) -> Option<&'static Self> {
                match id {
                    #id_to_type
                    _ => None
                }
            }
        }
    }
}
