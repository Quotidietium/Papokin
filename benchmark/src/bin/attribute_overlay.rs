//! 轮次 42 基准：实体属性表写时物化覆盖层
//! （`FxHashMap<u8, AttributeInstance>` 全量抄写 → `AttributeMap`）。
//!
//! 对照形态（`LivingEntity.attributes` 的承载）：
//! - 全量抄写（旧态）：构造时把实体类型声明的全部属性（生物普遍
//!   26-36 条）逐条 insert 进 `FxHashMap<u8, AttributeInstance>`，
//!   hashbrown 终容量 56 桶约 3.2KB/实体外加 5 次增长分配；
//! - 写时物化覆盖层（新态）：条目 Vec（按 id 升序）+ 移除墓碑 +
//!   `&'static EntityType` 回退表，典型生物零堆分配，只在与静态
//!   表分叉（改基础值/加修饰符/动态加属性）时物化实例。
//!
//! 负载模型：20000 个生物实体（僵尸/骷髅/牛/羊/苦力怕/玩家混合，
//! 取自 papokin-data 真实类型表），相位含：构造（spawn）、20 轮
//! 热读（每实体移速值+最大生命基础值）、装备修饰符增删（30%/
//! 15%）、基础值改写（10%）、显式移除回落探针（5%，验证墓碑
//! 语义与旧 map 删除路径等价）、5 轮全量同步包构建。
//!
//! 语义等价由「读取值/基础值/回落分支/同步包清单」全量指纹全等
//! 证明（两臂共用同一指纹函数与同一套确定性操作脚本）。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use papokin_data::attributes::Attributes;
use papokin_data::entity::EntityType;
use rustc_hash::FxHashMap;

use crate::common::{AttrInst, Modifier, fp_f64, fp_mix, lcg_next};

mod common {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    /// 与生产 `AttributeInstance` 同形的实例（48B）。
    #[derive(Debug)]
    pub struct AttrInst {
        pub base_value: f64,
        pub modifiers: Vec<Modifier>,
        pub cached_value: AtomicU64,
        pub dirty: AtomicBool,
    }

    impl AttrInst {
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
                    0 => add_sum += m.amount,
                    1 => mul_base += m.amount,
                    _ => mul_total *= 1.0 + m.amount,
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

    /// 与生产 `Modifier` 同形（String + f64 + 操作码）。
    #[derive(Debug, Clone)]
    pub struct Modifier {
        pub id: String,
        pub amount: f64,
        pub operation: u8,
    }

    /// FNV-1a 混合（与既往轮次同源）。
    pub const fn fp_mix(fp: u64, x: u64) -> u64 {
        (fp ^ x).wrapping_mul(0x1000_0000_01b3)
    }

    pub const fn fp_f64(fp: u64, v: f64) -> u64 {
        fp_mix(fp, v.to_bits())
    }

    /// 确定性 LCG（操作脚本唯一随机源）。
    pub const fn lcg_next(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        *state >> 33
    }
}

mod old {
    use std::sync::atomic::Ordering;

    use super::common::{AttrInst, Modifier};
    use papokin_data::attributes::Attributes;

    /// 旧承载：构造时全量抄写类型表。
    pub struct OldAttrs {
        pub inner: rustc_hash::FxHashMap<u8, AttrInst>,
        pub entity_type: &'static papokin_data::entity::EntityType,
    }

    impl OldAttrs {
        pub fn spawn(entity_type: &'static papokin_data::entity::EntityType) -> Self {
            let mut inner = rustc_hash::FxHashMap::default();
            for (attr, base) in entity_type.attributes {
                inner.insert(attr.id, AttrInst::new(*base));
            }
            Self { inner, entity_type }
        }

        pub fn get_value(&self, attribute: &Attributes) -> f64 {
            self.inner
                .get(&attribute.id)
                .map_or(attribute.default_value, AttrInst::value)
        }

        pub fn get_base(&self, attribute: &Attributes) -> f64 {
            if let Some(instance) = self.inner.get(&attribute.id) {
                return instance.base_value;
            }
            self.entity_type
                .attributes
                .iter()
                .find(|a| a.0.id == attribute.id)
                .map_or(attribute.default_value, |a| a.1)
        }

