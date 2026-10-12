//! 轮次 45 基准：装备存储紧凑化（`EntityEquipment` 判别值直索引
//! + 掉落概率表去 `Arc` 盒）。
//!
//! 双臂结构复刻（主 crate 形态，轮次 25/26/44 先例）：
//! - 旧臂：`HashMap<EquipmentSlot, ItemStack>` +
//!   `Arc<Mutex<FxHashMap<EquipmentSlot, f32>>>`（掉落概率）；
//! - 新臂：`Option<Box<[Option<ItemStack>; 8]>>` 判别值直索引 +
//!   `Mutex<[f32; 8]>` 内联（默认值铺底）。
//!
//! 装备字段两臂都保留 `Arc<Mutex<..>>`（生产中与
//! `PlayerInventory` 共享，共享面不动）；差异全部落在容器形态
//! 与掉落概率表的盒装形态。
//!
//! 语义红线：键空间恒为 8 个规范槽（`EquippableImpl.slot` 为
//! `&'static`、`get_from_name` 返回静态表），判别值索引是完备键；
//! 旧 `HashMap` 迭代序本就随进程种子不定，指纹门两侧均按判别值
//! 排序规范化。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use papokin_data::data_component_impl::EquipmentSlot;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use rustc_hash::FxHashMap;

const ENTITY_COUNT: usize = 20_000;
const READ_ROUNDS: usize = 20;
const DEATH_COUNT: usize = 5_000;
const DEFAULT_CHANCE: f32 = 0.085;
/// 与生产 `SLOT_TABLE` 同构的判别值表（迭代还原槽位实例）。
static SLOT_TABLE: [EquipmentSlot; 8] = [
    EquipmentSlot::MAIN_HAND,
    EquipmentSlot::OFF_HAND,
    EquipmentSlot::FEET,
    EquipmentSlot::LEGS,
    EquipmentSlot::CHEST,
    EquipmentSlot::HEAD,
    EquipmentSlot::BODY,
    EquipmentSlot::SADDLE,
];

// ---------------------------------------------------------------------------
// 计数分配器
// ---------------------------------------------------------------------------

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
static FREED_BYTES: AtomicU64 = AtomicU64::new(0);

struct CountingAllocator;

