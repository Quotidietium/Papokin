//! 插件自定义伤害类型注册与构建器工具。
//!
//! 本模块提供流畅、类型安全的 API，用于定义、注册和查询
//! 伤害类型。自定义伤害类型通过 `damage_type`
//! 注册表同步到客户端，并可按名称通过
//! `damage_by_name` 对生物实体造成伤害。
//!
//! 仅能在服务器启动（插件加载）期间注册；
//! 注册表冻结后，注册会以
//! [`DamageTypeError::RegistrationFailed`] 失败。
//!
//! # Examples
//!
//! ```rust,ignore
//! use papokin_plugin_api::{
//!     damage_type::{DamageEffects, DamageScaling, DamageTypeBuilder},
//!     Context,
//! };
//!
//! fn register_damage_types(context: &Context) {
//!     context
//!         .register_damage_type(
//!             DamageTypeBuilder::new("my_plugin:frost", "frost")
//!                 .scaling(DamageScaling::Always)
//!                 .exhaustion(0.1)
//!                 .effects(DamageEffects::Freezing),
//!         )
//!         .expect("failed to register custom damage type");
//! }
//! ```

pub use crate::wit::papokin::plugin::damage_types::{
    CustomDamageType, DamageEffects, DamageScaling, DamageTypeManager, DeathMessageType,
};
use crate::{Context, Server};
use std::fmt;

/// 构建或注册自定义伤害类型时可能发生的错误。
#[derive(Clone, Debug, PartialEq)]
pub enum DamageTypeError {
    /// 伤害类型名称为空。
    EmptyName,
    /// 死亡消息 ID 为空。
    EmptyMessageId,
    /// 向服务器的伤害类型管理器注册失败。
    RegistrationFailed(String),
}

impl fmt::Display for DamageTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => write!(f, "伤害类型名称不能为空"),
            Self::EmptyMessageId => write!(f, "伤害类型消息 id 不能为空"),
            Self::RegistrationFailed(msg) => {
                write!(f, "注册伤害类型失败：{msg}")
            }
        }
    }
}

impl std::error::Error for DamageTypeError {}

/// 用于构建并校验 [`CustomDamageType`] 的流式构建器。
pub struct DamageTypeBuilder {
    name: String,
    message_id: String,
    scaling: DamageScaling,
    exhaustion: f32,
    effects: Option<DamageEffects>,
    death_message_type: DeathMessageType,
}

impl DamageTypeBuilder {
    /// 以给定的命名空间名称和死亡消息 id 创建新的伤害类型构建器。
    ///
    /// 该名称必须是 `minecraft` 命名空间之外的带命名空间 id
    /// （如 `"my_plugin:frost"`）；message id 是死亡消息键
    /// 后缀（`death.attack.<message_id>`）。
    ///
    /// # Example
    /// ```rust,ignore
    /// let builder = DamageTypeBuilder::new("my_plugin:frost", "frost");
    /// ```
    #[must_use]
    pub fn new(name: impl Into<String>, message_id: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            message_id: message_id.into(),
            scaling: DamageScaling::WhenCausedByLivingNonPlayer,
            exhaustion: 0.1,
            effects: None,
            death_message_type: DeathMessageType::Default,
        }
    }

    /// 设置伤害量随难度缩放的条件
    /// （默认：[`DamageScaling::WhenCausedByLivingNonPlayer`]）。
    #[must_use]
    pub const fn scaling(mut self, scaling: DamageScaling) -> Self {
        self.scaling = scaling;
        self
    }

    /// 设置受到此伤害的玩家所增加的消耗（默认：0.1）。
    #[must_use]
    pub const fn exhaustion(mut self, exhaustion: f32) -> Self {
        self.exhaustion = exhaustion;
        self
    }

    /// 设置受伤音效/视觉效果的覆盖（默认：无）。
    #[must_use]
    pub const fn effects(mut self, effects: DamageEffects) -> Self {
        self.effects = Some(effects);
        self
    }

    /// 设置死亡消息的构成方式（默认：[`DeathMessageType::Default`]）。
    #[must_use]
    pub const fn death_message_type(mut self, death_message_type: DeathMessageType) -> Self {
        self.death_message_type = death_message_type;
        self
    }

    /// 验证伤害类型参数。
    ///
    /// # Errors
    /// 若校验失败，返回 [`DamageTypeError`]。
    pub fn validate(&self) -> Result<(), DamageTypeError> {
        if self.name.trim().is_empty() {
            return Err(DamageTypeError::EmptyName);
        }
        if self.message_id.trim().is_empty() {
            return Err(DamageTypeError::EmptyMessageId);
        }
        Ok(())
    }

    /// 构建校验后的 [`CustomDamageType`] 记录。
    ///
    /// # Errors
    /// 若校验失败，返回 [`DamageTypeError`]。
    pub fn build(self) -> Result<CustomDamageType, DamageTypeError> {
        self.validate()?;
        Ok(CustomDamageType {
            name: self.name,
            message_id: self.message_id,
            scaling: self.scaling,
            exhaustion: self.exhaustion,
            effects: self.effects,
            death_message_type: self.death_message_type,
        })
    }

    /// 直接向所提供的 [`DamageTypeManager`] 注册此自定义伤害类型。
    ///
    /// # Errors
    /// 若校验或注册失败，返回 [`DamageTypeError`]。
    pub fn register(self, manager: &DamageTypeManager) -> Result<(), DamageTypeError> {
        manager.register(self)
    }

    /// 向服务器注册此自定义伤害类型。
    ///
    /// # Errors
    /// 若校验或注册失败，返回 [`DamageTypeError`]。
    pub fn register_to_server(self, server: &Server) -> Result<(), DamageTypeError> {
        let manager = server.get_damage_type_manager();
        manager.register(self)
    }

    /// 向插件上下文注册此自定义伤害类型。
    ///
    /// # Errors
    /// 若校验或注册失败，返回 [`DamageTypeError`]。
    pub fn register_to_context(self, context: &Context) -> Result<(), DamageTypeError> {
        let manager = context.get_damage_type_manager();
        manager.register(self)
    }
}