        pub fn set_base(&mut self, attribute: &Attributes, new_base: f64) {
            if let Some(inst) = self.inner.get_mut(&attribute.id) {
                inst.base_value = new_base;
                inst.dirty.store(true, Ordering::Relaxed);
            } else {
                let ai = AttrInst::new(new_base);
                ai.dirty.store(true, Ordering::Relaxed);
                self.inner.insert(attribute.id, ai);
            }
        }

        /// 旧 `update_declared_attribute`：仅声明属性生效。
        pub fn update_declared(&mut self, attribute: &Attributes, m: Modifier) -> bool {
            let Some(base) = self
                .entity_type
                .attributes
                .iter()
                .find(|a| a.0.id == attribute.id)
                .map(|a| a.1)
            else {
                return false;
            };
            let inst = self
                .inner
                .entry(attribute.id)
                .or_insert_with(|| AttrInst::new(base));
            inst.add_or_replace_modifier(m);
            inst.dirty.store(true, Ordering::Relaxed);
            true
        }

        /// 旧 `update_existing_attribute`：无实例返回 `false`。
        pub fn update_existing(&mut self, attribute: &Attributes, id: &str) -> bool {
            let Some(inst) = self.inner.get_mut(&attribute.id) else {
                return false;
            };
            inst.remove_modifier(id);
            inst.dirty.store(true, Ordering::Relaxed);
            true
        }

        pub fn remove(&mut self, attribute: &Attributes) {
            self.inner.remove(&attribute.id);
        }

        /// 旧 `full_sync`：迭代 map 收集 (id, base, 修饰符数) 后按 id 排序。
        pub fn sync_list(&self) -> Vec<(u8, f64, usize)> {
            let mut list: Vec<(u8, f64, usize)> = self
                .inner
                .iter()
                .map(|(&id, inst)| (id, inst.base_value, inst.modifiers.len()))
                .collect();
            list.sort_unstable_by_key(|e| e.0);
            list
        }
    }
}

mod new {
    use std::sync::atomic::Ordering;

    use super::common::{AttrInst, Modifier};
    use papokin_data::attributes::Attributes;

    /// 三态视图（与生产 `AttributeView` 同构）。
    pub enum View<'a> {
        Live(&'a AttrInst),
        Declared { base: f64 },
        Absent,
    }

    impl View<'_> {
        pub fn value(&self) -> Option<f64> {
            match self {
                Self::Live(inst) => Some(inst.value()),
                Self::Declared { base } => Some(*base),
                Self::Absent => None,
            }
        }

        pub const fn base_value(&self) -> Option<f64> {
            match self {
                Self::Live(inst) => Some(inst.base_value),
                Self::Declared { base } => Some(*base),
                Self::Absent => None,
            }
        }
    }

    /// 每类型「属性 id → 声明基础值」直索引表（与生产 `TypeBases` 同构）。
    struct TypeBases {
        by_id: Box<[f64]>,
    }

    impl TypeBases {
        fn build(entity_type: &'static papokin_data::entity::EntityType) -> Self {
            let len = Attributes::ALL
                .iter()
                .map(|a| usize::from(a.id))
                .max()
                .map_or(0, |max| max + 1);
            let mut by_id = vec![f64::NAN; len];
            for (attr, base) in entity_type.attributes {
                by_id[usize::from(attr.id)] = *base;
            }
            Self {
                by_id: by_id.into_boxed_slice(),
            }
        }

        fn base(&self, id: u8) -> Option<f64> {
            let value = *self.by_id.get(usize::from(id))?;
            if value.is_nan() { None } else { Some(value) }
        }
    }

    fn type_bases(
        entity_type: &'static papokin_data::entity::EntityType,
    ) -> std::sync::Arc<TypeBases> {
        static CACHE: std::sync::OnceLock<
            std::sync::Mutex<rustc_hash::FxHashMap<u16, std::sync::Arc<TypeBases>>>,
        > = std::sync::OnceLock::new();
        let cache = CACHE.get_or_init(|| std::sync::Mutex::new(rustc_hash::FxHashMap::default()));
        let mut guard = cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard
            .entry(entity_type.id)
            .or_insert_with(|| std::sync::Arc::new(TypeBases::build(entity_type)))
            .clone()
    }

