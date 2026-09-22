use std::{collections::BTreeMap, fs};

use heck::ToPascalCase;
use papokin_util::HeightMap;
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use serde::Deserialize;
use syn::LitInt;

/// `entities.json` 中单个实体类型条目的原始反序列化结构。
#[derive(Deserialize)]
pub struct EntityType {
    /// 此实体类型的数字注册表 ID。
    pub id: u16,
    pub attributes: Option<Vec<BTreeMap<String, f64>>>,
    pub experience_reward: Option<u32>,
    /// 当可从提取的实体数据安全推导出时，为静态受伤音效事件名。
    pub hurt_sound: Option<String>,
    /// 当可从提取的实体数据安全推导出时，为静态死亡音效事件名。
    pub death_sound: Option<String>,
    /// 此实体能否被玩家或其他实体攻击。
    pub attackable: Option<bool>,
    /// 此实体是否被归类为生物（影响生成机制）。
    pub mob: Option<bool>,
    /// 每个区块允许的此实体类型最大数量（如有上限）。
    pub limit_per_chunk: Option<i32>,
    /// 此实体能否通过 `/summon` 命令生成。
    pub summonable: bool,
    /// 此实体是否免疫火焰伤害。
    pub fire_immune: bool,
    /// 此实体是否保存到世界文件中。
    pub saveable: bool,
    /// 控制此实体生成上限与持久性的生物类别。
    pub category: MobCategory,
    /// 此实体能否在远离玩家处生成（超出正常生成范围）。
    pub can_spawn_far_from_player: bool,
    /// 客户端追踪范围（以区块计）。
    pub client_tracking_range: u32,
    /// 以刻为单位的更新间隔。
    pub update_interval: u32,
    /// 是否应跟踪移动增量。
    pub track_deltas: bool,
    /// 以方块为单位的包围盒尺寸，格式为 `[width, height]`。
    pub dimension: [f32; 2],
    /// 以方块为单位的眼睛高度，用于视线计算。
    pub eye_height: f32,
    /// 生成时尺寸的缩放系数（例如史莱姆/岩浆怪）。
    pub spawn_dimensions_scale: f32,
    /// 自然生成的位置限制。
    pub spawn_restriction: SpawnRestriction,
}

/// 控制实体可在何处自然生成的限制。
#[derive(Deserialize)]
pub struct SpawnRestriction {
    /// 生成位置必须满足的地表或流体条件。
    location: SpawnLocation,
    /// 用于确定生成有效 Y 范围的高度图。
    heightmap: HeightMap,
}

/// 实体生成位置必须存在满足条件的地表或流体。
#[derive(Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SpawnLocation {
    InLava,
    InWater,
    OnGround,
    Unrestricted,
}

/// 生物的大类划分，决定其生成上限与持久化规则。
#[derive(Deserialize)]
#[expect(non_camel_case_types)]
#[expect(clippy::upper_case_acronyms)]
pub enum MobCategory {
    MONSTER,
    CREATURE,
    AMBIENT,
    AXOLOTLS,
    UNDERGROUND_WATER_CREATURE,
    WATER_CREATURE,
    WATER_AMBIENT,
    MISC,
}

/// 将原始实体名称字符串与其反序列化后的 [`EntityType`] 数据配对，用于生成 token。
pub struct NamedEntityType<'a>(&'a str, &'a EntityType);

