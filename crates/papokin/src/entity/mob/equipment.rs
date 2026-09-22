//! 生物生成装备系统。
//!
//! 本模块处理生物生成时的自动装备穿戴，与
//! 原版 Minecraft 的 `populateDefaultEquipmentSlots` 与
//! `populateDefaultEquipmentEnchantments` 的行为。其特性包括：
//!
//! - 数据驱动的 `EQUIPMENT_REGISTRY`，将 13 种生物类型映射到其武器/盔甲
//!   配置。
//! - 精确复刻原版 `RegionalDifficulty` 计算（游戏时间、区块驻留时间、
//!   月相）。
//! - 带互斥集冲突解决的加权附魔选择，以及
//!   基于消耗的等级确定。
//! - 死亡时各槽位带有抢夺加成的掉落概率。
//!
//! 注册表中未列出的生物生成时不带装备，与原版行为一致
//! （并非所有生物类型都有装备定义）。

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::LazyLock;

use papokin_data::AttributeModifierSlot;
use papokin_data::Enchantment;
use papokin_data::attributes::Attributes;
use papokin_data::data_component_impl::{
    AttributeModifiersImpl, CustomNameImpl, EnchantmentsImpl, EquipmentSlot, EquipmentType,
    EquippableImpl, IDSet, Operation,
};
use papokin_data::entity::EntityType;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_data::tag::{Tag, Taggable};
use papokin_util::difficulty::Difficulty;
use papokin_util::math::vector3::Vector3;
use rand::RngExt;

use crate::entity::EntityBase;
use crate::entity::mob::{Mob, MobEntity};

// ══════════════════════════════════════════════════════════════════
// 从原版 Minecraft 26.2 提取的全局常量
// 来源：Mob.java、DifficultyInstance.java、DropChances.java、
// EnchantmentsByCostWithDifficulty.java
// ══════════════════════════════════════════════════════════════════

/// 生物穿戴盔甲的基础概率（经 `specialMultiplier` 缩放之前）。
/// 来自原版 `Mob.MAX_WEARING_ARMOR_CHANCE`。
pub const WEARING_ARMOR_CHANCE: f32 = 0.15;

/// 每次尝试将盔甲档次提升到下一材料等级的概率。
/// 来自原版 `Mob.WEARING_ARMOR_UPGRADE_MATERIAL_CHANCE`。
pub const ARMOR_UPGRADE_MATERIAL_CHANCE: f32 = 0.1087;

/// 盔甲等级选择的升级尝试最大次数。
/// 来自原版 `Mob.WEARING_ARMOR_UPGRADE_MATERIAL_ATTEMPTS`。
pub const ARMOR_UPGRADE_MATERIAL_ATTEMPTS: f32 = 3.0;

/// 生物死亡时装备的默认每槽位掉落几率。
/// 来自原版 `Mob.DEFAULT_EQUIPMENT_DROP_CHANCE`。
pub const DEFAULT_EQUIPMENT_DROP_CHANCE: f32 = 0.085;

/// 生成时武器获得附魔的基础概率（`specialMultiplier` 之前）。
/// 来自原版 `Mob.MAX_ENCHANTED_WEAPON_CHANCE`。
pub const WEAPON_ENCHANT_CHANCE: f32 = 0.25;

/// 生成时盔甲获得附魔的基础概率（`specialMultiplier` 之前）。
/// 来自原版 `Mob.MAX_ENCHANTED_ARMOR_CHANCE`。
pub const ARMOR_ENCHANT_CHANCE: f32 = 0.5;

/// 生物生成装备的最低附魔成本。
/// 来自原版 `mob_spawn_equipment.json`。
pub const MOB_SPAWN_ENCHANT_MIN_COST: i32 = 5;

/// 加到最小值上的成本跨度，按 `specialMultiplier` 缩放。
/// 来自原版 `mob_spawn_equipment.json`。
pub const MOB_SPAWN_ENCHANT_COST_SPAN: i32 = 17;

// ══════════════════════════════════════════════════════════════════
// 盔甲层级 —— 与原版 Mob.getEquipmentForSlot() 完全一致
// 原版近似：盔甲类型选择（基础 0-2 加上 3 个升级
// 按原版 Mob.populateDefaultEquipmentSlots 以 10.87% 概率尝试）以及
// 部分盔甲几率（困难难度为 0.1 / 其他为 0.25）。
// 类型 0=皮革，1=铜，2=金，3=锁链，4=铁，5=钻石
// 槽位顺序：HEAD、CHEST、LEGS、FEET
// ══════════════════════════════════════════════════════════════════

static ARMOR_TIERS: LazyLock<[[&'static Item; 4]; 6]> = LazyLock::new(|| {
    [
        [
            &Item::LEATHER_HELMET,
            &Item::LEATHER_CHESTPLATE,
            &Item::LEATHER_LEGGINGS,
            &Item::LEATHER_BOOTS,
        ],
        [
            &Item::COPPER_HELMET,
            &Item::COPPER_CHESTPLATE,
            &Item::COPPER_LEGGINGS,
            &Item::COPPER_BOOTS,
        ],
        [
            &Item::GOLDEN_HELMET,
            &Item::GOLDEN_CHESTPLATE,
            &Item::GOLDEN_LEGGINGS,
            &Item::GOLDEN_BOOTS,
        ],
        [
            &Item::CHAINMAIL_HELMET,
            &Item::CHAINMAIL_CHESTPLATE,
            &Item::CHAINMAIL_LEGGINGS,
            &Item::CHAINMAIL_BOOTS,
        ],
        [
            &Item::IRON_HELMET,
            &Item::IRON_CHESTPLATE,
            &Item::IRON_LEGGINGS,
            &Item::IRON_BOOTS,
        ],
        [
            &Item::DIAMOND_HELMET,
            &Item::DIAMOND_CHESTPLATE,
            &Item::DIAMOND_LEGGINGS,
            &Item::DIAMOND_BOOTS,
        ],
    ]
});