    /// 新承载：写时物化覆盖层（与生产 `AttributeMap` 同构）。
    pub struct OverlayAttrs {
        entity_type: &'static papokin_data::entity::EntityType,
        bases: std::sync::Arc<TypeBases>,
        entries: Vec<(u8, AttrInst)>,
        removed: Vec<u8>,
    }

    impl OverlayAttrs {
        pub fn spawn(entity_type: &'static papokin_data::entity::EntityType) -> Self {
            Self {
                entity_type,
                bases: type_bases(entity_type),
                entries: Vec::new(),
                removed: Vec::new(),
            }
        }

        fn declared_base(&self, id: u8) -> Option<f64> {
            self.bases.base(id)
        }

        pub fn view(&self, id: u8) -> View<'_> {
            if let Ok(pos) = self.entries.binary_search_by_key(&id, |(k, _)| *k) {
                return View::Live(&self.entries[pos].1);
            }
            if self.removed.contains(&id) {
                return View::Absent;
            }
            self.declared_base(id)
                .map_or(View::Absent, |base| View::Declared { base })
        }

        pub fn get_value(&self, attribute: &Attributes) -> f64 {
            self.view(attribute.id)
                .value()
                .unwrap_or(attribute.default_value)
        }

        pub fn get_base(&self, attribute: &Attributes) -> f64 {
            if let Some(base) = self.view(attribute.id).base_value() {
                return base;
            }
            self.entity_type
                .attributes
                .iter()
                .find(|a| a.0.id == attribute.id)
                .map_or(attribute.default_value, |a| a.1)
        }

        fn get_mut(&mut self, id: u8) -> Option<&mut AttrInst> {
            let pos = self.entries.binary_search_by_key(&id, |(k, _)| *k);
            match pos {
                Ok(pos) => Some(&mut self.entries[pos].1),
                Err(pos) => {
                    if self.removed.contains(&id) {
                        return None;
                    }
                    let base = self.declared_base(id)?;
                    self.entries.insert(pos, (id, AttrInst::new(base)));
                    Some(&mut self.entries[pos].1)
                }
            }
        }

        fn get_or_insert_with(&mut self, id: u8, init: impl FnOnce() -> AttrInst) -> &mut AttrInst {
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

        pub fn set_base(&mut self, attribute: &Attributes, new_base: f64) {
            if let Some(inst) = self.get_mut(attribute.id) {
                inst.base_value = new_base;
                inst.dirty.store(true, Ordering::Relaxed);
            } else {
                let ai = AttrInst::new(new_base);
                ai.dirty.store(true, Ordering::Relaxed);
                if let Err(pos) = self
                    .entries
                    .binary_search_by_key(&attribute.id, |(k, _)| *k)
                {
                    self.entries.insert(pos, (attribute.id, ai));
                }
                self.removed.retain(|&r| r != attribute.id);
            }
        }

        pub fn update_declared(&mut self, attribute: &Attributes, m: Modifier) -> bool {
            let Some(base) = self.declared_base(attribute.id) else {
                return false;
            };
            let inst = self.get_or_insert_with(attribute.id, || AttrInst::new(base));
            inst.add_or_replace_modifier(m);
            inst.dirty.store(true, Ordering::Relaxed);
            true
        }

        pub fn update_existing(&mut self, attribute: &Attributes, id: &str) -> bool {
            let Some(inst) = self.get_mut(attribute.id) else {
                return false;
            };
            inst.remove_modifier(id);
            inst.dirty.store(true, Ordering::Relaxed);
            true
        }

        pub fn remove(&mut self, attribute: &Attributes) {
            if let Ok(pos) = self
                .entries
                .binary_search_by_key(&attribute.id, |(k, _)| *k)
            {
                self.entries.remove(pos);
            }
            if self.declared_base(attribute.id).is_some() && !self.removed.contains(&attribute.id) {
                self.removed.push(attribute.id);
            }
        }

        /// 新 `full_sync`：合并视图（声明条目 + 覆盖层额外条目）收集排序。
        pub fn sync_list(&self) -> Vec<(u8, f64, usize)> {
            let mut list: Vec<(u8, f64, usize)> =
                Vec::with_capacity(self.entity_type.attributes.len() + self.entries.len());
            for (attr, base) in self.entity_type.attributes {
                if self.removed.contains(&attr.id) {
                    continue;
                }
                let (base, count) = self
                    .entries
                    .binary_search_by_key(&attr.id, |(k, _)| *k)
                    .map_or((*base, 0), |pos| {
                        (
                            self.entries[pos].1.base_value,
                            self.entries[pos].1.modifiers.len(),
                        )
                    });
                list.push((attr.id, base, count));
            }
            for (id, inst) in &self.entries {
                if self.declared_base(*id).is_none() {
                    list.push((*id, inst.base_value, inst.modifiers.len()));
                }
            }
            list.sort_unstable_by_key(|e| e.0);
            list
        }
    }
}

