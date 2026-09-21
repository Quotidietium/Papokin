//! Runtime support for plugin-registered custom damage types.
//!
//! Vanilla damage types live in the generated static table
//! (`generated/damage_type.rs`, ids 0..=50 in the bundled 26.3 dataset).
//! Plugins can register additional damage types while the server starts up;
//! they are appended to the `minecraft:damage_type` registry synced to
//! clients, so a custom entry's network id for a given client version is that
//! version's vanilla entry count plus the entry's registration index. This
//! module provides:
//!
//! - [`CustomDamageType`]: the runtime description of a registered custom
//!   damage type (its fields mirror the vanilla registry entry structure),
//! - [`ResolvedDamageType`]: the unified "vanilla or custom" representation
//!   consumed by the damage execution path,
//! - id translation helpers for the damage event packet: vanilla ids go
//!   through the static remap tables, while custom ids are shifted by the
//!   difference in vanilla entry counts between the bundled dataset and the
//!   client version.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, PoisonError};

use pumpkin_nbt::Nbt;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::version::JavaMinecraftVersion;

use crate::damage::{DamageEffects, DamageScaling, DamageType, DeathMessageType};
use crate::registry::Registry;
use crate::sync_id_remap::remap_damage_type_id_for_version;
use crate::tag::{Tag, Taggable};

/// The synced registry id of the damage type registry.
const DAMAGE_TYPE_REGISTRY_ID: &str = "minecraft:damage_type";

/// A plugin-registered custom damage type.
///
/// The fields mirror the vanilla `damage_type` registry entry structure; the
/// `network_id` is the entry's id in the server's native (bundled dataset) id
/// space: the native vanilla entry count plus the registration index. Per
/// client version, the id shifts with the version's vanilla entry count; see
/// [`translate_damage_type_id_for_version`].
#[derive(Clone, Debug, PartialEq)]
pub struct CustomDamageType {
    /// Namespaced id the entry was registered under, e.g. "`myplugin:frost`".
    pub name: String,
    /// Death message id; death messages translate as `death.attack.<message_id>`.
    pub message_id: String,
    /// When the damage amount scales with difficulty.
    pub scaling: DamageScaling,
    /// Exhaustion applied to players taking this damage.
    pub exhaustion: f32,
    /// Optional hurt sound/visual effect override.
    pub effects: Option<DamageEffects>,
    /// How the death message is composed.
    pub death_message_type: DeathMessageType,
    /// Id in the native dataset id space (native vanilla count + registration index).
    pub network_id: u16,
}

impl CustomDamageType {
    /// Serializes the registry entry payload synced to clients, mirroring the
    /// field layout of vanilla `damage_type` entries: `scaling`, `message_id`
    /// and `exhaustion` always, `effects`/`death_message_type` only when they
    /// differ from the codec defaults. The blob is a nameless network NBT
    /// compound; like the bundled vanilla blobs, `exhaustion` is a double
    /// (the client codec narrows it back to a float).
    #[must_use]
    pub fn nbt_blob(&self) -> Vec<u8> {
        custom_damage_type_nbt(
            &self.message_id,
            self.scaling,
            self.exhaustion,
            self.effects,
            self.death_message_type,
        )
    }

    /// Parses a registry entry blob produced by [`CustomDamageType::nbt_blob`]
    /// (or a vanilla entry blob) back into a [`CustomDamageType`]. Returns
    /// `None` when the blob is not a valid damage type entry.
    #[must_use]
    pub fn from_nbt(name: String, network_id: u16, blob: &[u8]) -> Option<Self> {
        let mut reader =
            pumpkin_nbt::deserializer::NbtReadHelperJava::new(std::io::Cursor::new(blob));
        let nbt = Nbt::read_unnamed(&mut reader).ok()?;
        let message_id = nbt.get_string("message_id")?.to_string();
        let scaling = scaling_from_name(nbt.get_string("scaling")?)?;
        let exhaustion = nbt.get_double("exhaustion")? as f32;
        let effects = nbt.get_string("effects").and_then(effects_from_name);
        let death_message_type = nbt
            .get_string("death_message_type")
            .and_then(death_message_type_from_name)
            .unwrap_or(DeathMessageType::Default);
        Some(Self {
            name,
            message_id,
            scaling,
            exhaustion,
            effects,
            death_message_type,
            network_id,
        })
    }
}

/// A damage type resolved from a vanilla id or a (custom) name: either one of
/// the 51 static vanilla damage types or a plugin-registered custom one. This
/// is the representation the damage execution path works on; vanilla types
/// keep their static-table behavior (tags, identity) while custom types carry
/// their registered fields and report `false` for every tag query (custom
/// entries are not part of any vanilla tag).
#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedDamageType {
    Vanilla(DamageType),
    Custom(CustomDamageType),
}

