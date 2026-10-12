use crate::entity::mob::Mob;
use papokin_util::math::subtract_angles;

pub mod body_rotation_control;
pub mod flying_move_control;
pub mod jump_control;
pub mod look_control;
pub mod move_control;
pub mod smooth_swimming_look_control;
pub mod smooth_swimming_move_control;

pub trait Control: Send + Sync {
    fn change_angle(&self, start: f32, end: f32, max_change: f32) -> f32 {
        let i = subtract_angles(start, end);
        let j = i.clamp(-max_change, max_change);
        start + j
    }
}

pub trait MoveControlTrait: Control {
    fn tick(&mut self, mob: &dyn Mob);

    fn set_wanted_position(&mut self, _x: f64, _y: f64, _z: f64, _speed_modifier: f64) {}

    fn strafe(&mut self, _forward: f32, _right: f32) {}

    fn has_wanted(&self) -> bool {
        false
    }
}

/// 移动控制的逐实体内联承载（轮次 44）。
///
/// 旧承载 `Mutex<Box<dyn MoveControlTrait>>` 对每个生物做一次
/// 堆分配装盒；常用变体（地面默认、史莱姆跳跃）直接内联进枚举，
/// 未接线的特殊控制仍走 `Other` 盒保持兼容。
pub enum AnyMoveControl {
    /// 地面默认控制（全体生物的构造默认）。
    Ground(move_control::MoveControl),
    /// 史莱姆跳跃控制。
    Slime(crate::entity::mob::slime::SlimeMoveControl),
    /// 未内联的特殊控制（按需装盒）。
    Other(Box<dyn MoveControlTrait>),
}

impl Control for AnyMoveControl {}

impl MoveControlTrait for AnyMoveControl {
    fn tick(&mut self, mob: &dyn Mob) {
        match self {
            Self::Ground(control) => control.tick(mob),
            Self::Slime(control) => control.tick(mob),
            Self::Other(control) => control.tick(mob),
        }
    }

    fn set_wanted_position(&mut self, x: f64, y: f64, z: f64, speed_modifier: f64) {
        match self {
            Self::Ground(control) => control.set_wanted_position(x, y, z, speed_modifier),
            Self::Slime(control) => control.set_wanted_position(x, y, z, speed_modifier),
            Self::Other(control) => control.set_wanted_position(x, y, z, speed_modifier),
        }
    }

    fn strafe(&mut self, forward: f32, right: f32) {
        match self {
            Self::Ground(control) => control.strafe(forward, right),
            Self::Slime(control) => control.strafe(forward, right),
            Self::Other(control) => control.strafe(forward, right),
        }
    }

    fn has_wanted(&self) -> bool {
        match self {
            Self::Ground(control) => control.has_wanted(),
            Self::Slime(control) => control.has_wanted(),
            Self::Other(control) => control.has_wanted(),
        }
    }
}

impl AnyMoveControl {
    /// 全体生物的构造默认（地面控制，零堆分配）。
    #[must_use]
    pub fn ground_default() -> Self {
        Self::Ground(move_control::MoveControl::default())
    }
}