// ---------------------------------------------------------------------------
// 计数分配器
// ---------------------------------------------------------------------------

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);
static FREED_BYTES: AtomicUsize = AtomicUsize::new(0);

struct Counting;

// SAFETY: 仅在系统分配器外包裹计数，转发语义与系统分配器一致。
unsafe impl GlobalAlloc for Counting {
    // SAFETY: 同上，纯转发。
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: 转发系统分配器，契约由调用方（分配器 trait）保证。
        unsafe { System.alloc(layout) }
    }

    // SAFETY: 同上，纯转发。
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        FREED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: 转发系统分配器，ptr 来自本分配器的 alloc。
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn resident() -> usize {
    ALLOC_BYTES.load(Ordering::Relaxed) - FREED_BYTES.load(Ordering::Relaxed)
}

fn counters() -> (usize, usize) {
    (ALLOCS.load(Ordering::Relaxed), resident())
}

// ---------------------------------------------------------------------------
// 语料与相位
// ---------------------------------------------------------------------------

/// 实体混合（模拟真实服务器的生物构成，取自 papokin-data 类型表）。
const ENTITY_MIX: &[(&EntityType, usize)] = &[
    (&EntityType::ZOMBIE, 5000),
    (&EntityType::SKELETON, 4000),
    (&EntityType::COW, 4000),
    (&EntityType::SHEEP, 3000),
    (&EntityType::CREEPER, 2000),
    (&EntityType::PLAYER, 2000),
];

const TOTAL_ENTITIES: usize = 20000;
const READ_ROUNDS: usize = 20;
const SYNC_ROUNDS: usize = 5;

/// 构造脚本：每个实体一份确定性随机状态（两臂共用同一脚本）。
const fn churn_for(state: &mut u64) -> (bool, bool, bool, bool) {
    let equip = lcg_next(state) % 10 < 3;
    let unequip = equip && lcg_next(state).is_multiple_of(2);
    let base_write = lcg_next(state).is_multiple_of(10);
    let remove_probe = lcg_next(state).is_multiple_of(20);
    (equip, unequip, base_write, remove_probe)
}

/// 两臂共用的操作脚本参数（装备修饰符内容）。
fn weapon_modifiers() -> Vec<Modifier> {
    vec![
        Modifier {
            id: "minecraft:generic_attack_damage".to_string(),
            amount: 7.0,
            operation: 0,
        },
        Modifier {
            id: "minecraft:generic_attack_speed".to_string(),
            amount: -2.5,
            operation: 0,
        },
    ]
}

fn armor_modifier() -> Modifier {
    Modifier {
        id: "minecraft:generic_armor".to_string(),
        amount: 0.1,
        operation: 0,
    }
}

struct PhaseStats {
    spawn_allocs: usize,
    spawn_ms: f64,
    read_ms: f64,
    read_allocs: usize,
    churn_ms: f64,
    churn_allocs: usize,
    sync_ms: f64,
    sync_allocs: usize,
}

