use heck::{ToShoutySnakeCase, ToUpperCamelCase};
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, HashSet},
    fs,
};
use syn::{Ident, LitInt, LitStr};

use crate::block::BlockStateId;

/// 将流体名称（如 `water`）转换为其 SCREAMING_SNAKE_CASE 常量标识符。
fn const_fluid_name_from_fluid_name(fluid: &str) -> String {
    fluid.to_shouty_snake_case()
}

/// 从流体派生名称派生出 PascalCase 属性组结构体名称。
fn property_group_name_from_derived_name(name: &str) -> String {
    format!("{name}_fluid_properties").to_upper_camel_case()
}

/// 将流体属性的原始 snake_case 名称映射到生成的枚举类型名。
struct PropertyVariantMapping {
    /// JSON 中呈现的原始属性名称（例如 `"level"`）。
    original_name: String,
    /// 为此属性生成的枚举的 PascalCase 名称（例如 `"Level"`）。
    property_enum: String,
}

/// 共享同一组方块属性的一组流体的聚合数据。
struct PropertyCollectionData {
    /// 此组中所有流体共享的属性到枚举映射列表。
    variant_mappings: Vec<PropertyVariantMapping>,
    /// 属于此属性组的流体名称。
    fluid_names: Vec<String>,
}

impl PropertyCollectionData {
    /// 向此属性组追加一个流体名称。
    pub fn add_fluid_name(&mut self, fluid_name: String) {
        self.fluid_names.push(fluid_name);
    }

    /// 根据一组变体映射创建新的 `PropertyCollectionData`（此时尚无流体）。
    pub const fn from_mappings(variant_mappings: Vec<PropertyVariantMapping>) -> Self {
        Self {
            variant_mappings,
            fluid_names: Vec::new(),
        }
    }

    /// 从第一个流体的名称派生出该属性组的代表名称。
    pub fn derive_name(&self) -> String {
        format!("{}_like", self.fluid_names[0])
    }
}

/// 一个流体方块属性描述符，保存属性名称及其允许的取值。
#[derive(Deserialize, Clone, Debug)]
pub struct PropertyStruct {
    /// 生成的枚举名称（PascalCase，例如 `"Level"`）。
    pub name: String,
    /// 此属性允许的字符串值（例如 `["0", "1", …, "8"]`）。
    pub values: Vec<String>,
}

impl ToTokens for PropertyStruct {
    /// 为此属性生成枚举定义及 `EnumVariants` impl。
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = Ident::new(&self.name, Span::call_site());

        let variant_count = self.values.len() as u16;
        let values_index = (0..self.values.len() as u16).collect::<Vec<_>>();

        let ident_values = self.values.iter().map(|value| {
            let value_str = if value.chars().all(char::is_numeric) {
                format!("L{value}")
            } else {
                value.clone()
            };
            Ident::new(&value_str.to_upper_camel_case(), Span::call_site())
        });

        let values_2 = ident_values.clone();
        let values_3 = ident_values.clone();

        let from_values = self.values.iter().map(|value| {
            let value_str = if value.chars().all(char::is_numeric) {
                format!("L{value}")
            } else {
                value.clone()
            };
            let ident = Ident::new(&value_str.to_upper_camel_case(), Span::call_site());
            quote! {
                #value => Self::#ident
            }
        });
        let to_values = self.values.iter().map(|value| {
            let value_str = if value.chars().all(char::is_numeric) {
                format!("L{value}")
            } else {
                value.clone()
            };
            let ident = Ident::new(&value_str.to_upper_camel_case(), Span::call_site());
            quote! {
                Self::#ident => #value
            }
        });

        tokens.extend(quote! {
            #[derive(Clone, Copy, Eq, PartialEq)]
            pub enum #name {
                #(#ident_values),*
            }

            impl EnumVariants for #name {
                fn variant_count() -> u16 {
                    #variant_count
                }

                fn to_index(&self) -> u16 {
                    match self {
                        #(Self::#values_2 => #values_index),*
                    }
                }

                fn from_index(index: u16) -> Self {
                    match index {
                        #(#values_index => Self::#values_3,)*
                        _ => panic!("无效索引：{index}"),
                    }
                }

                fn to_value(&self) -> &str {
                    match self {
                        #(#to_values),*
                    }
                }

                fn from_value(value: &str) -> Self {
                    match value {
                        #(#from_values),*,
                        _ => panic!("无效值：{value:?}"),
                    }
                }
            }
        });
    }
}