// SAFETY: 仅统计后原样转发系统分配器，不改变任何分配语义
unsafe impl std::alloc::GlobalAlloc for CountingAllocator {
    // SAFETY: 转发系统分配器，统计本身无分配语义约束
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        // SAFETY: 转发系统分配器，无额外不变量
        unsafe { std::alloc::System.alloc(layout) }
    }

    // SAFETY: 转发系统分配器，ptr 语义由调用方保证
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        FREED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        // SAFETY: ptr/layout 来自配对的 alloc 返回值
        unsafe { std::alloc::System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

fn allocs() -> u64 {
    ALLOCS.load(Ordering::Relaxed)
}

fn resident() -> u64 {
    ALLOC_BYTES.load(Ordering::Relaxed) - FREED_BYTES.load(Ordering::Relaxed)
}

const fn fp_mix(fp: u64, value: u64) -> u64 {
    (fp ^ value).wrapping_mul(0x1000_0000_01b3)
}

// ---------------------------------------------------------------------------
// 旧臂：HashMap 承载
// ---------------------------------------------------------------------------

/// 复刻旧 `EntityEquipment`。
#[derive(Clone)]
struct OldEquip {
    equipment: HashMap<EquipmentSlot, ItemStack>,
}

impl OldEquip {
    fn new() -> Self {
        Self {
            equipment: HashMap::new(),
        }
    }

    fn put(&mut self, slot: &EquipmentSlot, stack: ItemStack) -> ItemStack {
        self.equipment
            .insert(slot.clone(), stack)
            .unwrap_or_else(|| ItemStack::EMPTY.clone())
    }

    fn get_opt(&self, slot: &EquipmentSlot) -> Option<&ItemStack> {
        self.equipment.get(slot)
    }

    fn remove(&mut self, slot: &EquipmentSlot) -> Option<ItemStack> {
        self.equipment.remove(slot)
    }

    fn iter_present(&self) -> Vec<(u8, &ItemStack)> {
        // 排序规范化：旧 HashMap 迭代序不定（生产消费方解耦，
        // 此处按判别值排序对齐指纹门）。
        let mut v: Vec<(u8, &ItemStack)> = self
            .equipment
            .iter()
            .map(|(slot, stack)| (slot.discriminant() as u8, stack))
            .collect();
        v.sort_by_key(|(d, _)| *d);
        v
    }
}

struct OldEntity {
    equipment: Arc<Mutex<OldEquip>>,
    drop_chances: Arc<Mutex<FxHashMap<EquipmentSlot, f32>>>,
    hostile: bool,
}

// ---------------------------------------------------------------------------
// 新臂：判别值直索引
// ---------------------------------------------------------------------------

/// 复刻新 `EntityEquipment`。
#[derive(Clone)]
struct NewEquip {
    slots: Option<Box<[Option<ItemStack>; 8]>>,
}

impl NewEquip {
    const fn new() -> Self {
        Self { slots: None }
    }

    fn put(&mut self, slot: &EquipmentSlot, stack: ItemStack) -> ItemStack {
        let idx = slot.discriminant() as usize;
        let slots = self
            .slots
            .get_or_insert_with(|| Box::new(std::array::from_fn(|_| None)));
        slots[idx]
            .replace(stack)
            .unwrap_or_else(|| ItemStack::EMPTY.clone())
    }

    fn get_opt(&self, slot: &EquipmentSlot) -> Option<&ItemStack> {
        let idx = slot.discriminant() as usize;
        self.slots.as_ref()?.get(idx)?.as_ref()
    }

    fn remove(&mut self, slot: &EquipmentSlot) -> Option<ItemStack> {
        let idx = slot.discriminant() as usize;
        self.slots.as_mut()?.get_mut(idx)?.take()
    }

    fn iter_present(&self) -> Vec<(u8, &ItemStack)> {
        let mut v: Vec<(u8, &ItemStack)> = self
            .slots
            .as_deref()
            .into_iter()
            .flat_map(|slots| slots.iter().enumerate())
            .filter_map(|(idx, stack)| stack.as_ref().map(|s| (idx as u8, s)))
            .collect();
        v.sort_by_key(|(d, _)| *d);
        v
    }
}

struct NewEntity {
    equipment: Arc<Mutex<NewEquip>>,
    drop_chances: Mutex<[f32; 8]>,
    hostile: bool,
}

// ---------------------------------------------------------------------------
// 装备画像（两臂共用）
// ---------------------------------------------------------------------------

/// 第 i 个敌对生物的装备表（主手武器 + 按画像的护甲件）。
fn equip_plan(i: usize) -> &'static [(&'static EquipmentSlot, &'static Item, f32)] {
    const BOW: (&EquipmentSlot, &Item, f32) = (&EquipmentSlot::MAIN_HAND, &Item::BOW, 0.085);
    const SWORD: (&EquipmentSlot, &Item, f32) =
        (&EquipmentSlot::MAIN_HAND, &Item::IRON_SWORD, 0.085);
    match i % 4 {
        0 => {
            static P: [(&EquipmentSlot, &Item, f32); 1] = [BOW];
            &P
        }
        1 => {
            static P: [(&EquipmentSlot, &Item, f32); 2] = [SWORD, BOOTS];
            &P
        }
        2 => {
            static P: [(&EquipmentSlot, &Item, f32); 3] = [SWORD, BOOTS, CHESTPLATE];
            &P
        }
        _ => {
            static P: [(&EquipmentSlot, &Item, f32); 4] = [SWORD, BOOTS, CHESTPLATE, HELMET];
            &P
        }
    }
}

const BOOTS: (&EquipmentSlot, &Item, f32) = (&EquipmentSlot::FEET, &Item::IRON_BOOTS, 0.085);
const CHESTPLATE: (&EquipmentSlot, &Item, f32) =
    (&EquipmentSlot::CHEST, &Item::IRON_CHESTPLATE, 0.5);
const HELMET: (&EquipmentSlot, &Item, f32) = (&EquipmentSlot::HEAD, &Item::IRON_HELMET, 0.25);

/// 掉落结算画像（敌对 45%、被动 55%）。
const fn is_hostile(i: usize) -> bool {
    i % 20 < 9
}

// ---------------------------------------------------------------------------
// 双臂驱动
// ---------------------------------------------------------------------------

struct ArmStats {
    resident: u64,
    spawn_allocs: u64,
    spawn_ms: f64,
    battery_ms: f64,
    death_allocs: u64,
}