// --- 旧臂相位 ---------------------------------------------------------------

fn run_old_arm() -> (u64, PhaseStats, usize) {
    let (a0, _r0) = counters();
    let start = Instant::now();
    let mut corpus: Vec<old::OldAttrs> = Vec::with_capacity(TOTAL_ENTITIES);
    for &(entity_type, count) in ENTITY_MIX {
        for _ in 0..count {
            corpus.push(old::OldAttrs::spawn(entity_type));
        }
    }
    let spawn_stats = counters();
    let spawn_ms = start.elapsed().as_secs_f64() * 1000.0;

    let mut fp: u64 = 0xcbf2_9ce4_8422_2325;
    let mut read_state = 0x1234_5678u64;
    let read_start = Instant::now();
    for _ in 0..READ_ROUNDS {
        for e in &corpus {
            fp = fp_f64(fp, e.get_value(&Attributes::MOVEMENT_SPEED));
            fp = fp_mix(fp, lcg_next(&mut read_state));
            fp = fp_f64(fp, e.get_base(&Attributes::MAX_HEALTH));
        }
    }
    let read_ms = read_start.elapsed().as_secs_f64() * 1000.0;
    let after_read = counters();

    let churn_start = Instant::now();
    let mut states = Vec::with_capacity(TOTAL_ENTITIES);
    let mut s = 0xdead_beefu64;
    for _ in 0..TOTAL_ENTITIES {
        states.push(s);
        let _ = lcg_next(&mut s);
    }
    for (i, e) in corpus.iter_mut().enumerate() {
        let (equip, unequip, base_write, remove_probe) = {
            let st = &mut states[i];
            churn_for(st)
        };
        if equip {
            for m in weapon_modifiers() {
                let attr = if m.operation == 0 && m.amount > 0.0 {
                    &Attributes::ATTACK_DAMAGE
                } else {
                    &Attributes::ATTACK_SPEED
                };
                let _ = e.update_declared(attr, m);
            }
            let _ = e.update_declared(&Attributes::KNOCKBACK_RESISTANCE, armor_modifier());
        }
        if unequip {
            let _ = e.update_existing(
                &Attributes::ATTACK_DAMAGE,
                "minecraft:generic_attack_damage",
            );
            let _ = e.update_existing(&Attributes::KNOCKBACK_RESISTANCE, "minecraft:generic_armor");
        }
        if base_write {
            e.set_base(&Attributes::MAX_HEALTH, 40.0);
            fp = fp_f64(fp, e.get_base(&Attributes::MAX_HEALTH));
        }
        if remove_probe {
            e.remove(&Attributes::FOLLOW_RANGE);
            fp = fp_f64(fp, e.get_value(&Attributes::FOLLOW_RANGE));
            fp = fp_mix(
                fp,
                u64::from(u8::from(
                    e.update_existing(&Attributes::FOLLOW_RANGE, "minecraft:none"),
                )),
            );
        }
    }
    let churn_ms = churn_start.elapsed().as_secs_f64() * 1000.0;
    let after_churn = counters();

    let sync_start = Instant::now();
    for _ in 0..SYNC_ROUNDS {
        for e in &corpus {
            for (id, base, count) in e.sync_list() {
                fp = fp_mix(fp, u64::from(id));
                fp = fp_f64(fp, base);
                fp = fp_mix(fp, count as u64);
            }
        }
    }
    let sync_ms = sync_start.elapsed().as_secs_f64() * 1000.0;
    let after_sync = counters();

    let final_resident = resident();
    let stats = PhaseStats {
        spawn_allocs: spawn_stats.0 - a0,
        spawn_ms,
        read_ms,
        read_allocs: after_read.0 - spawn_stats.0,
        churn_ms,
        churn_allocs: after_churn.0 - after_read.0,
        sync_ms,
        sync_allocs: after_sync.0 - after_churn.0,
    };
    (fp, stats, final_resident)
}

// --- 新臂相位 ---------------------------------------------------------------