/// 一个完全解析好的属性组结构体，可直接生成为 `FluidProperties` impl。
struct FluidPropertyStruct {
    /// 此流体组的底层属性集合数据。
    data: PropertyCollectionData,
}

impl ToTokens for FluidPropertyStruct {
    /// 为此属性组生成结构体定义及其 `FluidProperties` impl。
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let struct_name = property_group_name_from_derived_name(&self.data.derive_name());
        let name = Ident::new(&struct_name, Span::call_site());

        let values = self.data.variant_mappings.iter().map(|entry| {
            let key = Ident::new_raw(&entry.original_name, Span::call_site());
            let value = Ident::new(&entry.property_enum, Span::call_site());

            quote! {
                #key: #value
            }
        });

        let fluid_names = &self.data.fluid_names;

        let field_names: Vec<_> = self
            .data
            .variant_mappings
            .iter()
            .rev()
            .map(|entry| Ident::new_raw(&entry.original_name, Span::call_site()))
            .collect();

        let field_types: Vec<_> = self
            .data
            .variant_mappings
            .iter()
            .rev()
            .map(|entry| Ident::new(&entry.property_enum, Span::call_site()))
            .collect();

        let to_props_values = self.data.variant_mappings.iter().map(|entry| {
            let key = &entry.original_name;
            let key2 = Ident::new_raw(&entry.original_name, Span::call_site());

            quote! {
                (#key.to_string(), self.#key2.to_value().to_string()),
            }
        });

        let from_props_values = self.data.variant_mappings.iter().map(|entry| {
            let key = &entry.original_name;
            let key2 = Ident::new_raw(&entry.original_name, Span::call_site());
            let value = Ident::new(&entry.property_enum, Span::call_site());

            quote! {
                #key => fluid_props.#key2 = #value::from_value(&value)
            }
        });

        tokens.extend(quote! {
            #[derive(Clone, Copy, Eq, PartialEq)]
            pub struct #name {
                #(pub #values),*
            }

            impl FluidProperties for #name {
                #[allow(unused_assignments)]
                fn to_index(&self) -> u16 {
                    let mut index = 0;
                    let mut multiplier = 1;

                    #(
                        index += self.#field_names.to_index() * multiplier;
                        multiplier *= #field_types::variant_count();
                    )*

                    index
                }

                #[allow(unused_assignments)]
                fn from_index(mut index: u16) -> Self {
                    Self {
                        #(
                            #field_names: {
                                let value = index % #field_types::variant_count();
                                index /= #field_types::variant_count();
                                #field_types::from_index(value)
                            }
                        ),*
                    }
                }

                fn to_state_id(&self, fluid: &Fluid) -> BlockStateId {
                    if ![#(#fluid_names),*].contains(&fluid.name) {
                        panic!("{} 不是 {} 的有效流体", fluid.name, #struct_name);
                    }

                    let prop_index = self.to_index();
                    if prop_index < fluid.states.len() as u16 {
                        fluid.states[prop_index as usize].block_state_id
                    } else {
                        fluid.states[fluid.default_state_index as usize].block_state_id
                    }
                }

                fn from_state_id(id: BlockStateId, fluid: &Fluid) -> Self {
                    if ![#(#fluid_names),*].contains(&fluid.name) {
                        panic!("{} 不是 {} 的有效流体", &fluid.name, #struct_name);
                    }

                    for (idx, state) in fluid.states.iter().enumerate() {
                        if state.block_state_id == id {
                            return Self::from_index(idx as u16);
                        }
                    }

                    Self::from_index(fluid.default_state_index)
                }

                fn default(fluid: &Fluid) -> Self {
                    if ![#(#fluid_names),*].contains(&fluid.name) {
                        panic!("{} 不是 {} 的有效流体", &fluid.name, #struct_name);
                    }

                    Self::from_index(fluid.default_state_index)
                }

                fn to_props(&self) -> Vec<(String, String)> {
                   vec![#(#to_props_values)*]
                }

                fn from_props(props: Vec<(String, String)>, fluid: &Fluid) -> Self {
                    if ![#(#fluid_names),*].contains(&fluid.name) {
                        panic!("{} 不是 {} 的有效流体", &fluid.name, #struct_name);
                    }

                    let mut fluid_props = Self::default(fluid);

                    for (key, value) in props {
                        match key.as_str() {
                            #(#from_props_values),*,
                            _ => panic!("无效键：{key}"),
                        }
                    }

                    fluid_props
                }
            }
        });
    }
}

