//! 实体属性表的写时物化覆盖层。
//!
//! 旧承载是构造时把实体类型声明的全部属性（生物普遍 26-36 条）
//! 逐条插入 `FxHashMap<u8, AttributeInstance>`：hashbrown 终容量
//! 56 桶约 3.2KB/实体外加 5 次增长分配，而其中绝大多数条目终生
//! 保持「基础值 = 类型表值、修饰符为空」的初生态——这份表本质
//! 是静态类型表的逐实体抄写。
//!
//! 覆盖层只在与静态表分叉时物化实例（写时物化），典型生物的
//! 覆盖层为空（零堆分配）。三态读取视图 [`AttributeView`] 把
//! 「覆盖层命中 / 静态表声明 / 不在场」统一成与旧 map 逐点对齐
//! 的语义，各调用方的回落路径（全局默认/类型表值/报错）保持
//! 原位不变。

use super::attributes::AttributeInstance;
use super::attributes::Modifier;
use papokin_data::attributes::Attributes;
use papokin_data::entity::EntityType;

/// 属性读取视图。
#[derive(Debug, Clone, Copy)]
pub enum AttributeView<'a> {
    /// 覆盖层已物化的实例（与静态表分叉或动态添加）。
    Live(&'a AttributeInstance),
    /// 实体类型声明了该属性且未分叉：等同初生实例
    /// （计算值 = 基础值，修饰符为空）。
    Declared {
        /// 该属性在此实体类型上的声明基础值。
        base: f64,
    },
    /// 不在场：未声明且未动态添加，或已被显式移除。
    Absent,
}

impl AttributeView<'_> {
    /// 是否「在场」（旧 map 的 `contains_key` 语义）。
    #[must_use]
    pub const fn is_present(&self) -> bool {
        !matches!(self, Self::Absent)
    }

    /// 基础值；不在场时为 `None`。
    #[must_use]
    pub const fn base_value(&self) -> Option<f64> {
        match self {
            Self::Live(inst) => Some(inst.base_value),
            Self::Declared { base } => Some(*base),
            Self::Absent => None,
        }
    }

    /// 计算值；不在场时为 `None`。
    #[must_use]
    pub fn value(&self) -> Option<f64> {
        match self {
            Self::Live(inst) => Some(inst.value()),
            Self::Declared { base } => Some(*base),
            Self::Absent => None,
        }
    }

    /// 修饰符切片；不在场或未分叉时为空切片。
    #[must_use]
    pub fn modifiers(&self) -> &[Modifier] {
        match self {
            Self::Live(inst) => &inst.modifiers,
            Self::Declared { base: _ } | Self::Absent => &[],
        }
    }
}

/// 实体属性覆盖层。
#[derive(Debug)]
pub struct AttributeMap {
    /// 实体类型静态表：未分叉属性的取值来源。
    entity_type: &'static EntityType,
    /// 每类型「属性 id → 声明基础值」直接索引表（懒构建、跨实体
    /// 共享）：读取一次下标即达，替代对类型表的线性扫描。
    bases: std::sync::Arc<TypeBases>,
    /// 已物化条目，按属性 id 升序（二分查找）。
    entries: Vec<(u8, AttributeInstance)>,
    /// 被显式移除的声明属性墓碑：读取须回落到「不在场」
    /// （复刻旧 map `remove` 后的取值路径——读取方对缺失
    /// 实例的回落目标是全局默认值而非类型表值）。
    removed: Vec<u8>,
}

/// 以属性 id 为下标的声明基础值表；未声明槽位为 NaN。
///
/// 生成表的基础值全部是有限数字面量（原版数据），NaN 可安全
/// 充当「未声明」哨兵；构建时的 `debug_assert` 兜底把关。
#[derive(Debug)]
struct TypeBases {
    by_id: Box<[f64]>,
}

impl TypeBases {
    fn build(entity_type: &EntityType) -> Self {
        let len = Attributes::ALL
            .iter()
            .map(|a| usize::from(a.id))
            .max()
            .map_or(0, |max| max + 1);
        let mut by_id = vec![f64::NAN; len];
        for (attr, base) in entity_type.attributes {
            debug_assert!(base.is_finite(), "生成表基础值应为有限数");
            by_id[usize::from(attr.id)] = *base;
        }
        Self {
            by_id: by_id.into_boxed_slice(),
        }
    }

    /// 声明基础值；未声明（或越界 id）时为 `None`。
    #[must_use]
    pub fn base(&self, id: u8) -> Option<f64> {
        let value = *self.by_id.get(usize::from(id))?;
        if value.is_nan() { None } else { Some(value) }
    }
}