fn run_old_arm() -> (u64, ArmStats) {
    let fp: u64 = 0xcbf2_9ce4_8422_2325;
    let a0 = allocs();
    let start = Instant::now();

    let mut corpus: Vec<OldEntity> = Vec::with_capacity(ENTITY_COUNT);
    for i in 0..ENTITY_COUNT {
        let hostile = is_hostile(i);
        let equip = OldEquip::new();
        let entity = OldEntity {
            equipment: Arc::new(Mutex::new(equip)),
            drop_chances: Arc::new(Mutex::new(FxHashMap::default())),
            hostile,
        };
        if hostile {
            let plan = equip_plan(i);
            let mut guard = entity
                .equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut chances = entity
                .drop_chances
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for (slot, item, chance) in plan {
                guard.put(slot, ItemStack::new(1, item));
                chances.insert((*slot).clone(), *chance);
            }
        }
        corpus.push(entity);
    }
    let spawn_allocs = allocs() - a0;
    let spawn_ms = start.elapsed().as_secs_f64() * 1000.0;

    let (mut fp, battery_ms) = old_read_battery(&corpus, fp);

    // 死亡掉落（drop_equipment 形态：逐槽概率裁决 + remove）
    let d0 = allocs();
    for (i, e) in corpus.iter_mut().enumerate().take(DEATH_COUNT) {
        if !e.hostile {
            continue;
        }
        let chances = e
            .drop_chances
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let stored: Vec<u8> = {
            let guard = e
                .equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            guard.iter_present().into_iter().map(|(d, _)| d).collect()
        };
        for d in stored {
            let slot = &SLOT_TABLE[d as usize];
            let mut chance = chances.get(slot).copied().unwrap_or(DEFAULT_CHANCE);
            let preserved = chance > 1.0;
            chance = (chance + 0.01).min(1.0);
            if !preserved && 0.5f32 >= chance {
                continue;
            }
            let mut guard = e
                .equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let item = guard
                .remove(slot)
                .unwrap_or_else(|| ItemStack::EMPTY.clone());
            if item.is_empty() {
                continue;
            }
            fp = fp_mix(fp, item.item.id as u64);
        }
        let _ = i;
    }
    let death_allocs = allocs() - d0;

    (
        fp,
        ArmStats {
            resident: resident(),
            spawn_allocs,
            spawn_ms,
            battery_ms,
            death_allocs,
        },
    )
}

/// 旧臂读电池：主手克隆读 + 护甲四槽借用读 + 概率读 + 同步聚合。
fn old_read_battery(corpus: &[OldEntity], mut fp: u64) -> (u64, f64) {
    let battery_start = Instant::now();
    for round in 0..READ_ROUNDS {
        for (i, e) in corpus.iter().enumerate() {
            // 主手读取（held_item 形态：克隆）
            let held = {
                let guard = e
                    .equipment
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                guard
                    .get_opt(&EquipmentSlot::MAIN_HAND)
                    .cloned()
                    .unwrap_or_else(|| ItemStack::EMPTY.clone())
            };
            fp = fp_mix(fp, held.item_count as u64);
            fp = fp_mix(fp, u64::from(round as u8));
            // 护甲四槽探测（附魔检查形态：借用）
            let guard = e
                .equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for slot in [
                &EquipmentSlot::HEAD,
                &EquipmentSlot::CHEST,
                &EquipmentSlot::LEGS,
                &EquipmentSlot::FEET,
            ] {
                if let Some(stack) = guard.get_opt(slot) {
                    fp = fp_mix(fp, stack.item.id as u64);
                }
            }
            // 掉落概率读取
            let chances = e
                .drop_chances
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if e.hostile {
                for (slot, _, _) in equip_plan(i) {
                    let c = chances.get(*slot).copied().unwrap_or(DEFAULT_CHANCE);
                    fp = fp_mix(fp, c.to_bits() as u64);
                }
            }
            // 同步聚合（CSetEquipment 形态）
            for (d, stack) in guard.iter_present() {
                fp = fp_mix(fp, (d as u64) << 8 | stack.item_count as u64);
            }
        }
    }
    (fp, battery_start.elapsed().as_secs_f64() * 1000.0)
}