/// `fluids.json` 中单个流体状态条目的原始反序列化结构。
#[derive(Deserialize, Clone)]
struct FluidState {
    /// 此流体填充整方块的占比（0.0–1.0）。
    height: f32,
    /// 流体的数字等级（0 = 源头，1–7 = 流动）。
    level: i16,
    /// 此状态是否表示空的（空气）流体槽位。
    is_empty: bool,
    /// 此状态下流体的抗爆强度。
    blast_resistance: f32,
    /// 用于在世界中标识此流体状态的方块状态 ID。
    block_state_id: BlockStateId,
    /// 流体是否静止（而非流动）。
    is_still: bool,
    // 我们将从现有字段推导 is_source 和 falling，而不要求 JSON 中提供它们
}

/// 对流体状态的轻量级引用，将流体相对索引与
/// 去重后的部分状态索引。
#[derive(Clone, Debug)]
struct FluidStateRef {
    /// 此状态在其流体状态数组中的索引。
    pub id: u16,
    /// 指向全局 `FLUID_STATES` 去重表的索引。
    pub state_idx: u16,
}
impl ToTokens for FluidStateRef {
    /// 生成一个 `FluidStateRef { id, state_idx }` 结构体字面量。
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let id = LitInt::new(&self.id.to_string(), Span::call_site());
        let state_idx = LitInt::new(&self.state_idx.to_string(), Span::call_site());
        tokens.extend(quote! {
            FluidStateRef {
                id: #id,
                state_idx: #state_idx,
            }
        });
    }
}

/// 流体的单个方块状态属性，列于 `fluids.json` 中。
#[derive(Deserialize, Clone)]
struct Property {
    /// 属性名称（例如 `"level"`）。
    name: String,
    /// 此属性的有效字符串值列表。
    values: Vec<String>,
}

/// `fluids.json` 中单个流体条目的原始反序列化结构。
#[derive(Deserialize, Clone)]
pub struct Fluid {
    /// 流体的注册表名称（例如 `"water"`）。
    pub name: String,
    /// 标识此流体类型的数字 ID。
    pub id: u16,
    /// 此流体暴露的方块状态属性（如 `level`、`falling`）。
    properties: Vec<Property>,
    /// 默认流体状态在 `states` 中的索引。
    default_state_index: u16,
    /// 此流体所有可能的状态，每种属性组合对应一个。
    states: Vec<FluidState>,
    /// 每次流动蔓延步骤之间相隔的刻数（越小越快）。
    #[serde(default = "default_flow_speed")]
    flow_speed: u32,
    /// 此流体从源头可水平流动的最大方块数。
    #[serde(default = "default_flow_distance")]
    flow_distance: u32,
    /// 两个相邻的源方块能否创建新的源方块。
    #[serde(default)]
    can_convert_to_source: bool,
}

/// 默认流动速度（扩散步骤之间的刻数），与原版水一致。
const fn default_flow_speed() -> u32 {
    5 // 默认为水的 speed
}

/// 默认水平流动距离（以方块计），与原版水一致。
const fn default_flow_distance() -> u32 {
    4 // 默认为水的 distance
}