impl ToTokens for NamedEntityType<'_> {
    /// 为被包装的实体生成 `EntityType { … }` 结构体字面量 token 流。
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = self.0;
        let entity = self.1;
        let id = LitInt::new(&entity.id.to_string(), proc_macro2::Span::call_site());

        let attribute_tokens = entity
            .attributes
            .as_ref()
            .map(|vec| {
                vec.iter()
                    .map(|map| {
                        let (key, value) = map.iter().next().unwrap();
                        let key = key.strip_prefix("minecraft:").unwrap_or(key);
                        // 将点替换为下划线并转为大写以命名枚举（例如 generic.max_health -> GENERIC_MAX_HEALTH）
                        let enum_variant =
                            format_ident!("{}", key.replace('.', "_").to_uppercase());

                        quote! { (Attributes::#enum_variant, #value) }
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let attributes_field = quote! { &[#(#attribute_tokens),*] };

        let attackable = if let Some(a) = entity.attackable {
            quote! { Some(#a) }
        } else {
            quote! { None }
        };

        let death_sound = if let Some(sound_name) = entity.death_sound.as_ref() {
            let sound_ident = format_ident!("{}", sound_name.to_pascal_case());
            quote! { Some(Sound::#sound_ident) }
        } else {
            quote! { None }
        };

        let hurt_sound = if let Some(sound_name) = entity.hurt_sound.as_ref() {
            let sound_ident = format_ident!("{}", sound_name.to_pascal_case());
            quote! { Some(Sound::#sound_ident) }
        } else {
            quote! { None }
        };

        let spawn_restriction_location = match entity.spawn_restriction.location {
            SpawnLocation::InLava => quote! {SpawnLocation::InLava},
            SpawnLocation::InWater => quote! {SpawnLocation::InWater},
            SpawnLocation::OnGround => quote! {SpawnLocation::OnGround},
            SpawnLocation::Unrestricted => quote! {SpawnLocation::Unrestricted},
        };

        let spawn_restriction_heightmap = match entity.spawn_restriction.heightmap {
            HeightMap::WorldSurfaceWg => quote! { HeightMap::WorldSurfaceWg },
            HeightMap::WorldSurface => quote! { HeightMap::WorldSurface },
            HeightMap::OceanFloorWg => quote! { HeightMap::OceanFloorWg },
            HeightMap::OceanFloor => quote! { HeightMap::OceanFloor },
            HeightMap::MotionBlocking => quote! { HeightMap::MotionBlocking },
            HeightMap::MotionBlockingNoLeaves => quote! { HeightMap::MotionBlockingNoLeaves },
        };

        let spawn_restriction = quote! { SpawnRestriction {
            location: #spawn_restriction_location,
            heightmap: #spawn_restriction_heightmap,
        }};

        let spawn_category = match entity.category {
            MobCategory::MONSTER => quote! { MobCategory::MONSTER },
            MobCategory::CREATURE => quote! { MobCategory::CREATURE },
            MobCategory::AMBIENT => quote! { MobCategory::AMBIENT },
            MobCategory::AXOLOTLS => quote! { MobCategory::AXOLOTLS },
            MobCategory::UNDERGROUND_WATER_CREATURE => {
                quote! { MobCategory::UNDERGROUND_WATER_CREATURE }
            }
            MobCategory::WATER_CREATURE => quote! { MobCategory::WATER_CREATURE },
            MobCategory::WATER_AMBIENT => quote! { MobCategory::WATER_AMBIENT },
            MobCategory::MISC => quote! { MobCategory::MISC },
        };

        let saveable = entity.saveable;
        let summonable = entity.summonable;
        let fire_immune = entity.fire_immune;
        let eye_height = entity.eye_height;
        assert!(
            !(entity.mob.is_none() && name != "player"),
            "missing field 'mob', entity name {name}"
        );
        assert!(
            !(entity.limit_per_chunk.is_none() && name != "player"),
            "missing field 'mob', entity name {name}"
        );
        let mob = entity.mob.unwrap_or(false);
        let limit_per_chunk = entity.limit_per_chunk.unwrap_or(0);
        let can_spawn_far_from_player = entity.can_spawn_far_from_player;

        let dimension0 = entity.dimension[0];
        let dimension1 = entity.dimension[1];
        let spawn_dimensions_scale = entity.spawn_dimensions_scale;
        let experience_reward = entity.experience_reward.unwrap_or(0);
        let client_tracking_range = entity.client_tracking_range;
        let update_interval = entity.update_interval;
        let track_deltas = entity.track_deltas;

        tokens.extend(quote! {
            EntityType {
                id: #id,
                attributes: #attributes_field,
                experience_reward: #experience_reward,
                hurt_sound: #hurt_sound,
                death_sound: #death_sound,
                attackable: #attackable,
                mob: #mob,
                saveable: #saveable,
                limit_per_chunk: #limit_per_chunk,
                summonable: #summonable,
                fire_immune: #fire_immune,
                category: &#spawn_category,
                can_spawn_far_from_player: #can_spawn_far_from_player,
                client_tracking_range: #client_tracking_range,
                update_interval: #update_interval,
                track_deltas: #track_deltas,
                dimension: [#dimension0, #dimension1], // 正确构造数组
                eye_height: #eye_height,
                spawn_dimensions_scale: #spawn_dimensions_scale,
                spawn_restriction: #spawn_restriction,
                resource_name: #name,
            }
        });
    }
}

/// 生成 `EntityType` 结构体、`MobCategory`、`SpawnRestriction` 的 `TokenStream`，
/// 以及 `from_raw`/`from_name` 查找方法。
pub fn build() -> TokenStream {
    let json: BTreeMap<String, EntityType> =
        serde_json::from_str(&fs::read_to_string("../../assets/entities.json").unwrap())
            .expect("解析 entities.json 失败");

    let mut consts = TokenStream::new();
    let mut type_from_raw_id_arms = TokenStream::new();
    let mut type_from_name = TokenStream::new();
    let mut all_variants = TokenStream::new();

    for (name, entity) in &json {
        let id = entity.id as u8;
        let id_lit = LitInt::new(&id.to_string(), proc_macro2::Span::call_site());
        let upper_name = format_ident!("{}", name.to_uppercase());

        let entity_tokens = NamedEntityType(name, entity).to_token_stream();

        consts.extend(quote! {
            pub const #upper_name: EntityType = #entity_tokens;
        });

        type_from_raw_id_arms.extend(quote! {
            #id_lit => Some(&Self::#upper_name),
        });

        type_from_name.extend(quote! {
            #name => Some(&Self::#upper_name),
        });

        all_variants.extend(quote! {
            &Self::#upper_name,
        })
    }
    quote! {
        use crate::data_component_impl::IDSetContent;
        use crate::tag::Taggable;
        use crate::tag::RegistryKey;
        use crate::attributes::Attributes;
        use crate::sound::Sound;
        use papokin_util::HeightMap;
        use papokin_util::math::boundingbox::BoundingBox;
        use papokin_util::math::vector3::Vector3;
        use std::hash::Hash;

        #[derive(Debug, Clone)]
        pub struct EntityType {
            pub id: u16,
            pub attributes: &'static [(Attributes, f64)],
            pub experience_reward: u32,
            pub hurt_sound: Option<Sound>,
            pub death_sound: Option<Sound>,
            pub attackable: Option<bool>,
            pub mob: bool,
            pub saveable: bool,
            pub limit_per_chunk: i32,
            pub summonable: bool,
            pub fire_immune: bool,
            pub category: &'static MobCategory,
            pub can_spawn_far_from_player: bool,
            pub client_tracking_range: u32,
            pub update_interval: u32,
            pub track_deltas: bool,
            pub dimension: [f32; 2],
            pub eye_height: f32,
            pub spawn_dimensions_scale: f32,
            pub spawn_restriction: SpawnRestriction,
            pub resource_name: &'static str,
        }

        impl Hash for EntityType {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.id.hash(state);
            }
        }

        impl PartialEq for EntityType {
            fn eq(&self, other: &Self) -> bool {
                self.id == other.id
            }
        }

        impl Eq for EntityType {}

        impl Taggable for EntityType {
            #[inline]
            fn tag_key() -> RegistryKey {
                RegistryKey::EntityType
            }

            #[inline]
            fn registry_key(&self) -> &str {
                self.resource_name
            }

            #[inline]
            fn registry_id(&self) -> u16 {
                self.id
            }
        }


        #[derive(Debug, Clone)]
        pub struct SpawnRestriction {
            pub location: SpawnLocation,
            pub heightmap: HeightMap,
        }

        #[derive(Debug, Clone)]
        pub enum SpawnLocation {
            InLava,
            InWater,
            OnGround,
            Unrestricted,
        }

        #[derive(Debug)]
        pub struct MobCategory {
            pub id: usize, // mojang 没有此字段
            pub max: i32,
            pub is_friendly: bool,
            pub is_persistent: bool,
            pub despawn_distance: i32,
        }

        impl PartialEq for MobCategory {
            fn eq(&self, other: &Self) -> bool {
                self.id == other.id
            }
        }

        impl MobCategory {
            pub const NO_DESPAWN_DISTANCE: i32 = 32;
            pub const MONSTER: MobCategory = MobCategory {
                id: 0,
                max: 70,
                is_friendly: false,
                is_persistent: false,
                despawn_distance: 128,
            };
            pub const CREATURE: MobCategory = MobCategory {
                id: 1,
                max: 10,
                is_friendly: true,
                is_persistent: true,
                despawn_distance: 128,
            };
            pub const AMBIENT: MobCategory = MobCategory {
                id: 2,
                max: 15,
                is_friendly: true,
                is_persistent: false,
                despawn_distance: 128,
            };
            pub const AXOLOTLS: MobCategory = MobCategory {
                id: 3,
                max: 5,
                is_friendly: true,
                is_persistent: false,
                despawn_distance: 128,
            };
            pub const UNDERGROUND_WATER_CREATURE: MobCategory = MobCategory {
                id: 4,
                max: 5,
                is_friendly: true,
                is_persistent: false,
                despawn_distance: 128,
            };
            pub const WATER_CREATURE: MobCategory = MobCategory {
                id: 5,
                max: 5,
                is_friendly: true,
                is_persistent: true,
                despawn_distance: 128,
            };
            pub const WATER_AMBIENT: MobCategory = MobCategory {
                id: 6,
                max: 20,
                is_friendly: true,
                is_persistent: false,
                despawn_distance: 64,
            };
            pub const MISC: MobCategory = MobCategory {
                id: 7,
                max: -1,
                is_friendly: true,
                is_persistent: true,
                despawn_distance: 128,
            };
            pub const SPAWNING_CATEGORIES: [&'static Self; 8] = [
                &Self::MONSTER,
                &Self::CREATURE,
                &Self::AMBIENT,
                &Self::AXOLOTLS,
                &Self::UNDERGROUND_WATER_CREATURE,
                &Self::WATER_CREATURE,
                &Self::WATER_AMBIENT,
                &Self::MISC,
            ];
        }

        impl EntityType {
            #consts

            pub const ALL: &'static [&'static Self] = &[#all_variants];

            pub const fn from_raw(id: u16) -> Option<&'static Self> {
                match id {
                    #type_from_raw_id_arms
                    _ => None
                }
            }

            pub fn from_name(name: &str) -> Option<&'static Self> {
                let name = name.strip_prefix("minecraft:").unwrap_or(name);
                match name {
                    #type_from_name
                    _ => None
                }
            }

            #[must_use]
            pub const fn width(&self) -> f32 {
                self.dimension[0]
            }

            #[must_use]
            pub const fn height(&self) -> f32 {
                self.dimension[1]
            }

            #[must_use]
            pub fn get_spawn_bounding_box(&self, x: f64, y: f64, z: f64) -> BoundingBox {
                let half_width = f64::from(self.spawn_dimensions_scale * self.dimension[0] / 2.0);
                let height = f64::from(self.spawn_dimensions_scale * self.dimension[1]);
                BoundingBox::new(
                    Vector3::new(x - half_width, y, z - half_width),
                    Vector3::new(x + half_width, y + height, z + half_width),
                )
            }

            #[must_use]
            pub fn get_spawn_aabb(&self, x: f64, y: f64, z: f64) -> BoundingBox {
                self.get_spawn_bounding_box(x, y, z)
            }
        }

        impl IDSetContent for EntityType {
            fn registry_id(&self) -> u16 {
                Taggable::registry_id(self)
            }

            fn to_string(&self) -> String {
                Taggable::registry_key(self).to_string()
            }

            fn from_id(id: u16) -> Option<&'static Self> {
                EntityType::from_raw(id)
            }

            fn from_str(name: &str) -> Option<&'static Self> {
                EntityType::from_name(name)
            }
        }
    }
}