/// 新臂读电池：与旧臂逐操作对齐。
fn new_read_battery(corpus: &[NewEntity], mut fp: u64) -> (u64, f64) {
    let battery_start = Instant::now();
    for round in 0..READ_ROUNDS {
        for (i, e) in corpus.iter().enumerate() {
            let held = {
                let guard = e
                    .equipment
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                guard
                    .get_opt(&EquipmentSlot::MAIN_HAND)
                    .cloned()
                    .unwrap_or_else(|| ItemStack::EMPTY.clone())
            };
            fp = fp_mix(fp, held.item_count as u64);
            fp = fp_mix(fp, u64::from(round as u8));
            let guard = e
                .equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for slot in [
                &EquipmentSlot::HEAD,
                &EquipmentSlot::CHEST,
                &EquipmentSlot::LEGS,
                &EquipmentSlot::FEET,
            ] {
                if let Some(stack) = guard.get_opt(slot) {
                    fp = fp_mix(fp, stack.item.id as u64);
                }
            }
            let chances = e
                .drop_chances
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if e.hostile {
                for (slot, _, _) in equip_plan(i) {
                    let c = chances[(*slot).discriminant() as usize];
                    fp = fp_mix(fp, c.to_bits() as u64);
                }
            }
            for (d, stack) in guard.iter_present() {
                fp = fp_mix(fp, (d as u64) << 8 | stack.item_count as u64);
            }
        }
    }
    (fp, battery_start.elapsed().as_secs_f64() * 1000.0)
}

fn run_new_arm() -> (u64, ArmStats) {
    let fp: u64 = 0xcbf2_9ce4_8422_2325;
    let a0 = allocs();
    let start = Instant::now();

    let mut corpus: Vec<NewEntity> = Vec::with_capacity(ENTITY_COUNT);
    for i in 0..ENTITY_COUNT {
        let hostile = is_hostile(i);
        let entity = NewEntity {
            equipment: Arc::new(Mutex::new(NewEquip::new())),
            drop_chances: Mutex::new([DEFAULT_CHANCE; 8]),
            hostile,
        };
        if hostile {
            let plan = equip_plan(i);
            let mut guard = entity
                .equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut chances = entity
                .drop_chances
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for (slot, item, chance) in plan {
                guard.put(slot, ItemStack::new(1, item));
                chances[(*slot).discriminant() as usize] = *chance;
            }
        }
        corpus.push(entity);
    }
    let spawn_allocs = allocs() - a0;
    let spawn_ms = start.elapsed().as_secs_f64() * 1000.0;

    let (mut fp, battery_ms) = new_read_battery(&corpus, fp);

    let d0 = allocs();
    for e in corpus.iter_mut().take(DEATH_COUNT) {
        if !e.hostile {
            continue;
        }
        let chances = e
            .drop_chances
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let stored: Vec<u8> = {
            let guard = e
                .equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            guard.iter_present().into_iter().map(|(d, _)| d).collect()
        };
        for d in stored {
            let slot = &SLOT_TABLE[d as usize];
            let mut chance = chances[(*slot).discriminant() as usize];
            let preserved = chance > 1.0;
            chance = (chance + 0.01).min(1.0);
            if !preserved && 0.5f32 >= chance {
                continue;
            }
            let mut guard = e
                .equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let item = guard
                .remove(slot)
                .unwrap_or_else(|| ItemStack::EMPTY.clone());
            if item.is_empty() {
                continue;
            }
            fp = fp_mix(fp, item.item.id as u64);
        }
    }
    let death_allocs = allocs() - d0;

    (
        fp,
        ArmStats {
            resident: resident(),
            spawn_allocs,
            spawn_ms,
            battery_ms,
            death_allocs,
        },
    )
}

// ---------------------------------------------------------------------------
// 闸门与报告
// ---------------------------------------------------------------------------

fn main() {
    println!("轮次 45 基准：装备存储紧凑化（判别值直索引 + 掉落概率去 Arc 盒）");
    println!(
        "语料：{ENTITY_COUNT} 实体（45% 敌对装备 1-4 槽）· 读 {READ_ROUNDS} 轮 · 死亡 {DEATH_COUNT}"
    );
    println!();

    let (old_fp, old) = run_old_arm();
    let (new_fp, new) = run_new_arm();
    let v = evaluate(&old, &new, old_fp, new_fp);
    print_gates(&v, &old, &new, old_fp, new_fp);
    write_report(&v, &old, &new, old_fp, new_fp);
}

struct Verdict {
    semantics_ok: bool,
    resident_ok: bool,
    spawn_allocs_ok: bool,
    battery_ok: bool,
    battery_ratio: f64,
    all_passed: bool,
}

