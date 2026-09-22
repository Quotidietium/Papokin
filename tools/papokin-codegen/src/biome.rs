use heck::ToPascalCase;
use std::{collections::BTreeMap, fs};

use heck::ToShoutySnakeCase;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use serde::Deserialize;
use serde_json::Value;
use syn::LitInt;

fn deserialize_carvers<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        String(String),
        Vec(Vec<String>),
    }

    match Option::<StringOrVec>::deserialize(deserializer)? {
        Some(StringOrVec::String(s)) => Ok(vec![s]),
        Some(StringOrVec::Vec(v)) => Ok(v),
        None => Ok(Vec::new()),
    }
}

/// `biome.json` 中单个生物群系条目的原始反序列化结构。
#[derive(Deserialize)]
pub struct Biome {
    /// 该生物群系是否有降水（雨或雪）。
    has_precipitation: bool,
    /// 生物群系的基础温度，影响天气与植被颜色。
    temperature: f32,
    /// 降水值，影响生物群系的湿润程度（影响下雪/下雨）。
    downfall: f32,
    /// 可选的修饰器，用于改变温度的施加方式。
    temperature_modifier: Option<TemperatureModifier>,
    #[serde(default, deserialize_with = "deserialize_carvers")]
    pub carvers: Vec<String>,
    /// 世界生成期间应用的地物资源位置字符串的嵌套列表。
    features: Vec<Vec<String>>,
    /// 每个区块刻中生物生成的概率（若未按生物群系覆盖）。
    creature_spawn_probability: Option<f32>,
    /// 此生物群系中每个实体类别的生成组数据。
    #[serde(default)]
    spawners: SpawnGroups,
    /// 每个实体的生成消耗预算条目，以带命名空间的实体 ID 为键。
    #[serde(default)]
    spawn_costs: BTreeMap<String, SpawnCosts>,
    /// 环境属性；26.3 将自然生物生成存储在此处。
    #[serde(default)]
    attributes: BTreeMap<String, Value>,
    /// 分配给此生物群系的数字注册表 ID。
    #[serde(default)]
    pub id: u8,
}

impl Biome {
    fn apply_natural_mob_spawns(&mut self) {
        let Some(attr) = self.attributes.get("minecraft:gameplay/natural_mob_spawns") else {
            return;
        };
        let Some(argument) = attr.get("argument") else {
            return;
        };
        if let Some(spawns) = argument.get("spawns_by_category")
            && let Ok(groups) = serde_json::from_value::<SpawnGroups>(spawns.clone())
        {
            self.spawners = groups;
        }
        if let Some(costs) = argument.get("spawn_costs")
            && let Ok(spawn_costs) = serde_json::from_value(costs.clone())
        {
            self.spawn_costs = spawn_costs;
        }
    }
}

/// 生物群系内所有实体类别的生成组数据。
#[derive(Deserialize, Default, PartialEq, Eq, Hash)]
struct SpawnGroups {
    /// 此生物群系的敌对生物生成器。
    #[serde(default)]
    monster: Vec<Spawner>,
    /// 此生物群系的环境生物生成器（例如蝙蝠）。
    #[serde(default)]
    ambient: Vec<Spawner>,
    /// 该生物群系的美西螈生成配置。
    #[serde(default)]
    axolotls: Vec<Spawner>,
    /// 此生物群系的被动生物生成配置（例如牛、羊）。
    #[serde(default)]
    creature: Vec<Spawner>,
    /// 此生物群系的其他杂项实体生成器。
    #[serde(default)]
    misc: Vec<Spawner>,
    /// 该生物群系的地下水生物生成器（例如发光鱿鱼）。
    #[serde(default)]
    underground_water_creature: Vec<Spawner>,
    /// 此生物群系的水生环境生物生成列表（例如鱼类）。
    #[serde(default)]
    water_ambient: Vec<Spawner>,
    /// 此生物群系的水生生物生成列表（例如海豚）。
    #[serde(default)]
    water_creature: Vec<Spawner>,
}