fn run_new_arm() -> (u64, PhaseStats, usize) {
    let (a0, _r0) = counters();
    let start = Instant::now();
    let mut corpus: Vec<new::OverlayAttrs> = Vec::with_capacity(TOTAL_ENTITIES);
    for &(entity_type, count) in ENTITY_MIX {
        for _ in 0..count {
            corpus.push(new::OverlayAttrs::spawn(entity_type));
        }
    }
    let spawn_stats = counters();
    let spawn_ms = start.elapsed().as_secs_f64() * 1000.0;

    let mut fp: u64 = 0xcbf2_9ce4_8422_2325;
    let mut read_state = 0x1234_5678u64;
    let read_start = Instant::now();
    for _ in 0..READ_ROUNDS {
        for e in &corpus {
            fp = fp_f64(fp, e.get_value(&Attributes::MOVEMENT_SPEED));
            fp = fp_mix(fp, lcg_next(&mut read_state));
            fp = fp_f64(fp, e.get_base(&Attributes::MAX_HEALTH));
        }
    }
    let read_ms = read_start.elapsed().as_secs_f64() * 1000.0;
    let after_read = counters();

    let churn_start = Instant::now();
    let mut states = Vec::with_capacity(TOTAL_ENTITIES);
    let mut s = 0xdead_beefu64;
    for _ in 0..TOTAL_ENTITIES {
        states.push(s);
        let _ = lcg_next(&mut s);
    }
    for (i, e) in corpus.iter_mut().enumerate() {
        let (equip, unequip, base_write, remove_probe) = {
            let st = &mut states[i];
            churn_for(st)
        };
        if equip {
            for m in weapon_modifiers() {
                let attr = if m.operation == 0 && m.amount > 0.0 {
                    &Attributes::ATTACK_DAMAGE
                } else {
                    &Attributes::ATTACK_SPEED
                };
                let _ = e.update_declared(attr, m);
            }
            let _ = e.update_declared(&Attributes::KNOCKBACK_RESISTANCE, armor_modifier());
        }
        if unequip {
            let _ = e.update_existing(
                &Attributes::ATTACK_DAMAGE,
                "minecraft:generic_attack_damage",
            );
            let _ = e.update_existing(&Attributes::KNOCKBACK_RESISTANCE, "minecraft:generic_armor");
        }
        if base_write {
            e.set_base(&Attributes::MAX_HEALTH, 40.0);
            fp = fp_f64(fp, e.get_base(&Attributes::MAX_HEALTH));
        }
        if remove_probe {
            e.remove(&Attributes::FOLLOW_RANGE);
            fp = fp_f64(fp, e.get_value(&Attributes::FOLLOW_RANGE));
            fp = fp_mix(
                fp,
                u64::from(u8::from(
                    e.update_existing(&Attributes::FOLLOW_RANGE, "minecraft:none"),
                )),
            );
        }
    }
    let churn_ms = churn_start.elapsed().as_secs_f64() * 1000.0;
    let after_churn = counters();

    let sync_start = Instant::now();
    for _ in 0..SYNC_ROUNDS {
        for e in &corpus {
            for (id, base, count) in e.sync_list() {
                fp = fp_mix(fp, u64::from(id));
                fp = fp_f64(fp, base);
                fp = fp_mix(fp, count as u64);
            }
        }
    }
    let sync_ms = sync_start.elapsed().as_secs_f64() * 1000.0;
    let after_sync = counters();

    let final_resident = resident();
    let stats = PhaseStats {
        spawn_allocs: spawn_stats.0 - a0,
        spawn_ms,
        read_ms,
        read_allocs: after_read.0 - spawn_stats.0,
        churn_ms,
        churn_allocs: after_churn.0 - after_read.0,
        sync_ms,
        sync_allocs: after_sync.0 - after_churn.0,
    };
    (fp, stats, final_resident)
}

// ---------------------------------------------------------------------------
// 闸门与报告
// ---------------------------------------------------------------------------

