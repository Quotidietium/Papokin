use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// 将字符串解析为 [`GameMode`] 失败时返回的错误。
#[derive(Debug, PartialEq, Eq)]
pub struct ParseGameModeError;

/// 表示玩家可处于的各种游戏模式。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameMode {
    /// 标准生存模式：玩家会受到伤害、收集资源并进行正常交互。
    Survival = 0,
    /// 创造模式：玩家拥有无限资源，可以飞行，且不会受到伤害。
    Creative = 1,
    /// 冒险模式：玩家没有合适工具时无法破坏方块。
    Adventure = 2,
    /// 旁观模式：玩家可穿过方块飞行，只观察而不交互。
    Spectator = 3,
}

impl GameMode {
    pub const VALUES: [Self; 4] = [
        Self::Survival,
        Self::Creative,
        Self::Adventure,
        Self::Spectator,
    ];

    /// 返回此游戏模式的显示字符串。
    #[must_use]
    pub const fn to_str(&self) -> &'static str {
        match self {
            Self::Survival => "Survival",
            Self::Creative => "Creative",
            Self::Adventure => "Adventure",
            Self::Spectator => "Spectator",
        }
    }

    /// 返回此游戏模式的小写名称。
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Survival => "survival",
            Self::Creative => "creative",
            Self::Adventure => "adventure",
            Self::Spectator => "spectator",
        }
    }
}

impl TryFrom<i8> for GameMode {
    type Error = ();

    /// 尝试将 `i8` 值转换为 [`GameMode`]。
    ///
    /// # Parameters
    /// - `value`：游戏模式的数字表示。
    ///
    /// # Returns
    /// - 若该值对应有效游戏模式，则返回 `Ok(GameMode)`：
    ///   - `0` → `Survival`
    ///   - `1` → `Creative`
    ///   - `2` → `Adventure`
    ///   - `3` → `Spectator`
    /// - 若该值不对应任何有效游戏模式，则返回 `Err(())`。
    fn try_from(value: i8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Survival),
            1 => Ok(Self::Creative),
            2 => Ok(Self::Adventure),
            3 => Ok(Self::Spectator),
            _ => Err(()),
        }
    }
}

impl TryFrom<i32> for GameMode {
    type Error = ();

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Survival),
            1 => Ok(Self::Creative),
            2 => Ok(Self::Adventure),
            3 => Ok(Self::Spectator),
            _ => Err(()),
        }
    }
}

impl TryFrom<u8> for GameMode {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Survival),
            1 => Ok(Self::Creative),
            2 => Ok(Self::Adventure),
            3 => Ok(Self::Spectator),
            _ => Err(()),
        }
    }
}

impl FromStr for GameMode {
    type Err = ParseGameModeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "survival" => Ok(Self::Survival),
            "creative" => Ok(Self::Creative),
            "adventure" => Ok(Self::Adventure),
            "spectator" => Ok(Self::Spectator),
            _ => Err(ParseGameModeError),
        }
    }
}
