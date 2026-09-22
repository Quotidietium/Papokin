use crate::wit::papokin::plugin::advancement::{AdvancementProgress, FrameType};

impl AdvancementProgress {
    ///若该进度已全部完成，则返回 `true`。
    #[must_use]
    pub const fn is_done(&self) -> bool {
        self.done
    }

    /// 检查某个判据是否已达成。
    #[must_use]
    pub fn is_criterion_done(&self, criterion: &str) -> bool {
        self.awarded_criteria.iter().any(|c| c == criterion)
    }

    ///返回已达成判据名称的切片。
    #[must_use]
    pub fn get_awarded_criteria(&self) -> &[String] {
        &self.awarded_criteria
    }

    ///返回剩余判据名称的切片。
    #[must_use]
    pub fn get_remaining_criteria(&self) -> &[String] {
        &self.remaining_criteria
    }
}

impl FrameType {
    /// 返回此边框类型对应的原版翻译键后缀或标识符。
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::Challenge => "challenge",
            Self::Goal => "goal",
        }
    }
}