fn main() {
    print_header();
    let (old_fp, old_stats, old_resident) = run_old_arm();
    let (new_fp, new_stats, new_resident) = run_new_arm();
    let verdict = evaluate(
        old_fp,
        new_fp,
        &old_stats,
        &new_stats,
        old_resident,
        new_resident,
    );
    print_gates(
        &verdict,
        old_fp,
        new_fp,
        old_resident,
        new_resident,
        &old_stats,
        &new_stats,
    );
    write_report(
        &verdict,
        old_fp,
        new_fp,
        old_resident,
        new_resident,
        &old_stats,
        &new_stats,
    );
}

struct Verdict {
    semantics_ok: bool,
    resident_ok: bool,
    spawn_allocs_ok: bool,
    read_ok: bool,
    read_ratio: f64,
    all_passed: bool,
}

fn evaluate(
    old_fp: u64,
    new_fp: u64,
    old_stats: &PhaseStats,
    new_stats: &PhaseStats,
    old_resident: usize,
    new_resident: usize,
) -> Verdict {
    let semantics_ok = old_fp == new_fp;
    let resident_ok = new_resident < old_resident;
    let spawn_allocs_ok = new_stats.spawn_allocs < old_stats.spawn_allocs;
    let read_ratio = old_stats.read_ms / new_stats.read_ms;
    let read_ok = read_ratio >= 0.91;
    Verdict {
        semantics_ok,
        resident_ok,
        spawn_allocs_ok,
        read_ok,
        read_ratio,
        all_passed: semantics_ok && resident_ok && spawn_allocs_ok && read_ok,
    }
}

fn print_header() {
    println!("轮次 42 基准：实体属性表写时物化覆盖层（FxHashMap 全量抄写 → AttributeMap）");
    println!(
        "语料：{TOTAL_ENTITIES} 个生物实体（{} 种类型真实表）· 读 {READ_ROUNDS} 轮 · 同步 {SYNC_ROUNDS} 轮",
        ENTITY_MIX.len()
    );
    println!("条目形态：旧臂 hashbrown 56 桶级（56B/桶 + ctrl）· 新臂覆盖层（空表零分配）");
    println!();
}

fn print_gates(
    v: &Verdict,
    old_fp: u64,
    new_fp: u64,
    old_resident: usize,
    new_resident: usize,
    old_stats: &PhaseStats,
    new_stats: &PhaseStats,
) {
    println!("=== 闸门 ===");
    println!(
        "{} 操作脚本全量指纹全等（读取/回落/同步）：旧 0x{old_fp:016x} vs 新 0x{new_fp:016x}",
        mark(v.semantics_ok)
    );
    println!(
        "{} 驻留字节下降（主判据）：{old_resident} → {new_resident} B（×{:.3}，每实体 {:.1} → {:.1} B）",
        mark(v.resident_ok),
        old_resident as f64 / new_resident as f64,
        old_resident as f64 / TOTAL_ENTITIES as f64,
        new_resident as f64 / TOTAL_ENTITIES as f64
    );
    println!(
        "{} 构造分配次数下降：{} → {} 次（每实体 {:.3} → {:.3}）",
        mark(v.spawn_allocs_ok),
        old_stats.spawn_allocs,
        new_stats.spawn_allocs,
        old_stats.spawn_allocs as f64 / TOTAL_ENTITIES as f64,
        new_stats.spawn_allocs as f64 / TOTAL_ENTITIES as f64
    );
    println!(
        "{} 热读耗时不劣化（≥0.91×）：{:.2} → {:.2} ms（×{:.3}）",
        mark(v.read_ok),
        old_stats.read_ms,
        new_stats.read_ms,
        v.read_ratio
    );
    println!();
    println!("=== 数值 ===");
    println!(
        "构造：{:.2} → {:.2} ms（×{:.2}）",
        old_stats.spawn_ms,
        new_stats.spawn_ms,
        old_stats.spawn_ms / new_stats.spawn_ms
    );
    println!(
        "热读：{:.2} → {:.2} ms（×{:.2}），分配 {} → {}",
        old_stats.read_ms,
        new_stats.read_ms,
        v.read_ratio,
        old_stats.read_allocs,
        new_stats.read_allocs
    );
    println!(
        "增删改写：{:.2} → {:.2} ms（×{:.2}），分配 {} → {}",
        old_stats.churn_ms,
        new_stats.churn_ms,
        old_stats.churn_ms / new_stats.churn_ms,
        old_stats.churn_allocs,
        new_stats.churn_allocs
    );
    println!(
        "全量同步：{:.2} → {:.2} ms（×{:.2}），分配 {} → {}",
        old_stats.sync_ms,
        new_stats.sync_ms,
        old_stats.sync_ms / new_stats.sync_ms,
        old_stats.sync_allocs,
        new_stats.sync_allocs
    );
    println!(
        "闸门判定：{}",
        if v.all_passed {
            "全部通过，准予收编"
        } else {
            "存在未过闸门项，不予收编"
        }
    );
}

