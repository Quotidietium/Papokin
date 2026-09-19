#![allow(
    unused,
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
    clippy::undocumented_unsafe_blocks,
    clippy::if_then_some_else_none,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic
)]

/// The Minecraft version of the bundled dataset (blocks, items, entities...).
///
/// Connections older than this need their registry IDs remapped on the wire.
/// This is deliberately independent of `packet::CURRENT_MC_VERSION` (the
/// protocol target): the protocol can move to a different version while the
/// generated dataset — and therefore every remap table — still speaks 26.3.
pub const NATIVE_DATA_VERSION: pumpkin_util::version::JavaMinecraftVersion =
    pumpkin_util::version::JavaMinecraftVersion::V_26_3;

#[rustfmt::skip]
#[path = "generated/chunk_view_lut.rs"]
pub mod chunk_view_lut;

#[rustfmt::skip]
#[path = "generated/loot_table.rs"]
pub mod loot_table;
pub use loot_table as chest_loot_table;

#[cfg(feature = "item")]
#[rustfmt::skip]
#[path = "generated/item.rs"]
pub mod item;

#[cfg(feature = "item")]
pub mod item_stack;

#[cfg(feature = "packet")]
#[rustfmt::skip]
#[path = "generated/packet.rs"]
pub mod packet;

#[cfg(feature = "jukebox_song")]
#[rustfmt::skip]
#[path = "generated/jukebox_song.rs"]
pub mod jukebox_song;

#[cfg(feature = "translation")]
#[rustfmt::skip]
#[path = "generated/translation.rs"]
pub mod translation;

#[cfg(feature = "registry")]
#[rustfmt::skip]
#[path = "generated/registry.rs"]
pub mod registry;

#[cfg(feature = "screen")]
#[rustfmt::skip]
#[path = "generated/screen.rs"]
pub mod screen;

#[cfg(feature = "particle")]
#[rustfmt::skip]
#[path = "generated/particle.rs"]
pub mod particle;

#[cfg(feature = "statistic")]
#[rustfmt::skip]
#[path = "generated/statistic.rs"]
pub mod statistic;

#[cfg(feature = "sound")]
#[rustfmt::skip]
#[path = "generated/sound_category.rs"]
mod sound_category;

#[cfg(feature = "sound")]
#[rustfmt::skip]
#[path = "generated/sound.rs"]
mod sound_enum;

#[cfg(feature = "sound")]
pub mod sound {
    pub use crate::sound_category::*;
    pub use crate::sound_enum::*;
}

#[cfg(feature = "advancement")]
#[rustfmt::skip]
#[path = "generated/advancement.rs"]
pub mod advancement;

#[cfg(feature = "advancement")]
pub mod advancement_data;

#[cfg(feature = "advancement")]
pub use advancement::*;

#[cfg(feature = "recipes")]
#[rustfmt::skip]
#[path = "generated/recipes.rs"]
pub mod recipes;

#[cfg(feature = "data_component")]
#[rustfmt::skip]
#[path = "generated/data_component.rs"]
pub mod data_component;

#[cfg(feature = "data_component")]
pub mod data_component_impl;

#[cfg(feature = "attributes")]
#[rustfmt::skip]
#[path = "generated/attributes.rs"]
pub mod attributes;

#[cfg(feature = "tracked_data")]
#[rustfmt::skip]
#[path = "generated/tracked_data.rs"]
pub mod tracked_data;

#[cfg(feature = "meta_data_type")]
#[rustfmt::skip]
#[path = "generated/meta_data_type.rs"]
pub mod meta_data_type;

#[cfg(feature = "noise_parameter")]
#[rustfmt::skip]
#[path = "generated/noise_parameter.rs"]
pub mod noise_parameter;

#[cfg(feature = "biome")]
#[expect(clippy::unreachable)]
#[rustfmt::skip]
#[path = "generated/biome.rs"]
pub mod biome;

#[cfg(feature = "chunk_status")]
#[rustfmt::skip]
#[path = "generated/chunk_status.rs"]
pub mod chunk_status;

#[cfg(feature = "chunk")]
pub mod chunk {
    #[cfg(feature = "biome")]
    pub use super::biome::*;
    #[cfg(feature = "chunk_status")]
    pub use super::chunk_status::ChunkStatus;
    #[cfg(feature = "noise_parameter")]
    pub use super::noise_parameter::*;
}

#[cfg(feature = "game_event")]
#[rustfmt::skip]
#[path = "generated/game_event.rs"]
pub mod game_event;

