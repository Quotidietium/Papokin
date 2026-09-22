/// 条目或战利品池参与战利品生成所需满足的条件。
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum LootCondition {
    #[default]
    None,
    SilkTouch,
    NoSilkTouch,
    Shears,
    SilkTouchOrShears,
    NoSilkTouchOrShears,
    SurvivesExplosion,
    KilledByPlayer,
    RandomChance {
        chance: f32,
    },
    RandomChanceWithEnchantedBonus {
        unenchanted_chance: f32,
        enchanted_chance_base: f32,
        enchanted_chance_per_level_above_first: f32,
    },
    TableBonus {
        chances: &'static [f32],
    },
    AllOf(&'static [Self]),
}

/// 工具带有时运或抢夺附魔时的额外数量公式。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LootBonusFormula {
    OreDrops,
    UniformBonusCount(i32),
    BinomialWithBonusCount { extra: i32, probability: f32 },
}

/// 战利品池中的单个物品条目。
#[derive(Clone, Copy, Debug)]
pub struct LootEntry {
    /// 物品的注册表名称（例如 `"minecraft:diamond"`）。
    pub item: &'static str,
    /// 相对概率权重；数值越高越可能被选中。
    pub weight: i32,
    /// 最小堆叠数量（含）。
    pub min_count: i32,
    /// 最大堆叠数量（含）。
    pub max_count: i32,
    /// 此条目入选所需满足的条件。
    pub condition: LootCondition,
    /// 应用时运/抢夺时使用的额外公式（如果有）。
    pub bonus_formula: Option<LootBonusFormula>,
}

/// 战利品表中的一个抽取池。
#[derive(Clone, Copy, Debug)]
pub struct LootPool {
    /// 每次抽取时有资格被选中的物品条目。
    pub entries: &'static [LootEntry],
    /// 最小掷骰次数（含）。
    pub min_rolls: i32,
    /// 最大掷骰次数（含）。
    pub max_rolls: i32,
    /// 每次抽取中隐式“空”（无物品）结果的权重。
    /// 在原版中，这被建模为具有给定权重的 `minecraft:empty` 条目。
    pub empty_weight: i32,
    /// 整个战利品池运行所需的条件。
    pub condition: LootCondition,
}

/// 一个完整的战利品表，由一个或多个池组成。
#[derive(Clone, Copy, Debug)]
pub struct LootTable {
    /// 为此表生成战利品时要抽取的所有池。
    pub pools: &'static [LootPool],
}

pub type ChestLootEntry = LootEntry;
pub type ChestLootPool = LootPool;
pub type ChestLootTable = LootTable;
