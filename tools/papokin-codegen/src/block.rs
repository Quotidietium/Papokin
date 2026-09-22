use heck::{ToShoutySnakeCase, ToUpperCamelCase};
use papokin_util::math::{experience::Experience, vector3::Vector3};
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, HashSet},
    fs,
};
use syn::{Ident, LitInt, LitStr};

use crate::bitsets::{Bitset, gen_u16_bitset};

/// 将稀疏的索引-值列表转换为稠密数组，空缺处用 `None` token 填充。
///
/// # Arguments
/// – `array` – `(index, value)` 对，其中 `index` 为输出数组中的位置。
fn fill_array<T: Clone + ToTokens>(array: Vec<(u16, T)>) -> Vec<TokenStream> {
    let max_index = array.iter().map(|(index, _)| index).max().unwrap();
    let mut raw_id_from_state_id_ordered = vec![quote! { None }; (max_index + 1) as usize];

    for (state_id, id_lit) in array {
        raw_id_from_state_id_ordered[state_id as usize] = quote! { #id_lit };
    }

    raw_id_from_state_id_ordered
}

/// 将稀疏的 `(block_ident, state_index, state_id)` 列表转换为稠密的 `TokenStream` 数组。
///
/// # Arguments
/// – `array` – `(block_ident, state_index, state_id)` 三元组，将每个方块状态 ID 映射到方块常量与局部索引。
fn fill_state_array(array: Vec<(Ident, usize, u16)>) -> Vec<TokenStream> {
    let max_index = array.iter().map(|(_, _, index)| index).max().unwrap();
    let mut ret = vec![quote! { Missed State }; (max_index + 1) as usize];

    for (block, index, state_id) in array {
        let index_lit = LitInt::new(&index.to_string(), Span::call_site());
        ret[state_id as usize] = quote! { &Block::#block.states[#index_lit] };
    }

    ret
}

/// 将方块注册表名称转换为 SCREAMING_SNAKE_CASE 命名的常量标识符。
///
/// # Arguments
/// – `block` – 小写的注册表名称（例如 `"stone_slab"`）。
fn const_block_name_from_block_name(block: &str) -> String {
    block.to_shouty_snake_case()
}

/// 从派生的基础名称派生出方块属性组的 UpperCamelCase 结构体名称。
///
/// # Arguments
/// – `name` – 基础名称，如 `"oak_slab_like"`，会被转换为例如 `"OakSlabLikeProperties"`。
fn property_group_name_from_derived_name(name: &str) -> String {
    format!("{name}_properties").to_upper_camel_case()
}

fn common_suffix_group_alias(blocks: &[(String, u16)]) -> Option<String> {
    if blocks.is_empty() {
        return None;
    }
    let token_lists: Vec<Vec<&str>> = blocks
        .iter()
        .map(|(name, _)| name.split('_').collect())
        .collect();

    let first = &token_lists[0];
    let mut common_suffix_len = 0;

    for i in 1..=first.len() {
        let candidate_suffix = &first[first.len() - i..];
        let all_match = token_lists
            .iter()
            .all(|tokens| tokens.len() >= i && &tokens[tokens.len() - i..] == candidate_suffix);
        if all_match {
            common_suffix_len = i;
        } else {
            break;
        }
    }

    if common_suffix_len > 0 {
        let suffix_tokens = &first[first.len() - common_suffix_len..];
        let suffix_str = suffix_tokens.join("_");
        Some(format!("{suffix_str}_properties").to_upper_camel_case())
    } else {
        None
    }
}

/// 区分方块属性的两种运行时表示形式。
enum PropertyType {
    /// 该属性是简单的布尔值（`true`/`false`）。
    Bool,
    /// 该属性是一个整数，取值范围为闭区间。
    Int { min: u8, max: u8 },
    /// 该属性是一个枚举，具有由 `name` 标识的生成 Rust 类型。
    Enum { name: String },
}

/// 将单个方块属性字段映射到其 Rust 类型表示。
struct PropertyVariantMapping {
    /// JSON 中呈现的序列化属性名称（例如 `"facing"`）。
    original_name: String,
    /// 在生成的代码中表示此属性所用的 Rust 类型。
    property_type: PropertyType,
}

/// 为共享同一组方块属性的一组方块累积的数据。
struct PropertyCollectionData {
    /// 构成所生成结构体字段的属性到类型有序映射列表。
    variant_mappings: Vec<PropertyVariantMapping>,
    /// 属于此属性组的所有方块（按名称和数字 ID）。
    blocks: Vec<(String, u16)>,
}

impl PropertyCollectionData {
    /// 注册一个共享此属性组的附加方块。
    ///
    /// # Arguments
    /// – `block_name` – 方块的注册表名称。
    /// – `block_id` – 数字形式的方块 ID。
    pub fn add_block(&mut self, block_name: String, block_id: u16) {
        self.blocks.push((block_name, block_id));
    }

    /// 根据有序的属性映射列表创建新的 `PropertyCollectionData`（此时尚未注册任何方块）。
    ///
    /// # Arguments
    /// – `variant_mappings` – 该组的属性到类型映射。
    pub const fn from_mappings(variant_mappings: Vec<PropertyVariantMapping>) -> Self {
        Self {
            variant_mappings,
            blocks: Vec::new(),
        }
    }

    /// 从第一个注册的方块派生出该属性组的基础名称。
    pub fn derive_name(&self) -> String {
        format!("{}_like", self.blocks[0].0)
    }
}

/// 单个方块属性枚举的反序列化表示，用于生成 Rust 枚举定义。
#[derive(Deserialize, Clone, Debug)]
pub struct PropertyStruct {
    /// 用于生成的 Rust 枚举的 UpperCamelCase 名称（例如 `"Facing"`）。
    pub name: String,
    /// 变体值字符串的有序列表（例如 `["north", "south", ...]`）。
    pub values: Vec<String>,
}

impl ToTokens for PropertyStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if self.values[0] == "true" && self.values[1] == "false" {
            return;
        }

        let name = Ident::new(&self.name, Span::call_site());
        let count = self.values.len();
        let variant_count = count as u16;

        let is_number_values = !self.values.is_empty()
            && self.values.iter().all(|v| v.starts_with('L'))
            && self.values.iter().any(|v| v == "L1");

        let mut variants = Vec::with_capacity(count);
        let mut literals = Vec::with_capacity(count);
        let mut indices = Vec::with_capacity(count);

        for (i, raw_value) in self.values.iter().enumerate() {
            let ident = Ident::new(&raw_value.to_upper_camel_case(), Span::call_site());

            let literal_str = if is_number_values {
                raw_value.strip_prefix('L').unwrap_or(raw_value)
            } else {
                raw_value.as_str()
            };

            variants.push(ident);
            literals.push(literal_str);
            indices.push(i as u16);
        }
        tokens.extend(quote! {
            #[derive(Clone, Copy, Debug, Eq, PartialEq)]
            pub enum #name {
                #(#variants),*
            }

            impl EnumVariants for #name {
                fn variant_count() -> u16 {
                    #variant_count
                }

                fn to_index(&self) -> u16 {
                    match self {
                        #(Self::#variants => #indices),*
                    }
                }

                fn from_index(index: u16) -> Self {
                    match index {
                        #(#indices => Self::#variants,)*
                        _ => panic!("无效索引：{index}"),
                    }
                }

                fn to_value(&self) -> &'static str {
                    match self {
                        #(Self::#variants => #literals),*
                    }
                }

                fn from_value(value: &str) -> Self {
                    match value {
                        #(#literals => Self::#variants),*,
                        _ => panic!("无效值：{value}"),
                    }
                }
            }
        });
    }
}