/// 生成组内的单个实体生成器条目，定义于 `biome.json`。
#[derive(Hash, PartialEq, Eq)]
struct Spawner {
    /// 带命名空间的实体类型 ID（例如 `"minecraft:zombie"`）。
    r#type: String,
    /// 生成组中的最小实体数量。
    min_count: i32,
    /// 生成组中的最大实体数量。
    max_count: i32,
}

impl<'de> Deserialize<'de> for Spawner {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            r#type: String,
            #[serde(default, alias = "minCount")]
            min_count: Option<i32>,
            #[serde(default, alias = "maxCount")]
            max_count: Option<i32>,
            #[serde(default)]
            count: Option<Value>,
        }

        let raw = Raw::deserialize(deserializer)?;
        let (min_count, max_count) = match raw.count {
            Some(Value::Number(n)) => {
                let value = n.as_i64().unwrap_or(1) as i32;
                (value, value)
            }
            Some(Value::Object(object)) => {
                let min = object
                    .get("min_inclusive")
                    .and_then(Value::as_i64)
                    .unwrap_or(1) as i32;
                let max = object
                    .get("max_inclusive")
                    .and_then(Value::as_i64)
                    .unwrap_or(min as i64) as i32;
                (min, max)
            }
            _ => (raw.min_count.unwrap_or(1), raw.max_count.unwrap_or(1)),
        };
        Ok(Self {
            r#type: raw.r#type,
            min_count,
            max_count,
        })
    }
}

impl Spawner {
    /// 将此刷怪笼条目转换为 `TokenStream`，供生成的代码使用。
    pub fn to_tokens(&self) -> TokenStream {
        let r#type = &self.r#type;
        let min_count = &self.min_count;
        let max_count = &self.max_count;
        quote! {
            Spawner {
                r#type: #r#type,
                min_count: #min_count,
                max_count: #max_count,
            }
        }
    }
}

/// 控制此实体如何影响生物群系生成预算的生物生成成本数据。
#[derive(Deserialize, PartialEq)]
struct SpawnCosts {
    /// 此实体类型可从生成预算中消耗的最大能量。
    energy_budget: f64,
    /// 此实体每次生成时从预算中扣除的能量成本。
    charge: f64,
}

impl SpawnCosts {
    /// 将这些生成成本转换为 `TokenStream`，供生成的代码使用。
    pub fn to_tokens(&self) -> TokenStream {
        let energy_budget = &self.energy_budget;
        let charge = &self.charge;
        quote! {
            SpawnCosts {
                energy_budget: #energy_budget,
                charge: #charge,
            }
        }
    }
}

/// 可选的修饰器，用于调整生物群系温度的表现（例如冰冻生物群系）。
#[derive(Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
enum TemperatureModifier {
    /// 不修改；温度按原样使用。
    None,
    /// 无论基础值如何，温度被强制降至冰点以下。
    Frozen,
}

/// 一个闭区间范围，用作多噪声生物群系源树中的一个维度。
#[derive(Deserialize)]
struct ParameterRange {
    /// 噪声范围的下界（含）。
    min: i64,
    /// 噪声范围的上界（含）。
    max: i64,
}

impl ParameterRange {
    /// 将此参数范围转换为 `TokenStream`，供生成的代码使用。
    fn into_token_stream(self) -> TokenStream {
        let min = self.min;
        let max = self.max;

        quote! {
            ParameterRange {
                min: #min,
                max: #max
            }
        }
    }
}

/// 多重噪声生物群系源 k-d 树中的一个节点，要么是叶子节点（已解析的生物群系）
/// 或分支节点（带子节点的空间划分）。
#[derive(Deserialize)]
#[serde(tag = "_type", rename_all = "lowercase")]
enum BiomeTree {
    /// 解析为特定生物群系的终端节点。
    Leaf {
        /// 定义此叶节点在参数空间中位置的七个噪声参数范围。
        parameters: [ParameterRange; 7],
        /// 带命名空间的生物群系资源位置（例如 `"minecraft:plains"`）。
        biome: String,
    },
    /// 划分参数空间并将查询委派给子节点的内部节点。
    Branch {
        /// 界定此分支区域的七个噪声参数范围。
        parameters: [ParameterRange; 7],
        /// 此分支的子节点（叶子节点或更深的分支）。
        #[serde(rename = "subTree")]
        nodes: Box<[Self]>,
    },
}