fn evaluate(old: &ArmStats, new: &ArmStats, old_fp: u64, new_fp: u64) -> Verdict {
    let semantics_ok = old_fp == new_fp;
    let resident_ok = new.resident < old.resident;
    let spawn_allocs_ok = new.spawn_allocs < old.spawn_allocs;
    let battery_ratio = old.battery_ms / new.battery_ms;
    let battery_ok = battery_ratio >= 0.91;
    Verdict {
        semantics_ok,
        resident_ok,
        spawn_allocs_ok,
        battery_ok,
        battery_ratio,
        all_passed: semantics_ok && resident_ok && spawn_allocs_ok && battery_ok,
    }
}

fn print_gates(v: &Verdict, old: &ArmStats, new: &ArmStats, old_fp: u64, new_fp: u64) {
    println!("=== 闸门 ===");
    println!(
        "{} 电池指纹全等：旧 0x{old_fp:016x} vs 新 0x{new_fp:016x}",
        mark(v.semantics_ok)
    );
    println!(
        "{} 驻留字节下降（主判据）：{} → {} B（每实体 {:.1} → {:.1} B）",
        mark(v.resident_ok),
        old.resident,
        new.resident,
        old.resident as f64 / ENTITY_COUNT as f64,
        new.resident as f64 / ENTITY_COUNT as f64
    );
    println!(
        "{} 构造+装备分配次数下降：{} → {} 次",
        mark(v.spawn_allocs_ok),
        old.spawn_allocs,
        new.spawn_allocs
    );
    println!(
        "{} 读电池耗时不劣化（≥0.91×）：{:.2} → {:.2} ms（×{:.3}）",
        mark(v.battery_ok),
        old.battery_ms,
        new.battery_ms,
        v.battery_ratio
    );
    println!();
    println!("=== 数值 ===");
    println!(
        "构造+装备：{:.2} → {:.2} ms（×{:.2}）",
        old.spawn_ms,
        new.spawn_ms,
        old.spawn_ms / new.spawn_ms
    );
    println!(
        "死亡掉落分配：{} → {} 次",
        old.death_allocs, new.death_allocs
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

fn write_report(v: &Verdict, old: &ArmStats, new: &ArmStats, old_fp: u64, new_fp: u64) {
    use std::io::Write as _;

    let report = serde_json::json!({
        "round": 45,
        "topic": "equipment_slots_compact",
        "corpus": {
            "entities": ENTITY_COUNT,
            "hostile_ratio": "45%",
            "read_rounds": READ_ROUNDS,
            "deaths": DEATH_COUNT,
        },
        "gates": [
            {"name": "电池指纹全等", "passed": v.semantics_ok,
             "detail": format!("0x{old_fp:016x} vs 0x{new_fp:016x}")},
            {"name": "驻留字节下降（主判据）", "passed": v.resident_ok,
             "detail": format!("{} → {} B", old.resident, new.resident)},
            {"name": "构造+装备分配次数下降", "passed": v.spawn_allocs_ok,
             "detail": format!("{} → {}", old.spawn_allocs, new.spawn_allocs)},
            {"name": "读电池耗时不劣化（≥0.91×）", "passed": v.battery_ok,
             "detail": format!("×{:.3}", v.battery_ratio)},
        ],
        "old": {
            "resident_bytes": old.resident,
            "spawn_allocs": old.spawn_allocs,
            "spawn_ms": old.spawn_ms,
            "battery_ms": old.battery_ms,
            "death_allocs": old.death_allocs,
            "content_fingerprint": format!("0x{old_fp:016x}"),
        },
        "new": {
            "resident_bytes": new.resident,
            "spawn_allocs": new.spawn_allocs,
            "spawn_ms": new.spawn_ms,
            "battery_ms": new.battery_ms,
            "death_allocs": new.death_allocs,
            "content_fingerprint": format!("0x{new_fp:016x}"),
        },
        "sizes": {
            "old_equip_inline_bytes": std::mem::size_of::<HashMap<EquipmentSlot, ItemStack>>(),
            "new_equip_inline_bytes": std::mem::size_of::<NewEquip>(),
            "old_entity_inline_bytes": std::mem::size_of::<OldEntity>(),
            "new_entity_inline_bytes": std::mem::size_of::<NewEntity>(),
        },
        "outcome": if v.all_passed { "positive" } else { "negative" },
    });

    let path = std::path::Path::new("note/report/perf/round45-equipment-slots-compact.json");
    let Ok(mut file) = std::fs::File::create(path) else {
        eprintln!("写入报告失败");
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

// 抑制未使用告警：复刻体保留生产字段形状。
#[allow(dead_code)]
fn _touch(e: &OldEntity) -> u64 {
    let Ok(g) = e.equipment.lock() else { return 0 };
    g.equipment.len() as u64
}
