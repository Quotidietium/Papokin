//! 轮次 34 基准：自动服务器路径事件构造的订阅者先行门。
//!
//! 对照形态（第一梯队 19 处 fire 点：自然刷怪/区块生成刷怪 ×4、
//! 实体生成/物品掉落 ×2/战利品生成、漏斗吞吐 ×2、熔炉系宏 ×3、
//! 刷怪笼、信标效果 ×2、实体拾取、效果施加、姿态变化）：
//! - 旧臂（现状）：事件在检查订阅者之前就构造——事件字段里的
//!   `format!("minecraft:{}")` / `to_string()` String 在无插件
//!   监听时全部白付（`fire_blocking` 内部短路在构造之后，
//!   救不回调用方已付的分配）；
//! - 新臂（收编）：`has_handlers::<E>()` 先行，无监听时零构造
//!   零分配；有监听时构造与派发逐字不变（门谓词与
//!   `fire_blocking` 内部短路完全同源）。
//!
//! 负载模型（「自动服务器运转」混合轮，无订阅者；每轮复刻
//! 真实构造形态）：
//! - `PreCreatureSpawn`：`format!("minecraft:{}")` + reason
//!   `to_string()`（2 String）；
//! - `CreatureSpawn`：`resource_name` + reason 各一 `to_string()`
//!   （2 String）；
//! - `EntitySpawn`：`entity_type.id` `to_string()`（1 String）；
//! - `ItemSpawn` ×2（掉落）：`registry_key` `to_string()`
//!   （2 String）；
//! - 漏斗周期 4 槽：`InventoryMoveItem` `registry_key`
//!   `to_string()` ×4（4 String）；
//! - 每 8 轮一次熔炉周期：burn（1 String）+ smelt（2 String）。
//!
//! 预登记门（跑前钉死，FAIL 须如实记录）：
//! - G1 两臂语义指纹一致（每轮生成/传输/熔炼成功计数与
//!   cancelled=false 语义，成功路径两臂同构）；
//! - G2 旧臂分配 ≥ 轮数 × 11（每轮至少 11 个 String）、
//!   新臂 == 0；
//! - G3 新臂耗时 ≤ 旧臂；
//! - G4 旧臂毛字节 > 新臂（>0 vs 0）。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

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

/// 混合轮数。
const N_ROUNDS: u64 = 100_000;
/// 熔炉周期间隔（漏斗每 8 刻扫描、熔炉按烧炼节奏）。
const FURNACE_EVERY: u64 = 8;
/// 每轮保底 String 数（pre 2 + spawn 2 + entity 1 + item 2 + 漏斗 4）。
const STRINGS_PER_ROUND: u64 = 11;

const ENTITY_NAMES: [&str; 4] = ["zombie", "skeleton", "spider", "creeper"];
const REGISTRY_KEYS: [&str; 4] = [
    "minecraft:cobblestone",
    "minecraft:iron_ingot",
    "minecraft:coal",
    "minecraft:wheat",
];
const REASONS: [&str; 2] = ["NATURAL", "CHUNK_GENERATION"];

/// 模型事件：只保留 String 分配面（数值字段对分配无贡献）。
macro_rules! model_event {
    ($name:ident { $($field:ident),+ $(,)? }) => {
        #[allow(dead_code)] // 分配面模型：字段仅构造，经 black_box 保活
        struct $name {
            $($field: String),+
        }
    };
}

model_event!(ModelPreSpawn {
    entity_type,
    reason
});
model_event!(ModelCreatureSpawn {
    entity_type,
    reason
});
model_event!(ModelEntitySpawn { entity_type });
model_event!(ModelItemSpawn { registry_key });
model_event!(ModelMoveItem { registry_key });
model_event!(ModelFurnaceBurn { fuel_item });
model_event!(ModelFurnaceSmelt {
    source_item,
    result
});

/// 段读数：指纹 + 计量段分配 + 总耗时。
struct Report {
    fp: u64,
    delta: Delta,
    ns: u128,
}