impl BiomeTree {
    /// 将此生物群系树木节点转换为 `TokenStream`，供生成的代码使用。
    fn into_token_stream(self) -> TokenStream {
        match self {
            Self::Leaf { parameters, biome } => {
                let biome = format_ident!(
                    "{}",
                    biome
                        .strip_prefix("minecraft:")
                        .unwrap()
                        .to_shouty_snake_case()
                );
                let parameters = parameters.map(ParameterRange::into_token_stream);
                quote! {
                    BiomeTree::Leaf {
                        parameters: [#(#parameters),*],
                        biome: &Biome::#biome
                    }
                }
            }
            Self::Branch { parameters, nodes } => {
                let nodes = nodes
                    .into_iter()
                    .map(Self::into_token_stream)
                    .collect::<Vec<_>>();
                let parameters = parameters.map(ParameterRange::into_token_stream);
                quote! {
                    BiomeTree::Branch {
                        parameters: [#(#parameters),*],
                        nodes: &[#(#nodes),*]
                    }
                }
            }
        }
    }
}

/// 包含主世界与下界多噪声生物群系来源树的根容器。
#[derive(Deserialize)]
struct MultiNoiseBiomeSuppliers {
    /// 用于解析主世界维度生物群系的多噪声 k-d 树。
    overworld: BiomeTree,
    /// 用于解析下界维度生物群系的多噪声 k-d 树。
    nether: BiomeTree,
}

/// 生成 `Biome` 结构体、其常量、查找方法的 `TokenStream`，
/// 多噪声生物群系源树，以及 `BiomeTree` 搜索实现。
pub fn build() -> TokenStream {
    let dir = std::path::Path::new("../../assets/datapacks/26_3/data/minecraft/worldgen/biome");
    let mut biomes: BTreeMap<String, Biome> = BTreeMap::new();
    let mut entries: Vec<_> = fs::read_dir(dir)
        .expect("缺少 worldgen/biome 目录")
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .collect();
    entries.sort_by_key(|e| e.path());

    for (i, entry) in entries.iter().enumerate() {
        let stem = entry
            .path()
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let content = fs::read_to_string(entry.path()).expect("读取生物群系文件失败");
        let mut biome: Biome = serde_json::from_str(&content).expect("解析生物群系 JSON 失败");
        biome.apply_natural_mob_spawns();
        biome.id = i as u8;
        biomes.insert(stem, biome);
    }

    let biome_trees: MultiNoiseBiomeSuppliers = serde_json::from_str(
        &fs::read_to_string("../../assets/multi_noise_biome_tree.json").unwrap(),
    )
    .expect("解析 multi_noise_biome_tree.json 失败");

    let mut variants = TokenStream::new();
    let mut name_to_type = TokenStream::new();
    let mut id_to_type = TokenStream::new();
    let mut all_variants = TokenStream::new();

    for (name, biome) in biomes {
        // let full_name = format!("minecraft:{name}");
        let format_name = format_ident!("{}", name.to_shouty_snake_case());
        let has_precipitation = biome.has_precipitation;
        let temperature = biome.temperature;
        let downfall = biome.downfall;
        let carvers: Vec<TokenStream> = biome
            .carvers
            .iter()
            .map(|c| {
                let name = c.strip_prefix("minecraft:").unwrap_or(c);
                let variant_name = format_ident!("{}", name.to_uppercase());
                quote! { &crate::carver::#variant_name }
            })
            .collect();
        let features: Vec<TokenStream> = biome
            .features
            .iter()
            .map(|step| {
                let step_features: Vec<TokenStream> = step
                    .iter()
                    .map(|f| {
                        let name = f.strip_prefix("minecraft:").unwrap_or(f);
                        let variant_name = format_ident!("{}", name.to_pascal_case());
                        quote! { crate::placed_feature::PlacedFeature::#variant_name }
                    })
                    .collect();
                quote! { &[#(#step_features),*] }
            })
            .collect();

        let creature_spawn_probability = &biome.creature_spawn_probability.unwrap_or(0.1);

        let temperature_modifier = biome
            .temperature_modifier
            .unwrap_or(TemperatureModifier::None);

        let monster: Vec<_> = biome
            .spawners
            .monster
            .iter()
            .map(Spawner::to_tokens)
            .collect();
        let ambient: Vec<_> = biome
            .spawners
            .ambient
            .iter()
            .map(Spawner::to_tokens)
            .collect();
        let axolotls: Vec<_> = biome
            .spawners
            .axolotls
            .iter()
            .map(Spawner::to_tokens)
            .collect();
        let creature: Vec<_> = biome
            .spawners
            .creature
            .iter()
            .map(Spawner::to_tokens)
            .collect();
        let misc: Vec<_> = biome.spawners.misc.iter().map(Spawner::to_tokens).collect();
        let underground_water_creature: Vec<_> = biome
            .spawners
            .underground_water_creature
            .iter()
            .map(Spawner::to_tokens)
            .collect();
        let water_ambient: Vec<_> = biome
            .spawners
            .water_ambient
            .iter()
            .map(Spawner::to_tokens)
            .collect();
        let water_creature: Vec<_> = biome
            .spawners
            .water_creature
            .iter()
            .map(Spawner::to_tokens)
            .collect();

        let spawners = quote! {
            SpawnGroups {
                monster: &[#(#monster),*],
                ambient: &[#(#ambient),*],
                axolotls: &[#(#axolotls),*],
                creature: &[#(#creature),*],
                misc: &[#(#misc),*],
                underground_water_creature: &[#(#underground_water_creature),*],
                water_ambient: &[#(#water_ambient),*],
                water_creature: &[#(#water_creature),*],
            }
        };

        let spawn_costs: Vec<_> = biome
            .spawn_costs
            .iter()
            .map(|(name, cost)| {
                let cost_token = cost.to_tokens();
                let entity_type = name.strip_prefix("minecraft:").unwrap();
                quote! {
                    #entity_type => #cost_token
                }
            })
            .collect();

        let temperature_modifier = match temperature_modifier {
            TemperatureModifier::Frozen => quote! { TemperatureModifier::Frozen },
            TemperatureModifier::None => quote! { TemperatureModifier::None },
        };
        let index = LitInt::new(&biome.id.to_string(), Span::call_site());

        variants.extend([quote! {
            pub const #format_name: Biome = Biome {
                id: #index,
                registry_id: #name,
                weather: Weather::new(
                     #has_precipitation,
                     #temperature,
                     #temperature_modifier,
                     #downfall
                ),
                features: &[#(#features),*],
                carvers: &[#(#carvers),*],
                creature_spawn_probability: #creature_spawn_probability,
                spawners: #spawners,
                spawn_costs: phf::phf_map! {
                    #(#spawn_costs),*
                },
            };
        }]);

        name_to_type.extend(quote! { #name => Some(&Self::#format_name), });
        id_to_type.extend(quote! { #index => Some(&Self::#format_name), });
        all_variants.extend(quote! { &Self::#format_name, });
    }

    let overworld_tree = biome_trees.overworld.into_token_stream();
    let nether_tree = biome_trees.nether.into_token_stream();
    quote! {
        use crate::biome::de::Deserialize;
        use crate::entity_type::EntityType;
        use crate::tag::Taggable;
        use crate::tag::RegistryKey;
        use papokin_util::biome::{TemperatureModifier, Weather};
        use serde::{Deserializer, de};
        use std::{fmt, hash::{Hasher, Hash}};

        #[derive(Debug)]
        pub struct Biome {
            pub id: u8,
            pub registry_id: &'static str,
            pub weather: Weather,
            pub carvers: &'static [&'static crate::carver::CarverConfig],
            pub features: &'static [&'static [crate::placed_feature::PlacedFeature]],
            pub creature_spawn_probability: f32,
            pub spawners: SpawnGroups,
            pub spawn_costs: phf::Map<&'static str, SpawnCosts>,
        }

        impl PartialEq<u8> for Biome {
            fn eq(&self, other: &u8) -> bool {
                self.id == *other
            }
        }

        impl PartialEq<Biome> for u8 {
            fn eq(&self, other: &Biome) -> bool {
                *self == other.id
            }
        }

        #[derive(Debug)]
        pub struct SpawnGroups {
            pub monster: &'static [Spawner],
            pub ambient: &'static [Spawner],
            pub axolotls: &'static [Spawner],
            pub creature: &'static [Spawner],
            pub misc: &'static [Spawner],
            pub underground_water_creature: &'static [Spawner],
            pub water_ambient: &'static [Spawner],
            pub water_creature: &'static [Spawner],
        }

        #[derive(Debug)]
        pub struct Spawner {
            pub r#type: &'static str,
            pub min_count: i32,
            pub max_count: i32,
        }

        impl PartialEq for Biome {
            fn eq(&self, other: &Biome) -> bool {
                self.id == other.id
            }
        }

        impl Eq for Biome {}

        impl Hash for Biome {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.id.hash(state);
            }
        }

        #[derive(Debug)]
        pub struct SpawnCosts {
            pub energy_budget: f64,
            pub charge: f64,
        }

        impl<'de> Deserialize<'de> for &'static Biome {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                struct BiomeVisitor;

                impl de::Visitor<'_> for BiomeVisitor {
                    type Value = &'static Biome;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("a biome name as a string")
                    }

                    fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value, E> {
                        self.visit_str(&v)
                    }

                    fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                        let biome = Biome::from_name(value.strip_prefix("minecraft:").unwrap_or(value));
                        biome.ok_or_else(|| E::unknown_variant(value, &["unknown biome"]))
                    }
                }

                deserializer.deserialize_str(BiomeVisitor)
            }
        }

        impl Biome {
            #variants

            pub const ALL: &'static [&'static Self] = &[#all_variants];

            pub fn from_name(name: &str) -> Option<&'static Self> {
                match name {
                    #name_to_type
                    _ => None
                }
            }

            pub const fn from_id(id: u8) -> Option<&'static Self> {
                match id {
                    #id_to_type
                    _ => None
                }
            }
        }

        impl Taggable for Biome {
            #[inline]
            fn registry_id(&self) -> u16 {
                self.id as u16
            }
            #[inline]
            fn tag_key() -> RegistryKey {
                RegistryKey::WorldgenBiome
            }
            #[inline]
            fn registry_key(&self) -> &str {
                self.registry_id
            }
        }

        pub const QUANTIZATION_FACTOR: f32 = 10000.0;

        #[inline]
        #[must_use]
        pub const fn quantize_coord(coord: f32) -> i64 {
            (coord * QUANTIZATION_FACTOR) as i64
        }

        #[inline]
        #[must_use]
        pub const fn unquantize_coord(coord: i64) -> f32 {
            coord as f32 / QUANTIZATION_FACTOR
        }

        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        pub struct Parameter {
            pub min: i64,
            pub max: i64,
        }

        pub type ParameterRange = Parameter;

        impl Parameter {
            #[must_use]
            pub const fn new(min: i64, max: i64) -> Self {
                Self { min, max }
            }

            #[must_use]
            pub const fn point(min: f32) -> Self {
                Self::span(min, min)
            }

            #[must_use]
            pub const fn span(min: f32, max: f32) -> Self {
                assert!(min <= max, "min > max");
                Self {
                    min: quantize_coord(min),
                    max: quantize_coord(max),
                }
            }

            #[must_use]
            pub const fn span_quantized(min: i64, max: i64) -> Self {
                assert!(min <= max, "min > max");
                Self { min, max }
            }

            #[inline]
            #[must_use]
            pub const fn calc_distance(&self, noise: i64) -> i64 {
                self.distance(noise)
            }

            #[inline]
            #[must_use]
            pub const fn distance(&self, target: i64) -> i64 {
                let above = target - self.max;
                let below = self.min - target;
                if above > 0 {
                    above
                } else if below > 0 {
                    below
                } else {
                    0
                }
            }

            #[inline]
            #[must_use]
            pub const fn distance_parameter(&self, target: &Self) -> i64 {
                let above = target.min - self.max;
                let below = self.min - target.max;
                if above > 0 {
                    above
                } else if below > 0 {
                    below
                } else {
                    0
                }
            }

            #[inline]
            #[must_use]
            pub const fn span_with(&self, other: Option<&Self>) -> Self {
                match other {
                    None => *self,
                    Some(other) => Self {
                        min: if self.min < other.min { self.min } else { other.min },
                        max: if self.max > other.max { self.max } else { other.max },
                    },
                }
            }
        }

        impl fmt::Display for Parameter {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                if self.min == self.max {
                    write!(f, "{}", self.min)
                } else {
                    write!(f, "[{}-{}]", self.min, self.max)
                }
            }
        }

        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        pub struct TargetPoint {
            pub temperature: i64,
            pub humidity: i64,
            pub continentalness: i64,
            pub erosion: i64,
            pub depth: i64,
            pub weirdness: i64,
        }

        impl TargetPoint {
            #[must_use]
            pub const fn new(
                temperature: i64,
                humidity: i64,
                continentalness: i64,
                erosion: i64,
                depth: i64,
                weirdness: i64,
            ) -> Self {
                Self {
                    temperature,
                    humidity,
                    continentalness,
                    erosion,
                    depth,
                    weirdness,
                }
            }

            #[must_use]
            pub const fn to_parameter_array(&self) -> [i64; 7] {
                [
                    self.temperature,
                    self.humidity,
                    self.continentalness,
                    self.erosion,
                    self.depth,
                    self.weirdness,
                    0,
                ]
            }

            #[must_use]
            pub const fn convert_to_list(&self) -> [i64; 7] {
                self.to_parameter_array()
            }
        }

        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        pub struct ParameterPoint {
            pub temperature: Parameter,
            pub humidity: Parameter,
            pub continentalness: Parameter,
            pub erosion: Parameter,
            pub depth: Parameter,
            pub weirdness: Parameter,
            pub offset: i64,
        }

        impl ParameterPoint {
            #[must_use]
            pub const fn new(
                temperature: Parameter,
                humidity: Parameter,
                continentalness: Parameter,
                erosion: Parameter,
                depth: Parameter,
                weirdness: Parameter,
                offset: i64,
            ) -> Self {
                Self {
                    temperature,
                    humidity,
                    continentalness,
                    erosion,
                    depth,
                    weirdness,
                    offset,
                }
            }

            #[inline]
            #[must_use]
            pub const fn fitness(&self, target: &TargetPoint) -> i64 {
                let temp_dist = self.temperature.distance(target.temperature);
                let hum_dist = self.humidity.distance(target.humidity);
                let cont_dist = self.continentalness.distance(target.continentalness);
                let ero_dist = self.erosion.distance(target.erosion);
                let dep_dist = self.depth.distance(target.depth);
                let wei_dist = self.weirdness.distance(target.weirdness);

                temp_dist * temp_dist
                    + hum_dist * hum_dist
                    + cont_dist * cont_dist
                    + ero_dist * ero_dist
                    + dep_dist * dep_dist
                    + wei_dist * wei_dist
                    + self.offset * self.offset
            }

            #[must_use]
            pub const fn parameter_space(&self) -> [Parameter; 7] {
                [
                    self.temperature,
                    self.humidity,
                    self.continentalness,
                    self.erosion,
                    self.depth,
                    self.weirdness,
                    Parameter::new(self.offset, self.offset),
                ]
            }
        }

        #[must_use]
        pub const fn target(
            temperature: f32,
            humidity: f32,
            continentalness: f32,
            erosion: f32,
            depth: f32,
            weirdness: f32,
        ) -> TargetPoint {
            TargetPoint::new(
                quantize_coord(temperature),
                quantize_coord(humidity),
                quantize_coord(continentalness),
                quantize_coord(erosion),
                quantize_coord(depth),
                quantize_coord(weirdness),
            )
        }

        #[must_use]
        pub const fn parameters(
            temperature: f32,
            humidity: f32,
            continentalness: f32,
            erosion: f32,
            depth: f32,
            weirdness: f32,
            offset: f32,
        ) -> ParameterPoint {
            ParameterPoint::new(
                Parameter::point(temperature),
                Parameter::point(humidity),
                Parameter::point(continentalness),
                Parameter::point(erosion),
                Parameter::point(depth),
                Parameter::point(weirdness),
                quantize_coord(offset),
            )
        }

        #[derive(PartialEq)]
        pub enum BiomeTree {
            Leaf {
                parameters: [ParameterRange; 7],
                biome: &'static Biome,
            },
            Branch {
                parameters: [ParameterRange; 7],
                nodes: &'static [BiomeTree],
            },
        }


        impl BiomeTree {
           pub fn get(
                &'static self,
                point_list: &[i64; 7],
                previous_result_node: &mut Option<&'static BiomeTree>,
            ) -> &'static Biome {
                // 从上一个结果初始化最佳距离，以保持空间连贯性
                let mut best_dist = previous_result_node
                    .map(|node| node.get_squared_distance(point_list))
                    .unwrap_or(i64::MAX);

                let mut best_node = previous_result_node.unwrap_or(self);

                self.search(point_list, &mut best_dist, &mut best_node);

                match best_node {
                    BiomeTree::Leaf { biome, .. } => {
                        *previous_result_node = Some(best_node);
                        biome
                    }
                    // 使用有效的树数据时不应发生
                    _ => unreachable!("生物群系搜索未能找到叶节点"),
                }
            }

            fn search(
                &'static self,
                point: &[i64; 7],
                best_dist: &mut i64,
                best_node: &mut &'static BiomeTree,
            ) {
                let dist = self.get_squared_distance(point);

                // 剪枝：如果此分支/叶子比当前最优更远，则跳过它及其所有子节点
                if dist >= *best_dist {
                    return;
                }

                match self {
                    Self::Leaf { .. } => {
                        *best_dist = dist;
                        *best_node = self;
                    }
                    Self::Branch { nodes, .. } => {
                        for node in *nodes {
                            node.search(point, best_dist, best_node);
                        }
                    }
                }
            }

            #[inline(always)]
            fn get_squared_distance(&self, p: &[i64; 7]) -> i64 {
                let params = match self {
                    Self::Leaf { parameters, .. } => parameters,
                    Self::Branch { parameters, .. } => parameters,
                };

                let d0 = params[0].calc_distance(p[0]);
                let d1 = params[1].calc_distance(p[1]);
                let d2 = params[2].calc_distance(p[2]);
                let d3 = params[3].calc_distance(p[3]);
                let d4 = params[4].calc_distance(p[4]);
                let d5 = params[5].calc_distance(p[5]);
                let d6 = params[6].calc_distance(p[6]);

                d0 * d0 + d1 * d1 + d2 * d2 + d3 * d3 + d4 * d4 + d5 * d5 + d6 * d6
            }
        }

        pub const OVERWORLD_BIOME_SOURCE: BiomeTree = #overworld_tree;
        pub const NETHER_BIOME_SOURCE: BiomeTree = #nether_tree;
    }
}