/// 生成 `Fluid` 结构体、`FluidState`、`FluidProperties` trait 的 `TokenStream`，
/// 每种流体各自的属性枚举，以及所有查找函数。
pub fn build() -> TokenStream {
    let fluids: Vec<Fluid> =
        match serde_json::from_str(&fs::read_to_string("../../assets/fluids.json").unwrap()) {
            Ok(fluids) => fluids,
            Err(e) => panic!("解析 fluids.json 失败：{e}"),
        };

    let mut constants = TokenStream::new();
    let mut id_matches = Vec::new();
    let mut type_from_name = TokenStream::new();
    let mut type_from_raw_id_arms = TokenStream::new();
    let mut fluid_from_state_id = TokenStream::new();

    // 收集 from_state_id 分支，按区间宽度排序（窄的在前）
    struct StateIdArm {
        start: u16,
        end: u16,
        const_name: String,
    }
    let mut state_id_arms: Vec<StateIdArm> = Vec::new();

    let mut fluid_properties_from_state_and_name = TokenStream::new();
    let mut fluid_properties_from_props_and_name = TokenStream::new();

    // 用于创建属性 `enum`。
    let mut property_enums: BTreeMap<String, PropertyStruct> = BTreeMap::new();
    // 流体的属性实现。
    let mut fluid_properties: Vec<FluidPropertyStruct> = Vec::new();
    // 一组属性名到具有这些属性的流体的映射。
    let mut property_collection_map: BTreeMap<Vec<String>, PropertyCollectionData> =
        BTreeMap::new();
    // 确保没有 `enum` 冲突的校验器。
    let mut enum_to_values: BTreeMap<String, Vec<String>> = BTreeMap::new();

    // 收集唯一的流体状态以创建部分状态
    let mut unique_states = Vec::new();
    let mut optimized_fluids: Vec<(String, FluidStateRef)> = Vec::new();

    for fluid in fluids {
        let id_name = LitStr::new(&fluid.name, Span::call_site());
        let const_ident = format_ident!("{}", fluid.name.to_shouty_snake_case());
        let state_id_start = fluid
            .states
            .iter()
            .map(|state| state.block_state_id.0)
            .min()
            .unwrap();
        let state_id_end = fluid
            .states
            .iter()
            .map(|state| state.block_state_id.0)
            .max()
            .unwrap();

        let id_lit = LitInt::new(&fluid.id.to_string(), Span::call_site());
        let mut properties = TokenStream::new();
        if fluid.properties.is_empty() {
            properties.extend(quote!(None));
        } else {
            let internal_properties = fluid.properties.iter().map(|property| {
                let key = LitStr::new(&property.name, Span::call_site());
                let values = property
                    .values
                    .iter()
                    .map(|value| LitStr::new(value, Span::call_site()));

                quote! {
                    (#key, &[
                        #(#values),*
                    ])
                }
            });
            properties.extend(quote! {
                Some(&[
                    #(#internal_properties),*
                ])
            });
        }

        for (idx, state) in fluid.states.iter().enumerate() {
            // 通过比较关键字段，检查该状态是否已在 `unique_states` 中
            let already_exists = unique_states.iter().any(|s: &FluidState| {
                s.height == state.height
                    && s.level == state.level
                    && s.is_empty == state.is_empty
                    && s.blast_resistance == state.blast_resistance
                    && s.is_still == state.is_still
            });
            if !already_exists {
                unique_states.push(state.clone());
            }
            // 创建对该状态的引用
            let state_idx = unique_states
                .iter()
                .position(|s| {
                    s.height == state.height
                        && s.level == state.level
                        && s.is_empty == state.is_empty
                        && s.blast_resistance == state.blast_resistance
                        && s.is_still == state.is_still
                })
                .unwrap() as u16;
            optimized_fluids.push((
                fluid.name.clone(),
                FluidStateRef {
                    id: idx as u16,
                    state_idx,
                },
            ));
        }
        state_id_arms.push(StateIdArm {
            start: state_id_start,
            end: state_id_end,
            const_name: const_fluid_name_from_fluid_name(&fluid.name),
        });

        type_from_name.extend(quote! {
            #id_name => Some(&Self::#const_ident),
        });

        type_from_raw_id_arms.extend(quote! {
            #id_lit => Some(&Self::#const_ident),
        });

        let fluid_states = fluid.states.iter().map(|state| {
            let height = state.height;
            let level = state.level;
            let is_empty = state.is_empty;
            let blast_resistance = state.blast_resistance;
            let block_state_id = state.block_state_id;
            let is_still = state.is_still;
            // 根据现有字段推导这些值
            let is_source = is_still; // 等级为 0 仍意味着它是源
            let falling = false; // 默认为 false——我们将在流体行为代码中处理下落

            quote! {
                FluidState {
                    height: #height,
                    level: #level,
                    is_empty: #is_empty,
                    blast_resistance: #blast_resistance,
                    block_state_id: #block_state_id,
                    is_still: #is_still,
                    is_source: #is_source,
                    falling: #falling,
                }
            }
        });
        let state_id = fluid.default_state_index;
        let flow_speed = fluid.flow_speed;
        let flow_distance = fluid.flow_distance;
        let can_convert_to_source = fluid.can_convert_to_source;

        id_matches.push(quote! {
            #id_name => Some(#id_lit),
        });

        constants.extend(quote! {
            pub const #const_ident: Fluid = Fluid {
                id: #id_lit,
                name: #id_name,
                properties: #properties,
                states: &[#(#fluid_states),*],
                default_state_index: #state_id,
                flow_speed: #flow_speed,
                flow_distance: #flow_distance,
                can_convert_to_source: #can_convert_to_source,
            };
        });

        let mut property_collection = HashSet::new();
        let mut property_mapping = Vec::new();
        for property in &fluid.properties {
            property_collection.insert(property.name.clone());

            // 获取映射后的属性 `enum` 名称
            let renamed_property = property.name.to_upper_camel_case();

            let expected_values = enum_to_values
                .entry(renamed_property.clone())
                .or_insert_with(|| property.values.clone());

            assert_eq!(
                expected_values, &property.values,
                "Enum overlap for '{}' ({:?} vs {:?})",
                property.name, property.values, expected_values
            );

            property_mapping.push(PropertyVariantMapping {
                original_name: property.name.clone(),
                property_enum: renamed_property.clone(),
            });

            // 如果此属性还没有 `enum`，就创建一个。
            let _ = property_enums
                .entry(renamed_property.clone())
                .or_insert_with(|| PropertyStruct {
                    name: renamed_property,
                    values: property.values.clone(),
                });
        }

        if !property_collection.is_empty() {
            let mut property_collection = Vec::from_iter(property_collection);
            property_collection.sort();
            property_collection_map
                .entry(property_collection)
                .or_insert_with(|| PropertyCollectionData::from_mappings(property_mapping))
                .add_fluid_name(fluid.name.clone());
        }
    }

    let unique_fluid_states = unique_states.iter().map(|state| {
        let height = state.height;
        let level = state.level;
        let is_empty = state.is_empty;
        let blast_resistance = state.blast_resistance;
        let block_state_id = state.block_state_id;
        let is_still = state.is_still;
        let is_source = is_still;
        let falling = false;
        quote! {
            PartialFluidState {
                height: #height,
                level: #level,
                is_empty: #is_empty,
                blast_resistance: #blast_resistance,
                block_state_id: #block_state_id,
                is_still: #is_still,
                is_source: #is_source,
                falling: #falling,
            }
        }
    });

    // 先匹配更窄的范围，使静态流体先于其流动形态被匹配
    state_id_arms.sort_by_key(|arm| arm.end - arm.start);
    for arm in &state_id_arms {
        let start = LitInt::new(&arm.start.to_string(), Span::call_site());
        let end = LitInt::new(&arm.end.to_string(), Span::call_site());
        let const_ident = format_ident!("{}", arm.const_name);
        fluid_from_state_id.extend(quote! {
            #start..=#end => Some(&Fluid::#const_ident),
        });
    }

    for property_group in property_collection_map.into_values() {
        for fluid_name in &property_group.fluid_names {
            let const_fluid_name = Ident::new(
                &const_fluid_name_from_fluid_name(fluid_name),
                Span::call_site(),
            );
            let property_name = Ident::new(
                &property_group_name_from_derived_name(&property_group.derive_name()),
                Span::call_site(),
            );

            fluid_properties_from_state_and_name.extend(quote! {
                #fluid_name => Box::new(#property_name::from_state_id(id, &Fluid::#const_fluid_name)),
            });

            fluid_properties_from_props_and_name.extend(quote! {
                #fluid_name => Box::new(#property_name::from_props(props, &Fluid::#const_fluid_name)),
            });
        }

        fluid_properties.push(FluidPropertyStruct {
            data: property_group,
        });
    }

    let fluid_props = fluid_properties.iter().map(ToTokens::to_token_stream);
    let properties = property_enums.values().map(ToTokens::to_token_stream);

    quote! {
        use std::hash::{Hash, Hasher};
        use crate::tag::{Taggable, RegistryKey};
        use crate::BlockStateId;
        use papokin_util::resource_location::{FromResourceLocation, ResourceLocation, ToResourceLocation};

        #[derive(Clone)]
        pub struct PartialFluidState {
            pub height: f32,
            pub level: i16,
            pub is_empty: bool,
            pub blast_resistance: f32,
            pub block_state_id: BlockStateId,
            pub is_still: bool,
            pub is_source: bool,
            pub falling: bool,
        }

        #[derive(Clone)]
        pub struct FluidState {
            pub height: f32,
            pub level: i16,
            pub is_empty: bool,
            pub blast_resistance: f32,
            pub block_state_id: BlockStateId,
            pub is_still: bool,
            pub is_source: bool,
            pub falling: bool,
        }

        #[derive(Clone)]
        pub struct FluidStateRef {
            pub id: u16,
            pub state_idx: u16,
        }

        #[derive(Clone)]
        pub struct Fluid {
            pub id: u16,
            pub name: &'static str,
            pub properties: Option<&'static [(&'static str, &'static [&'static str])]>,
            pub states: &'static [FluidState],
            pub default_state_index: u16,
            pub flow_speed: u32,
            pub flow_distance: u32,
            pub can_convert_to_source: bool,
        }

        impl Hash for Fluid {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.id.hash(state);
            }
        }

        impl PartialEq for Fluid {
            fn eq(&self, other: &Self) -> bool {
                self.id == other.id
            }
        }

        impl Eq for Fluid {}

        pub const FLUID_STATES: &[PartialFluidState] = &[
            #(#unique_fluid_states),*
        ];

        pub trait EnumVariants {
            fn variant_count() -> u16;
            fn to_index(&self) -> u16;
            fn from_index(index: u16) -> Self;
            fn to_value(&self) -> &str;
            fn from_value(value: &str) -> Self;
        }

        pub trait FluidProperties where Self: 'static {
            // 将属性转换为索引（`0` 到 `N-1`）。
            fn to_index(&self) -> u16;
            // 将索引转换回属性。
            fn from_index(index: u16) -> Self where Self: Sized;

            // 将属性转换为状态 id。
            fn to_state_id(&self, fluid: &Fluid) -> BlockStateId;
            // 将状态 id 转换回属性。
            fn from_state_id(id: BlockStateId, fluid: &Fluid) -> Self where Self: Sized;
            // 获取默认属性。
            fn default(fluid: &Fluid) -> Self where Self: Sized;

            // 将属性转换为 `(name, value)` 的 `Vec`
            fn to_props(&self) -> Vec<(String, String)>;

            // 将属性转换为流体状态，并叠加到默认状态上。
            fn from_props(props: Vec<(String, String)>, fluid: &Fluid) -> Self where Self: Sized;
        }

        pub fn get_fluid(registry_id: &str) -> Option<&'static Fluid> {
           let key = registry_id.strip_prefix("minecraft:").unwrap_or(registry_id);
           Fluid::from_registry_key(key)
        }

        impl Fluid {
            #constants

            pub fn from_registry_key(name: &str) -> Option<&'static Self> {
                match name {
                    #type_from_name
                    _ => None
                }
            }

            pub const fn from_id(id: u16) -> Option<&'static Self> {
                match id {
                    #type_from_raw_id_arms
                    _ => None
                }
            }

            #[allow(unreachable_patterns, clippy::match_overlapping_arm)]
            pub const fn from_state_id(id: BlockStateId) -> Option<&'static Self> {
                match id.as_u16() {
                    #fluid_from_state_id
                    _ => None
                }
            }


            pub fn ident_to_fluid_id(name: &str) -> Option<u8> {
                match name {
                    #(#id_matches)*
                    _ => None
                }
            }

            #[track_caller]
            #[doc = r" Get the properties of the fluid."]
            pub fn properties(&self, id: BlockStateId) -> Box<dyn FluidProperties> {
                match self.name {
                    #fluid_properties_from_state_and_name
                    _ => panic!("无效的 state_id")
                }
            }

            #[track_caller]
            #[doc = r" Get the properties of the fluid."]
            pub fn from_properties(&self, props: Vec<(String, String)>) -> Box<dyn FluidProperties> {
                match self.name {
                    #fluid_properties_from_props_and_name
                    _ => panic!("无效属性")
                }
            }

            pub fn same_fluid_type(a: u16, b: u16) -> bool {
                a == b
                    || (a == 1 && b == 2)
                    || (a == 2 && b == 1)
                    || (a == 3 && b == 4)
                    || (a == 4 && b == 3)
            }
            pub fn matches_type(&self, other: &Fluid) -> bool {
                Self::same_fluid_type(self.id, other.id)
            }
            pub fn to_flowing(&self) -> &'static Fluid {
                match self.id {
                    2 => &Fluid::FLOWING_WATER,
                    4 => &Fluid::FLOWING_LAVA,
                    _ => Fluid::from_id(self.id).unwrap_or(&Fluid::EMPTY),
                }
            }

            // 添加了流体行为的辅助方法
            pub fn is_source(&self, state_id: BlockStateId) -> bool {
                let idx = (state_id.as_u16() as usize) % self.states.len();
                self.states[idx].is_source
            }

            pub fn is_falling(&self, state_id: BlockStateId) -> bool {
                let idx = (state_id.as_u16() as usize) % self.states.len();
                self.states[idx].falling
            }

            pub fn get_level(&self, state_id: BlockStateId) -> i16 {
                let idx = (state_id.as_u16() as usize) % self.states.len();
                self.states[idx].level
            }

            pub fn get_height(&self, state_id: BlockStateId) -> f32 {
                let idx = (state_id.as_u16() as usize) % self.states.len();
                self.states[idx].height
            }
        }

        impl ToResourceLocation for &'static Fluid {
            fn to_resource_location(&self) -> ResourceLocation {
                format!("minecraft:{}", self.name)
            }
        }

        impl FromResourceLocation for &'static Fluid {
            fn from_resource_location(resource_location: &ResourceLocation) -> Option<Self> {
                Fluid::from_registry_key(resource_location.strip_prefix("minecraft:").unwrap_or(resource_location))
            }
        }

        impl FluidStateRef {
            pub fn get_state(&self) -> FluidState {
                let partial_state = &FLUID_STATES[self.state_idx as usize];
                FluidState {
                    height: partial_state.height,
                    level: partial_state.level,
                    is_empty: partial_state.is_empty,
                    blast_resistance: partial_state.blast_resistance,
                    block_state_id: partial_state.block_state_id,
                    is_still: partial_state.is_still,
                    is_source: partial_state.is_source,
                    falling: partial_state.falling,
                }
            }
        }

        impl Taggable for Fluid {
            #[inline]
            fn tag_key() -> RegistryKey {
                RegistryKey::Fluid
            }

            #[inline]
            fn registry_key(&self) -> &str {
                self.name
            }

            #[inline]
            fn registry_id(&self) -> u16 {
                self.id
            }
        }

        // 添加了 FluidLevel 枚举及所需常量
        pub const FLUID_LEVEL_SOURCE: i32 = 0;
        pub const FLUID_LEVEL_FLOWING_MAX: i32 = 8;
        pub const FLUID_MIN_HEIGHT: f32 = 0.0;
        pub const FLUID_MAX_HEIGHT: f32 = 1.0;

        #(#properties)*

        #(#fluid_props)*
    }
}
