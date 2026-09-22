//! 对插件注册的自定义伤害类型的运行时支持。
//!
//! 原版伤害类型位于生成的静态表中
//! （`generated/damage_type.rs`，内置 26.3 数据集中 id 为 0..=50）。
//! 插件可以在服务器启动期间注册额外的伤害类型；
//! 它们会被追加到同步给
//! 客户端，因此给定客户端版本下自定义条目的网络 id 就是该
//! 版本的原版条目数加上该条目的注册索引。这样，
//! 模块提供：
//!
//! - [`CustomDamageType`]：已注册自定义伤害类型的运行时描述
//!   伤害类型（其字段与原版注册表条目结构一致），
//! - [`ResolvedDamageType`]：统一的“原版或自定义”表示
//!   供伤害执行路径使用，
//! - 伤害事件数据包的 id 转换辅助函数：原版 id 走
//!   通过静态重映射表转换，而自定义 id 则按该
//!   内置数据集与客户端版本之间的原版条目数差异
//!   客户端版本。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, PoisonError};

use papokin_nbt::Nbt;
use papokin_nbt::compound::NbtCompound;
use papokin_util::version::JavaMinecraftVersion;

use crate::damage::{DamageEffects, DamageScaling, DamageType, DeathMessageType};
use crate::registry::Registry;
use crate::sync_id_remap::remap_damage_type_id_for_version;
use crate::tag::{Tag, Taggable};

/// 伤害类型注册表的同步注册表 ID。
const DAMAGE_TYPE_REGISTRY_ID: &str = "minecraft:damage_type";

/// 插件注册的自定义伤害类型。
///
/// 这些字段沿用原版 `damage_type` 注册表条目的结构；
/// `network_id` 是条目在服务器原生（内置数据集）id
/// 空间：原版原生条目数加上注册索引。每个
/// 客户端版本，id 会随该版本的原版条目数偏移；参见
/// [`translate_damage_type_id_for_version`]。
#[derive(Clone, Debug, PartialEq)]
pub struct CustomDamageType {
    /// 条目注册时使用的带命名空间 id，例如 "`myplugin:frost`"。
    pub name: String,
    /// 死亡消息 ID；死亡消息翻译为 `death.attack.<message_id>`。
    pub message_id: String,
    /// 伤害量随难度缩放的情况。
    pub scaling: DamageScaling,
    /// 施加给受此伤害玩家的饥饿消耗。
    pub exhaustion: f32,
    /// 可选的受伤音效/视觉效果覆盖。
    pub effects: Option<DamageEffects>,
    /// 死亡消息的构成方式。
    pub death_message_type: DeathMessageType,
    /// 原生数据集 ID 空间中的 ID（原版原生数量 + 注册索引）。
    pub network_id: u16,
}

impl CustomDamageType {
    /// 序列化同步到客户端的注册表条目负载，使其与
    /// 原版 `damage_type` 条目的字段布局：`scaling`、`message_id`
    /// 以及 `exhaustion`（始终发送），`effects`/`death_message_type` 仅当
    /// 与编解码器默认值不同。该数据块是无名的网络 NBT
    /// 复合标签；与内置的原版数据块一样，`exhaustion` 是 double
    /// (客户端编解码器会将其收窄回浮点数)。
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