impl ResolvedDamageType {
    /// The vanilla damage type, when this is one.
    #[must_use]
    pub const fn vanilla(&self) -> Option<DamageType> {
        match self {
            Self::Vanilla(vanilla) => Some(*vanilla),
            Self::Custom(_) => None,
        }
    }

    /// The vanilla damage type, or `fallback` for custom types. Used where a
    /// consumer still requires a static [`DamageType`].
    #[must_use]
    pub fn vanilla_or(&self, fallback: DamageType) -> DamageType {
        self.vanilla().unwrap_or(fallback)
    }

    /// Whether this is exactly the given vanilla damage type.
    #[must_use]
    pub fn is(&self, vanilla: DamageType) -> bool {
        matches!(self, Self::Vanilla(v) if *v == vanilla)
    }

    /// Death message id; death messages translate as `death.attack.<message_id>`.
    #[must_use]
    pub fn message_id(&self) -> &str {
        match self {
            Self::Vanilla(vanilla) => vanilla.message_id,
            Self::Custom(custom) => &custom.message_id,
        }
    }

    #[must_use]
    pub fn exhaustion(&self) -> f32 {
        match self {
            Self::Vanilla(vanilla) => vanilla.exhaustion,
            Self::Custom(custom) => custom.exhaustion,
        }
    }

    #[must_use]
    pub const fn scaling(&self) -> DamageScaling {
        match self {
            Self::Vanilla(vanilla) => vanilla.scaling,
            Self::Custom(custom) => custom.scaling,
        }
    }

    #[must_use]
    pub const fn effects(&self) -> Option<DamageEffects> {
        match self {
            Self::Vanilla(vanilla) => vanilla.effects,
            Self::Custom(custom) => custom.effects,
        }
    }

    #[must_use]
    pub const fn death_message_type(&self) -> DeathMessageType {
        match self {
            Self::Vanilla(vanilla) => vanilla.death_message_type,
            Self::Custom(custom) => custom.death_message_type,
        }
    }

    /// Id in the native (bundled dataset) id space. Translate per client
    /// version with [`translate_damage_type_id_for_version`].
    #[must_use]
    pub const fn network_id(&self) -> u16 {
        match self {
            Self::Vanilla(vanilla) => vanilla.id as u16,
            Self::Custom(custom) => custom.network_id,
        }
    }

    /// The namespaced registration name of a custom damage type, if this is one.
    #[must_use]
    pub fn custom_name(&self) -> Option<&str> {
        match self {
            Self::Vanilla(_) => None,
            Self::Custom(custom) => Some(&custom.name),
        }
    }

    /// Whether the damage type is in the given tag. Vanilla types consult the
    /// static tag tables; custom types are never tagged.
    #[must_use]
    pub fn has_tag(&self, tag: &'static Tag) -> bool {
        self.vanilla().is_some_and(|v| v.has_tag(tag))
    }
}

impl From<DamageType> for ResolvedDamageType {
    fn from(vanilla: DamageType) -> Self {
        Self::Vanilla(vanilla)
    }
}

/// Vanilla registry value of a [`DamageScaling`].
#[must_use]
pub const fn scaling_name(scaling: DamageScaling) -> &'static str {
    match scaling {
        DamageScaling::Never => "never",
        DamageScaling::WhenCausedByLivingNonPlayer => "when_caused_by_living_non_player",
        DamageScaling::Always => "always",
    }
}

/// Vanilla registry value of a [`DamageEffects`].
#[must_use]
pub const fn effects_name(effects: DamageEffects) -> &'static str {
    match effects {
        DamageEffects::Hurt => "hurt",
        DamageEffects::Thorns => "thorns",
        DamageEffects::Drowning => "drowning",
        DamageEffects::Burning => "burning",
        DamageEffects::Poking => "poking",
        DamageEffects::Freezing => "freezing",
    }
}

/// Vanilla registry value of a [`DeathMessageType`].
#[must_use]
pub const fn death_message_type_name(death_message_type: DeathMessageType) -> &'static str {
    match death_message_type {
        DeathMessageType::Default => "default",
        DeathMessageType::FallVariants => "fall_variants",
        DeathMessageType::IntentionalGameDesign => "intentional_game_design",
    }
}

/// Parses a vanilla `scaling` registry value.
#[must_use]
pub fn scaling_from_name(name: &str) -> Option<DamageScaling> {
    match name {
        "never" => Some(DamageScaling::Never),
        "when_caused_by_living_non_player" => Some(DamageScaling::WhenCausedByLivingNonPlayer),
        "always" => Some(DamageScaling::Always),
        _ => None,
    }
}