#[cfg(feature = "game_rules")]
#[rustfmt::skip]
#[path ="generated/game_rules.rs"]
pub mod game_rules;

#[cfg(feature = "entity_pose")]
#[rustfmt::skip]
#[path = "generated/entity_pose.rs"]
mod entity_pose;

#[cfg(feature = "entity_status")]
#[rustfmt::skip]
#[path = "generated/entity_status.rs"]
pub mod entity_status;

#[cfg(feature = "entity_type")]
#[rustfmt::skip]
#[path = "generated/entity_type.rs"]
mod entity_type;

#[cfg(feature = "spawn_egg")]
#[rustfmt::skip]
#[path = "generated/spawn_egg.rs"]
mod spawn_egg;

#[cfg(feature = "dimension")]
#[rustfmt::skip]
#[path = "generated/dimension.rs"]
pub mod dimension;

#[cfg(feature = "environment_attribute")]
#[rustfmt::skip]
#[path = "generated/environment_attribute.rs"]
pub mod environment_attribute;

#[cfg(feature = "environment_attribute")]
pub use environment_attribute::*;

#[cfg(feature = "enchantment")]
#[rustfmt::skip]
#[path = "generated/enchantment.rs"]
pub mod enchantment;

#[cfg(feature = "enchantment")]
pub use enchantment::*;

#[cfg(feature = "entity")]
pub mod entity {
    #[cfg(feature = "entity_pose")]
    pub use super::entity_pose::*;
    #[cfg(feature = "entity_status")]
    pub use super::entity_status::*;
    #[cfg(feature = "entity_type")]
    pub use super::entity_type::*;
    #[cfg(feature = "spawn_egg")]
    pub use super::spawn_egg::*;
}

#[cfg(feature = "world_event")]
#[rustfmt::skip]
#[path = "generated/world_event.rs"]
mod world_event;

#[cfg(feature = "message_type")]
#[rustfmt::skip]
#[path = "generated/message_type.rs"]
mod message_type;

#[cfg(feature = "world")]
pub mod world {
    #[cfg(feature = "message_type")]
    pub use super::message_type::*;
    #[cfg(feature = "world_event")]
    pub use super::world_event::*;
}

#[rustfmt::skip]
#[path = "generated/placed_feature.rs"]
pub mod placed_feature;

#[rustfmt::skip]
#[path = "generated/configured_feature.rs"]
pub mod configured_feature;

#[cfg(feature = "scoreboard")]
#[rustfmt::skip]
#[path = "generated/scoreboard_slot.rs"]
pub mod scoreboard;

#[cfg(feature = "damage")]
#[rustfmt::skip]
#[path = "generated/damage_type.rs"]
pub mod damage;

#[cfg(feature = "fluid")]
#[rustfmt::skip]
#[path = "generated/fluid.rs"]
pub mod fluid;

#[cfg(feature = "block")]
#[expect(clippy::unreachable)]
#[rustfmt::skip]
#[path = "generated/block.rs"]
pub mod block_properties;

#[cfg(feature = "block")]
#[rustfmt::skip]
#[path = "generated/block_state_remap.rs"]
pub mod block_state_remap;

/// Remaps ids of *synced* (dynamic) registries — biome, damage type, etc. —
/// from the bundled dataset's id space to the id space each client version
/// actually receives at configuration time. Unlike the hardcoded registries
/// above, these ids are defined by the registry data the server itself sends.
#[rustfmt::skip]
#[path = "generated/sync_id_remap.rs"]
pub mod sync_id_remap;

#[cfg(feature = "item_id_remap")]
#[rustfmt::skip]
#[path = "generated/item_id_remap.rs"]
pub mod item_id_remap;

#[cfg(feature = "entity_id_remap")]
#[rustfmt::skip]
#[path = "generated/entity_id_remap.rs"]
pub mod entity_id_remap;

#[cfg(feature = "sound_id_remap")]
#[rustfmt::skip]
#[path = "generated/sound_id_remap.rs"]
pub mod sound_id_remap;

#[cfg(feature = "particle_id_remap")]
#[rustfmt::skip]
#[path = "generated/particle_id_remap.rs"]
pub mod particle_id_remap;

#[cfg(feature = "menu_id_remap")]
#[rustfmt::skip]
#[path = "generated/menu_id_remap.rs"]
pub mod menu_id_remap;