    /// 解析由 [`CustomDamageType::nbt_blob`] 生成的注册表条目二进制块
    /// (或原版条目的 blob) 转换回 [`CustomDamageType`]。返回
    /// 当该数据不是有效的伤害类型条目时返回 `None`。
    #[must_use]
    pub fn from_nbt(name: String, network_id: u16, blob: &[u8]) -> Option<Self> {
        let mut reader =
            papokin_nbt::deserializer::NbtReadHelperJava::new(std::io::Cursor::new(blob));
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

/// 一种伤害类型，从原版 ID 或（自定义）名称解析而来：可以是其中之一
/// 51 种静态原版伤害类型之一，或是插件注册的自定义类型。此
/// 是伤害执行路径所操作的表示形式；原版类型
/// 保留其静态表行为（标签、同一性），而自定义类型则携带
/// 其各自注册的字段，并对每个标签查询返回 `false`（自定义
/// 条目不属于任何原版标签）。
#[derive(Clone, Debug, PartialEq)]
pub enum ResolvedDamageType {
    Vanilla(DamageType),
    Custom(CustomDamageType),
}

impl ResolvedDamageType {
    /// 原版伤害类型（当此伤害属于原版类型时）。
    #[must_use]
    pub const fn vanilla(&self) -> Option<DamageType> {
        match self {
            Self::Vanilla(vanilla) => Some(*vanilla),
            Self::Custom(_) => None,
        }
    }

    /// 原版伤害类型，自定义类型则为 `fallback`。适用于
    /// 消费方仍需要一个静态的 [`DamageType`]。
    #[must_use]
    pub fn vanilla_or(&self, fallback: DamageType) -> DamageType {
        self.vanilla().unwrap_or(fallback)
    }

    /// 这是否恰好为给定的原版伤害类型。
    #[must_use]
    pub fn is(&self, vanilla: DamageType) -> bool {
        matches!(self, Self::Vanilla(v) if *v == vanilla)
    }

    /// 死亡消息 ID；死亡消息翻译为 `death.attack.<message_id>`。
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

    /// 原生（内置数据集）ID 空间中的 ID。需按客户端转换
    /// 针对其他版本，请使用 [`translate_damage_type_id_for_version`]。
    #[must_use]
    pub const fn network_id(&self) -> u16 {
        match self {
            Self::Vanilla(vanilla) => vanilla.id as u16,
            Self::Custom(custom) => custom.network_id,
        }
    }

    /// 自定义伤害类型的命名空间注册名（如果这是自定义伤害类型）。
    #[must_use]
    pub fn custom_name(&self) -> Option<&str> {
        match self {
            Self::Vanilla(_) => None,
            Self::Custom(custom) => Some(&custom.name),
        }
    }

    /// 该伤害类型是否位于给定标签中。原版类型会查询
    /// 静态标签表；自定义类型绝不会被加标签。
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

/// [`DamageScaling`] 的原版注册表值。
#[must_use]
pub const fn scaling_name(scaling: DamageScaling) -> &'static str {
    match scaling {
        DamageScaling::Never => "never",
        DamageScaling::WhenCausedByLivingNonPlayer => "when_caused_by_living_non_player",
        DamageScaling::Always => "always",
    }
}

/// [`DamageEffects`] 的原版注册表值。
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

/// [`DeathMessageType`] 的原版注册表值。
#[must_use]
pub const fn death_message_type_name(death_message_type: DeathMessageType) -> &'static str {
    match death_message_type {
        DeathMessageType::Default => "default",
        DeathMessageType::FallVariants => "fall_variants",
        DeathMessageType::IntentionalGameDesign => "intentional_game_design",
    }
}

/// 解析原版 `scaling` 注册表值。
#[must_use]
pub fn scaling_from_name(name: &str) -> Option<DamageScaling> {
    match name {
        "never" => Some(DamageScaling::Never),
        "when_caused_by_living_non_player" => Some(DamageScaling::WhenCausedByLivingNonPlayer),
        "always" => Some(DamageScaling::Always),
        _ => None,
    }
}

/// 解析原版 `effects` 注册表值。
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

/// 解析原版 `death_message_type` 注册表值。
#[must_use]
pub fn death_message_type_from_name(name: &str) -> Option<DeathMessageType> {
    match name {
        "default" => Some(DeathMessageType::Default),
        "fall_variants" => Some(DeathMessageType::FallVariants),
        "intentional_game_design" => Some(DeathMessageType::IntentionalGameDesign),
        _ => None,
    }
}

/// 为自定义伤害类型构建同步到客户端的网络 NBT 负载，
/// 镜像 `damage_type` 中原版条目的字段布局，
/// 注册表：`scaling`、`message_id` 和 `exhaustion`（双精度浮点数，与
/// 内置原版数据块）始终包含，`effects`/`death_message_type` 仅当
/// 它们与编解码器默认值不同的地方。结果是一个无名的网络 NBT
/// 复合标签。
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

/// 内置（原生）数据集中的伤害类型条目数量，即
/// 原生 id 空间中可供自定义条目使用的第一个网络 id。
/// 已缓存；该静态表在运行时从不改变。
#[must_use]
pub fn native_damage_type_count() -> u16 {
    static NATIVE_COUNT: OnceLock<u16> = OnceLock::new();
    *NATIVE_COUNT.get_or_init(|| {
        // 缺失的表会使每个 id 看起来都是自定义的；`u16::MAX` 保持
        // 使该转换在这种情况下成为纯粹的原版重映射。
        damage_type_entry_count(JavaMinecraftVersion::V_26_3).unwrap_or(u16::MAX)
    })
}

/// `version` 的客户端在……中收到的原版伤害类型条目数
/// 同步注册表数据，即自定义条目可用的第一个网络 id。
/// 条目。当该版本没有已同步的伤害类型
/// 注册表。计数按协议版本缓存。
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
    // 竞争中的线程可能已缓存同一版本；静态
    // 表无论哪种方式都相同，因此覆盖是无害的。
    cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(key, count);
    count
}

