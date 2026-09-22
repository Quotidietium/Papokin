use papokin_data::attributes::Attributes;
use papokin_data::entity::EntityType;
use rustc_hash::FxHashMap;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
#[repr(i8)]
pub enum ModifierOperation {
    Add = 0,           // 添加值
    MultiplyBase = 1,  // 乘以基础值（base * (1 + x)）
    MultiplyTotal = 2, // 乘以总量（最后应用）
}

#[derive(Clone, Debug, PartialEq)]
pub struct Modifier {
    pub id: String,
    pub amount: f64,
    pub operation: ModifierOperation,
}

/// 运行时使用的每实体属性实例。
#[derive(Debug)]
pub struct AttributeInstance {
    pub base_value: f64,
    pub modifiers: Vec<Modifier>,
    pub cached_value: AtomicU64,
    pub dirty: AtomicBool,
}

impl AttributeInstance {
    #[must_use]
    pub const fn new(base_value: f64) -> Self {
        Self {
            base_value,
            modifiers: Vec::new(),
            cached_value: AtomicU64::new(base_value.to_bits()),
            dirty: AtomicBool::new(false),
        }
    }

    pub fn value(&self) -> f64 {
        if !self.dirty.load(Ordering::Relaxed) {
            return f64::from_bits(self.cached_value.load(Ordering::Relaxed));
        }

        let mut value = self.base_value;

        let mut add_sum = 0.0;
        let mut mul_base = 0.0;
        let mut mul_total = 1.0;
        for m in &self.modifiers {
            match m.operation {
                ModifierOperation::Add => add_sum += m.amount,
                ModifierOperation::MultiplyBase => mul_base += m.amount,
                ModifierOperation::MultiplyTotal => mul_total *= 1.0 + m.amount,
            }
        }

        value += add_sum;
        value *= 1.0 + mul_base;
        value *= mul_total;

        if value.is_nan() || value.is_infinite() {
            value = self.base_value;
        }

        self.cached_value.store(value.to_bits(), Ordering::Relaxed);
        self.dirty.store(false, Ordering::Relaxed);

        value
    }

    pub fn add_or_replace_modifier(&mut self, modifier: Modifier) {
        if let Some(pos) = self.modifiers.iter().position(|m| m.id == modifier.id) {
            self.modifiers.remove(pos);
        }
        self.modifiers.push(modifier);
        self.dirty.store(true, Ordering::Relaxed);
    }

    pub fn remove_modifier(&mut self, id: &str) {
        if let Some(pos) = self.modifiers.iter().position(|m| m.id == id) {
            self.modifiers.swap_remove(pos);
        }
        self.dirty.store(true, Ordering::Relaxed);
    }
}

/// 在单个数据包中为给定生物实体发送多个属性的更新。
pub fn send_attribute_updates_for_living(
    living: &crate::entity::living::LivingEntity,
    attributes: Vec<Attributes>,
) {
    use papokin_protocol::codec::var_int::VarInt;
    use papokin_protocol::java::client::play::AttributeModifier as JeAttrMod;
    use papokin_protocol::java::client::play::CUpdateAttributes as JePacket;
    use papokin_protocol::java::client::play::Property as JeProperty;

    let mut je_properties: Vec<JeProperty> = Vec::with_capacity(attributes.len());

    for attribute in attributes {
        let base_value = living.get_attribute_base(&attribute);

        // 拉取该属性的修饰符
        let mut modifiers = Vec::new();
        if let Some(inst) = living
            .attributes
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&attribute.id)
        {
            for mod_inst in &inst.modifiers {
                modifiers.push(JeAttrMod::new(
                    mod_inst.id.clone(),
                    mod_inst.amount,
                    mod_inst.operation as i8,
                ));
            }
        }

        // 将修饰符移入属性
        je_properties.push(JeProperty::new(
            VarInt(i32::from(attribute.id)),
            base_value,
            modifiers,
        ));
    }

    let je_packet = JePacket::new(living.entity.entity_id.into(), je_properties);
    let world = living.entity.world.load();
    world.broadcast_packet_all(&je_packet);
}

impl Clone for AttributeInstance {
    fn clone(&self) -> Self {
        Self {
            base_value: self.base_value,
            modifiers: self.modifiers.clone(),
            cached_value: AtomicU64::new(self.cached_value.load(Ordering::Relaxed)),
            dirty: AtomicBool::new(self.dirty.load(Ordering::Relaxed)),
        }
    }
}

/// 存储每种实体类型基础属性覆盖项的注册表。
/// 内部存储从 `entity_type.id` 到 `FxHashMap`<attribute.id, f64> 的映射，以实现 O(1) 查找。
#[derive(Default)]
pub struct AttributeRegistry {
    map: FxHashMap<u16, FxHashMap<u8, f64>>,
}