#[cfg(feature = "recipe_serializer_id_remap")]
#[rustfmt::skip]
#[path = "generated/recipe_serializer_id_remap.rs"]
pub mod recipe_serializer_id_remap;

#[cfg(feature = "argument_type_id_remap")]
#[rustfmt::skip]
#[path = "generated/argument_type_id_remap.rs"]
pub mod argument_type_id_remap;

#[cfg(feature = "attribute_id_remap")]
#[rustfmt::skip]
#[path = "generated/attribute_id_remap.rs"]
pub mod attribute_id_remap;

#[cfg(feature = "block_entity_type_id_remap")]
#[rustfmt::skip]
#[path = "generated/block_entity_type_id_remap.rs"]
pub mod block_entity_type_id_remap;

#[cfg(feature = "custom_stat_id_remap")]
#[rustfmt::skip]
#[path = "generated/custom_stat_id_remap.rs"]
pub mod custom_stat_id_remap;

#[cfg(feature = "data_component_type_id_remap")]
#[rustfmt::skip]
#[path = "generated/data_component_type_id_remap.rs"]
pub mod data_component_type_id_remap;

#[cfg(feature = "enchantment_id_remap")]
#[rustfmt::skip]
#[path = "generated/enchantment_id_remap.rs"]
pub mod enchantment_id_remap;

#[cfg(feature = "environment_attribute_id_remap")]
#[rustfmt::skip]
#[path = "generated/environment_attribute_id_remap.rs"]
pub mod environment_attribute_id_remap;

#[cfg(feature = "painting_variant_id_remap")]
#[rustfmt::skip]
#[path = "generated/painting_variant_id_remap.rs"]
pub mod painting_variant_id_remap;

#[cfg(feature = "slot_display_id_remap")]
#[rustfmt::skip]
#[path = "generated/slot_display_id_remap.rs"]
pub mod slot_display_id_remap;

#[cfg(feature = "bedrock_creative")]
#[rustfmt::skip]
#[path = "generated/bedrock_creative.rs"]
pub mod bedrock_creative;

#[cfg(feature = "bedrock_biome")]
#[rustfmt::skip]
#[path = "generated/bedrock_biome.rs"]
pub mod bedrock_biome;

#[cfg(feature = "tag")]
#[rustfmt::skip]
#[path = "generated/tag.rs"]
pub mod tag;

#[cfg(feature = "noise_router")]
#[rustfmt::skip]
#[path = "generated/noise_router.rs"]
pub mod noise_router;

#[cfg(feature = "composter")]
#[rustfmt::skip]
#[path = "generated/composter_increase_chance.rs"]
pub mod composter_increase_chance;

#[cfg(feature = "flower_pot")]
#[rustfmt::skip]
#[path = "generated/flower_pot_transformations.rs"]
pub mod flower_pot_transformations;

#[cfg(feature = "fuels")]
#[rustfmt::skip]
#[path = "generated/fuels.rs"]
pub mod fuels;

#[cfg(feature = "effect")]
#[rustfmt::skip]
#[path = "generated/effect.rs"]
pub mod effect;

#[cfg(feature = "effect")]
#[rustfmt::skip]
#[path = "generated/status_effect.rs"]
pub mod status_effect;

#[cfg(feature = "structures")]
#[rustfmt::skip]
#[path = "generated/structures.rs"]
pub mod structures;

#[cfg(feature = "potion")]
#[rustfmt::skip]
#[path = "generated/potion.rs"]
pub mod potion;

#[cfg(feature = "potion_brewing")]
#[rustfmt::skip]
#[path = "generated/potion_brewing.rs"]
pub mod potion_brewing;

#[cfg(feature = "recipe_remainder")]
#[rustfmt::skip]
#[path = "generated/recipe_remainder.rs"]
pub mod recipe_remainder;

#[cfg(feature = "block")]
mod block_direction;
#[cfg(feature = "block")]
pub mod block_rotation;
#[cfg(feature = "block")]
pub mod block_state;
#[cfg(feature = "block")]
mod blocks;

#[cfg(feature = "block")]
pub use block_direction::{BlockDirection, FacingExt, HorizontalFacingExt};
#[cfg(feature = "block")]
pub use block_rotation::{Mirror, Rotation, transform_block_properties, transform_rail_shape};
#[cfg(feature = "block")]
pub use block_state::{BlockState, BlockStateId};
#[cfg(feature = "block")]
pub use blocks::{Block, BlockId, SpawnFloorPredicate};

