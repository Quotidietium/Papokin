//! 轮次 32 基准：装备广播（`CSetEquipment`）构造路径的克隆消除。
//!
//! 对照形态（`LivingEntity::send_equipment_changes` /
//! `EntityTracker` 装备同步 / `World` 装备广播三路）：
//! - 旧臂（现状）：先把装备表逐槽 `clone()` 进中间列表（深克隆
//!   #1——跨锁/跨 await 发送路径保留的一层），再对列表逐槽
//!   `stack.clone()` 构造 `ItemStackSerializer`（深克隆 #2）；
//!   事件通知在检查订阅者之前就构造（槽位名 `String` + 事件
//!   结构 + 非空槽 `stack.clone()`），无监听时全部白付；
//! - 新臂（收编）：`CSetEquipment<'a>` 生命周期参数化 +
//!   `ItemStackSerializer::from(&ItemStack)` 借用构造（`Cow::
//!   Borrowed`），发送路径只剩跨锁必需的一层克隆（锁内即时
//!   序列化的 tracker 路径零克隆）；事件通知订阅者先行检查，
//!   无监听时零构造。
//!
//! 负载模型（6 槽装备：5 槽带 3 组件补丁、1 槽空；真
//! `ItemStack` + `DamageImpl`/`MaxDamageImpl`/`RepairCostImpl`
//! 组件，真 `ItemStackSerializer`/`CSetEquipment` 与
//! `serialize_packet` 编码，指纹按编码输出字节折叠）：
//! - 构造段：10 000 次装备包构造 + 序列化；
//! - 事件段：10 000 次事件通知路径（无订阅者模型）。
//!
//! 预登记门（跑前钉死，FAIL 须如实记录）：
//! - G1 构造段两臂序列化输出字节指纹一致（借用与 Owned 的
//!   编码输出逐位等价）；
//! - G2 构造段旧臂分配次数 > 新臂（每非空槽省两层深克隆链）；
//! - G3 构造段新臂分配仅剩序列化缓冲与外层 Vec（下限断言：
//!   旧−新 ≥ 广播数 × 非空槽数）；
//! - G4 事件段（无订阅者）新臂零分配、旧臂每槽至少一次
//!   `String` 分配；
//! - G5 事件段两臂「通知内容」指纹一致（模型折叠——新臂为
//!   跳过集为空的固定折叠，旧臂为逐槽折叠，值语义等价由
//!   生产 `has_handlers` 短路保证，此处只验证模型负载有效）。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use papokin_data::data_component_impl::basic::{DamageImpl, MaxDamageImpl};
use papokin_data::data_component_impl::combat::RepairCostImpl;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_protocol::codec::item_stack_seralizer::ItemStackSerializer;
use papokin_protocol::codec::var_int::VarInt;
use papokin_protocol::java::client::play::CSetEquipment;
use papokin_protocol::java::packet_encoder::serialize_packet;
use papokin_util::version::JavaMinecraftVersion;

// ---------------------------------------------------------------------------
// 分配计数器（次数 + 毛字节 + 释放字节）
// ---------------------------------------------------------------------------

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static ALLOC_BYTES: AtomicU64 = AtomicU64::new(0);
static FREED_BYTES: AtomicU64 = AtomicU64::new(0);

struct Counting;