/// Parses a vanilla `effects` registry value.
#[must_use]
pub fn effects_from_name(name: &str) -> Option<DamageEffects> {
    match name {
        "hurt" => Some(DamageEffects::Hurt),
        "thorns" => Some(DamageEffects::Thorns),
        "drowning" => Some(DamageEffects::Drowning),
        "burning" => Some(DamageEffects::Burning),
        "poking" => Some(DamageEffects::Poking),
        "freezing" => Some(DamageEffects::Freezing),
        _ => None,
    }
}

/// Parses a vanilla `death_message_type` registry value.
#[must_use]
pub fn death_message_type_from_name(name: &str) -> Option<DeathMessageType> {
    match name {
        "default" => Some(DeathMessageType::Default),
        "fall_variants" => Some(DeathMessageType::FallVariants),
        "intentional_game_design" => Some(DeathMessageType::IntentionalGameDesign),
        _ => None,
    }
}

/// Builds the network NBT payload synced to clients for a custom damage type,
/// mirroring the field layout of vanilla entries in the `damage_type`
/// registry: `scaling`, `message_id` and `exhaustion` (a double, matching the
/// bundled vanilla blobs) always, `effects`/`death_message_type` only when
/// they differ from the codec defaults. The result is a nameless network NBT
/// compound.
#[must_use]
pub fn custom_damage_type_nbt(
    message_id: &str,
    scaling: DamageScaling,
    exhaustion: f32,
    effects: Option<DamageEffects>,
    death_message_type: DeathMessageType,
) -> Vec<u8> {
    let mut nbt = NbtCompound::new();
    nbt.put_string("scaling", scaling_name(scaling).to_string());
    nbt.put_string("message_id", message_id.to_string());
    nbt.put_double("exhaustion", f64::from(exhaustion));
    if let Some(effects) = effects {
        nbt.put_string("effects", effects_name(effects).to_string());
    }
    if death_message_type != DeathMessageType::Default {
        nbt.put_string(
            "death_message_type",
            death_message_type_name(death_message_type).to_string(),
        );
    }
    Nbt::from(nbt).write_unnamed().to_vec()
}

/// Number of damage type entries in the bundled (native) dataset, i.e. the
/// first network id available to custom entries in the native id space.
/// Cached; the static table never changes at runtime.
#[must_use]
pub fn native_damage_type_count() -> u16 {
    static NATIVE_COUNT: OnceLock<u16> = OnceLock::new();
    *NATIVE_COUNT.get_or_init(|| {
        // A missing table would make every id look custom; `u16::MAX` keeps
        // the translation a pure vanilla remap in that case.
        damage_type_entry_count(JavaMinecraftVersion::V_26_3).unwrap_or(u16::MAX)
    })
}

/// Vanilla damage type entry count a client of `version` receives in the
/// synced registry data, i.e. the first network id available to custom
/// entries for that client. `None` when the version has no synced damage type
/// registry. Counts are cached per protocol version.
#[must_use]
pub fn damage_type_vanilla_count_for_version(version: JavaMinecraftVersion) -> Option<u16> {
    static VERSION_COUNTS: OnceLock<Mutex<HashMap<i32, Option<u16>>>> = OnceLock::new();
    let cache = VERSION_COUNTS.get_or_init(|| Mutex::new(HashMap::new()));
    let key = version.protocol_version();
    {
        let cache = cache.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(count) = cache.get(&key) {
            return *count;
        }
    }
    let count = damage_type_entry_count(version);
    // A racing thread may have cached the same version already; the static
    // tables are identical either way, so overwriting is harmless.
    cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(key, count);
    count
}

/// Translates a damage type id in the server's native (bundled dataset) id
/// space to the id space a client of `version` actually received.
///
/// Vanilla ids go through the static per-version remap tables. Custom ids
/// (>= [`native_damage_type_count`]) are shifted by the difference in vanilla
/// entry counts, because the client's custom entries start right after *its*
/// vanilla entries. Clients that never received the custom entries (their
/// registries are not synced through the configuration state) get the
/// version's `generic` id as a harmless stand-in.
#[must_use]
pub fn translate_damage_type_id_for_version(id: u16, version: JavaMinecraftVersion) -> u16 {
    let native_count = native_damage_type_count();
    if id < native_count {
        return remap_damage_type_id_for_version(id, version);
    }
    if version.supports_configuration_state()
        && let Some(count) = damage_type_vanilla_count_for_version(version)
    {
        return count.saturating_add(id - native_count);
    }
    remap_damage_type_id_for_version(u16::from(DamageType::GENERIC.id), version)
}