#[cfg(feature = "material_rule")]
#[rustfmt::skip]
#[path = "generated/material_rule.rs"]
pub mod material_rule;

#[cfg(feature = "noise_settings")]
#[rustfmt::skip]
#[path = "generated/noise_settings.rs"]
pub mod noise_settings;

#[cfg(feature = "chunk_gen_settings")]
pub use noise_settings as chunk_gen_settings;

#[cfg(feature = "carver")]
#[rustfmt::skip]
#[path = "generated/carver.rs"]
pub mod carver;

#[cfg(feature = "villager")]
#[rustfmt::skip]
#[path = "generated/villager.rs"]
pub mod villager;

#[cfg(feature = "slot_ranges")]
#[rustfmt::skip]
#[path = "generated/slot_ranges.rs"]
pub mod slot_ranges;

#[cfg(feature = "map_color")]
#[rustfmt::skip]
#[path = "generated/map_color.rs"]
pub mod map_color;

#[cfg(feature = "map_decoration")]
#[rustfmt::skip]
#[path = "generated/map_decoration.rs"]
pub mod map_decoration;

#[cfg(feature = "dye_color")]
#[rustfmt::skip]
#[path = "generated/dye_color.rs"]
pub mod dye_color;

#[cfg(feature = "block_transformer")]
#[rustfmt::skip]
#[path = "generated/block_transformer.rs"]
pub mod block_transformer;

#[cfg(feature = "trial_spawner")]
#[rustfmt::skip]
#[path = "generated/trial_spawner.rs"]
pub mod trial_spawner;

#[cfg(test)]
mod sync_id_remap_tests {
    use crate::biome::Biome;
    use crate::damage::DamageType;
    use crate::sync_id_remap::{
        BIOME_SYNC_REMAP_V_26_3_TO_V_1_21_11, DAMAGE_TYPE_SYNC_REMAP_V_26_3_TO_V_1_21_11,
        remap_biome_id_for_version, remap_damage_type_id_for_version,
    };
    use pumpkin_util::version::JavaMinecraftVersion;

    /// Sorted index of the entry in the 1.21.11 datapack folder — i.e. the id
    /// the 1.21.11 client assigns it when the server syncs the registry.
    #[test]
    fn biome_sync_remap_matches_1_21_11_registry() {
        let v = JavaMinecraftVersion::V_1_21_11;

        // Native-version connections stay in the dataset id space.
        assert_eq!(
            remap_biome_id_for_version(u16::from(Biome::PLAINS.id), JavaMinecraftVersion::V_26_3),
            u16::from(Biome::PLAINS.id)
        );
        // Shared biomes translate to their 1.21.11 synced index.
        assert_eq!(
            remap_biome_id_for_version(u16::from(Biome::PLAINS.id), v),
            40
        );
        assert_eq!(
            remap_biome_id_for_version(u16::from(Biome::FOREST.id), v),
            21
        );
        // 26.x-only biomes fall back to the closest 1.21.11 biome.
        assert_eq!(
            remap_biome_id_for_version(u16::from(Biome::DAPPLED_FOREST.id), v),
            21
        );
        assert_eq!(
            remap_biome_id_for_version(u16::from(Biome::SULFUR_CAVES.id), v),
            15
        );
        // No entry may exceed the 1.21.11 registry's size (65 biomes), or the
        // client would fail its palette lookup.
        assert!(
            BIOME_SYNC_REMAP_V_26_3_TO_V_1_21_11
                .iter()
                .all(|&id| id < 65)
        );
    }

    #[test]
    fn damage_type_sync_remap_matches_1_21_11_registry() {
        let v = JavaMinecraftVersion::V_1_21_11;

        assert_eq!(
            remap_damage_type_id_for_version(
                u16::from(DamageType::GENERIC.id),
                JavaMinecraftVersion::V_26_3
            ),
            u16::from(DamageType::GENERIC.id)
        );
        // Entries before the 26.x-only `sulfur_cube_hot` keep their index.
        assert_eq!(
            remap_damage_type_id_for_version(u16::from(DamageType::HOT_FLOOR.id), v),
            u16::from(DamageType::HOT_FLOOR.id)
        );
        assert_eq!(
            remap_damage_type_id_for_version(u16::from(DamageType::SULFUR_CUBE_HOT.id), v),
            20 // hot_floor
        );
        assert!(
            DAMAGE_TYPE_SYNC_REMAP_V_26_3_TO_V_1_21_11
                .iter()
                .all(|&id| id < 50)
        );
    }
}