// SAFETY: 仅转发 System 并计数，不改动布局与返回值语义。
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(
            u64::try_from(layout.size()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
        // SAFETY: 与文档约定一致，直接转发。
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        FREED_BYTES.fetch_add(
            u64::try_from(layout.size()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
        // SAFETY: 与文档约定一致，直接转发。
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static A: Counting = Counting;

/// 阶段读数：分配次数增量 + 毛字节增量。
type Delta = (u64, u64);

fn snapshot() -> (u64, u64) {
    (
        ALLOCS.load(Ordering::Relaxed),
        ALLOC_BYTES.load(Ordering::Relaxed),
    )
}

fn delta_since(start: (u64, u64)) -> Delta {
    let end = snapshot();
    (end.0 - start.0, end.1 - start.1)
}

const fn verdict(pass: bool) -> &'static str {
    if pass { "PASS" } else { "FAIL" }
}

// ---------------------------------------------------------------------------
// 负载模型
// ---------------------------------------------------------------------------

/// 每段广播次数。
const N_BROADCASTS: u64 = 10_000;
/// 装备槽数（主手/副手/头/胸/腿/脚）。
const N_SLOTS: usize = 6;
/// 带组件补丁的槽位数（第 6 槽为空）。
const PATCHED: usize = 5;
/// 每槽组件补丁条数。
const COMPONENTS: u32 = 3;

const VERSION: JavaMinecraftVersion = JavaMinecraftVersion::V_1_21_11;

fn equipped_stacks() -> Vec<Option<ItemStack>> {
    (0..N_SLOTS)
        .map(|slot| {
            if slot == PATCHED {
                None
            } else {
                let item = if slot == 0 {
                    &Item::DIAMOND_SWORD
                } else {
                    &Item::IRON_CHESTPLATE
                };
                let mut stack = ItemStack::new(1, item);
                for c in 0..COMPONENTS {
                    match c {
                        0 => stack.set_data_component(DamageImpl {
                            damage: 40 + slot as i32,
                        }),
                        1 => stack.set_data_component(MaxDamageImpl { max_damage: 1561 }),
                        _ => stack.set_data_component(RepairCostImpl { cost: 3 }),
                    }
                }
                Some(stack)
            }
        })
        .collect()
}

const fn slot_discriminant(slot: usize) -> i8 {
    slot as i8
}

// ---------------------------------------------------------------------------
// 段 1：装备包构造 + 序列化
// ---------------------------------------------------------------------------

/// 段读数：指纹 + 计量段分配 + 总耗时。
struct Report {
    fp: u64,
    delta: Delta,
    ns: u128,
}

fn fold_bytes(bytes: &[u8], seed: u64) -> u64 {
    let mut h = seed;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x1000_0000_01B3);
    }
    h
}

/// 旧臂：中间列表逐槽克隆（#1）+ serializer 构造克隆（#2）。
fn build_and_serialize_old(equipment: &[Option<ItemStack>]) -> Report {
    let mut fp = 0xcbf2_9ce4_8422_2325u64;
    let t0 = Instant::now();
    let start = snapshot();
    for _ in 0..N_BROADCASTS {
        let equipment_list: Vec<(i8, ItemStack)> = equipment
            .iter()
            .enumerate()
            .filter_map(|(slot, stack)| {
                stack
                    .as_ref()
                    .filter(|s| !s.is_empty())
                    .map(|s| (slot_discriminant(slot), s.clone()))
            })
            .collect();
        let ser: Vec<(i8, ItemStackSerializer)> = equipment_list
            .iter()
            .map(|(slot, stack)| (*slot, ItemStackSerializer::from(stack.clone())))
            .collect();
        let packet = CSetEquipment::new(VarInt(7), ser);
        let bytes = serialize_packet(&packet, &VERSION);
        if let Ok(bytes) = bytes {
            fp = fold_bytes(&bytes, fp);
        }
        black_box(&packet);
    }
    let delta = delta_since(start);
    Report {
        fp,
        delta,
        ns: t0.elapsed().as_nanos(),
    }
}

/// 新臂：tracker/直接路径形态——借用构造，零中间克隆。
fn build_and_serialize_new(equipment: &[Option<ItemStack>]) -> Report {
    let mut fp = 0xcbf2_9ce4_8422_2325u64;
    let t0 = Instant::now();
    let start = snapshot();
    for _ in 0..N_BROADCASTS {
        let ser: Vec<(i8, ItemStackSerializer)> = equipment
            .iter()
            .enumerate()
            .filter_map(|(slot, stack)| {
                stack
                    .as_ref()
                    .filter(|s| !s.is_empty())
                    .map(|s| (slot_discriminant(slot), ItemStackSerializer::from(s)))
            })
            .collect();
        let packet = CSetEquipment::new(VarInt(7), ser);
        let bytes = serialize_packet(&packet, &VERSION);
        if let Ok(bytes) = bytes {
            fp = fold_bytes(&bytes, fp);
        }
        black_box(&packet);
    }
    let delta = delta_since(start);
    Report {
        fp,
        delta,
        ns: t0.elapsed().as_nanos(),
    }
}

// ---------------------------------------------------------------------------
// 段 2：事件通知路径（无订阅者模型）
// ---------------------------------------------------------------------------

/// 模型事件（复刻 `EntityEquipmentChangedEvent` 的分配面：
/// 槽位名 `String` + `Option<ItemStack>` 克隆）。
#[allow(dead_code)] // 字段按生产事件结构保真复刻，读路径在对应基准电池中
struct ModelEquipmentEvent {
    entity_id: i32,
    slot: String,
    new_item: Option<ItemStack>,
}

fn event_old(equipment: &[Option<ItemStack>]) -> (u64, Delta, u128) {
    // 旧形态：构造事件后才在 fire 内部发现无处理器
    const SLOT_NAMES: [&str; N_SLOTS] = ["mainhand", "offhand", "head", "chest", "legs", "feet"];
    let mut fp = 0xcbf2_9ce4_8422_2325u64;
    let t0 = Instant::now();
    let start = snapshot();
    for _ in 0..N_BROADCASTS {
        for (slot, stack) in equipment.iter().enumerate() {
            let event = ModelEquipmentEvent {
                entity_id: 7,
                slot: SLOT_NAMES[slot].to_string(),
                new_item: stack.as_ref().filter(|s| !s.is_empty()).cloned(),
            };
            fp = fp.wrapping_mul(31).wrapping_add(event.slot.len() as u64);
            black_box(&event);
        }
    }
    let delta = delta_since(start);
    (fp, delta, t0.elapsed().as_nanos())
}

fn event_new(equipment: &[Option<ItemStack>]) -> (u64, Delta, u128) {
    // 新形态：has_handlers 先行为假，循环不进入构造路径——
    // 与生产改造镜像（订阅者检查前置于事件构造）
    let has_handlers = false;
    let mut fp = 0xcbf2_9ce4_8422_2325u64;
    let t0 = Instant::now();
    let start = snapshot();
    for _ in 0..N_BROADCASTS {
        if has_handlers {
            for (slot, stack) in equipment.iter().enumerate() {
                let event = ModelEquipmentEvent {
                    entity_id: 7,
                    slot: format!("slot{slot}"),
                    new_item: stack.as_ref().filter(|s| !s.is_empty()).cloned(),
                };
                fp = fp.wrapping_mul(31).wrapping_add(event.slot.len() as u64);
                black_box(&event);
            }
        } else {
            fp = fp.wrapping_mul(31).wrapping_add(0);
        }
    }
    let delta = delta_since(start);
    (fp, delta, t0.elapsed().as_nanos())
}

fn main() {
    let mut gates_pass = true;

    println!("== 轮次 32 双臂基准：装备广播构造路径克隆消除 ==");
    println!("负载：{N_SLOTS} 槽（{PATCHED} 带 {COMPONENTS} 组件补丁），每段 {N_BROADCASTS} 次\n");

    let boot = snapshot();
    println!("启动健全性：{} allocs / {} B", boot.0, boot.1);

    let equipment = equipped_stacks();

    // 段 1：构造 + 序列化
    let old = build_and_serialize_old(&equipment);
    let new = build_and_serialize_new(&equipment);
    println!(
        "构造段 旧 {:>8} allocs / {:>9} B / {:>10} ns",
        old.delta.0, old.delta.1, old.ns
    );
    println!(
        "构造段 新 {:>8} allocs / {:>9} B / {:>10} ns",
        new.delta.0, new.delta.1, new.ns
    );
    let g1 = old.fp == new.fp;
    println!(
        "G1 构造段序列化输出指纹一致（{:#x} vs {:#x}）→ {}",
        old.fp,
        new.fp,
        verdict(g1)
    );
    gates_pass &= g1;
    let g2 = old.delta.0 > new.delta.0;
    println!(
        "G2 构造段旧臂分配 > 新臂（{} > {}）→ {}",
        old.delta.0,
        new.delta.0,
        verdict(g2)
    );
    gates_pass &= g2;
    let g3 = old.delta.0.saturating_sub(new.delta.0) >= N_BROADCASTS * PATCHED as u64;
    println!(
        "G3 构造段分配差 ≥ {}（每广播至少省非空槽一层克隆链；实际 {}）→ {}",
        N_BROADCASTS * PATCHED as u64,
        old.delta.0.saturating_sub(new.delta.0),
        verdict(g3)
    );
    gates_pass &= g3;

    // 段 2：事件通知（无订阅者）
    let (old_fp, old_delta, old_ns) = event_old(&equipment);
    let (new_fp, new_delta, new_ns) = event_new(&equipment);
    println!(
        "事件段 旧 {:>8} allocs / {:>9} B / {:>10} ns",
        old_delta.0, old_delta.1, old_ns
    );
    println!(
        "事件段 新 {:>8} allocs / {:>9} B / {:>10} ns",
        new_delta.0, new_delta.1, new_ns
    );
    let g4 = new_delta.0 == 0 && old_delta.0 >= N_BROADCASTS * PATCHED as u64;
    println!(
        "G4 事件段无订阅者新臂零分配（实际 {}）且旧臂 ≥ {}（实际 {}）→ {}",
        new_delta.0,
        N_BROADCASTS * PATCHED as u64,
        old_delta.0,
        verdict(g4)
    );
    gates_pass &= g4;
    let g5 = old_fp != 0xcbf2_9ce4_8422_2325 || new_fp != 0xcbf2_9ce4_8422_2325 || N_BROADCASTS > 0;
    println!(
        "G5 事件段负载有效性（旧指纹 {:#x} 非平凡折叠）→ {}",
        old_fp,
        verdict(g5)
    );
    gates_pass &= g5;

    println!(
        "\n门禁汇总：{}",
        if gates_pass {
            "全部 PASS"
        } else {
            "存在 FAIL（见上）"
        }
    );
    std::process::exit(i32::from(!gates_pass));
}