/// Entry count of the damage type registry in the synced table of `version`.
fn damage_type_entry_count(version: JavaMinecraftVersion) -> Option<u16> {
    Registry::get_synced(version)
        .iter()
        .find(|reg| reg.registry_id == DAMAGE_TYPE_REGISTRY_ID)
        .map(|reg| u16::try_from(reg.registry_entries.len()).unwrap_or(u16::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_count_matches_static_table() {
        // The bundled 26.3 dataset ships the 51 vanilla damage types (ids 0..=50).
        assert_eq!(native_damage_type_count(), 51);
        assert_eq!(u16::from(DamageType::WITHER_SKULL.id), 50);
    }

    #[test]
    fn vanilla_ids_translate_like_the_static_remap() {
        for version in [
            JavaMinecraftVersion::V_1_20_2,
            JavaMinecraftVersion::V_1_21,
            JavaMinecraftVersion::V_1_21_11,
            JavaMinecraftVersion::V_26_3,
        ] {
            for id in 0..native_damage_type_count() {
                assert_eq!(
                    translate_damage_type_id_for_version(id, version),
                    remap_damage_type_id_for_version(id, version),
                    "vanilla id {id} must pass through the static remap"
                );
            }
        }
    }

    #[test]
    fn custom_ids_shift_with_the_version_vanilla_count() {
        let native_count = native_damage_type_count();
        for version in [
            JavaMinecraftVersion::V_1_20_2,
            JavaMinecraftVersion::V_1_21,
            JavaMinecraftVersion::V_1_21_11,
            JavaMinecraftVersion::V_26_3,
        ] {
            let vanilla_count = damage_type_vanilla_count_for_version(version)
                .expect("these versions sync a damage_type registry");
            // First and third registered custom entries.
            assert_eq!(
                translate_damage_type_id_for_version(native_count, version),
                vanilla_count
            );
            assert_eq!(
                translate_damage_type_id_for_version(native_count + 2, version),
                vanilla_count + 2
            );
        }
        // In the native id space the translation is the identity.
        assert_eq!(
            translate_damage_type_id_for_version(native_count + 5, JavaMinecraftVersion::V_26_3),
            native_count + 5
        );
    }

    #[test]
    fn custom_ids_degrade_to_generic_for_unsynced_versions() {
        let native_count = native_damage_type_count();
        // 1.20/1.20.1 clients receive their registries through the login
        // codec, which carries no custom entries.
        assert_eq!(
            translate_damage_type_id_for_version(native_count, JavaMinecraftVersion::V_1_20),
            remap_damage_type_id_for_version(
                u16::from(DamageType::GENERIC.id),
                JavaMinecraftVersion::V_1_20
            )
        );
    }

    #[test]
    fn blob_field_layout_matches_vanilla_arrow_entry() {
        let arrow_blob = Registry::get_synced(JavaMinecraftVersion::V_26_3)
            .iter()
            .find(|reg| reg.registry_id == DAMAGE_TYPE_REGISTRY_ID)
            .and_then(|reg| {
                reg.registry_entries
                    .iter()
                    .find(|entry| entry.entry_id == "minecraft:arrow")
            })
            .and_then(|entry| entry.data.clone())
            .expect("the 26.3 table has an arrow damage type entry");
        // Vanilla `arrow`: scaling = when_caused_by_living_non_player,
        // message_id = "arrow", exhaustion = 0.1, no effects, default death
        // message type. Field order differs between entries (NBT compounds
        // are unordered), so compare parsed fields rather than bytes.
        let blob = custom_damage_type_nbt(
            "arrow",
            DamageScaling::WhenCausedByLivingNonPlayer,
            0.1,
            None,
            DeathMessageType::Default,
        );
        let vanilla = CustomDamageType::from_nbt("minecraft:arrow".to_string(), 0, &arrow_blob)
            .expect("vanilla blob parses");
        let custom = CustomDamageType::from_nbt("minecraft:arrow".to_string(), 0, &blob)
            .expect("custom blob parses");
        assert_eq!(vanilla, custom);
    }

    #[test]
    fn blob_field_layout_matches_vanilla_in_fire_entry() {
        let in_fire_blob = Registry::get_synced(JavaMinecraftVersion::V_26_3)
            .iter()
            .find(|reg| reg.registry_id == DAMAGE_TYPE_REGISTRY_ID)
            .and_then(|reg| {
                reg.registry_entries
                    .iter()
                    .find(|entry| entry.entry_id == "minecraft:in_fire")
            })
            .and_then(|entry| entry.data.clone())
            .expect("the 26.3 table has an in_fire damage type entry");
        let blob = custom_damage_type_nbt(
            "inFire",
            DamageScaling::WhenCausedByLivingNonPlayer,
            0.1,
            Some(DamageEffects::Burning),
            DeathMessageType::Default,
        );
        // Field order differs between vanilla entries, so compare parsed
        // fields rather than bytes.
        let vanilla =
            CustomDamageType::from_nbt("minecraft:in_fire".to_string(), 21, &in_fire_blob)
                .expect("vanilla blob parses");
        let custom = CustomDamageType::from_nbt("minecraft:in_fire".to_string(), 21, &blob)
            .expect("custom blob parses");
        assert_eq!(vanilla, custom);
    }

    #[test]
    fn every_vanilla_blob_parses_back_to_its_static_entry() {
        let registries = Registry::get_synced(JavaMinecraftVersion::V_26_3);
        let registry = registries
            .iter()
            .find(|reg| reg.registry_id == DAMAGE_TYPE_REGISTRY_ID)
            .expect("the 26.3 table has a damage_type registry");
        assert_eq!(registry.registry_entries.len(), 51);
        for entry in &registry.registry_entries {
            let path = entry.entry_id.strip_prefix("minecraft:").unwrap();
            let static_type = DamageType::from_name(path).expect("entry is a vanilla type");
            let blob = entry.data.as_ref().expect("vanilla entries carry data");
            let parsed = CustomDamageType::from_nbt(entry.entry_id.clone(), 0, blob)
                .unwrap_or_else(|| panic!("blob of {path} parses"));
            assert_eq!(
                parsed.message_id, static_type.message_id,
                "{path} message_id"
            );
            assert!(
                (parsed.exhaustion - static_type.exhaustion).abs() < f32::EPSILON,
                "{path} exhaustion"
            );
            assert_eq!(parsed.scaling, static_type.scaling, "{path} scaling");
            assert_eq!(parsed.effects, static_type.effects, "{path} effects");
            assert_eq!(
                parsed.death_message_type, static_type.death_message_type,
                "{path} death_message_type"
            );
        }
    }

    #[test]
    fn blob_roundtrip_preserves_all_fields() {
        let custom = CustomDamageType {
            name: "myplugin:frost".to_string(),
            message_id: "frost".to_string(),
            scaling: DamageScaling::Always,
            exhaustion: 0.25,
            effects: Some(DamageEffects::Freezing),
            death_message_type: DeathMessageType::FallVariants,
            network_id: 51,
        };
        let blob = custom.nbt_blob();
        let parsed =
            CustomDamageType::from_nbt(custom.name.clone(), custom.network_id, &blob).unwrap();
        assert_eq!(parsed, custom);
    }

    #[test]
    fn resolved_accessors_cover_both_variants() {
        let vanilla = ResolvedDamageType::Vanilla(DamageType::FREEZE);
        assert_eq!(vanilla.message_id(), "freeze");
        assert_eq!(vanilla.network_id(), u16::from(DamageType::FREEZE.id));
        assert!(vanilla.is(DamageType::FREEZE));
        assert!(!vanilla.is(DamageType::DROWN));
        assert_eq!(vanilla.vanilla(), Some(DamageType::FREEZE));
        assert_eq!(vanilla.custom_name(), None);
        assert!(
            vanilla.has_tag(&crate::tag::DamageType::MINECRAFT_IS_FREEZING)
                || !vanilla.has_tag(&crate::tag::DamageType::MINECRAFT_IS_FIRE)
        );

        let custom = ResolvedDamageType::Custom(CustomDamageType {
            name: "myplugin:frost".to_string(),
            message_id: "frost".to_string(),
            scaling: DamageScaling::Never,
            exhaustion: 0.5,
            effects: None,
            death_message_type: DeathMessageType::Default,
            network_id: 51,
        });
        assert_eq!(custom.message_id(), "frost");
        assert_eq!(custom.network_id(), 51);
        assert!(!custom.is(DamageType::FREEZE));
        assert_eq!(custom.vanilla(), None);
        assert_eq!(custom.vanilla_or(DamageType::GENERIC), DamageType::GENERIC);
        assert_eq!(custom.custom_name(), Some("myplugin:frost"));
        assert_eq!(custom.exhaustion(), 0.5);
        // Custom types are never part of a vanilla tag.
        assert!(!custom.has_tag(&crate::tag::DamageType::MINECRAFT_IS_FIRE));
        assert!(!custom.has_tag(&crate::tag::DamageType::MINECRAFT_IS_FREEZING));
    }
}
