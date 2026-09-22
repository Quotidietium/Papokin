use std::{collections::BTreeMap, fs};

use crate::placed_feature::value_to_int_provider;
use heck::ToShoutySnakeCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use serde::Deserialize;

/// 将 CSS 风格的十六进制颜色字符串（例如 `"#78a7ff"` 或 `"#ccffffff"`）解析为有符号 32 位整数。
///
/// # Returns
/// 以 `i32` 表示的颜色，如果输入不是有效的十六进制颜色则为 `None`。
fn parse_hex_color(s: &str) -> Option<i32> {
    if let Some(stripped) = s.strip_prefix('#') {
        u32::from_str_radix(stripped, 16).ok().map(|v| v as i32)
    } else {
        None
    }
}

/// `dimension.json` 中单个维度条目的原始反序列化结构。
#[derive(Deserialize)]
pub struct Dimension {
    /// 此维度是否有天空光源（即不是洞穴或下界）。
    pub has_skylight: bool,
    /// 此维度是否有基岩天花板（例如下界）。
    pub has_ceiling: bool,
    /// 加到所有方块上的环境光照等级，绕过正常的天空光/方块光计算。
    pub ambient_light: f32,
    /// 坐标缩放系数，用于将此维度中的位置映射到主世界坐标。
    pub coordinate_scale: f64,
    /// 可建造/区块范围的最小 Y 等级（含）。
    pub min_y: i32,
    /// 可建造/区块范围的总高度（以方块为单位）。
    pub height: i32,
    /// 生物 AI 和传送门可用的最大 Y 等级（可能小于 `min_y + height`）。
    pub logical_height: i32,
    /// 充当无限燃烧源的方块的标签键（例如 `"minecraft:infiniburn_overworld"`）。
    pub infiniburn: String,
    pub monster_spawn_light_level: serde_json::Value,
    pub monster_spawn_block_light_limit: u8,
    /// 此维度中固定的白昼时间值，若时间正常推进则为 `None`。
    #[serde(rename = "fixed_time")]
    pub fixed_time: Option<i64>,
    /// 此维度中的时间是否固定（现代 26.2 字段）。
    #[serde(default, rename = "has_fixed_time")]
    pub has_fixed_time: Option<bool>,
    /// 环境属性映射（视觉、玩法、音频）。
    #[serde(default)]
    pub attributes: Option<serde_json::Value>,
    /// 控制昼夜进程的可选时间线资源键。
    #[serde(default)]
    pub timelines: Option<String>,
}