fn write_report(
    v: &Verdict,
    old_fp: u64,
    new_fp: u64,
    old_resident: usize,
    new_resident: usize,
    old_stats: &PhaseStats,
    new_stats: &PhaseStats,
) {
    use std::io::Write as _;

    let report = serde_json::json!({
        "round": 42,
        "topic": "attribute_overlay",
        "corpus": {
            "entities": TOTAL_ENTITIES,
            "type_mix": ENTITY_MIX.iter().map(|(t, n)| format!("{}:{}", t.resource_name, n)).collect::<Vec<_>>(),
            "read_rounds": READ_ROUNDS,
            "sync_rounds": SYNC_ROUNDS,
        },
        "gates": [
            {"name": "操作脚本全量指纹全等", "passed": v.semantics_ok,
             "detail": format!("旧 0x{old_fp:016x} vs 新 0x{new_fp:016x}")},
            {"name": "驻留字节下降（主判据）", "passed": v.resident_ok,
             "detail": format!("{old_resident} → {new_resident} B（×{:.3}）", old_resident as f64 / new_resident as f64)},
            {"name": "构造分配次数下降", "passed": v.spawn_allocs_ok,
             "detail": format!("{} → {}", old_stats.spawn_allocs, new_stats.spawn_allocs)},
            {"name": "热读耗时不劣化（≥0.91×）", "passed": v.read_ok,
             "detail": format!("×{:.3}", v.read_ratio)},
        ],
        "old": {
            "resident_bytes": old_resident,
            "spawn_allocs": old_stats.spawn_allocs,
            "spawn_ms": old_stats.spawn_ms,
            "read_ms": old_stats.read_ms,
            "read_allocs": old_stats.read_allocs,
            "churn_ms": old_stats.churn_ms,
            "churn_allocs": old_stats.churn_allocs,
            "sync_ms": old_stats.sync_ms,
            "sync_allocs": old_stats.sync_allocs,
            "content_fingerprint": format!("0x{old_fp:016x}"),
        },
        "new": {
            "resident_bytes": new_resident,
            "spawn_allocs": new_stats.spawn_allocs,
            "spawn_ms": new_stats.spawn_ms,
            "read_ms": new_stats.read_ms,
            "read_allocs": new_stats.read_allocs,
            "churn_ms": new_stats.churn_ms,
            "churn_allocs": new_stats.churn_allocs,
            "sync_ms": new_stats.sync_ms,
            "sync_allocs": new_stats.sync_allocs,
            "content_fingerprint": format!("0x{new_fp:016x}"),
        },
        "sizes": {
            "instance_bytes": std::mem::size_of::<AttrInst>(),
            "old_map_inline_bytes": std::mem::size_of::<FxHashMap<u8, AttrInst>>(),
        },
        "outcome": if v.all_passed { "positive" } else { "negative" },
    });

    let path = std::path::Path::new("note/report/perf/round42-attribute-overlay.json");
    let Ok(mut file) = std::fs::File::create(path) else {
        eprintln!("写入报告失败：路径不可创建 {}", path.display());
        return;
    };
    if let Err(err) = file.write_all(
        serde_json::to_string_pretty(&report)
            .unwrap_or_default()
            .as_bytes(),
    ) {
        eprintln!("写入报告失败：{err}");
    }
}

const fn mark(ok: bool) -> &'static str {
    if ok { "√" } else { "×" }
}
