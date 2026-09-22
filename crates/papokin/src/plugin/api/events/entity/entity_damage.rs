use papokin_data::damage::DamageType;
use papokin_data::damage_ext::ResolvedDamageType;
use papokin_macros::{Event, cancellable};

/// 实体受到伤害时发生的事件。
#[cancellable]
#[derive(Event, Clone)]
pub struct EntityDamageEvent {
    /// 受到伤害的实体 ID。
    pub entity_id: i32,

    /// 受到的伤害量。
    pub damage: f32,

    /// 伤害类型。当伤害是由插件注册的
    /// 自定义伤害类型时，此值为 `DamageType::GENERIC`，且
    /// [`Self::custom_damage_type`] 携带该自定义类型的命名空间 id。
    pub damage_type: DamageType,

    /// 此伤害类型对应的插件注册自定义伤害类型的命名空间 id
    /// 造成伤害所用的类型（如有）。
    pub custom_damage_type: Option<String>,
}

impl EntityDamageEvent {
    #[must_use]
    pub const fn new(entity_id: i32, damage_type: DamageType, damage_amount: f32) -> Self {
        Self {
            entity_id,
            damage: damage_amount,
            damage_type,
            custom_damage_type: None,
            cancelled: false,
        }
    }

    /// 为已解析的（原版或自定义）伤害类型创建事件。
    /// 自定义类型通过 [`Self::custom_damage_type`] 报告，并
    /// 以 `DamageType::GENERIC` 作为静态替身。
    #[must_use]
    pub fn new_resolved(
        entity_id: i32,
        damage_type: &ResolvedDamageType,
        damage_amount: f32,
    ) -> Self {
        Self {
            entity_id,
            damage: damage_amount,
            damage_type: damage_type.vanilla_or(DamageType::GENERIC),
            custom_damage_type: damage_type.custom_name().map(str::to_string),
            cancelled: false,
        }
    }
}
