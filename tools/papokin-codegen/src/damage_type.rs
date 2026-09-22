use heck::ToShoutySnakeCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use serde::Deserialize;
use std::{collections::BTreeMap, fs};
use syn::{Ident, LitInt};

/// `damage_type.json` 中单个伤害类型条目的原始反序列化包装器。
#[derive(Deserialize)]
struct DamageTypeEntry {
    /// 此伤害类型的数字注册表 ID。
    id: u8,
    /// 描述此伤害类型行为与消息文本的组件数据。
    components: DamageTypeData,
}

/// 描述某伤害类型行为与消息文本的组件数据。
#[derive(Deserialize)]
pub struct DamageTypeData {
    /// 显示哪种死亡消息变体；缺省时为 `Default`。
    death_message_type: Option<DeathMessageType>,
    /// 玩家受到此伤害时施加的饥饿消耗值。
    exhaustion: f32,
    /// 受到此伤害时触发的音效与视觉效果。
    effects: Option<DamageEffects>,
    /// 用于查找死亡消息字符串的翻译键片段。
    message_id: String,
    /// 该伤害类型是否以及何时随游戏难度缩放。
    scaling: DamageScaling,
}

/// 实体受到此类伤害时应用的音效与视觉效果。
#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum DamageEffects {
    Hurt,
    Thorns,
    Drowning,
    Burning,
    Poking,
    Freezing,
}

/// 判断此伤害类型是否随难度缩放。
#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum DamageScaling {
    Never,
    WhenCausedByLivingNonPlayer,
    Always,
}

/// 控制此伤害类型显示哪种死亡消息变体。
#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum DeathMessageType {
    Default,
    FallVariants,
    IntentionalGameDesign,
}

/// 生成 `DamageType` 结构体、其关联枚举和常量的 `TokenStream`。
pub fn build() -> TokenStream {
    let dir = std::path::Path::new("../../assets/datapacks/26_3/data/minecraft/damage_type");
    let mut damage_types: BTreeMap<String, DamageTypeData> = BTreeMap::new();
    let mut entries: Vec<_> = fs::read_dir(dir)
        .expect("缺少 damage_type 目录")
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .collect();
    entries.sort_by_key(|e| e.path());

    for entry in entries {
        let path = entry.path();
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        let content = fs::read_to_string(&path).expect("读取 damage_type 文件失败");
        let data: DamageTypeData =
            serde_json::from_str(&content).expect("解析 damage_type JSON 失败");
        damage_types.insert(stem, data);
    }

    let mut constants = Vec::new();
    let mut type_from_name = TokenStream::new();
    let mut type_from_id = TokenStream::new();

    for (id, (name, data)) in damage_types.iter().enumerate() {
        let const_ident = format_ident!("{}", name.to_shouty_snake_case());
        let resource_name = name.to_lowercase();

        type_from_name.extend(quote! {
            #resource_name => Some(Self::#const_ident),
        });

        let id_lit = LitInt::new(&id.to_string(), proc_macro2::Span::call_site());
        type_from_id.extend(quote! {
            #id_lit => Some(Self::#const_ident),
        });

        let data = data;
        let death_message_type = if let Some(msg) = &data.death_message_type {
            let msg_ident = Ident::new(&format!("{msg:?}"), proc_macro2::Span::call_site());
            quote! { DeathMessageType::#msg_ident }
        } else {
            quote! { DeathMessageType::Default }
        };

        let effects = if let Some(msg) = &data.effects {
            let msg_ident = Ident::new(&format!("{msg:?}"), proc_macro2::Span::call_site());
            quote! { Some(DamageEffects::#msg_ident) }
        } else {
            quote! { None }
        };

        let exhaustion = data.exhaustion;
        let message_id = &data.message_id;
        let scaling_ident = Ident::new(
            &format!("{:?}", data.scaling),
            proc_macro2::Span::call_site(),
        );
        let scaling = quote! {DamageScaling::#scaling_ident};

        constants.push(quote! {
            pub const #const_ident: DamageType = DamageType {
                death_message_type: #death_message_type,
                exhaustion: #exhaustion,
                effects: #effects,
                message_id: #message_id,
                scaling: #scaling,
                id: #id_lit,
            };
        });
    }

    quote! {
        use crate::tag::{RegistryKey, Tag, Taggable};

        #[derive(Clone, Copy, PartialEq, Debug)]
        pub struct DamageType {
            pub death_message_type: DeathMessageType,
            pub exhaustion: f32,
            pub effects: Option<DamageEffects>,
            pub message_id: &'static str,
            pub scaling: DamageScaling,
            pub id: u8,
        }

        #[derive(Clone, Copy, PartialEq, Debug)]
        pub enum DeathMessageType {
            Default,
            FallVariants,
            IntentionalGameDesign,
        }

        #[derive(Clone, Copy, PartialEq, Debug)]
        pub enum DamageEffects {
            Hurt,
            Thorns,
            Drowning,
            Burning,
            Poking,
            Freezing,
        }

        #[derive(Clone, Copy, PartialEq, Debug)]
        pub enum DamageScaling {
            Never,
            WhenCausedByLivingNonPlayer,
            Always,
        }

        impl DamageType {
            #(#constants)*

            #[doc = r" Try to parse a damage type from a resource location string."]
            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    #type_from_name
                    _ => None
                }
            }

            #[doc = r" Try to parse a damage type from a numeric registry id."]
            pub const fn from_id(id: u8) -> Option<Self> {
                match id {
                    #type_from_id
                    _ => None
                }
            }
        }

        impl Taggable for DamageType {
            #[inline]
            fn tag_key() -> RegistryKey {
                RegistryKey::DamageType
            }
            #[inline]
            fn registry_key(&self) -> &str {
                self.message_id
            }
            #[inline]
            fn registry_id(&self) -> u16 {
                self.id as u16
            }
        }
    }
}