static ARMOR_POPULATION_ORDER: [EquipmentSlot; 4] = [
    EquipmentSlot::HEAD,
    EquipmentSlot::CHEST,
    EquipmentSlot::LEGS,
    EquipmentSlot::FEET,
];

// ══════════════════════════════════════════════════════════════════
// 附魔池 — 按物品类别精心整理，再按
// 装备槽位与生效时的互斥集合
// ══════════════════════════════════════════════════════════════════

static MELEE_WEAPON_ENCHANTS: [&Enchantment; 9] = [
    &Enchantment::SHARPNESS,
    &Enchantment::SMITE,
    &Enchantment::BANE_OF_ARTHROPODS,
    &Enchantment::KNOCKBACK,
    &Enchantment::FIRE_ASPECT,
    &Enchantment::LOOTING,
    &Enchantment::SWEEPING_EDGE,
    &Enchantment::UNBREAKING,
    &Enchantment::MENDING,
];

static TRIDENT_ENCHANTS: [&Enchantment; 6] = [
    &Enchantment::IMPALING,
    &Enchantment::CHANNELING,
    &Enchantment::RIPTIDE,
    &Enchantment::LOYALTY,
    &Enchantment::UNBREAKING,
    &Enchantment::MENDING,
];

static BOW_ENCHANTS: [&Enchantment; 6] = [
    &Enchantment::POWER,
    &Enchantment::PUNCH,
    &Enchantment::FLAME,
    &Enchantment::INFINITY,
    &Enchantment::UNBREAKING,
    &Enchantment::MENDING,
];

static CROSSBOW_ENCHANTS: [&Enchantment; 5] = [
    &Enchantment::QUICK_CHARGE,
    &Enchantment::MULTISHOT,
    &Enchantment::PIERCING,
    &Enchantment::UNBREAKING,
    &Enchantment::MENDING,
];

static FISHING_ROD_ENCHANTS: [&Enchantment; 4] = [
    &Enchantment::LUCK_OF_THE_SEA,
    &Enchantment::LURE,
    &Enchantment::UNBREAKING,
    &Enchantment::MENDING,
];

static HEAD_ARMOR_ENCHANTS: [&Enchantment; 9] = [
    &Enchantment::PROTECTION,
    &Enchantment::FIRE_PROTECTION,
    &Enchantment::BLAST_PROTECTION,
    &Enchantment::PROJECTILE_PROTECTION,
    &Enchantment::RESPIRATION,
    &Enchantment::AQUA_AFFINITY,
    &Enchantment::THORNS,
    &Enchantment::UNBREAKING,
    &Enchantment::MENDING,
];

static CHEST_ARMOR_ENCHANTS: [&Enchantment; 7] = [
    &Enchantment::PROTECTION,
    &Enchantment::FIRE_PROTECTION,
    &Enchantment::BLAST_PROTECTION,
    &Enchantment::PROJECTILE_PROTECTION,
    &Enchantment::THORNS,
    &Enchantment::UNBREAKING,
    &Enchantment::MENDING,
];

static LEGS_ARMOR_ENCHANTS: [&Enchantment; 8] = [
    &Enchantment::PROTECTION,
    &Enchantment::FIRE_PROTECTION,
    &Enchantment::BLAST_PROTECTION,
    &Enchantment::PROJECTILE_PROTECTION,
    &Enchantment::THORNS,
    &Enchantment::UNBREAKING,
    &Enchantment::MENDING,
    &Enchantment::SWIFT_SNEAK,
];

static FEET_ARMOR_ENCHANTS: [&Enchantment; 11] = [
    &Enchantment::PROTECTION,
    &Enchantment::FIRE_PROTECTION,
    &Enchantment::BLAST_PROTECTION,
    &Enchantment::PROJECTILE_PROTECTION,
    &Enchantment::FEATHER_FALLING,
    &Enchantment::DEPTH_STRIDER,
    &Enchantment::FROST_WALKER,
    &Enchantment::SOUL_SPEED,
    &Enchantment::THORNS,
    &Enchantment::UNBREAKING,
    &Enchantment::MENDING,
];

// ══════════════════════════════════════════════════════════════════
// 装备表注册表
// ══════════════════════════════════════════════════════════════════

/// 武器选择表中的一个加权条目。
/// 对应原版生物 `populateDefaultEquipmentSlots` 中的加权随机选择。
#[derive(Clone, Copy)]
pub struct WeaponEntry {
    /// 可能给予的物品。
    pub item: &'static Item,
    /// 选择池中的相对权重。
    pub weight: f32,
}