/// 每类型直索引表缓存（懒构建，每类型只建一次、全体实体共享）。
static TYPE_BASES: std::sync::OnceLock<
    std::sync::Mutex<rustc_hash::FxHashMap<u16, std::sync::Arc<TypeBases>>>,
> = std::sync::OnceLock::new();

fn type_bases(entity_type: &'static EntityType) -> std::sync::Arc<TypeBases> {
    let cache = TYPE_BASES.get_or_init(|| std::sync::Mutex::new(rustc_hash::FxHashMap::default()));
    let mut guard = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard
        .entry(entity_type.id)
        .or_insert_with(|| std::sync::Arc::new(TypeBases::build(entity_type)))
        .clone()
}

impl AttributeMap {
    /// 创建空覆盖层（条目零堆分配；类型直索引表首类型实体构建）。
    #[must_use]
    pub fn new(entity_type: &'static EntityType) -> Self {
        Self {
            entity_type,
            bases: type_bases(entity_type),
            entries: Vec::new(),
            removed: Vec::new(),
        }
    }

    /// 静态表声明基础值；未声明时为 `None`。
    fn declared_base(&self, id: u8) -> Option<f64> {
        self.bases.base(id)
    }

    /// 三态读取视图。
    #[must_use]
    pub fn view(&self, id: u8) -> AttributeView<'_> {
        if let Ok(pos) = self.entries.binary_search_by_key(&id, |(k, _)| *k) {
            return AttributeView::Live(&self.entries[pos].1);
        }
        if self.removed.contains(&id) {
            return AttributeView::Absent;
        }
        self.declared_base(id)
            .map_or(AttributeView::Absent, |base| AttributeView::Declared {
                base,
            })
    }

    /// 是否「在场」（旧 `contains_key`）。
    #[must_use]
    pub fn is_present(&self, id: u8) -> bool {
        self.view(id).is_present()
    }

    /// 可变访问并在需要时物化：
    /// - 覆盖层命中 → 直接返回（旧 map 对已存在键的 `get_mut`）；
    /// - 声明但未分叉 → 物化一个基础值取自类型表的实例再返回
    ///   （旧 map 构造时已为全部声明属性建实例，`get_mut` 必命中）；
    /// - 墓碑或未声明 → `None`（旧 map 对已移除/不存在的键返回
    ///   `None`，调用方据此走报错或静默跳过路径）。
    pub fn get_mut(&mut self, id: u8) -> Option<&mut AttributeInstance> {
        let pos = self.entries.binary_search_by_key(&id, |(k, _)| *k);
        match pos {
            Ok(pos) => Some(&mut self.entries[pos].1),
            Err(pos) => {
                if self.removed.contains(&id) {
                    return None;
                }
                let base = self.declared_base(id)?;
                self.entries.insert(pos, (id, AttributeInstance::new(base)));
                Some(&mut self.entries[pos].1)
            }
        }
    }

    /// 取或插入（旧 `entry().or_insert_with`）：覆盖层命中则返回；
    /// 否则物化 `init` 给出的实例。两条路径都清除墓碑——旧
    /// `entry` 语义在 `remove` 之后会重新插入键。
    pub fn get_or_insert_with(
        &mut self,
        id: u8,
        init: impl FnOnce() -> AttributeInstance,
    ) -> &mut AttributeInstance {
        let pos = self.entries.binary_search_by_key(&id, |(k, _)| *k);
        let pos = match pos {
            Ok(pos) => pos,
            Err(pos) => {
                self.removed.retain(|&r| r != id);
                self.entries.insert(pos, (id, init()));
                pos
            }
        };
        &mut self.entries[pos].1
    }

    /// 插入或替换（旧 `HashMap::insert` 后者胜语义）：原位替换
    /// 已物化条目、否则按 id 序插入，并清除墓碑。
    pub fn insert(&mut self, id: u8, inst: AttributeInstance) {
        match self.entries.binary_search_by_key(&id, |(k, _)| *k) {
            Ok(pos) => self.entries[pos].1 = inst,
            Err(pos) => self.entries.insert(pos, (id, inst)),
        }
        self.removed.retain(|&r| r != id);
    }

    /// 显式移除（旧 `remove`）：删除物化条目；对声明属性压入
    /// 墓碑使后续读取「不在场」。对不在场的属性是空操作，
    /// 重复移除不产生重复墓碑。
    pub fn remove(&mut self, id: u8) {
        if let Ok(pos) = self.entries.binary_search_by_key(&id, |(k, _)| *k) {
            self.entries.remove(pos);
        }
        if self.declared_base(id).is_some() && !self.removed.contains(&id) {
            self.removed.push(id);
        }
    }

    /// 遍历全部在场属性（合并视图）：类型表声明条目（覆盖层
    /// 命中处给 `Live`，墓碑跳过）+ 覆盖层中动态添加的未声明
    /// 条目。迭代顺序未定义，调用方如需定序须自行排序。
    pub fn for_each_present(&self, mut f: impl FnMut(u8, AttributeView<'_>)) {
        for (attr, base) in self.entity_type.attributes {
            let id = attr.id;
            if self.removed.contains(&id) {
                continue;
            }
            let view = self
                .entries
                .binary_search_by_key(&id, |(k, _)| *k)
                .map_or(AttributeView::Declared { base: *base }, |pos| {
                    AttributeView::Live(&self.entries[pos].1)
                });
            f(id, view);
        }
        for (id, inst) in &self.entries {
            if self.declared_base(*id).is_none() {
                f(*id, AttributeView::Live(inst));
            }
        }
    }

    /// 在场属性数（合并视图计数，供容量提示使用）。
    #[must_use]
    pub const fn present_count(&self) -> usize {
        self.entity_type.attributes.len() + self.entries.len() - self.removed.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use papokin_data::attributes::Attributes;
    use papokin_data::entity::EntityType;

    /// 旧承载的语义基线：fresh 实体上声明属性的读取值是类型表
    /// 值而非全局默认值（僵尸 `FOLLOW_RANGE=35` ≠ 默认 32）。
    #[test]
    fn fresh_declared_read_uses_type_table_base() {
        let map = AttributeMap::new(&EntityType::ZOMBIE);
        assert!(map.is_present(Attributes::FOLLOW_RANGE.id));
        assert_eq!(
            map.view(Attributes::FOLLOW_RANGE.id).value(),
            Some(35.0),
            "僵尸 follow_range 应取类型表值 35，而非全局默认 32"
        );
        assert_eq!(
            map.view(Attributes::FOLLOW_RANGE.id).base_value(),
            Some(35.0)
        );
        assert!(map.view(Attributes::FOLLOW_RANGE.id).modifiers().is_empty());
    }

    /// 未声明且未动态添加的属性不在场（僵尸无 `attack_speed`）。
    #[test]
    fn undeclared_attribute_is_absent() {
        let map = AttributeMap::new(&EntityType::ZOMBIE);
        assert!(!map.is_present(Attributes::ATTACK_SPEED.id));
        assert_eq!(map.view(Attributes::ATTACK_SPEED.id).base_value(), None);
        assert_eq!(map.view(Attributes::ATTACK_SPEED.id).value(), None);
    }

    /// 写时物化：`get_mut` 对声明属性命中并把后续读取转为 `Live`
    /// （按生产写路径的约定，写后置 dirty）。
    #[test]
    fn get_mut_materializes_declared_attribute() {
        let mut map = AttributeMap::new(&EntityType::ZOMBIE);
        let inst = map.get_mut(Attributes::MAX_HEALTH.id);
        assert!(
            inst.is_some(),
            "声明属性的 get_mut 必命中（旧 map 构造即建实例）"
        );
        let inst = inst.unwrap();
        inst.base_value = 40.0;
        inst.dirty.store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(matches!(
            map.view(Attributes::MAX_HEALTH.id),
            AttributeView::Live(_)
        ));
        assert_eq!(map.view(Attributes::MAX_HEALTH.id).value(), Some(40.0));
    }

    /// `get_mut` 对未声明属性返回 `None`（不物化捏造实例）。
    #[test]
    fn get_mut_undeclared_returns_none() {
        let mut map = AttributeMap::new(&EntityType::ZOMBIE);
        assert!(map.get_mut(Attributes::ATTACK_SPEED.id).is_none());
        assert!(!map.is_present(Attributes::ATTACK_SPEED.id));
    }

    /// 显式移除：声明属性读取转为不在场（回落全局默认的取值
    /// 路径由调用方持有），且不重复压墓碑。
    #[test]
    fn remove_declared_makes_absent() {
        let mut map = AttributeMap::new(&EntityType::ZOMBIE);
        map.remove(Attributes::FOLLOW_RANGE.id);
        map.remove(Attributes::FOLLOW_RANGE.id);
        assert!(!map.is_present(Attributes::FOLLOW_RANGE.id));
        assert_eq!(map.view(Attributes::FOLLOW_RANGE.id).value(), None);
        // 墓碑不重复：present_count 少 1 而非 2
        assert_eq!(map.present_count(), EntityType::ZOMBIE.attributes.len() - 1);
    }

    /// 移除已物化的声明属性：条目删除 + 墓碑生效。
    #[test]
    fn remove_materialized_declared_tombstones() {
        let mut map = AttributeMap::new(&EntityType::ZOMBIE);
        if let Some(inst) = map.get_mut(Attributes::MAX_HEALTH.id) {
            inst.base_value = 99.0;
        }
        map.remove(Attributes::MAX_HEALTH.id);
        assert!(!map.is_present(Attributes::MAX_HEALTH.id));
        assert!(map.get_mut(Attributes::MAX_HEALTH.id).is_none());
    }

    /// 移除动态添加的未声明属性：无需墓碑即自然不在场。
    #[test]
    fn remove_dynamic_undeclared_is_natural_absent() {
        let mut map = AttributeMap::new(&EntityType::ZOMBIE);
        map.get_or_insert_with(Attributes::ATTACK_SPEED.id, || AttributeInstance::new(4.0))
            .base_value = 4.0;
        assert!(map.is_present(Attributes::ATTACK_SPEED.id));
        map.remove(Attributes::ATTACK_SPEED.id);
        assert!(!map.is_present(Attributes::ATTACK_SPEED.id));
    }

    /// `get_or_insert_with` 复活墓碑（旧 `entry` 语义：`remove` 后
    /// 再 `entry` 会重新插入）。
    #[test]
    fn get_or_insert_with_revives_tombstone() {
        let mut map = AttributeMap::new(&EntityType::ZOMBIE);
        map.remove(Attributes::FOLLOW_RANGE.id);
        let inst =
            map.get_or_insert_with(Attributes::FOLLOW_RANGE.id, || AttributeInstance::new(35.0));
        inst.add_or_replace_modifier(super::super::attributes::Modifier {
            id: "test:mod".into(),
            amount: 1.0,
            operation: super::super::attributes::ModifierOperation::Add,
        });
        assert!(map.is_present(Attributes::FOLLOW_RANGE.id));
        assert_eq!(map.view(Attributes::FOLLOW_RANGE.id).value(), Some(36.0));
    }

    /// 合并视图：fresh 实体遍历全部声明属性；移除一个少一个；
    /// 动态添加一个多一个。
    #[test]
    fn merged_view_counts_match_old_map_contents() {
        let mut map = AttributeMap::new(&EntityType::ZOMBIE);
        let mut count = 0;
        map.for_each_present(|_, _| count += 1);
        assert_eq!(count, EntityType::ZOMBIE.attributes.len());

        map.remove(Attributes::ARMOR.id);
        let mut ids = Vec::new();
        map.for_each_present(|id, _| ids.push(id));
        assert_eq!(ids.len(), EntityType::ZOMBIE.attributes.len() - 1);
        assert!(!ids.contains(&Attributes::ARMOR.id));

        map.get_or_insert_with(Attributes::ATTACK_SPEED.id, || AttributeInstance::new(4.0));
        let mut ids = Vec::new();
        map.for_each_present(|id, _| ids.push(id));
        assert_eq!(ids.len(), EntityType::ZOMBIE.attributes.len());
        assert!(ids.contains(&Attributes::ATTACK_SPEED.id));
    }

    /// Live 视图按修饰符全公式计算，Declared 视图值恒等于基础值。
    #[test]
    fn live_view_computes_with_modifiers() {
        use super::super::attributes::{Modifier, ModifierOperation};
        let mut map = AttributeMap::new(&EntityType::ZOMBIE);
        let inst = map.get_mut(Attributes::MOVEMENT_SPEED.id).unwrap();
        inst.add_or_replace_modifier(Modifier {
            id: "minecraft:sprinting".into(),
            amount: 0.300_000_011_920_928_96,
            operation: ModifierOperation::MultiplyTotal,
        });
        let expected = 0.230_000_004_172_325_1f64 * (1.0 + 0.300_000_011_920_928_96f64);
        let got = map.view(Attributes::MOVEMENT_SPEED.id).value().unwrap();
        assert!(
            (got - expected).abs() < 1e-9,
            "带疾跑修饰符的移速应为 {expected}，实为 {got}"
        );
    }
}
