use std::str::FromStr;

use num_derive::{FromPrimitive, ToPrimitive};
use serde::{Deserialize, Serialize};

/// 从字符串解析 [`Difficulty`] 失败时返回的错误。
pub struct ParseDifficultyError;

/// 表示游戏的难度等级。
///
/// 每个数值对应一个特定的难度：
/// - `Peaceful`（0）：不生成敌对生物，生命值自然恢复。
/// - `Easy`（1）：标准玩法，敌对生物生成时伤害降低。
/// - `Normal`（2）：标准难度，生物造成全额伤害。
/// - `Hard`（3）：敌对生物造成额外伤害，且生命值恢复受限。
#[derive(Serialize, Deserialize, FromPrimitive, ToPrimitive, PartialEq, Eq, Clone, Copy, Debug)]
pub enum Difficulty {
    /// 无敌对生物；生命值自然恢复。
    Peaceful = 0,
    /// 简单难度；敌对生物造成的伤害降低。
    Easy = 1,
    /// 普通难度；标准的生物伤害与生命值。
    Normal = 2,
    /// 困难难度；怪物伤害提高，生命值恢复受限。
    Hard = 3,
}

impl Difficulty {
    /// 获取此难度的小写名称。
    /// 例如，[`Difficulty::Peaceful`] 会得到 `"peaceful"`。
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Peaceful => "peaceful",
            Self::Easy => "easy",
            Self::Normal => "normal",
            Self::Hard => "hard",
        }
    }

    /// 获取此难度的翻译键。
    /// 例如，[`Difficulty::Peaceful`] 会得到 `"options.difficulty.peaceful"`。
    #[must_use]
    pub const fn translation_key(self) -> &'static str {
        match self {
            Self::Peaceful => "options.difficulty.peaceful",
            Self::Easy => "options.difficulty.easy",
            Self::Normal => "options.difficulty.normal",
            Self::Hard => "options.difficulty.hard",
        }
    }
}

impl FromStr for Difficulty {
    type Err = ParseDifficultyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "peaceful" => Ok(Self::Peaceful),
            "easy" => Ok(Self::Easy),
            "normal" => Ok(Self::Normal),
            "hard" => Ok(Self::Hard),
            _ => Err(ParseDifficultyError),
        }
    }
}