/// 可通过 [`DamageTypeManager`] 注册的伤害类型 trait。
pub trait RegistrableDamageType {
    /// 向所提供的伤害类型管理器注册此伤害类型。
    ///
    /// # Errors
    /// 若校验或注册失败，返回 [`DamageTypeError`]。
    fn register(self, manager: &DamageTypeManager) -> Result<(), DamageTypeError>;
}

impl RegistrableDamageType for DamageTypeBuilder {
    fn register(self, manager: &DamageTypeManager) -> Result<(), DamageTypeError> {
        let damage_type = self.build()?;
        manager
            .register_damage_type(&damage_type)
            .map_err(DamageTypeError::RegistrationFailed)
    }
}

impl RegistrableDamageType for CustomDamageType {
    fn register(self, manager: &DamageTypeManager) -> Result<(), DamageTypeError> {
        manager
            .register_damage_type(&self)
            .map_err(DamageTypeError::RegistrationFailed)
    }
}

impl DamageTypeManager {
    /// 向服务器注册一个自定义伤害类型。
    ///
    /// 接受 [`DamageTypeBuilder`] 或 [`CustomDamageType`]。
    ///
    /// # Errors
    /// 若校验或注册失败，返回 [`DamageTypeError`]。
    pub fn register(&self, damage_type: impl RegistrableDamageType) -> Result<(), DamageTypeError> {
        damage_type.register(self)
    }

    /// 按命名空间名称获取已注册的自定义伤害类型。
    #[must_use]
    pub fn get(&self, name: &str) -> Option<CustomDamageType> {
        self.get_damage_type(name)
    }

    /// 检查服务器上是否注册了某个自定义伤害类型名。
    #[must_use]
    pub fn has(&self, name: &str) -> bool {
        self.has_damage_type(name)
    }

    /// 按注册顺序返回所有已注册自定义伤害类型的名称。
    #[must_use]
    pub fn get_all_custom_names(&self) -> Vec<String> {
        self.get_all_custom_damage_type_names()
    }
}

impl Context {
    /// 返回全局伤害类型管理器，用于注册和查询自定义伤害类型。
    #[must_use]
    pub fn get_damage_type_manager(&self) -> DamageTypeManager {
        self.get_server().get_damage_type_manager()
    }

    /// 向服务器注册一个自定义伤害类型。
    ///
    /// # Errors
    /// 若校验或注册失败，返回 [`DamageTypeError`]。
    pub fn register_damage_type(
        &self,
        damage_type: impl RegistrableDamageType,
    ) -> Result<(), DamageTypeError> {
        self.get_damage_type_manager().register(damage_type)
    }
}

impl Server {
    /// 向服务器注册一个自定义伤害类型。
    ///
    /// # Errors
    /// 若校验或注册失败，返回 [`DamageTypeError`]。
    pub fn register_damage_type(
        &self,
        damage_type: impl RegistrableDamageType,
    ) -> Result<(), DamageTypeError> {
        self.get_damage_type_manager().register(damage_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_type_builder_defaults() {
        let builder = DamageTypeBuilder::new("my_plugin:frost", "frost");
        assert!(matches!(
            builder.scaling,
            DamageScaling::WhenCausedByLivingNonPlayer
        ));
        assert!((builder.exhaustion - 0.1).abs() < f32::EPSILON);
        assert!(builder.effects.is_none());
        assert!(matches!(
            builder.death_message_type,
            DeathMessageType::Default
        ));
    }

    #[test]
    fn damage_type_builder_validation() {
        let builder = DamageTypeBuilder::new("", "frost");
        assert_eq!(builder.validate(), Err(DamageTypeError::EmptyName));

        let builder = DamageTypeBuilder::new("my_plugin:frost", "");
        assert_eq!(builder.validate(), Err(DamageTypeError::EmptyMessageId));
    }
}