/// 一轮自动负载的事件面（旧臂：无条件构造，fire 无处理器早退镜像）。
fn round_old(round: u64, fp: u64) -> u64 {
    let name = ENTITY_NAMES[(round % ENTITY_NAMES.len() as u64) as usize];
    let key = REGISTRY_KEYS[(round % REGISTRY_KEYS.len() as u64) as usize];
    let reason = REASONS[(round % REASONS.len() as u64) as usize];

    let mut fp = fp;
    // PreCreatureSpawn（2 String）
    let pre = ModelPreSpawn {
        entity_type: format!("minecraft:{name}"),
        reason: reason.to_string(),
    };
    black_box(&pre);
    // CreatureSpawn（2 String）
    let spawn = ModelCreatureSpawn {
        entity_type: name.to_string(),
        reason: reason.to_string(),
    };
    black_box(&spawn);
    fp = fp.wrapping_mul(31).wrapping_add(1);
    // EntitySpawn（1 String）
    let entity = ModelEntitySpawn {
        entity_type: name.to_string(),
    };
    black_box(&entity);
    // 掉落 2 件：ItemSpawn ×2（2 String）
    for _ in 0..2 {
        let item = ModelItemSpawn {
            registry_key: key.to_string(),
        };
        black_box(&item);
    }
    fp = fp.wrapping_mul(31).wrapping_add(2);
    // 漏斗周期 4 槽（4 String）
    for _ in 0..4 {
        let mv = ModelMoveItem {
            registry_key: key.to_string(),
        };
        black_box(&mv);
        fp = fp.wrapping_mul(31).wrapping_add(1);
    }
    // 熔炉周期（burn 1 + smelt 2）
    if round.is_multiple_of(FURNACE_EVERY) {
        let burn = ModelFurnaceBurn {
            fuel_item: key.to_string(),
        };
        black_box(&burn);
        let smelt = ModelFurnaceSmelt {
            source_item: key.to_string(),
            result: key.to_string(),
        };
        black_box(&smelt);
        fp = fp.wrapping_mul(31).wrapping_add(3);
    }
    fp
}

/// 一轮自动负载的事件面（新臂：订阅者先行门，无监听零构造）。
fn round_new(round: u64, fp: u64) -> u64 {
    let has_handlers = false;
    let mut fp = fp;
    if has_handlers {
        // 与旧臂同构（镜像生产改造：有监听时构造与派发逐字不变）
        fp = round_old(round, fp);
    } else {
        // 无监听：跳过全部构造，仅折叠成功计数语义（与旧臂逐次
        // 折叠同构——漏斗 4 槽为 4 次乘加而非一次）
        let name = ENTITY_NAMES[(round % ENTITY_NAMES.len() as u64) as usize];
        black_box(name);
        fp = fp.wrapping_mul(31).wrapping_add(1); // 生成成功
        fp = fp.wrapping_mul(31).wrapping_add(2); // 掉落 2 件
        for _ in 0..4 {
            fp = fp.wrapping_mul(31).wrapping_add(1); // 漏斗传输成功
        }
        if round.is_multiple_of(FURNACE_EVERY) {
            fp = fp.wrapping_mul(31).wrapping_add(3); // 熔炼完成
        }
    }
    fp
}

fn arm_old() -> Report {
    let mut fp = 0xcbf2_9ce4_8422_2325u64;
    let t0 = Instant::now();
    let start = snapshot();
    for round in 0..N_ROUNDS {
        fp = round_old(round, fp);
    }
    let delta = delta_since(start);
    Report {
        fp,
        delta,
        ns: t0.elapsed().as_nanos(),
    }
}

fn arm_new() -> Report {
    let mut fp = 0xcbf2_9ce4_8422_2325u64;
    let t0 = Instant::now();
    let start = snapshot();
    for round in 0..N_ROUNDS {
        fp = round_new(round, fp);
    }
    let delta = delta_since(start);
    Report {
        fp,
        delta,
        ns: t0.elapsed().as_nanos(),
    }
}

fn main() {
    let mut gates_pass = true;

    println!("== 轮次 34 双臂基准：自动路径事件构造订阅者先行门 ==");
    println!(
        "负载：{N_ROUNDS} 混合轮（刷怪+掉落+漏斗；每 {FURNACE_EVERY} 轮附熔炉周期），无订阅者模型\n"
    );

    let boot = snapshot();
    println!("启动健全性：{} allocs / {} B", boot.0, boot.1);

    let old = arm_old();
    let new = arm_new();
    println!(
        "旧臂 {:>8} allocs / {:>9} B / {:>10} ns",
        old.delta.0, old.delta.1, old.ns
    );
    println!(
        "新臂 {:>8} allocs / {:>9} B / {:>10} ns",
        new.delta.0, new.delta.1, new.ns
    );

    let g1 = old.fp == new.fp;
    println!(
        "G1 两臂语义指纹一致（{:#x} vs {:#x}）→ {}",
        old.fp,
        new.fp,
        verdict(g1)
    );
    gates_pass &= g1;
    let g2 = old.delta.0 >= N_ROUNDS * STRINGS_PER_ROUND && new.delta.0 == 0;
    println!(
        "G2 旧臂 ≥ {}（实际 {}）、新臂 == 0（实际 {}）→ {}",
        N_ROUNDS * STRINGS_PER_ROUND,
        old.delta.0,
        new.delta.0,
        verdict(g2)
    );
    gates_pass &= g2;
    let g3 = new.ns <= old.ns;
    println!(
        "G3 新臂耗时 ≤ 旧臂（{} ns ≤ {} ns）→ {}",
        new.ns,
        old.ns,
        verdict(g3)
    );
    gates_pass &= g3;
    let g4 = old.delta.1 > new.delta.1;
    println!(
        "G4 旧臂毛字节 > 新臂（{} B > {} B）→ {}",
        old.delta.1,
        new.delta.1,
        verdict(g4)
    );
    gates_pass &= g4;

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