impl AttributeRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 获取给定实体类型 ID 的 `attribute` 基础值。
    /// 如果不存在覆盖项，则返回 `attribute.default_value`。
    #[must_use]
    pub fn get_base_value(&self, entity_type_id: u16, attribute: &Attributes) -> f64 {
        self.map
            .get(&entity_type_id)
            .and_then(|map| map.get(&attribute.id))
            .copied()
            .unwrap_or(attribute.default_value)
    }

    /// 返回给定实体类型 id 的覆盖项向量。
    /// 这允许在生成时填充每个实体本地的属性实例。
    #[must_use]
    pub fn get_overrides_for_entity(&self, entity_type_id: u16) -> Option<Vec<(u8, f64)>> {
        self.map
            .get(&entity_type_id)
            .map(|m| m.iter().map(|(&k, &v)| (k, v)).collect())
    }
}

/// 以声明式方式为实体类型组装属性覆盖项的构建器。
#[derive(Default)]
pub struct AttributeBuilder {
    entries: Vec<(Attributes, f64)>,
}

impl AttributeBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn add(mut self, attribute: Attributes, base: f64) -> Self {
        self.entries.push((attribute, base));
        self
    }

    #[must_use]
    pub fn build(self) -> Vec<(Attributes, f64)> {
        self.entries
    }
}

impl AttributeRegistry {
    /// 注册由 `AttributeBuilder` 为 `entity_type` 创建的覆盖项。
    pub fn register_builder(
        &mut self,
        entity_type: &'static EntityType,
        builder: AttributeBuilder,
    ) {
        let inner = self.map.entry(entity_type.id).or_default();
        for (attr, val) in builder.build() {
            inner.insert(attr.id, val);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use papokin_data::attributes::Attributes;
    use papokin_data::entity::EntityType;

    #[test]
    fn player_base_attributes() {
        let speed_attr = EntityType::PLAYER
            .attributes
            .iter()
            .find(|(attr, _)| attr.id == Attributes::MOVEMENT_SPEED.id);
        assert!(speed_attr.is_some());
        let (_, base_speed) = speed_attr.unwrap();
        assert!((base_speed - 0.1).abs() < 1e-4);
    }

    #[test]
    fn sprinting_modifier_calculation() {
        let mut instance = AttributeInstance::new(0.1);
        assert!((instance.value() - 0.1).abs() < f64::EPSILON);

        let sprinting_mod = Modifier {
            id: "minecraft:sprinting".to_string(),
            amount: 0.300_000_011_920_928_96,
            operation: ModifierOperation::MultiplyTotal,
        };

        instance.add_or_replace_modifier(sprinting_mod);
        let sprinting_value = instance.value();
        // 0.1 * (1.0 + 0.30000001192092896) = 0.1300000011920929
        assert!((sprinting_value - 0.130_000_001_192_092_9).abs() < 1e-9);

        instance.remove_modifier("minecraft:sprinting");
        assert!((instance.value() - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn attribute_modifier_operations() {
        let mut instance = AttributeInstance::new(10.0);

        // ADD：10.0 + 2.0 + 3.0 = 15.0
        instance.add_or_replace_modifier(Modifier {
            id: "add_1".to_string(),
            amount: 2.0,
            operation: ModifierOperation::Add,
        });
        instance.add_or_replace_modifier(Modifier {
            id: "add_2".to_string(),
            amount: 3.0,
            operation: ModifierOperation::Add,
        });
        assert!((instance.value() - 15.0).abs() < f64::EPSILON);

        // MULTIPLY_BASE：15.0 * (1.0 + 0.5 + 0.2) = 15.0 * 1.7 = 25.5
        instance.add_or_replace_modifier(Modifier {
            id: "mul_base_1".to_string(),
            amount: 0.5,
            operation: ModifierOperation::MultiplyBase,
        });
        instance.add_or_replace_modifier(Modifier {
            id: "mul_base_2".to_string(),
            amount: 0.2,
            operation: ModifierOperation::MultiplyBase,
        });
        assert!((instance.value() - 25.5).abs() < f64::EPSILON);

        // MULTIPLY_TOTAL：25.5 * (1.0 + 0.1) * (1.0 + 0.2) = 25.5 * 1.1 * 1.2 = 33.66
        instance.add_or_replace_modifier(Modifier {
            id: "mul_total_1".to_string(),
            amount: 0.1,
            operation: ModifierOperation::MultiplyTotal,
        });
        instance.add_or_replace_modifier(Modifier {
            id: "mul_total_2".to_string(),
            amount: 0.2,
            operation: ModifierOperation::MultiplyTotal,
        });
        assert!((instance.value() - 33.66).abs() < 1e-9);
    }
}