/// 将伤害类型 ID 转换为服务器原生（内置数据集）的 ID
/// 空间映射到 `version` 客户端实际收到的 id 空间。
///
/// 原版 id 走静态的按版本重映射表。自定义 id
/// (>= [`native_damage_type_count`]) 的条目会按原版
/// 条目数，因为客户端的自定义条目紧接在*其*
/// 原版条目。从未收到自定义条目的客户端（它们的
/// 注册表不会通过配置状态同步）获得
/// 版本的 `generic` ID 作为无害的替身。
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

/// `version` 同步表中伤害类型注册表的条目数量。
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
        // 随附的 26.3 数据集包含全部 51 种原版伤害类型（id 0..=50）。
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
                .expect("这些版本会同步 damage_type 注册表");
            // 第一个和第三个注册的自定义条目
            assert_eq!(
                translate_damage_type_id_for_version(native_count, version),
                vanilla_count
            );
            assert_eq!(
                translate_damage_type_id_for_version(native_count + 2, version),
                vanilla_count + 2
            );
        }
        // 在原生 id 空间中，转换是恒等映射。
        assert_eq!(
            translate_damage_type_id_for_version(native_count + 5, JavaMinecraftVersion::V_26_3),
            native_count + 5
        );
    }

    #[test]
    fn custom_ids_degrade_to_generic_for_unsynced_versions() {
        let native_count = native_damage_type_count();
        // 1.20/1.20.1 客户端通过登录流程接收其注册表
        // codec，其中不含自定义条目。
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
            .expect("26.3 数据表包含 arrow 伤害类型条目");
        // 原版 `arrow`：scaling = when_caused_by_living_non_player，
        // message_id = "arrow"，exhaustion = 0.1，无效果，默认死亡
        // 消息类型。各条目的字段顺序不同（NBT 复合标签
        // 无序），因此比较解析后的字段而非字节。
        let blob = custom_damage_type_nbt(
            "arrow",
            DamageScaling::WhenCausedByLivingNonPlayer,
            0.1,
            None,
            DeathMessageType::Default,
        );
        let vanilla = CustomDamageType::from_nbt("minecraft:arrow".to_string(), 0, &arrow_blob)
            .expect("原版数据块可解析");
        let custom = CustomDamageType::from_nbt("minecraft:arrow".to_string(), 0, &blob)
            .expect("自定义二进制大对象应能解析");
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
            .expect("26.3 数据表包含 in_fire 伤害类型条目");
        let blob = custom_damage_type_nbt(
            "inFire",
            DamageScaling::WhenCausedByLivingNonPlayer,
            0.1,
            Some(DamageEffects::Burning),
            DeathMessageType::Default,
        );
        // 原版各条目的字段顺序不同，因此比较解析后的
        // 字段而非字节。
        let vanilla =
            CustomDamageType::from_nbt("minecraft:in_fire".to_string(), 21, &in_fire_blob)
                .expect("原版数据块可解析");
        let custom = CustomDamageType::from_nbt("minecraft:in_fire".to_string(), 21, &blob)
            .expect("自定义二进制大对象应能解析");
        assert_eq!(vanilla, custom);
    }

    #[test]
    fn every_vanilla_blob_parses_back_to_its_static_entry() {
        let registries = Registry::get_synced(JavaMinecraftVersion::V_26_3);
        let registry = registries
            .iter()
            .find(|reg| reg.registry_id == DAMAGE_TYPE_REGISTRY_ID)
            .expect("26.3 表中应当有 damage_type 注册表");
        assert_eq!(registry.registry_entries.len(), 51);
        for entry in &registry.registry_entries {
            let path = entry.entry_id.strip_prefix("minecraft:").unwrap();
            let static_type = DamageType::from_name(path).expect("条目应当是原版类型");
            let blob = entry.data.as_ref().expect("原版条目应当带有数据");
            let parsed = CustomDamageType::from_nbt(entry.entry_id.clone(), 0, blob)
                .unwrap_or_else(|| panic!("{path} 的 blob 应当能解析"));
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
        // 自定义类型永远不会是原版标签的一部分。
        assert!(!custom.has_tag(&crate::tag::DamageType::MINECRAFT_IS_FIRE));
        assert!(!custom.has_tag(&crate::tag::DamageType::MINECRAFT_IS_FREEZING));
    }
}