/// 生成 `Dimension` 结构体、其常量以及 `from_name` 查找的 `TokenStream`。
pub fn build() -> TokenStream {
    let dir = std::path::Path::new("../../assets/datapacks/26_3/data/minecraft/dimension_type");
    let mut dimensions: BTreeMap<String, Dimension> = BTreeMap::new();
    let mut entries: Vec<_> = fs::read_dir(dir)
        .expect("缺少 dimension_type 目录")
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .collect();
    entries.sort_by_key(|e| e.path());

    for entry in entries {
        let path = entry.path();
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        let key = format!("minecraft:{stem}");
        let content = fs::read_to_string(&path).expect("读取维度文件失败");
        let dim: Dimension = serde_json::from_str(&content).expect("解析维度 JSON 失败");
        dimensions.insert(key, dim);
    }

    let mut variants = TokenStream::new();
    let mut name_to_type = TokenStream::new();

    // 带索引迭代以生成唯一的数字 ID
    for (id, (name, dim)) in dimensions.into_iter().enumerate() {
        let id = id as u8; // 主世界=0，下界=1，末地=2（通常）
        let format_name = format_ident!(
            "{}",
            name.strip_prefix("minecraft:")
                .unwrap_or(&name)
                .to_shouty_snake_case()
        );

        let attrs = dim.attributes.as_ref();

        // 视觉环境属性
        let sky_color = attrs
            .and_then(|a| a.get("minecraft:visual/sky_color"))
            .and_then(|v| v.as_str())
            .and_then(parse_hex_color);
        let fog_color = attrs
            .and_then(|a| a.get("minecraft:visual/fog_color"))
            .and_then(|v| v.as_str())
            .and_then(parse_hex_color);
        let cloud_color = attrs
            .and_then(|a| a.get("minecraft:visual/cloud_color"))
            .and_then(|v| v.as_str())
            .and_then(parse_hex_color);
        let ambient_light_color = attrs
            .and_then(|a| a.get("minecraft:visual/ambient_light_color"))
            .and_then(|v| v.as_str())
            .and_then(parse_hex_color);
        let sky_light_color = attrs
            .and_then(|a| a.get("minecraft:visual/sky_light_color"))
            .and_then(|v| v.as_str())
            .and_then(parse_hex_color);

        let sky_light_factor = attrs
            .and_then(|a| a.get("minecraft:visual/sky_light_factor"))
            .and_then(|v| v.as_f64())
            .map(|f| f as f32);
        let fog_start_distance = attrs
            .and_then(|a| a.get("minecraft:visual/fog_start_distance"))
            .and_then(|v| v.as_f64())
            .map(|f| f as f32);
        let fog_end_distance = attrs
            .and_then(|a| a.get("minecraft:visual/fog_end_distance"))
            .and_then(|v| v.as_f64())
            .map(|f| f as f32);
        let cloud_height = attrs
            .and_then(|a| a.get("minecraft:visual/cloud_height"))
            .and_then(|v| v.as_f64())
            .map(|f| f as f32);

        // 游戏玩法环境属性
        let sky_light_level = attrs
            .and_then(|a| a.get("minecraft:gameplay/sky_light_level"))
            .and_then(|v| v.as_f64())
            .map(|f| f as f32);
        let water_evaporates = attrs
            .and_then(|a| a.get("minecraft:gameplay/water_evaporates"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let fast_lava = attrs
            .and_then(|a| a.get("minecraft:gameplay/fast_lava"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let respawn_anchor_works = attrs
            .and_then(|a| a.get("minecraft:gameplay/respawn_anchor_works"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let piglins_zombify = attrs
            .and_then(|a| a.get("minecraft:gameplay/piglins_zombify"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let snow_golem_melts = attrs
            .and_then(|a| a.get("minecraft:gameplay/snow_golem_melts"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let can_start_raid = attrs
            .and_then(|a| a.get("minecraft:gameplay/can_start_raid"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let nether_portal_spawns_piglin = attrs
            .and_then(|a| a.get("minecraft:gameplay/nether_portal_spawns_piglin"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let bed_rule_obj = attrs.and_then(|a| a.get("minecraft:gameplay/bed_rule"));
        let (can_sleep_token, can_set_spawn_token, explodes) = if let Some(b) = bed_rule_obj {
            let can_sleep = b
                .get("can_sleep")
                .and_then(|v| v.as_str())
                .unwrap_or("when_dark");
            let can_set_spawn = b
                .get("can_set_spawn")
                .and_then(|v| v.as_str())
                .unwrap_or("always");
            let explodes = b.get("explodes").and_then(|v| v.as_bool()).unwrap_or(false);

            let sleep_ident = match can_sleep {
                "always" => quote!(BedRuleOption::Always),
                "never" => quote!(BedRuleOption::Never),
                _ => quote!(BedRuleOption::WhenDark),
            };
            let spawn_ident = match can_set_spawn {
                "when_dark" => quote!(BedRuleOption::WhenDark),
                "never" => quote!(BedRuleOption::Never),
                _ => quote!(BedRuleOption::Always),
            };
            (sleep_ident, spawn_ident, explodes)
        } else {
            (
                quote!(BedRuleOption::WhenDark),
                quote!(BedRuleOption::Always),
                false,
            )
        };

        let fixed_time = if let Some(t) = dim.fixed_time {
            quote! { Some(#t) }
        } else {
            quote! { None }
        };
        let has_fixed_time = dim.has_fixed_time.unwrap_or(dim.fixed_time.is_some());

        let monster_spawn_light_level = value_to_int_provider(&dim.monster_spawn_light_level);
        let monster_spawn_block_light_limit = dim.monster_spawn_block_light_limit;
        let ambient_light = dim.ambient_light;
        let coordinate_scale = dim.coordinate_scale;
        let height = dim.height;
        let min_y = dim.min_y;
        let logical_height = dim.logical_height;
        let has_skylight = dim.has_skylight;
        let has_ceiling = dim.has_ceiling;

        // 将 infiniburn 规范化为始终带有命名空间
        let infiniburn = if dim.infiniburn.contains(':') {
            dim.infiniburn.clone()
        } else {
            format!("minecraft:{}", dim.infiniburn)
        };
        let timelines = dim.timelines.map(|t| {
            if t.contains(':') {
                t
            } else {
                format!("minecraft:{}", t)
            }
        });

        let minecraft_name = if name.contains(':') {
            name.clone()
        } else {
            format!("minecraft:{name}")
        };

        let sky_color_literal = if let Some(c) = sky_color {
            quote! { Some(#c) }
        } else {
            quote! { None }
        };
        let fog_color_literal = if let Some(c) = fog_color {
            quote! { Some(#c) }
        } else {
            quote! { None }
        };
        let cloud_color_literal = if let Some(c) = cloud_color {
            quote! { Some(#c) }
        } else {
            quote! { None }
        };
        let ambient_light_color_literal = if let Some(c) = ambient_light_color {
            quote! { Some(#c) }
        } else {
            quote! { None }
        };
        let sky_light_color_literal = if let Some(c) = sky_light_color {
            quote! { Some(#c) }
        } else {
            quote! { None }
        };
        let sky_light_factor_literal = if let Some(f) = sky_light_factor {
            quote! { Some(#f) }
        } else {
            quote! { None }
        };
        let fog_start_distance_literal = if let Some(f) = fog_start_distance {
            quote! { Some(#f) }
        } else {
            quote! { None }
        };
        let fog_end_distance_literal = if let Some(f) = fog_end_distance {
            quote! { Some(#f) }
        } else {
            quote! { None }
        };
        let cloud_height_literal = if let Some(f) = cloud_height {
            quote! { Some(#f) }
        } else {
            quote! { None }
        };
        let sky_light_level_literal = if let Some(f) = sky_light_level {
            quote! { Some(#f) }
        } else {
            quote! { None }
        };
        let timelines_literal = if let Some(t) = timelines.clone() {
            quote! { Some(#t) }
        } else {
            quote! { None }
        };

        variants.extend(quote! {
            pub const #format_name: Self = Self {
                id: #id,
                minecraft_name: #minecraft_name,
                fixed_time: #fixed_time,
                has_fixed_time: #has_fixed_time,
                has_skylight: #has_skylight,
                has_ceiling: #has_ceiling,
                coordinate_scale: #coordinate_scale,
                min_y: #min_y,
                height: #height,
                logical_height: #logical_height,
                infiniburn: #infiniburn,
                ambient_light: #ambient_light,
                monster_spawn_light_level: #monster_spawn_light_level,
                monster_spawn_block_light_limit: #monster_spawn_block_light_limit,
                sky_color: #sky_color_literal,
                fog_color: #fog_color_literal,
                cloud_color: #cloud_color_literal,
                ambient_light_color: #ambient_light_color_literal,
                sky_light_color: #sky_light_color_literal,
                sky_light_factor: #sky_light_factor_literal,
                fog_start_distance: #fog_start_distance_literal,
                fog_end_distance: #fog_end_distance_literal,
                cloud_height: #cloud_height_literal,
                sky_light_level: #sky_light_level_literal,
                water_evaporates: #water_evaporates,
                fast_lava: #fast_lava,
                respawn_anchor_works: #respawn_anchor_works,
                piglins_zombify: #piglins_zombify,
                snow_golem_melts: #snow_golem_melts,
                can_start_raid: #can_start_raid,
                nether_portal_spawns_piglin: #nether_portal_spawns_piglin,
                bed_rule: BedRule {
                    can_sleep: #can_sleep_token,
                    can_set_spawn: #can_set_spawn_token,
                    explodes: #explodes,
                },
                timelines: #timelines_literal,
            };
        });

        name_to_type.extend(quote! {
            #minecraft_name => Some(&Self::#format_name),
        });
    }

    quote!(
        use papokin_util::math::int_provider::{
            BiasedToBottomIntProvider, ClampedIntProvider, TrapezoidIntProvider, ClampedNormalIntProvider,
            ConstantIntProvider, IntProvider, NormalIntProvider, UniformIntProvider,
            WeightedEntry, WeightedListIntProvider,
        };

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum BedRuleOption {
            Always,
            WhenDark,
            Never,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct BedRule {
            pub can_sleep: BedRuleOption,
            pub can_set_spawn: BedRuleOption,
            pub explodes: bool,
        }

        impl BedRule {
            #[inline]
            #[must_use]
            pub const fn can_sleep(&self, is_dark_outside: bool) -> bool {
                match self.can_sleep {
                    BedRuleOption::Always => true,
                    BedRuleOption::WhenDark => is_dark_outside,
                    BedRuleOption::Never => false,
                }
            }

            #[inline]
            #[must_use]
            pub const fn can_set_spawn(&self, is_dark_outside: bool) -> bool {
                match self.can_set_spawn {
                    BedRuleOption::Always => true,
                    BedRuleOption::WhenDark => is_dark_outside,
                    BedRuleOption::Never => false,
                }
            }
        }

        #[derive(Debug, Clone)]
        pub struct Dimension {
            pub id: u8,
            pub minecraft_name: &'static str,
            pub fixed_time: Option<i64>,
            pub has_fixed_time: bool,
            pub has_skylight: bool,
            pub has_ceiling: bool,
            pub coordinate_scale: f64,
            pub min_y: i32,
            pub height: i32,
            pub logical_height: i32,
            pub infiniburn: &'static str,
            pub ambient_light: f32,
            pub monster_spawn_light_level: IntProvider,
            pub monster_spawn_block_light_limit: u8,
            pub sky_color: Option<i32>,
            pub fog_color: Option<i32>,
            pub cloud_color: Option<i32>,
            pub ambient_light_color: Option<i32>,
            pub sky_light_color: Option<i32>,
            pub sky_light_factor: Option<f32>,
            pub fog_start_distance: Option<f32>,
            pub fog_end_distance: Option<f32>,
            pub cloud_height: Option<f32>,
            pub sky_light_level: Option<f32>,
            pub water_evaporates: bool,
            pub fast_lava: bool,
            pub respawn_anchor_works: bool,
            pub piglins_zombify: bool,
            pub snow_golem_melts: bool,
            pub can_start_raid: bool,
            pub nether_portal_spawns_piglin: bool,
            pub bed_rule: BedRule,
            pub timelines: Option<&'static str>,
        }

        impl Dimension {
            #variants

            pub fn from_name(name: &str) -> Option<&'static Self> {
                match name {
                    #name_to_type
                    _ => None
                }
            }

            #[inline]
            #[must_use]
            pub const fn effective_sky_light_level(&self) -> f32 {
                if let Some(level) = self.sky_light_level {
                    level
                } else if self.has_skylight {
                    15.0
                } else {
                    0.0
                }
            }
        }
        impl PartialEq for Dimension {
            fn eq(&self, other: &Self) -> bool {
                 self.id == other.id
            }
       }
        impl Eq for Dimension {}
    )
}
