use papokin_util::math::position::BlockPos;
use papokin_util::math::vector2::Vector2;
use papokin_util::math::vector3::Vector3;
use papokin_util::text::TextComponent;
use std::sync::Arc;

use crate::argument_types::entity_anchor::EntityAnchor;
use crate::errors::command_syntax_error::CommandSyntaxError;

pub trait ReturnValueCallable: Send + Sync {
    fn call(&self, value: ReturnValue);
}

pub type ReturnValueCallback = Arc<dyn ReturnValueCallable>;

#[derive(Clone, Default)]
pub struct ResultValueTaker(pub Vec<ReturnValueCallback>);

impl ResultValueTaker {
    #[must_use]
    pub fn merge(taker_1: &Self, taker_2: &Self) -> Self {
        let mut takers = Vec::with_capacity(taker_1.0.len() + taker_2.0.len());
        for taker in &taker_1.0 {
            takers.push(taker.clone());
        }
        for taker in &taker_2.0 {
            takers.push(taker.clone());
        }
        Self(takers)
    }

    #[must_use]
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn call(&self, return_value: ReturnValue) {
        for callback in &self.0 {
            callback.call(return_value);
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ReturnValue {
    Success(i32),
    Failure,
}

impl ReturnValue {
    #[must_use]
    pub const fn success_value(self) -> bool {
        match self {
            Self::Success(_) => true,
            Self::Failure => false,
        }
    }

    #[must_use]
    pub const fn result_value(self) -> i32 {
        match self {
            Self::Success(value) => value,
            Self::Failure => 0,
        }
    }
}

/// 用于命令分发与执行的抽象命令来源 trait。
pub trait CommandSource: Clone + Send + Sync + 'static {
    /// 向命令发送者发送反馈消息。
    fn send_message(&self, message: TextComponent);

    /// 向命令发送者发送错误消息。
    fn send_error(&self, error: TextComponent) {
        self.send_message(error);
    }

    /// 为命令的返回值触发回调。
    fn call_result(&self, _result: ReturnValue) {}

    /// 检查此命令来源是否拥有指定权限。
    fn has_permission(&self, _permission: &str) -> bool {
        true
    }

    /// 返回此命令源在世界中的位置。
    fn position(&self) -> Vector3<f64> {
        Vector3::default()
    }

    /// 返回此命令源的旋转（俯仰角，偏航角）。
    fn rotation(&self) -> Vector2<f32> {
        Vector2::default()
    }

    /// 检查给定位置的方块是否已加载。
    fn check_block_loaded(&self, _pos: &BlockPos) -> Result<(), CommandSyntaxError> {
        Ok(())
    }

    ///返回给定实体锚点的锚定位置。
    fn anchor_position(&self, _anchor: EntityAnchor) -> Vector3<f64> {
        self.position()
    }

    ///返回此命令来源当前的实体锚点。
    fn entity_anchor(&self) -> EntityAnchor {
        EntityAnchor::Feet
    }
}

#[derive(Clone, Default)]
pub struct DummySource {
    pub position: Vector3<f64>,
    pub rotation: Vector2<f32>,
    pub entity_anchor: EntityAnchor,
    pub command_result_taker: ResultValueTaker,
}

impl DummySource {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn dummy() -> Self {
        Self::default()
    }
}

impl CommandSource for DummySource {
    fn send_message(&self, _message: TextComponent) {}
    fn send_error(&self, _error: TextComponent) {}
    fn position(&self) -> Vector3<f64> {
        self.position
    }
    fn rotation(&self) -> Vector2<f32> {
        self.rotation
    }
    fn entity_anchor(&self) -> EntityAnchor {
        self.entity_anchor
    }
    fn call_result(&self, result: ReturnValue) {
        self.command_result_taker.call(result);
    }
}

impl CommandSource for () {
    fn send_message(&self, _message: TextComponent) {}
}

impl<S: CommandSource> CommandSource for Arc<S> {
    fn send_message(&self, message: TextComponent) {
        (**self).send_message(message);
    }

    fn send_error(&self, error: TextComponent) {
        (**self).send_error(error);
    }

    fn has_permission(&self, permission: &str) -> bool {
        (**self).has_permission(permission)
    }

    fn position(&self) -> Vector3<f64> {
        (**self).position()
    }

    fn rotation(&self) -> Vector2<f32> {
        (**self).rotation()
    }

    fn entity_anchor(&self) -> EntityAnchor {
        (**self).entity_anchor()
    }

    fn anchor_position(&self, anchor: EntityAnchor) -> Vector3<f64> {
        (**self).anchor_position(anchor)
    }

    fn call_result(&self, result: ReturnValue) {
        (**self).call_result(result);
    }

    fn check_block_loaded(&self, pos: &BlockPos) -> Result<(), CommandSyntaxError> {
        (**self).check_block_loaded(pos)
    }
}