/// 生物生成时如何选择其主手武器。
#[derive(Clone, Copy)]
pub enum WeaponConfig {
    /// 总是给出此确切物品（例如骷髅 → 弓）。
    Always(&'static Item),
    /// 总是给出其中一个按权重选择的物品（例如猪灵武器）。
    AlwaysWeighted(&'static [WeaponEntry]),
    /// 以随难度而定的概率给予一把加权武器。
    Chance {
        /// 基础难度为困难时的概率。
        on_hard: f32,
        /// 在所有其他难度下的概率。
        otherwise: f32,
        /// 用于抽取的加权物品池。
        items: &'static [WeaponEntry],
    },
    /// 无武器。
    None,
}

/// 一个按槽位的盔甲条目，具有独立的生成概率。
pub struct ArmorSlotEntry {
    /// 此盔甲占用的装备槽位。
    pub slot: &'static EquipmentSlot,
    /// 盔甲物品。
    pub item: &'static Item,
    /// 此槽位获得盔甲的独立概率。
    pub chance: f32,
}

/// 生物生成时如何选择其盔甲。
#[derive(Clone, Copy)]
pub enum ArmorConfig {
    /// 使用原版算法：随机等级（0-2 基础 + 3 次升级尝试于
    /// 每次 10.87%），部分护甲破坏几率（困难难度 10%，其他难度 25%）。
    /// 参见 [`select_vanilla_armor`]。
    Vanilla,
    /// 具有独立几率的各槽位自定义条目（例如猪灵的金色盔甲）。
    CustomPerSlot(&'static [ArmorSlotEntry]),
    /// 无盔甲。
    None,
}

/// 单一生物类型的装备定义。所有装备在生成时随机化
/// 使用 [`RegionalDifficulty`] 计算按世界/按区块的缩放系数。
pub struct MobEquipmentDef {
    /// 实体资源名称（例如 `"zombie"`、`"skeleton"`）。
    pub entity_type: &'static str,
    /// 主手武器配置。
    pub weapon: WeaponConfig,
    /// 盔甲配置。
    pub armor: ArmorConfig,
    /// 是否可以应用生成时的附魔。
    pub enchanted: bool,
    /// 此生物能否随机拾取地面上的战利品。
    pub can_pick_up_loot: bool,
}

/// 所有在生成时获得装备的生物的注册表。
///
/// 将实体资源名称映射到其装备定义。仅列出的生物适用
/// 此处将获得武器、盔甲、附魔和掉落概率设置。
/// 未列出的生物生成时不带装备（与原版一致——并非所有生物都有
/// 装备表）。
pub static EQUIPMENT_REGISTRY: LazyLock<HashMap<&'static str, MobEquipmentDef>> =
    LazyLock::new(|| {
        static ZOMBIE_WEAPONS: [WeaponEntry; 3] = [
            WeaponEntry {
                item: &Item::IRON_SWORD,
                weight: 1.0,
            },
            WeaponEntry {
                item: &Item::IRON_SPEAR,
                weight: 1.0,
            },
            WeaponEntry {
                item: &Item::IRON_SHOVEL,
                weight: 4.0,
            },
        ];

        static DROWNED_WEAPONS: [WeaponEntry; 2] = [
            WeaponEntry {
                item: &Item::TRIDENT,
                weight: 10.0,
            },
            WeaponEntry {
                item: &Item::FISHING_ROD,
                weight: 6.0,
            },
        ];

        static PIGLIN_WEAPONS: [WeaponEntry; 3] = [
            WeaponEntry {
                item: &Item::CROSSBOW,
                weight: 5.0,
            },
            WeaponEntry {
                item: &Item::GOLDEN_SWORD,
                weight: 4.5,
            },
            WeaponEntry {
                item: &Item::GOLDEN_SPEAR,
                weight: 0.5,
            },
        ];

        static PIGLIN_ARMOR: [ArmorSlotEntry; 4] = [
            ArmorSlotEntry {
                slot: &EquipmentSlot::HEAD,
                item: &Item::GOLDEN_HELMET,
                chance: 0.1,
            },
            ArmorSlotEntry {
                slot: &EquipmentSlot::CHEST,
                item: &Item::GOLDEN_CHESTPLATE,
                chance: 0.1,
            },
            ArmorSlotEntry {
                slot: &EquipmentSlot::LEGS,
                item: &Item::GOLDEN_LEGGINGS,
                chance: 0.1,
            },
            ArmorSlotEntry {
                slot: &EquipmentSlot::FEET,
                item: &Item::GOLDEN_BOOTS,
                chance: 0.1,
            },
        ];

        static ZOMBIFIED_PIGLIN_WEAPONS: [WeaponEntry; 2] = [
            WeaponEntry {
                item: &Item::GOLDEN_SWORD,
                weight: 19.0,
            },
            WeaponEntry {
                item: &Item::GOLDEN_SPEAR,
                weight: 1.0,
            },
        ];

        let mut m = HashMap::new();

        // ─── 僵尸 ───
        m.insert(
            "zombie",
            MobEquipmentDef {
                entity_type: "zombie",
                weapon: WeaponConfig::Chance {
                    on_hard: 0.05,
                    otherwise: 0.01,
                    items: &ZOMBIE_WEAPONS,
                },
                armor: ArmorConfig::Vanilla,
                enchanted: true,
                can_pick_up_loot: true,
            },
        );

        // ─── 尸壳 ───
        m.insert(
            "husk",
            MobEquipmentDef {
                entity_type: "husk",
                weapon: WeaponConfig::Chance {
                    on_hard: 0.05,
                    otherwise: 0.01,
                    items: &ZOMBIE_WEAPONS,
                },
                armor: ArmorConfig::Vanilla,
                enchanted: true,
                can_pick_up_loot: true,
            },
        );

        // ─── 僵尸村民 ───
        m.insert(
            "zombie_villager",
            MobEquipmentDef {
                entity_type: "zombie_villager",
                weapon: WeaponConfig::Chance {
                    on_hard: 0.05,
                    otherwise: 0.01,
                    items: &ZOMBIE_WEAPONS,
                },
                armor: ArmorConfig::Vanilla,
                enchanted: true,
                can_pick_up_loot: true,
            },
        );

        // ─── 溺尸 ───
        m.insert(
            "drowned",
            MobEquipmentDef {
                entity_type: "drowned",
                weapon: WeaponConfig::Chance {
                    on_hard: 0.10,
                    otherwise: 0.10,
                    items: &DROWNED_WEAPONS,
                },
                armor: ArmorConfig::None,
                enchanted: true,
                can_pick_up_loot: true,
            },
        );

        // ─── 僵尸猪灵 ───
        m.insert(
            "zombified_piglin",
            MobEquipmentDef {
                entity_type: "zombified_piglin",
                weapon: WeaponConfig::AlwaysWeighted(&ZOMBIFIED_PIGLIN_WEAPONS),
                armor: ArmorConfig::None,
                enchanted: true,
                can_pick_up_loot: false,
            },
        );

        // ─── 骷髅 ───
        m.insert(
            "skeleton",
            MobEquipmentDef {
                entity_type: "skeleton",
                weapon: WeaponConfig::Always(&Item::BOW),
                armor: ArmorConfig::Vanilla,
                enchanted: true,
                can_pick_up_loot: true,
            },
        );

        // ─── 流浪者 ───
        m.insert(
            "stray",
            MobEquipmentDef {
                entity_type: "stray",
                weapon: WeaponConfig::Always(&Item::BOW),
                armor: ArmorConfig::Vanilla,
                enchanted: true,
                can_pick_up_loot: true,
            },
        );

        // ─── 沼骸 ───
        m.insert(
            "bogged",
            MobEquipmentDef {
                entity_type: "bogged",
                weapon: WeaponConfig::Always(&Item::BOW),
                armor: ArmorConfig::Vanilla,
                enchanted: true,
                can_pick_up_loot: true,
            },
        );

        // ─── 凋灵骷髅 ───
        m.insert(
            "wither_skeleton",
            MobEquipmentDef {
                entity_type: "wither_skeleton",
                weapon: WeaponConfig::Always(&Item::STONE_SWORD),
                armor: ArmorConfig::None,
                enchanted: false,
                can_pick_up_loot: false,
            },
        );

        // ─── 猪灵 ───
        m.insert(
            "piglin",
            MobEquipmentDef {
                entity_type: "piglin",
                weapon: WeaponConfig::AlwaysWeighted(&PIGLIN_WEAPONS),
                armor: ArmorConfig::CustomPerSlot(&PIGLIN_ARMOR),
                enchanted: true,
                can_pick_up_loot: false,
            },
        );

        // ─── 掠夺者 ───
        m.insert(
            "pillager",
            MobEquipmentDef {
                entity_type: "pillager",
                weapon: WeaponConfig::Always(&Item::CROSSBOW),
                armor: ArmorConfig::None,
                enchanted: false,
                can_pick_up_loot: false,
            },
        );

        // ─── 卫道士 ───
        m.insert(
            "vindicator",
            MobEquipmentDef {
                entity_type: "vindicator",
                weapon: WeaponConfig::Always(&Item::IRON_AXE),
                armor: ArmorConfig::None,
                enchanted: true,
                can_pick_up_loot: false,
            },
        );

        m
    });

// ══════════════════════════════════════════════════════════════════
// 区域难度 —— 与原版 DifficultyInstance.java 完全一致
// 原版近似：与原版 Minecraft 26.2 完全相同的公式
// DifficultyInstance，包括钳制后的区域难度、特殊
// 乘数（0-1 线性）以及有效难度（2-4 范围）。
// ══════════════════════════════════════════════════════════════════

/// 为特定世界区块计算出的难度值。
///
/// 对应原版的 `DifficultyInstance`。用于缩放装备生成几率，
/// 附魔消耗，以及战利品拾取标志。
#[derive(Clone, Copy)]
pub struct RegionalDifficulty {
    /// 世界的基础难度等级（`Easy`、`Normal`、`Hard`）。
    pub base_difficulty: Difficulty,
    /// 根据游戏时间、居住时间和月相计算出的有效难度。
    /// 限制在 `[2.0, 4.0]` 范围内（和平模式为 `0.0`）。
    pub effective_difficulty: f32,
    /// 由 `effective_difficulty` 得出的 `[0.0, 1.0]` 范围内的线性乘数。
    /// 当为 `0.0` 时（新区块+游戏早期），没有装备、附魔或
    /// 战利品拾取标志会被应用。
    pub special_multiplier: f32,
}

impl RegionalDifficulty {
    /// 计算给定世界位置的难度。
    ///
    /// 查找区块的已驻留时间，并将其与世界的对应值相结合
    /// 难度、游戏时间与月相。
    pub fn at(world: &Arc<crate::world::World>, pos: Vector3<f64>) -> Self {
        let level_info = world.level_info.load();
        let difficulty = level_info.difficulty;
        let time_of_day = world.level_time.try_lock().map_or(0, |t| t.time_of_day);
        let inhabited_time = {
            let chunk_x = (pos.x / 16.0).floor() as i32;
            let chunk_z = (pos.z / 16.0).floor() as i32;
            world
                .level
                .loaded_chunks
                .get(&papokin_util::math::vector2::Vector2::new(chunk_x, chunk_z))
                .map_or(0, |c| {
                    c.inhabited_time.load(std::sync::atomic::Ordering::Relaxed)
                })
        };
        let moon_brightness = moon_brightness(time_of_day);

        Self::calculate(difficulty, time_of_day, inhabited_time, moon_brightness)
    }

    /// 由原始输入直接计算。供 `at()` 和测试使用。
    #[must_use]
    pub fn calculate(
        difficulty: Difficulty,
        total_game_time: i64,
        chunk_inhabited_time: u64,
        moon_brightness: f32,
    ) -> Self {
        if difficulty == Difficulty::Peaceful {
            return Self {
                base_difficulty: difficulty,
                effective_difficulty: 0.0,
                special_multiplier: 0.0,
            };
        }

        let is_hard = difficulty == Difficulty::Hard;

        let mut scale = 0.75f32;
        let global_scale = ((total_game_time as f32 - 72000.0) / 1440000.0).clamp(0.0, 1.0) * 0.25;
        scale += global_scale;

        let mut local_scale = 0.0f32;
        local_scale += (chunk_inhabited_time as f32 / 3600000.0).clamp(0.0, 1.0)
            * if is_hard { 1.0 } else { 0.75 };
        local_scale += (moon_brightness * 0.25).clamp(0.0, global_scale);

        if difficulty == Difficulty::Easy {
            local_scale *= 0.5;
        }

        let difficulty_id = match difficulty {
            Difficulty::Peaceful => 0,
            Difficulty::Easy => 1,
            Difficulty::Normal => 2,
            Difficulty::Hard => 3,
        };

        let effective = difficulty_id as f32 * (scale + local_scale);

        let special_multiplier = if effective < 2.0 {
            0.0
        } else if effective > 4.0 {
            1.0
        } else {
            (effective - 2.0) / 2.0
        };

        Self {
            base_difficulty: difficulty,
            effective_difficulty: effective,
            special_multiplier,
        }
    }

    /// 由 `special_multiplier` 缩放的随机检定。
    ///
    ///以 `base_chance * special_multiplier` 的概率返回 `true`。
    /// `special_multiplier` 为 `0.0` 时，这里总是返回 `false`（与原版
    /// 在全新 Normal/Easy 世界中的行为）。
    #[must_use]
    pub fn should_happen(&self, base_chance: f32) -> bool {
        rand::random::<f32>() < base_chance * self.special_multiplier
    }
}

/// 给定时刻的月亮亮度系数（0.0 到 1.0）。
/// 相位 0 为满月，相位 4 为新月。
#[must_use]
fn moon_brightness(time_of_day: i64) -> f32 {
    let phase = (time_of_day / 24000 % 8) as i32;
    (phase - 4).abs() as f32 / 4.0
}

// ══════════════════════════════════════════════════════════════════
// 附魔系统 — 模拟原版 EnchantmentsByCostWithDifficulty
//
// 带互斥集冲突解决的加权选择，以及基于成本的
// 等级计算。改为按装备类别使用精选的扁平池
// 基于数据包的 enchantment_provider/mob_spawn_equipment.json。
// ══════════════════════════════════════════════════════════════════

/// 在 `[min, min + specialMultiplier * span]` 范围内取随机附魔消耗。
#[must_use]
fn spawn_enchant_cost(special_multiplier: f32) -> i32 {
    let min = MOB_SPAWN_ENCHANT_MIN_COST;
    let max = min + (special_multiplier * MOB_SPAWN_ENCHANT_COST_SPAN as f32).round() as i32;
    let mut rng = rand::rng();
    rng.random_range(min..=max)
}

/// 返回费用可承受的最高附魔等级。
#[must_use]
fn enchantment_level_from_cost(enchant: &Enchantment, cost: i32) -> i32 {
    for lvl in (1..=enchant.max_level).rev() {
        if cost >= enchant.min_cost.calculate(lvl) {
            return lvl;
        }
    }
    1
}

/// 为物品/槽位组合选择附魔池。
///
/// 为每个装备类别（近战、三叉戟、弓、……）使用精选的扁平池
/// 弩、钓鱼竿以及各盔甲槽位）。这是对
/// 原版数据驱动的 `mob_spawn_equipment` 附魔提供器，它
/// 通过 `supported_items` 标签进行过滤。
#[must_use]
fn enchant_pool_for(item: &Item, slot: &EquipmentSlot) -> &'static [&'static Enchantment] {
    let key = item.registry_key;
    if key.contains("sword")
        || key.contains("spear")
        || key.contains("axe")
        || key.contains("shovel")
    {
        &MELEE_WEAPON_ENCHANTS
    } else if key.contains("trident") {
        &TRIDENT_ENCHANTS
    } else if key.contains("bow") {
        &BOW_ENCHANTS
    } else if key.contains("crossbow") {
        &CROSSBOW_ENCHANTS
    } else if key.contains("fishing_rod") {
        &FISHING_ROD_ENCHANTS
    } else if *slot == EquipmentSlot::HEAD {
        &HEAD_ARMOR_ENCHANTS
    } else if *slot == EquipmentSlot::CHEST {
        &CHEST_ARMOR_ENCHANTS
    } else if *slot == EquipmentSlot::LEGS {
        &LEGS_ARMOR_ENCHANTS
    } else if *slot == EquipmentSlot::FEET {
        &FEET_ARMOR_ENCHANTS
    } else {
        &[]
    }
}

/// 检查 `candidate` 是否与任何已应用的附魔冲突
/// 通过原版互斥集合（例如 `exclusive_set_damage`）。
#[must_use]
fn conflicts_with(candidate: &Enchantment, applied: &[&Enchantment]) -> bool {
    if let Some(excl) = candidate.exclusive_set {
        let excl_keys = excl.0;
        for existing in applied {
            if excl_keys.contains(&existing.registry_key) {
                return true;
            }
        }
    }
    false
}

/// 使用带权池选择，将多个附魔应用到物品堆上。
///
/// 从随机代价开始（按 `special_multiplier` 缩放），并挑选
/// 按权重挑选附魔，解决互斥集冲突，并决定
/// 由剩余费用推算等级。每轮迭代费用减半，因此
/// 后添加的附魔会获得更低的等级。
pub fn apply_vanilla_enchantments(
    stack: &mut ItemStack,
    slot: &EquipmentSlot,
    special_multiplier: f32,
) {
    let pool = enchant_pool_for(stack.item, slot);
    if pool.is_empty() {
        return;
    }

    let mut cost = spawn_enchant_cost(special_multiplier);
    let mut applied: Vec<&Enchantment> = Vec::new();
    let mut rng = rand::rng();

    loop {
        let candidates: Vec<&Enchantment> = pool
            .iter()
            .copied()
            .filter(|e| !applied.contains(e) && !conflicts_with(e, &applied))
            .collect();

        if candidates.is_empty() {
            break;
        }

        let total_weight: f32 = candidates.iter().map(|e| e.weight as f32).sum();
        if total_weight <= 0.0 {
            break;
        }

        let mut roll = rng.random_range(0.0..total_weight);
        let mut selected: Option<&Enchantment> = None;
        for e in &candidates {
            roll -= e.weight as f32;
            if roll <= 0.0 {
                selected = Some(e);
                break;
            }
        }
        let Some(&fallback) = candidates.last() else {
            break;
        };
        let selected = selected.unwrap_or(fallback);

        let level = enchantment_level_from_cost(selected, cost);
        stack.add_enchantment(selected, level.clamp(1, selected.max_level) as u16);
        applied.push(selected);

        cost /= 2;
        if cost < 1 {
            break;
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// 装备填充
//
// 镜像生物特有的 `finalizeSpawn` / `populateDefaultEquipmentSlots`
// 来自原版的 Zombie、AbstractSkeleton、WitherSkeleton、Piglin、
// 掠夺者、卫道士、溺尸和僵尸猪灵。
// ══════════════════════════════════════════════════════════════════

/// 从武器条目表中进行加权随机选择。
#[must_use]
fn weighted_select_item(items: &[WeaponEntry]) -> &'static Item {
    let total: f32 = items.iter().map(|e| e.weight).sum();
    let mut rng = rand::rng();
    let mut roll: f32 = rng.random_range(0.0..total);
    for entry in items {
        roll -= entry.weight;
        if roll <= 0.0 {
            return entry.item;
        }
    }
    items.last().map_or(&Item::AIR, |e| e.item)
}

/// 使用原版算法选择盔甲。
///
/// 1. 随机基础等级（0-2），最多 3 次升级尝试，每次成功率 10.87%。
/// 2. 按 HEAD→CHEST→LEGS→FEET 顺序遍历，有机会提前停止（困难 10%，
///    否则为 25%）——难度越高，产生的碎片越少。
/// 3. 每件装备都采用默认的装备掉落几率。
#[must_use]
fn select_vanilla_armor(difficulty: &RegionalDifficulty) -> Vec<(EquipmentSlot, ItemStack, f32)> {
    let mut rng = rand::rng();

    let mut armor_type = rng.random_range(0..3);
    let mut i = 1;
    while (i as f32) <= ARMOR_UPGRADE_MATERIAL_ATTEMPTS {
        if rng.random::<f32>() < ARMOR_UPGRADE_MATERIAL_CHANCE {
            armor_type += 1;
        }
        i += 1;
    }
    armor_type = armor_type.min(5);

    let tier = &ARMOR_TIERS[armor_type];

    let partial_chance = if difficulty.base_difficulty == Difficulty::Hard {
        0.1f32
    } else {
        0.25f32
    };

    let mut pieces = Vec::new();
    let mut first = true;
    for (i, slot) in ARMOR_POPULATION_ORDER.iter().enumerate() {
        if !first && rng.random::<f32>() < partial_chance {
            break;
        }
        first = false;
        pieces.push((
            slot.clone(),
            create_equipment_item(tier[i]),
            DEFAULT_EQUIPMENT_DROP_CHANCE,
        ));
    }
    pieces
}

/// 为生物装备创建全新的满耐久度 `ItemStack`。
/// 原版生物生成时总是携带满耐久的装备。
#[must_use]
fn create_equipment_item(item: &'static Item) -> ItemStack {
    ItemStack::new(1, item)
}

/// 为生物定义生成装备物品、槽位和掉落几率。
///
/// 处理完整的武器与盔甲选择逻辑，包括附魔
/// 应用需要 `def.enchanted` 为 true 且与难度相关的
/// 随机检查通过。
#[must_use]
fn equip_mob_from_def(
    def: &MobEquipmentDef,
    difficulty: &RegionalDifficulty,
) -> Vec<(EquipmentSlot, ItemStack, f32)> {
    let mut changes: Vec<(EquipmentSlot, ItemStack, f32)> = Vec::new();

    // ── 武器 ──
    match def.weapon {
        WeaponConfig::Always(item) => {
            let mut stack = create_equipment_item(item);
            if def.enchanted && difficulty.should_happen(WEAPON_ENCHANT_CHANCE) {
                apply_vanilla_enchantments(
                    &mut stack,
                    &EquipmentSlot::MAIN_HAND,
                    difficulty.special_multiplier,
                );
            }
            changes.push((
                EquipmentSlot::MAIN_HAND,
                stack,
                DEFAULT_EQUIPMENT_DROP_CHANCE,
            ));
        }
        WeaponConfig::AlwaysWeighted(items) => {
            let item = weighted_select_item(items);
            let mut stack = create_equipment_item(item);
            if def.enchanted && difficulty.should_happen(WEAPON_ENCHANT_CHANCE) {
                apply_vanilla_enchantments(
                    &mut stack,
                    &EquipmentSlot::MAIN_HAND,
                    difficulty.special_multiplier,
                );
            }
            changes.push((
                EquipmentSlot::MAIN_HAND,
                stack,
                DEFAULT_EQUIPMENT_DROP_CHANCE,
            ));
        }
        WeaponConfig::Chance {
            on_hard,
            otherwise,
            items,
        } => {
            let chance = if difficulty.base_difficulty == Difficulty::Hard {
                on_hard
            } else {
                otherwise
            };
            if rand::random::<f32>() < chance {
                let item = weighted_select_item(items);
                let mut stack = create_equipment_item(item);
                if def.enchanted && difficulty.should_happen(WEAPON_ENCHANT_CHANCE) {
                    apply_vanilla_enchantments(
                        &mut stack,
                        &EquipmentSlot::MAIN_HAND,
                        difficulty.special_multiplier,
                    );
                }
                changes.push((
                    EquipmentSlot::MAIN_HAND,
                    stack,
                    DEFAULT_EQUIPMENT_DROP_CHANCE,
                ));
            }
        }
        WeaponConfig::None => {}
    }

    // ── 盔甲 ──
    match def.armor {
        ArmorConfig::Vanilla => {
            if difficulty.should_happen(WEARING_ARMOR_CHANCE) {
                let armor_pieces = select_vanilla_armor(difficulty);
                for (slot, mut stack, drop_chance) in armor_pieces {
                    if def.enchanted && difficulty.should_happen(ARMOR_ENCHANT_CHANCE) {
                        apply_vanilla_enchantments(
                            &mut stack,
                            &slot,
                            difficulty.special_multiplier,
                        );
                    }
                    changes.push((slot, stack, drop_chance));
                }
            }
        }
        ArmorConfig::CustomPerSlot(entries) => {
            for entry in entries {
                if rand::random::<f32>() < entry.chance {
                    let mut stack = create_equipment_item(entry.item);
                    if def.enchanted && difficulty.should_happen(ARMOR_ENCHANT_CHANCE) {
                        apply_vanilla_enchantments(
                            &mut stack,
                            entry.slot,
                            difficulty.special_multiplier,
                        );
                    }
                    changes.push((entry.slot.clone(), stack, DEFAULT_EQUIPMENT_DROP_CHANCE));
                }
            }
        }
        ArmorConfig::None => {}
    }

    changes
}

// ══════════════════════════════════════════════════════════════════
// 公共入口
// ══════════════════════════════════════════════════════════════════

/// 在生物生成时为其装备武器/盔甲/附魔。
///
/// 从 `EntityBase::init_data_tracker` 的全面实现中调用，用于
/// 所有生物类型。在其中查找该生物的装备定义
/// [`EQUIPMENT_REGISTRY`]，在生物
/// 位置，生成装备并存入实体的装备槽，
/// 并将更改广播给附近的玩家。
///
/// 未列入注册表的生物将静默地不获得任何装备。
pub fn equip_mob_on_spawn(mob: &dyn EntityBase, world: &Arc<crate::world::World>) {
    let entity_type = mob.get_entity().entity_type;
    let pos = mob.get_entity().pos.load();
    let difficulty = RegionalDifficulty::at(world, pos);

    let Some(living) = mob.get_living_entity() else {
        return;
    };

    let entity_name = entity_type.resource_name;

    let Some(def) = EQUIPMENT_REGISTRY.get(entity_name) else {
        return;
    };

    let mut equipment = living
        .entity_equipment
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut drop_chances = living
        .equipment_drop_chances
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let changes_with_drops = equip_mob_from_def(def, &difficulty);

    let mut equipment_changes: Vec<(EquipmentSlot, ItemStack)> = Vec::new();

    for (slot, stack, drop_chance) in changes_with_drops {
        equipment.put(&slot, stack.clone());
        drop_chances.insert(slot.clone(), drop_chance);
        equipment_changes.push((slot, stack));
    }

    drop(equipment);
    drop(drop_chances);

    living.send_equipment_changes(&equipment_changes);
}

#[must_use]
pub fn is_equippable_in_slot(
    entity_type: &EntityType,
    stack: &ItemStack,
    slot: &EquipmentSlot,
) -> bool {
    stack.get_data_component::<EquippableImpl>().map_or_else(
        || *slot == EquipmentSlot::MAIN_HAND,
        |equippable| {
            *equippable.slot == *slot
                && equippable
                    .allowed_entities
                    .as_ref()
                    .is_none_or(|allowed| match allowed {
                        IDSet::Tag(tag) => entity_type.is_tagged_with(tag).unwrap_or(false),
                        IDSet::IDs(ids) => ids.iter().any(|id| id.id == entity_type.id),
                    })
        },
    )
}

#[must_use]
pub fn get_equipment_slot_for_item(stack: &ItemStack) -> EquipmentSlot {
    stack
        .get_data_component::<EquippableImpl>()
        .map_or(EquipmentSlot::MAIN_HAND, |equippable| {
            equippable.slot.clone()
        })
}

#[must_use]
/// `new_item` 是否是 `slot` 中 `current_item` 的升级。
pub fn can_replace_current_item(
    mob: &MobEntity,
    preferred_weapon_type: Option<&'static Tag>,
    new_item: &ItemStack,
    current_item: &ItemStack,
    slot: &EquipmentSlot,
) -> bool {
    if current_item.is_empty() {
        return true;
    }
    if slot.is_armor_slot() {
        compare_armor(mob, new_item, current_item, slot)
    } else {
        *slot == EquipmentSlot::MAIN_HAND
            && compare_weapons(mob, preferred_weapon_type, new_item, current_item, slot)
    }
}

fn compare_armor(
    mob: &MobEntity,
    new_item: &ItemStack,
    current_item: &ItemStack,
    slot: &EquipmentSlot,
) -> bool {
    if current_item.get_enchantment_level(&Enchantment::BINDING_CURSE) > 0 {
        return false;
    }
    let new_defense = approximate_attribute_with(mob, new_item, &Attributes::ARMOR, slot);
    let old_defense = approximate_attribute_with(mob, current_item, &Attributes::ARMOR, slot);
    let new_toughness =
        approximate_attribute_with(mob, new_item, &Attributes::ARMOR_TOUGHNESS, slot);
    let old_toughness =
        approximate_attribute_with(mob, current_item, &Attributes::ARMOR_TOUGHNESS, slot);
    if new_defense != old_defense {
        return new_defense > old_defense;
    }
    if new_toughness != old_toughness {
        return new_toughness > old_toughness;
    }
    can_replace_equal_item(new_item, current_item)
}

fn compare_weapons(
    mob: &MobEntity,
    preferred_weapon_type: Option<&'static Tag>,
    new_item: &ItemStack,
    current_item: &ItemStack,
    slot: &EquipmentSlot,
) -> bool {
    if let Some(preferred) = preferred_weapon_type {
        let current_preferred = current_item.item.has_tag(preferred);
        let new_preferred = new_item.item.has_tag(preferred);
        if current_preferred && !new_preferred {
            return false;
        }
        if !current_preferred && new_preferred {
            return true;
        }
    }
    let new_damage = approximate_attribute_with(mob, new_item, &Attributes::ATTACK_DAMAGE, slot);
    let old_damage =
        approximate_attribute_with(mob, current_item, &Attributes::ATTACK_DAMAGE, slot);
    if new_damage != old_damage {
        return new_damage > old_damage;
    }
    can_replace_equal_item(new_item, current_item)
}

fn approximate_attribute_with(
    mob: &MobEntity,
    stack: &ItemStack,
    attribute: &Attributes,
    slot: &EquipmentSlot,
) -> f64 {
    let base_value = mob.living_entity.get_attribute_base(attribute);
    let mut add_value = 0.0;
    let mut add_multiplied_base = 0.0;
    let mut multiplied_total = 1.0;
    if let Some(modifiers) = stack.get_data_component::<AttributeModifiersImpl>() {
        for modifier in modifiers.attribute_modifiers.iter() {
            if modifier.r#type.id != attribute.id || !attribute_slot_matches(&modifier.slot, slot) {
                continue;
            }
            match modifier.operation {
                Operation::AddValue => add_value += modifier.amount,
                Operation::AddMultipliedBase => add_multiplied_base += modifier.amount,
                Operation::AddMultipliedTotal => multiplied_total *= 1.0 + modifier.amount,
            }
        }
    }
    (base_value + add_value) * (1.0 + add_multiplied_base) * multiplied_total
}

fn attribute_slot_matches(group: &AttributeModifierSlot, slot: &EquipmentSlot) -> bool {
    match group {
        AttributeModifierSlot::Any => true,
        AttributeModifierSlot::MainHand => *slot == EquipmentSlot::MAIN_HAND,
        AttributeModifierSlot::OffHand => *slot == EquipmentSlot::OFF_HAND,
        AttributeModifierSlot::Hand => slot.slot_type() == EquipmentType::Hand,
        AttributeModifierSlot::Feet => *slot == EquipmentSlot::FEET,
        AttributeModifierSlot::Legs => *slot == EquipmentSlot::LEGS,
        AttributeModifierSlot::Chest => *slot == EquipmentSlot::CHEST,
        AttributeModifierSlot::Head => *slot == EquipmentSlot::HEAD,
        AttributeModifierSlot::Armor => slot.slot_type() == EquipmentType::HumanoidArmor,
        AttributeModifierSlot::Body => *slot == EquipmentSlot::BODY,
        AttributeModifierSlot::Saddle => *slot == EquipmentSlot::SADDLE,
    }
}

#[must_use]
pub fn can_replace_equal_item(new_item: &ItemStack, current_item: &ItemStack) -> bool {
    let enchantment_count = |stack: &ItemStack| {
        stack
            .get_data_component::<EnchantmentsImpl>()
            .map_or(0, |enchantments| enchantments.enchantment.len())
    };
    let new_enchantments = enchantment_count(new_item);
    let current_enchantments = enchantment_count(current_item);
    if new_enchantments != current_enchantments {
        return new_enchantments > current_enchantments;
    }
    let new_damage = new_item.get_damage();
    let current_damage = current_item.get_damage();
    if new_damage != current_damage {
        return new_damage < current_damage;
    }
    new_item.get_data_component::<CustomNameImpl>().is_some()
        && current_item
            .get_data_component::<CustomNameImpl>()
            .is_none()
}

#[must_use]
/// 若 `stack` 比当前穿戴的更好则装备它，返回实际装备的物品。
pub fn equip_item_if_possible(mob: &dyn Mob, stack: ItemStack) -> ItemStack {
    let mob_entity = mob.get_mob_entity();
    let entity = &mob_entity.living_entity.entity;
    let mut slot = get_equipment_slot_for_item(&stack);
    if !is_equippable_in_slot(entity.entity_type, &stack, &slot) {
        return ItemStack::EMPTY.clone();
    }

    let item_in = |slot: &EquipmentSlot| {
        mob_entity
            .living_entity
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(slot)
    };
    let mut current = item_in(&slot);
    let mut can_replace = mob.can_replace_current_item(&stack, &current, &slot);
    if slot.is_armor_slot() && !can_replace {
        slot = EquipmentSlot::MAIN_HAND;
        current = item_in(&slot);
        can_replace = current.is_empty();
    }
    if !can_replace {
        return ItemStack::EMPTY.clone();
    }

    let drop_chance = mob_entity.drop_chance(&slot);
    if !current.is_empty() && (rand::random::<f32>() - 0.1).max(0.0) < drop_chance {
        mob_entity.spawn_at_location(current);
    }

    let mut stack = stack;
    let to_equip = limit_for_slot(&slot, &mut stack);
    mob_entity.set_item_slot_and_drop_when_killed(&slot, to_equip.clone());
    mob_entity
        .persistence_required
        .store(true, std::sync::atomic::Ordering::Relaxed);
    to_equip
}

// 手部槽位没有堆叠数量限制，其他每个槽位只能放一件物品。
fn limit_for_slot(slot: &EquipmentSlot, stack: &mut ItemStack) -> ItemStack {
    if slot.slot_type() == EquipmentType::Hand {
        std::mem::replace(stack, ItemStack::EMPTY.clone())
    } else {
        stack.split(1)
    }
}

#[cfg(test)]
mod tests {
    use super::moon_brightness;

    #[test]
    fn moon_brightness_matches_the_vanilla_phase_table() {
        let expected = [1.0, 0.75, 0.5, 0.25, 0.0, 0.25, 0.5, 0.75];
        for (phase, want) in expected.into_iter().enumerate() {
            let time_of_day = phase as i64 * 24000;
            assert!(
                (moon_brightness(time_of_day) - want).abs() < f32::EPSILON,
                "phase {phase}: got {}, want {want}",
                moon_brightness(time_of_day)
            );
        }
    }

    #[test]
    fn moon_brightness_wraps_every_eight_days() {
        for phase in 0..8i64 {
            assert!(
                (moon_brightness(phase * 24000) - moon_brightness((phase + 8) * 24000)).abs()
                    < f32::EPSILON,
                "phase {phase} does not wrap"
            );
        }
    }
}
