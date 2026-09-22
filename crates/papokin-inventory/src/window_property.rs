//! 窗口属性定义。
//!
//! 本模块定义需要同步的容器专属 UI 属性
//! 服务器与客户端之间的同步。这些包括进度条、燃料指示器以及
//! 容器界面中的其他可视元素。
//!
//! # Window Properties
//!
//! 属性由唯一 ID 标识，并发送给客户端以更新
//! 容器的可视状态：
//! - 熔炉：火焰图标动画、熔炼进度
//! - 附魔台：等级需求、可用附魔
//! - 酿造台：酿造时间、燃料等级
//! - 铁砧：修理费用
//!
//! 属性 ID 映射请参见 Minecraft Wiki。

/// 用于可转换为窗口属性 ID 的类型的 trait。
pub trait WindowPropertyTrait {
    /// 将此属性转换为其协议 ID。
    fn to_id(self) -> i16;
}

/// 一个具有特定值的窗口属性。
///
/// 用于向客户端发送属性更新（例如熔炉进度条）。
pub struct WindowProperty<T: WindowPropertyTrait> {
    /// 被追踪的属性类型（例如熔炉火焰图标、进度箭头）。
    window_property: T,
    /// 属性的当前值。
    value: i16,
}

impl<T: WindowPropertyTrait> WindowProperty<T> {
    /// 创建一个新的窗口属性。
    ///
    /// # Arguments
    /// - `window_property` - 属性类型
    /// - `value` - 属性值
    #[must_use]
    pub const fn new(window_property: T, value: i16) -> Self {
        Self {
            window_property,
            value,
        }
    }

    /// 将此属性转换为 (id, value) 元组。
    #[must_use]
    pub fn into_tuple(self) -> (i16, i16) {
        (self.window_property.to_id(), self.value)
    }
}

/// 熔炉窗口属性。
pub enum Furnace {
    /// 火焰图标动画等级（0-250）。
    FireIcon,
    /// 燃料最大燃烧时间。
    MaximumFuelBurnTime,
    /// 箭的进度动画（0-250）。
    ProgressArrow,
    /// 最大熔炼进度时间。
    MaximumProgress,
}

/// 附魔台的窗口属性。
pub enum EnchantmentTable {
    /// 特定槽位的经验等级要求。
    LevelRequirement { slot: u8 },
    /// 附魔生成使用的随机种子。
    EnchantmentSeed,
    /// 特定槽位的附魔 ID。
    EnchantmentId { slot: u8 },
    /// 特定槽位的附魔等级。
    EnchantmentLevel { slot: u8 },
}

// TODO: 不要再使用魔法数字
impl WindowPropertyTrait for EnchantmentTable {
    fn to_id(self) -> i16 {
        use EnchantmentTable::{
            EnchantmentId, EnchantmentLevel, EnchantmentSeed, LevelRequirement,
        };

        i16::from(match self {
            LevelRequirement { slot } => slot,
            EnchantmentSeed => 3,
            EnchantmentId { slot } => 4 + slot,
            EnchantmentLevel { slot } => 7 + slot,
        })
    }
}

/// 信标窗口属性。
pub enum Beacon {
    /// 效果强度等级（1-4）。
    PowerLevel,
    /// 第一个选中的药水效果 ID。
    FirstPotionEffect,
    /// 第二个选定的药水效果 ID。
    SecondPotionEffect,
}

/// 铁砧界面窗口属性。
pub enum Anvil {
    /// 以经验等级计的总修复费用。
    RepairCost,
}

impl WindowPropertyTrait for Anvil {
    fn to_id(self) -> i16 {
        match self {
            Self::RepairCost => 0,
        }
    }
}

/// 酿造台窗口属性。
pub enum BrewingStand {
    /// 酿造进度 (0-400)。
    BrewTime,
    /// 剩余燃料时间（0-20）。
    FuelTime,
}

/// 切石机窗口属性。
pub enum Stonecutter {
    /// 所选配方的 ID。
    SelectedRecipe,
}

/// 织布机窗口属性。
pub enum Loom {
    /// 所选图案的 ID。
    SelectedPattern,
}

/// 讲台窗口属性。
pub enum Lectern {
    /// 当前正在查看的页码。
    PageNumber,
}

pub trait PropertyDelegate: Sync + Send {
    fn get_property(&self, index: i32) -> i32;
    fn set_property(&self, index: i32, value: i32);
    fn get_properties_size(&self) -> i32;
}

/// 用于从烧炼类方块实体中提取熔炼经验的 trait。
pub trait ExperienceContainer: Send + Sync {
    /// 提取并重置累积的经验，将总量作为整数返回
    fn extract_experience(&self) -> i32;
}