/// 代码生成包装器，为单个属性组生成完整的 `BlockProperties` impl。
struct BlockPropertyStruct {
    /// 用于生成结构体及其 trait 实现的属性组数据。
    data: PropertyCollectionData,
    /// 此属性结构体的类型别名。
    aliases: Vec<Ident>,
}

impl ToTokens for BlockPropertyStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let struct_name = property_group_name_from_derived_name(&self.data.derive_name());
        let name = Ident::new(&struct_name, Span::call_site());

        // 生成结构体字段
        let fields = self.data.variant_mappings.iter().map(|entry| {
            let key = Ident::new_raw(&entry.original_name, Span::call_site());
            match &entry.property_type {
                PropertyType::Bool => quote! { pub #key: bool },
                PropertyType::Int { .. } => quote! { pub #key: u8 },
                PropertyType::Enum { name } => {
                    let value = Ident::new(name, Span::call_site());
                    quote! { pub #key: #value }
                }
            }
        });

        let block_ids = self.data.blocks.iter().map(|(name, _)| {
            Ident::new(&const_block_name_from_block_name(name), Span::call_site())
        });

        let to_index_logic = self.data.variant_mappings.iter().rev().map(|entry| {
            let field = Ident::new_raw(&entry.original_name, Span::call_site());
            match &entry.property_type {
                PropertyType::Bool => quote! { (!self.#field as u16, 2) },
                PropertyType::Int { min, max } => {
                    let count = (max - min + 1) as u16;

                    if *min > 0 {
                        quote! { ((self.#field - #min) as u16, #count) }
                    } else {
                        quote! { (self.#field as u16, #count) }
                    }
                }
                PropertyType::Enum { name } => {
                    let ty = Ident::new(name, Span::call_site());
                    quote! { (self.#field.to_index(), #ty::variant_count()) }
                }
            }
        });

        let from_index_body = self
            .data
            .variant_mappings
            .iter()
            .rev()
            .map(|entry| {
                let field_name = Ident::new_raw(&entry.original_name, Span::call_site());
                match &entry.property_type {
                    PropertyType::Bool => quote! {
                        #field_name: {
                            let value = index % 2;
                            index /= 2;
                            value == 0
                        }
                    },
                    PropertyType::Int { min, max } => {
                        let count = (max - min + 1) as u16;
                        let val = if *min > 0 {
                            quote! { value + #min }
                        } else {
                            quote! {value}
                        };
                        quote! {
                            #field_name: {
                                let value = (index % #count) as u8;
                                index /= #count;
                                #val
                            }
                        }
                    }
                    PropertyType::Enum { name } => {
                        let enum_ident = Ident::new(name, Span::call_site());
                        quote! {
                            #field_name: {
                                let value = index % #enum_ident::variant_count();
                                index /= #enum_ident::variant_count();
                                #enum_ident::from_index(value)
                            }
                        }
                    }
                }
            })
            .collect::<Vec<_>>();

        let to_props_entries = self.data.variant_mappings.iter().map(|entry| {
            let key_str = &entry.original_name;
            let field = Ident::new_raw(&entry.original_name, Span::call_site());
            match &entry.property_type {
                PropertyType::Bool => quote! {
                    (#key_str, if self.#field { "true" } else { "false" })
                },
                PropertyType::Int { min, max } => {
                    let mut arms = Vec::new();
                    for i in *min..=*max {
                        let i_str = i.to_string();
                        arms.push(quote! { #i => #i_str });
                    }
                    quote! {
                        (#key_str, match self.#field {
                            #(#arms,)*
                            _ => unreachable!()
                        })
                    }
                }
                PropertyType::Enum { .. } => quote! {
                    (#key_str, self.#field.to_value())
                },
            }
        });

        let from_props_keys = self
            .data
            .variant_mappings
            .iter()
            .map(|entry| &entry.original_name);
        let from_props_values = self.data.variant_mappings.iter().map(|entry| {
            let field_name = Ident::new_raw(&entry.original_name, Span::call_site());
            match &entry.property_type {
                PropertyType::Bool => quote! {
                    block_props.#field_name = matches!(*value, "true")
                },
                PropertyType::Int { min, max } => {
                    let mut arms = Vec::new();
                    for i in *min..=*max {
                        let i_str = i.to_string();
                        arms.push(quote! { #i_str => #i });
                    }
                    quote! {
                        block_props.#field_name = match *value {
                            #(#arms,)*
                            _ => #min,
                        }
                    }
                }
                PropertyType::Enum { name } => {
                    let enum_ident = Ident::new(name, Span::call_site());
                    quote! {
                        block_props.#field_name = #enum_ident::from_value(value)
                    }
                }
            }
        });

        let from_props_loop_body = if self.data.variant_mappings.len() > 1 {
            quote! {
                match *key {
                    #(#from_props_keys => #from_props_values),*,
                    _ => {}, //
                }
            }
        } else {
            let key = from_props_keys.into_iter().next();
            let val = from_props_values.into_iter().next();
            quote! { if *key == #key { #val } }
        };

        let aliases = self.aliases.iter().map(|alias| {
            quote! { pub type #alias = #name; }
        });

        tokens.extend(quote! {
            #[derive(Clone, Copy, Eq, PartialEq)]
            pub struct #name {
                #(#fields),*
            }

            #(#aliases)*

            impl #name {
                #[inline]
                #[must_use]
                pub fn from_index(mut index: u16) -> Self {
                    Self {
                        #(#from_index_body),*
                    }
                }

                #[inline]
                #[must_use]
                pub fn from_state_id(id: BlockStateId) -> Self {
                    let block = Block::from_state_id(id);
                    let min_id = block.states[0].id.as_u16();
                    Self::from_index(id.as_u16() - min_id)
                }

                #[inline]
                #[must_use]
                pub fn to_state_id(&self, block: &Block) -> BlockStateId {
                    <Self as BlockProperties>::to_state_id(self, block)
                }

                #[inline]
                #[must_use]
                pub fn default(block: &Block) -> Self {
                    <Self as BlockProperties>::default(block)
                }
            }

            impl BlockProperties for #name {
               fn to_index(&self) -> u16 {
                    let (index, _) = [#(#to_index_logic),*]
                        .iter()
                        .fold((0, 1), |(curr, mul), &(val, count)| (curr + val * mul, mul * count));
                    index
                }

                #[allow(unused_assignments)]
                fn from_index(index: u16) -> Self {
                    Self::from_index(index)
                }

                #[inline]
                #[allow(clippy::manual_range_patterns)]
                fn handles_block_id(block_id: BlockId) -> bool where Self: Sized {
                    matches!(block_id, #(BlockId::#block_ids)|*)
                }

                fn to_state_id(&self, block: &Block) -> BlockStateId {
                    if !Self::handles_block_id(block.id) {
                        panic!("{} 对 {} 不是有效方块", block.name, #struct_name);
                    }
                    block.states[self.to_index() as usize].id
                }

                fn from_state_id(id: BlockStateId, block: &Block) -> Self {
                    debug_assert!(
                        Self::handles_block_id(block.id),
                        "{} is not a valid block for {}", block.name, #struct_name
                    );

                    let min_id = block.states[0].id.as_u16();
                    let max_id = block.states.last().map(|s| s.id.as_u16()).unwrap_or(min_id);

                    if (min_id..=max_id).contains(&id.as_u16()) {
                        Self::from_index(id.as_u16() - min_id)
                    } else {
                        #[cfg(debug_assertions)]
                        panic!("状态 ID {} 对 {} 不存在", id, block.name);

                        #[cfg(not(debug_assertions))]
                        Self::from_index(0)
                    }
                }

                fn default(block: &Block) -> Self {
                    if !Self::handles_block_id(block.id) {
                        panic!("{} 对 {} 不是有效方块", block.name, #struct_name);
                    }
                    Self::from_state_id(block.default_state.id)
                }

                fn to_props(&self) -> Vec<(&'static str, &'static str)> {
                   vec![ #(#to_props_entries),* ]
                }

                #[allow(clippy::manual_range_patterns)]
                fn from_props(props: &[(&str, &str)], block: &Block) -> Self {
                    #[cfg(debug_assertions)]
                    if !Self::handles_block_id(block.id) {
                        panic!("{} 对 {} 不是有效方块", block.name, #struct_name);
                    }
                    let mut block_props = Self::default(block);
                    for (key, value) in props {
                        #from_props_loop_body
                    }
                    block_props
                }
            }
        });
    }
}

/// 反序列化得到的方块可燃性数据。
#[derive(Deserialize)]
pub struct FlammableStruct {
    /// 火焰蔓延到相邻方块的概率 (0–300)。
    pub spread_chance: u8,
    /// 方块自身被烧毁的概率 (0–300)。
    pub burn_chance: u8,
}

impl ToTokens for FlammableStruct {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let spread_chance = &self.spread_chance;
        let burn_chance = &self.burn_chance;

        tokens.extend(quote! {
            Flammable {
                spread_chance: #spread_chance,
                burn_chance: #burn_chance,
            }
        });
    }
}

/// 轴对齐包围盒，用于描述方块的碰撞形状与轮廓形状。
#[derive(Deserialize, Clone, Copy)]
pub struct BoundingBox {
    /// 边界框的最小角点。
    pub min: Vector3<f64>,
    /// 边界框的最大角点。
    pub max: Vector3<f64>,
}

impl ToTokens for BoundingBox {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let min_x = &self.min.x;
        let min_y = &self.min.y;
        let min_z = &self.min.z;

        let max_x = &self.max.x;
        let max_y = &self.max.y;
        let max_z = &self.max.z;

        tokens.extend(quote! {
            BoundingBox {
                min: Vector3::new(#min_x, #min_y, #min_z),
                max: Vector3::new(#max_x, #max_y, #max_z),
            }
        });
    }
}

#[derive(Deserialize, Copy, Clone, PartialEq, Eq)]
pub struct BlockStateId(pub u16);

impl ToTokens for BlockStateId {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let inner = self.0;
        tokens.extend(quote! { BlockStateId::new(#inner).unwrap() });
    }
}

/// `blocks.json` 中存储的单个方块状态的反序列化表示。
#[derive(Deserialize, Clone)]
pub struct BlockState {
    /// 此方块状态的全局唯一数字 ID。
    pub id: BlockStateId,
    /// 编码布尔状态属性（空气、随机刻等）的位域。
    pub state_flags: u16,
    /// 编码方块哪些面为实心的位域。
    pub side_flags: u8,
    /// 此状态对应的音符盒乐器名称。TODO: 将其改为枚举
    pub instrument: String,
    /// 此方块状态发出的光照等级（0–15）。
    pub luminance: u8,
    /// 活塞与此方块状态的交互方式。
    pub piston_behavior: PistonBehavior,
    /// 此方块状态的挖掘硬度。
    pub hardness: f32,
    /// 碰撞形状各分段在全局形状数组中的索引。
    pub collision_shapes: Vec<u16>,
    /// 轮廓（选择）形状各分段在全局形状数组中的索引。
    pub outline_shapes: Vec<u16>,
    /// 非完整方块时用于光照传播的不透明度值。
    pub opacity: Option<u8>,
    /// 关联的方块实体类型 ID（如果有）。
    pub block_entity_type: Option<u16>,
}

/// 描述活塞如何与方块交互。
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PistonBehavior {
    /// 方块可以被正常推动和拉动。
    #[serde(alias = "PUSH_PULL")]
    Normal,
    /// 方块在被推动时会被破坏。
    #[serde(alias = "POPPED")]
    Destroy,
    /// 方块阻止活塞移动。
    Block,
    /// 活塞完全忽略该方块。
    #[serde(alias = "IMMOVEABLE")]
    Ignore,
    /// 方块只能被推动，不能被拉动。
    #[serde(alias = "PUSH")]
    PushOnly,
}

impl PistonBehavior {
    fn to_tokens(&self) -> TokenStream {
        match self {
            Self::Normal => quote! { PistonBehavior::Normal },
            Self::Destroy => quote! { PistonBehavior::Destroy },
            Self::Block => quote! { PistonBehavior::Block },
            Self::Ignore => quote! { PistonBehavior::Ignore },
            Self::PushOnly => quote! { PistonBehavior::PushOnly },
        }
    }
}

#[derive(Deserialize, Copy, Clone)]
#[serde(rename_all = "snake_case")]
enum SpawnFloorPredicate {
    Never,
    Always,
    OcelotOrParrot,
    PolarBear,
    FireImmune,
}

impl ToTokens for SpawnFloorPredicate {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(match self {
            Self::Never => quote! { SpawnFloorPredicate::Never },
            Self::Always => quote! { SpawnFloorPredicate::Always },
            Self::OcelotOrParrot => quote! { SpawnFloorPredicate::OcelotOrParrot },
            Self::PolarBear => quote! { SpawnFloorPredicate::PolarBear },
            Self::FireImmune => quote! { SpawnFloorPredicate::FireImmune },
        });
    }
}

impl BlockState {
    /// 位标志，表示此状态为空气方块。
    const IS_AIR: u16 = 1 << 0;

    const IS_LIQUID: u16 = 1 << 5;

    /// 位标志，表示此状态会接收随机刻事件。
    const HAS_RANDOM_TICKS: u16 = 1 << 9;

    const IS_SOLID_RENDER: u16 = 1 << 10;
    const CAN_OCCLUDE: u16 = 1 << 11;
    const HAS_ANALOG_OUTPUT_SIGNAL: u16 = 1 << 12;

    ///若该方块状态会接收随机刻事件，则返回 `true`。
    const fn has_random_ticks(&self) -> bool {
        self.state_flags & Self::HAS_RANDOM_TICKS != 0
    }

    ///若该方块状态是空气变体，则返回 `true`。
    pub const fn is_air(&self) -> bool {
        self.state_flags & Self::IS_AIR != 0
    }

    pub const fn is_liquid(&self) -> bool {
        self.state_flags & Self::IS_LIQUID != 0
    }

    pub const fn is_solid_render(&self) -> bool {
        self.state_flags & Self::IS_SOLID_RENDER != 0
    }

    pub const fn can_occlude(&self) -> bool {
        self.state_flags & Self::CAN_OCCLUDE != 0
    }

    pub const fn has_analog_output_signal(&self) -> bool {
        self.state_flags & Self::HAS_ANALOG_OUTPUT_SIGNAL != 0
    }

    /// 生成用于代码生成的 `BlockState { … }` 结构体字面量 token 流。
    fn to_tokens(&self) -> TokenStream {
        let mut tokens = TokenStream::new();
        let id = self.id;
        let state_flags = LitInt::new(&self.state_flags.to_string(), Span::call_site());
        let side_flags = LitInt::new(&self.side_flags.to_string(), Span::call_site());
        let instrument = format_ident!("{}", self.instrument.to_upper_camel_case());
        let luminance = LitInt::new(&self.luminance.to_string(), Span::call_site());
        let hardness = self.hardness;
        let opacity = if let Some(opacity) = self.opacity {
            let opacity = LitInt::new(&opacity.to_string(), Span::call_site());
            quote! { #opacity }
        } else {
            quote! { 0 }
        };
        let block_entity_type = if let Some(block_entity_type) = self.block_entity_type {
            let block_entity_type = LitInt::new(&block_entity_type.to_string(), Span::call_site());
            quote! { #block_entity_type }
        } else {
            quote! { u16::MAX }
        };

        let collision_shapes = self
            .collision_shapes
            .iter()
            .map(|shape_id| LitInt::new(&shape_id.to_string(), Span::call_site()));
        let outline_shapes = self
            .outline_shapes
            .iter()
            .map(|shape_id| LitInt::new(&shape_id.to_string(), Span::call_site()));
        let piston_behavior = &self.piston_behavior.to_tokens();

        tokens.extend(quote! {
            BlockState {
                id: #id,
                state_flags: #state_flags,
                side_flags: #side_flags,
                instrument: NoteblockInstrument::#instrument,
                luminance: #luminance,
                piston_behavior: #piston_behavior,
                hardness: #hardness,
                collision_shapes: &[#(#collision_shapes),*],
                outline_shapes: &[#(#outline_shapes),*],
                opacity: #opacity,
                block_entity_type: #block_entity_type,
            }
        });
        tokens
    }
}

#[derive(Deserialize, Copy, Clone)]
pub struct BlockId(pub u16);

impl ToTokens for BlockId {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let inner = self.0;
        tokens.extend(quote! { BlockId::new(#inner).unwrap() });
    }
}

/// `blocks.json` 中存储的 Minecraft 方块的反序列化表示。
#[derive(Deserialize)]
pub struct Block {
    /// 协议中使用的数字方块 ID。
    pub id: BlockId,
    /// 不带 `minecraft:` 命名空间前缀的注册表名称。
    pub name: String,
    /// 方块显示名称的翻译键。
    // pub translation_key: String,
    /// 挖掘硬度；影响方块被破坏所需的时间。
    pub hardness: f32,
    /// 对爆炸的抗爆强度。
    pub blast_resistance: f32,
    pub map_color: u8,
    /// 对应物品的数字 ID（如果存在）。
    pub item_id: u16,
    /// 可燃性数据，仅当方块可被点燃时存在。
    pub flammable: Option<FlammableStruct>,
    /// 施加于在此方块上行走的实体的摩擦力。
    pub slipperiness: f32,
    /// 此方块内实体的水平速度倍率。
    pub velocity_multiplier: f32,
    /// 从此方块跳跃时应用的跳跃速度倍率。
    pub jump_velocity_multiplier: f32,
    /// 引用此方块所定义属性的哈希键。
    pub properties: Vec<i32>,
    /// 默认（规范）方块状态的方块状态 ID。
    pub default_state_id: BlockStateId,
    /// 此方块所有可能的状态，按状态 ID 排序。
    pub states: Vec<BlockState>,
    /// 挖掘该方块时掉落的经验点数（如果有）。
    pub experience: Option<Experience>,
    /// 原版应用的由位置导出的形状偏移（如果有）。
    shape_offset: Option<BlockShapeOffset>,
    /// 此方块的生成地面谓词（若有）。
    spawn_floor_predicate: Option<SpawnFloorPredicate>,
}

impl ToTokens for Block {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let id = self.id;
        let name = LitStr::new(&self.name, Span::call_site());
        //let translation_key = LitStr::new(&self.translation_key, Span::call_site());
        let hardness = &self.hardness;
        let blast_resistance = &self.blast_resistance;
        let map_color = &self.map_color;

        let item_id = LitInt::new(&self.item_id.to_string(), Span::call_site());
        let slipperiness = &self.slipperiness;
        let velocity_multiplier = &self.velocity_multiplier;
        let jump_velocity_multiplier = &self.jump_velocity_multiplier;
        let experience = if let Some(exp) = &self.experience {
            let exp_tokens = exp.to_token_stream();
            quote! { Some(#exp_tokens) }
        } else {
            quote! { None }
        };
        // 生成状态 token
        let states = self.states.iter().map(BlockState::to_tokens);

        let default_state_ref: &BlockState = self
            .states
            .iter()
            .find(|state| state.id == self.default_state_id)
            .unwrap();
        let mut default_state = default_state_ref.clone();
        default_state.id = default_state_ref.id;
        let default_state = default_state.to_tokens();
        let flammable = if let Some(flammable) = &self.flammable {
            let flammable_tokens = flammable.to_token_stream();
            quote! { Some(#flammable_tokens) }
        } else {
            quote! { None }
        };
        tokens.extend(quote! {
            Block {
                id: #id,
                name: #name,
                hardness: #hardness,
                blast_resistance: #blast_resistance,
                map_color: #map_color,
                slipperiness: #slipperiness,
                velocity_multiplier: #velocity_multiplier,
                jump_velocity_multiplier: #jump_velocity_multiplier,
                item_id: #item_id,
                default_state: &#default_state,
                states: &[#(#states),*],
                flammable: #flammable,
                experience: #experience,
            }
        });
    }
}

/// 生成的方块属性在 `properties.json` 中所分类的底层数据类型。
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum GeneratedPropertyType {
    /// 一个简单的 `true`/`false` 布尔属性。
    #[serde(rename = "boolean")]
    Boolean,
    /// 具有闭区间范围的整数属性。
    #[serde(rename = "int")]
    Int {
        /// 最小值（含）。
        min: u8,
        /// 最大值（含）。
        max: u8,
    },
    /// 一个具名变体枚举属性。
    #[serde(rename = "enum")]
    Enum {
        /// 变体字符串的有序列表。
        values: Vec<String>,
    },
}

/// 从 `properties.json` 反序列化得到的单个方块属性条目。
#[derive(Deserialize, Clone)]
pub struct GeneratedProperty {
    /// 一个稳定的整数哈希，用于跨方块标识此属性。
    hash_key: i32,
    /// 生成的 Rust 枚举类型所使用的名称。
    enum_name: String,
    /// 方块状态 JSON 中呈现的属性名称。
    serialized_name: String,
    /// 此属性的类型及可能的取值。
    #[serde(rename = "type")]
    #[serde(flatten)]
    property_type: GeneratedPropertyType,
}

impl GeneratedProperty {
    /// 将此反序列化后的属性转换为代码生成期间使用的中间 `Property`。
    fn to_property(&self) -> Property {
        let enum_name = match &self.property_type {
            GeneratedPropertyType::Boolean => "boolean".to_string(),
            GeneratedPropertyType::Int { min, max } => format!("integer_{min}_to_{max}"),
            GeneratedPropertyType::Enum { .. } => self.enum_name.clone(),
        };

        let values = match &self.property_type {
            GeneratedPropertyType::Boolean => {
                vec!["true".to_string(), "false".to_string()]
            }
            GeneratedPropertyType::Int { min, max } => {
                let mut values = Vec::new();
                for i in *min..=*max {
                    values.push(format!("L{i}"));
                }
                values
            }
            GeneratedPropertyType::Enum { values } => values.clone(),
        };

        Property {
            enum_name,
            serialized_name: self.serialized_name.clone(),
            values,
        }
    }
}

/// 构建属性组映射时使用的方块属性中间表示。
#[derive(Clone)]
struct Property {
    /// 此属性对应的 Rust 枚举类型名。
    enum_name: String,
    /// 此属性的 JSON 键名。
    serialized_name: String,
    /// 此属性可取的所有可能字符串值。
    values: Vec<String>,
}

/// 从 `blocks.json` 加载的所有方块资源的顶层容器。
#[derive(Deserialize)]
pub struct BlockAssets {
    /// 所有方块及其状态和属性。
    pub blocks: Vec<Block>,
    /// 方块状态引用的所有唯一包围盒。
    pub shapes: Vec<BoundingBox>,
    /// 所有方块实体类型的注册表名称。
    pub block_entity_types: Vec<String>,
}

#[derive(Deserialize)]
struct BlockShapeOffset {
    #[serde(rename = "type")]
    offset_type: BlockShapeOffsetType,
    max_horizontal: f32,
    max_vertical: f32,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum BlockShapeOffsetType {
    Xz,
    Xyz,
}

/// 读取所有方块资源，并生成完整的方块注册表 `TokenStream`。
pub fn build() -> TokenStream {
    let blocks_assets: BlockAssets =
        serde_json::from_str(&fs::read_to_string("../../assets/blocks.json").unwrap())
            .expect("解析 blocks.json 失败");

    let shape_offset_arms = blocks_assets
        .blocks
        .iter()
        .filter_map(|block| {
            let offset = block.shape_offset.as_ref()?;
            let block = format_ident!("{}", const_block_name_from_block_name(&block.name));
            let offset_type = match offset.offset_type {
                BlockShapeOffsetType::Xz => quote! { ShapeOffsetType::Xz },
                BlockShapeOffsetType::Xyz => quote! { ShapeOffsetType::Xyz },
            };
            let max_horizontal = offset.max_horizontal;
            let max_vertical = offset.max_vertical;

            Some(quote! {
                BlockId::#block => Some(ShapeOffset {
                    offset_type: #offset_type,
                    max_horizontal: #max_horizontal,
                    max_vertical: #max_vertical,
                }),
            })
        })
        .collect::<Vec<_>>();

    let generated_properties: Vec<GeneratedProperty> =
        serde_json::from_str(&fs::read_to_string("../../assets/properties.json").unwrap())
            .expect("解析 properties.json 失败");

    let generated_prop_map: BTreeMap<i32, &GeneratedProperty> = generated_properties
        .iter()
        .map(|p| (p.hash_key, p))
        .collect();

    let mut random_tick_states = Vec::new();
    let mut air_states = Vec::new();
    let mut liquid_states = Vec::new();

    let mut constants_list = Vec::new();
    let mut block_id_constants = Vec::new();
    let mut block_from_name_entries = Vec::new();
    let mut block_from_item_id_arms = Vec::new();
    let mut spawn_floor_predicate_arms = Vec::new();

    let mut raw_id_from_state_id_array = Vec::new();
    let mut type_from_raw_id_array = Vec::new();
    let mut state_from_state_id_array = Vec::<(Ident, usize, u16)>::new();

    let mut property_enums: BTreeMap<String, PropertyStruct> = BTreeMap::new();
    let mut block_properties: Vec<BlockPropertyStruct> = Vec::new();
    let mut property_collection_map: BTreeMap<Vec<i32>, PropertyCollectionData> = BTreeMap::new();
    let mut existing_item_ids: HashSet<u16> = HashSet::new();

    for block in blocks_assets.blocks {
        let mut property_collection = HashSet::new();
        let mut property_mapping = Vec::new();

        for property_hash in &block.properties {
            let generated_property = generated_prop_map
                .get(property_hash)
                .expect("generated_properties 中找不到属性哈希");

            property_collection.insert(generated_property.hash_key);

            let property = generated_property.to_property();
            let renamed_property = property.enum_name.to_upper_camel_case();

            let property_type = match &generated_property.property_type {
                GeneratedPropertyType::Boolean => PropertyType::Bool,
                GeneratedPropertyType::Int { min, max } => PropertyType::Int {
                    min: *min,
                    max: *max,
                },
                GeneratedPropertyType::Enum { .. } => PropertyType::Enum {
                    name: renamed_property.clone(),
                },
            };

            if let PropertyType::Enum { name } = &property_type {
                property_enums
                    .entry(name.clone())
                    .or_insert_with(|| PropertyStruct {
                        name: name.clone(),
                        values: property.values.clone(),
                    });
            }

            property_mapping.push(PropertyVariantMapping {
                original_name: property.serialized_name,
                property_type,
            });
        }

        let mut multiplier = 1;
        let mut property_descriptors = Vec::new();
        for hash in block.properties.iter().rev() {
            let gen_prop = generated_prop_map.get(hash).unwrap();
            let variant_count = match &gen_prop.property_type {
                GeneratedPropertyType::Boolean => 2,
                GeneratedPropertyType::Int { min, max } => (max - min + 1) as u16,
                GeneratedPropertyType::Enum { values } => values.len() as u16,
            };
            property_descriptors.push(quote! {
                PropertyDescriptor {
                    hash_key: #hash,
                    multiplier: #multiplier,
                    variant_count: #variant_count,
                }
            });
            multiplier *= variant_count;
        }

        let const_ident = format_ident!("{}", const_block_name_from_block_name(&block.name));
        let name_str = &block.name;
        let item_id = block.item_id;
        let block_id = block.id;

        if let Some(predicate) = block.spawn_floor_predicate {
            spawn_floor_predicate_arms.push(quote! {
                BlockId::#const_ident => #predicate,
            });
        }

        // let mut block_with_descriptors = block.clone();
        // block_with_descriptors.property_descriptors = property_descriptors;

        constants_list.push(quote! {
            pub const #const_ident: Self = #block;
        });

        block_id_constants.push(quote! {
            pub const #const_ident: Self = #block_id;
        });

        type_from_raw_id_array.push((block.id.0, quote! { &Block::#const_ident }));

        block_from_name_entries.push(quote! {
            #name_str => Block::#const_ident,
        });

        for (i, state) in block.states.iter().enumerate() {
            if state.has_random_ticks() {
                let state_id = LitInt::new(&state.id.0.to_string(), Span::call_site());
                random_tick_states.push(state_id);
            }
            if state.is_air() {
                let state_id = LitInt::new(&state.id.0.to_string(), Span::call_site());
                air_states.push(state_id);
            }
            if state.is_liquid() {
                let state_id = LitInt::new(&state.id.0.to_string(), Span::call_site());
                liquid_states.push(state_id);
            }

            raw_id_from_state_id_array.push((state.id.0, quote! { BlockId::#const_ident }));
            state_from_state_id_array.push((const_ident.clone(), i, state.id.0));
        }

        if !property_collection.is_empty() {
            let mut property_collection_vec: Vec<i32> = property_collection.into_iter().collect();
            property_collection_vec.sort_unstable();

            property_collection_map
                .entry(property_collection_vec)
                .or_insert_with(|| PropertyCollectionData::from_mappings(property_mapping))
                .add_block(block.name.clone(), block.id.0);
        }

        if existing_item_ids.insert(item_id) {
            block_from_item_id_arms.push(quote! {
                #item_id => Some(&Self::#const_ident),
            });
        }
    }

    let mut block_properties_from_state_and_block_id_arms = Vec::new();
    let mut block_properties_from_props_and_name_arms = Vec::new();

    let mut emitted_aliases = HashSet::new();
    for property_group in property_collection_map.values() {
        emitted_aliases.insert(property_group_name_from_derived_name(
            &property_group.derive_name(),
        ));
    }

    for property_group in property_collection_map.into_values() {
        let struct_name = property_group_name_from_derived_name(&property_group.derive_name());
        let property_name = Ident::new(&struct_name, Span::call_site());

        let mut group_aliases = Vec::new();

        if let Some(canonical) = common_suffix_group_alias(&property_group.blocks) {
            if emitted_aliases.insert(canonical.clone()) {
                group_aliases.push(Ident::new(&canonical, Span::call_site()));
            }
        }

        for (b_name, _) in &property_group.blocks {
            let alias_name = format!("{}_properties", b_name).to_upper_camel_case();
            if emitted_aliases.insert(alias_name.clone()) {
                group_aliases.push(Ident::new(&alias_name, Span::call_site()));
            }
        }

        let idents: Box<_> = property_group
            .blocks
            .iter()
            .map(|(name, _)| Ident::new(&const_block_name_from_block_name(name), Span::call_site()))
            .collect();

        block_properties_from_state_and_block_id_arms.push(quote! {
            #(BlockId::#idents)|* => Box::new(#property_name::from_state_id(state_id)),
        });
        block_properties_from_props_and_name_arms.push(quote! {
            #(BlockId::#idents)|* => Box::new(#property_name::from_props(props, self)),
        });

        block_properties.push(BlockPropertyStruct {
            data: property_group,
            aliases: group_aliases,
        });
    }

    let shapes = blocks_assets.shapes.iter().map(ToTokens::to_token_stream);

    let air_state_ids = quote! { #(#air_states)|* };
    let liquid_state_ids = quote! { #(#liquid_states)|* };

    let block_props = block_properties.iter().map(ToTokens::to_token_stream);
    let properties = property_enums.values().map(ToTokens::to_token_stream);

    let block_entity_types = blocks_assets
        .block_entity_types
        .iter()
        .map(|entity_type| LitStr::new(entity_type, Span::call_site()));

    let raw_id_from_state_id_ordered = fill_array(raw_id_from_state_id_array);
    let max_state_id = raw_id_from_state_id_ordered.len();
    let raw_id_from_state_id = quote! { #(#raw_id_from_state_id_ordered),* };

    let type_from_raw_id_vec = fill_array(type_from_raw_id_array);
    let max_type_id = type_from_raw_id_vec.len();
    let type_from_raw_id_items = quote! { #(#type_from_raw_id_vec),* };

    let state_from_state_id_vec = fill_state_array(state_from_state_id_array);
    let max_state_id_2 = state_from_state_id_vec.len();
    let state_from_state_id = quote! { #(#state_from_state_id_vec),* };

    assert_eq!(max_state_id, max_state_id_2);

    let Bitset {
        items,
        mod_ident,
        contains_ident,
    } = &gen_u16_bitset(
        "RANDOM_TICKS",
        &random_tick_states
            .iter()
            .map(|it| it.base10_parse().unwrap())
            .collect::<Vec<u16>>(),
    );

    quote! {
        #[allow(clippy::wildcard_imports, clippy::enum_glob_use, clippy::too_many_lines, clippy::match_same_arms)]
        use papokin_util::math::boundingbox::BoundingBox;

        use crate::{
            BlockState, BlockStateId, Block, BlockId,
            blocks::{Flammable, ShapeOffset, ShapeOffsetType, SpawnFloorPredicate},
        };
        use crate::block_state::PistonBehavior;
        use papokin_util::math::int_provider::{UniformIntProvider, IntProvider, NormalIntProvider};
        use papokin_util::math::experience::Experience;
        use papokin_util::math::vector3::Vector3;
        use std::collections::BTreeMap;

        #items

        #[derive(Clone, Copy, Debug)]
        pub struct BlockProperty {
            pub name: &'static str,
            pub values: &'static [&'static str],
        }

        pub trait BlockProperties where Self: 'static {
            fn to_index(&self) -> u16;
            fn from_index(index: u16) -> Self where Self: Sized;
            fn handles_block_id(id: BlockId) -> bool where Self: Sized;
            fn to_state_id(&self, block: &Block) -> BlockStateId;
            fn from_state_id(id: BlockStateId, block: &Block) -> Self where Self: Sized;
            fn default(block: &Block) -> Self where Self: Sized;
            fn to_props(&self) -> Vec<(&'static str, &'static str)>;
            fn from_props(props: &[(&str, &str)], block: &Block) -> Self where Self: Sized;
        }

        pub trait EnumVariants {
            fn variant_count() -> u16;
            fn to_index(&self) -> u16;
            fn from_index(index: u16) -> Self;
            fn to_value(&self) -> &'static str;
            fn from_value(value: &str) -> Self;
        }

        pub const COLLISION_SHAPES: &[BoundingBox] = &[
            #(#shapes),*
        ];

        pub const BLOCK_ENTITY_TYPES: &[&str] = &[
            #(#block_entity_types),*
        ];

        #[inline(always)]
        #[must_use]
        pub const fn is_air(id: BlockStateId) -> bool {
            matches!(id.as_u16(), #air_state_ids)
        }

         #[inline(always)]
         #[must_use]
        pub const fn is_liquid(id: BlockStateId) -> bool {
            matches!(id.as_u16(), #liquid_state_ids)
        }

        #[inline(always)]
        #[must_use]
        pub fn has_random_ticks(id: BlockStateId) -> bool {
            #mod_ident::#contains_ident(id.as_u16())
        }

        #[must_use]
        pub const fn blocks_movement(block_state: &BlockState, id: BlockId) -> bool {
            block_state.is_solid() && !matches!(id, BlockId::COBWEB | BlockId::BAMBOO_SAPLING)
        }

        impl BlockState {
            /// 从 [`BlockStateId`] 获取 [`BlockState`]。
            /// 如果需要访问方块，请改用 `BlockState::from_id_with_block`。
            #[inline]
            #[must_use]
            pub const fn from_id(id: BlockStateId) -> &'static Self {
                // Safety: 创建 BlockStateId 时我们总是校验此条件。
                // u16 字段是私有的且不可变。BlockStateId::STATE_COUNT 是 const u16。
                // 如果条件曾成立一次，就将永远成立。
                unsafe { std::hint::assert_unchecked(id.as_u16() < BlockStateId::STATE_COUNT) }
                // 这个提示可确保边界检查在发布构建中被优化掉。
                // 由于 debug_assertions 强制启用 -Zub_checks=yes（rust-lang/rust#123499）
                // 越界检查导致的 panic 在 profile.dev 中被替换为 ub 检查
                // https://doc.rust-lang.org/nightly/unstable-book/compiler-flags/ub-checks.html

                mappings::STATE_FROM_STATE_ID[id.as_u16() as usize]
            }

            #[doc = r" Get a block state from a state id and the corresponding block."]
            #[inline]
            #[must_use]
            pub const fn from_id_with_block(id: BlockStateId) -> (&'static Block, &'static Self) {
                let block = Block::from_state_id(id);
                let state = Self::from_id(id);
                (block, state)
            }
        }

        impl BlockStateId {
            pub const AIR: Self = Block::AIR.default_state.id;

            pub(crate) const STATE_COUNT: u16 = mappings::STATE_FROM_STATE_ID.len() as u16;
        }

        mod mappings {
            use crate::{Block, BlockId, BlockState, BlockStateId};
            use phf;

            pub(super) static BLOCK_FROM_NAME_MAP: phf::Map<&'static str, Block> = phf::phf_map!{
                #(#block_from_name_entries)*
            };

            pub(super) static BLOCK_ID_FROM_STATE_ID: [BlockId; #max_state_id] = [
                #raw_id_from_state_id
            ];

            pub(super) static TYPE_FROM_RAW_ID: [&Block; #max_type_id] = [
                #type_from_raw_id_items
            ];

            pub(super) static STATE_FROM_STATE_ID: [&BlockState; #max_state_id] = [
                #state_from_state_id
            ];
        }


        impl Block {
            #(#constants_list)*

            #[must_use]
            pub const fn spawn_floor_predicate(&self) -> SpawnFloorPredicate {
                match self.id {
                    #(#spawn_floor_predicate_arms)*
                    _ => SpawnFloorPredicate::Default,
                }
            }

            pub(crate) const fn shape_offset(&self) -> Option<ShapeOffset> {
                match self.id {
                    #(#shape_offset_arms)*
                    _ => None,
                }
            }

            #[doc = r" Try to parse a block from a resource location string."]
            #[inline]
            #[must_use]
            pub fn from_registry_key(name: &str) -> Option<&'static Self> {
                mappings::BLOCK_FROM_NAME_MAP.get(name)
            }

            #[doc = r" Try to get a block from a namespace prefixed name."]
            #[must_use]
            pub fn from_name(name: &str) -> Option<&'static Self> {
                let key = name.strip_prefix("minecraft:").unwrap_or(name);
                mappings::BLOCK_FROM_NAME_MAP.get(key)
            }

            /// 从 [`BlockId`] 获取 [`Block`]
            #[inline]
            #[must_use]
            pub const fn from_id(id: BlockId) -> &'static Self {
                // Safety: 创建 BlockId 时我们总是校验此条件。
                // u16 字段是私有的且不可变。BlockId::BLOCK_COUNT 是 const u16。
                // 如果条件曾成立一次，就将永远成立。
                unsafe { std::hint::assert_unchecked(id.as_u16() < BlockId::BLOCK_COUNT) }
                // 这个提示可确保边界检查在发布构建中被优化掉。
                // 由于 debug_assertions 强制启用 -Zub_checks=yes（rust-lang/rust#123499）
                // 越界检查导致的 panic 在 profile.dev 中被替换为 ub 检查
                // https://doc.rust-lang.org/nightly/unstable-book/compiler-flags/ub-checks.html

                mappings::TYPE_FROM_RAW_ID[id.as_u16() as usize]
            }

            /// 从状态 ID 获取 [`Block`]
            #[inline]
            #[must_use]
            pub const fn from_state_id(id: BlockStateId) -> &'static Self {
                Self::from_id(BlockId::from_state_id(id))
            }

            #[doc = r" Try to parse a block from an item id."]
            #[must_use]
            pub const fn from_item_id(id: u16) -> Option<&'static Self> {
                #[allow(unreachable_patterns)]
                match id {
                    #(#block_from_item_id_arms)*
                    _ => None
                }
            }

            #[track_caller]
            #[doc = r" Get the properties of the block."]
            pub fn properties(&self, state_id: BlockStateId) -> Option<Box<dyn BlockProperties>> {
                Some(match self.id {
                    #(#block_properties_from_state_and_block_id_arms)*
                    _ => return None,
                })
            }

            #[track_caller]
            #[doc = r" Get the properties of the block."]
            pub fn from_properties(&self, props: &[(&str, &str)]) -> Box<dyn BlockProperties> {
                match self.id {
                    #(#block_properties_from_props_and_name_arms)*
                    _ => panic!("无效属性")
                }
            }
        }

        impl BlockId {
            #(#block_id_constants)*

            pub(crate) const BLOCK_COUNT: u16 = mappings::TYPE_FROM_RAW_ID.len() as u16;

            /// 从 [`BlockStateId`] 获取 [`BlockId`]
            #[inline]
            #[must_use]
            pub const fn from_state_id(id: BlockStateId) -> BlockId {
                // Safety: 创建 BlockStateId 时我们总是校验此条件。
                // u16 字段是私有的且不可变。BlockStateId::STATE_COUNT 是 const u16。
                // 如果条件曾成立一次，就将永远成立。
                unsafe { std::hint::assert_unchecked(id.as_u16() < BlockStateId::STATE_COUNT) }
                // 这个提示可确保边界检查在发布构建中被优化掉。
                // 由于 debug_assertions 强制启用 -Zub_checks=yes（rust-lang/rust#123499）
                // 越界检查导致的 panic 在 profile.dev 中被替换为 ub 检查
                // https://doc.rust-lang.org/nightly/unstable-book/compiler-flags/ub-checks.html

                mappings::BLOCK_ID_FROM_STATE_ID[id.as_u16() as usize]
            }
        }

        #(#properties)*

        #(#block_props)*

        impl Facing {
            #[must_use]
            pub const fn opposite(&self) -> Self {
                match self {
                    Self::North => Self::South,
                    Self::South => Self::North,
                    Self::East => Self::West,
                    Self::West => Self::East,
                    Self::Up => Self::Down,
                    Self::Down => Self::Up,
                }
            }
        }

        impl HorizontalFacing {
            #[must_use]
            pub fn all() -> [Self; 4] {
                [
                    Self::North,
                    Self::South,
                    Self::West,
                    Self::East,
                ]
            }

            #[must_use]
            pub fn to_offset(&self) -> Vector3<i32> {
                match self {
                    Self::North => (0, 0, -1),
                    Self::South => (0, 0, 1),
                    Self::West => (-1, 0, 0),
                    Self::East => (1, 0, 0),
                }
                .into()
            }
            #[must_use]
            pub const fn to_axis(&self) -> HorizontalAxis {
                match self {
                    Self::North | Self::South => HorizontalAxis::Z,
                    Self::West | Self::East => HorizontalAxis::X,
                }
            }
             #[must_use]
            pub const fn to_facing(&self) -> HorizontalFacing {
                match self {
                    Self::North => HorizontalFacing::North,
                    Self::South => HorizontalFacing::South,
                    Self::West => HorizontalFacing::West,
                    Self::East => HorizontalFacing::East,
                }
            }
            #[must_use]
            pub const fn opposite(&self) -> Self {
                match self {
                    Self::North => Self::South,
                    Self::South => Self::North,
                    Self::West => Self::East,
                    Self::East => Self::West,
                }
            }

            #[must_use]
            pub const fn rotate_clockwise(&self) -> Self {
                match self {
                    Self::North => Self::East,
                    Self::South => Self::West,
                    Self::West => Self::North,
                    Self::East => Self::South,
                }
            }

            #[must_use]
            pub const fn rotate_counter_clockwise(&self) -> Self {
                match self {
                    Self::North => Self::West,
                    Self::South => Self::East,
                    Self::West => Self::South,
                    Self::East => Self::North,
                }
            }
        }

        impl RailShape {
            #[must_use]
            pub const fn is_ascending(&self) -> bool {
                matches!(self, Self::AscendingEast | Self::AscendingWest | Self::AscendingNorth | Self::AscendingSouth)
            }
        }

        impl RailShapeStraight {
            #[must_use]
            pub const fn is_ascending(&self) -> bool {
                matches!(self, Self::AscendingEast | Self::AscendingWest | Self::AscendingNorth | Self::AscendingSouth)
            }
        }
    }
}
