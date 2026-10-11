//! 轮次 35 基准：玩家交互与世界行为路径事件构造的订阅者先行门。
//!
//! 对照形态（轮 35 全量收编：第二梯队 64 处清单 + 扫描收编的第三
//! 梯队，合计约 130 个 fire 点——玩家容器/合成/锻造/牌匾/自定义
//! 载荷、桶装桶倒、告示牌命令预处理、发射器、活塞伸缩、龙相位、
//! 苦力怕充能、鸡下蛋、悬挂物、女巫三药、猪灵以物易物、实体跟踪
//! 器 show/hide 等）：
//! - 旧臂（现状）：事件在检查订阅者之前就构造——`to_string()` /
//!   `format!` / `clone()` 的 String 与 Vec 在无插件监听时全部
//!   白付（`fire_blocking` 内部短路在构造之后，救不回调用方
//!   已付的分配）；
//! - 新臂（收编）：`has_handlers::<E>()` 先行，无监听时零构造
//!   零分配；有监听时构造与派发逐字不变（门谓词与
//!   `fire_blocking` 内部短路完全同源）。
//!
//! 负载模型（「玩家交互 + 世界行为」混合轮，无订阅者；每轮复刻
//! 真实构造形态）：
//! - `PlayerSignCommandPreprocess`：command `clone()`（1 String）；
//! - `PlayerBucketFill`：`registry_key` `to_string()`（1 String）；
//! - `PlayerBucketEmpty`：同上（1 String）；
//! - `BlockDispense`：`registry_key` `to_string()`（1 String）；
//! - 每 4 轮活塞周期：`BlockPistonExtend` + `Retract` 各
//!   `format!("{dir:?}")`（2 String）；
//! - 每 8 轮龙相位切换：`current_phase`/`new_phase` 各
//!   `format!`（2 String）+ 苦力怕雷击充能 `"LIGHTNING"`
//!   `to_string()`（1 String）；
//! - 每 64 轮鸡下蛋：`"minecraft:egg"` `to_string()`（1 String）。
//!
//! 预登记门（跑前钉死，FAIL 须如实记录）：
//! - G1 两臂语义指纹一致（每轮交互/发射/活塞/相位成功计数
//!   折叠，成功路径两臂同构）；
//! - G2 旧臂分配 ≥ 轮数 × 4（每轮保底 4 个 String）、
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
/// 活塞周期间隔（红石时钟节奏）。
const PISTON_EVERY: u64 = 4;
/// 龙相位 / 苦力怕雷击间隔。
const RARE_EVERY: u64 = 8;
/// 鸡下蛋间隔（下蛋冷却远长于一轮交互）。
const EGG_EVERY: u64 = 64;
/// 每轮保底 String 数（sign 1 + bucket fill 1 + bucket empty 1 + dispense 1）。
const STRINGS_PER_ROUND: u64 = 4;

const REGISTRY_KEYS: [&str; 4] = [
    "minecraft:water_bucket",
    "minecraft:lava_bucket",
    "minecraft:arrow",
    "minecraft:fire_charge",
];
const COMMANDS: [&str; 4] = ["/say hi", "/tp @s 0 64 0", "/give @s bread 1", "/home"];
const PHASES: [&str; 4] = ["HoldingPattern", "StrafePlayer", "LandingApproach", "Dying"];
const FACINGS: [&str; 4] = ["North", "South", "East", "West"];

/// 模型事件：只保留 String 分配面（数值/坐标字段对分配无贡献）。
macro_rules! model_event {
    ($name:ident { $($field:ident),+ $(,)? }) => {
        #[allow(dead_code)] // 分配面模型：字段仅构造，经 black_box 保活
        struct $name {
            $($field: String),+
        }
    };
}

model_event!(ModelSignPreprocess { command });
model_event!(ModelBucketFill { bucket_item });
model_event!(ModelBucketEmpty { bucket_item });
model_event!(ModelDispense { item_name });
model_event!(ModelPistonEvent { direction });
model_event!(ModelDragonPhase {
    current_phase,
    new_phase
});
model_event!(ModelCreeperPower { power_source });
model_event!(ModelEggDrop { item_name });

/// 段读数：指纹 + 计量段分配 + 总耗时。
struct Report {
    fp: u64,
    delta: Delta,
    ns: u128,
}

/// 一轮交互负载的事件面（旧臂：无条件构造，fire 无处理器早退镜像）。
fn round_old(round: u64, fp: u64) -> u64 {
    let key = REGISTRY_KEYS[(round % REGISTRY_KEYS.len() as u64) as usize];
    let command = COMMANDS[(round % COMMANDS.len() as u64) as usize];
    let facing = FACINGS[(round % FACINGS.len() as u64) as usize];

    let mut fp = fp;
    // 告示牌命令预处理（1 String）
    let sign = ModelSignPreprocess {
        command: command.to_string(),
    };
    black_box(&sign);
    // 桶装（1 String）
    let fill = ModelBucketFill {
        bucket_item: key.to_string(),
    };
    black_box(&fill);
    // 桶倒（1 String）
    let empty = ModelBucketEmpty {
        bucket_item: key.to_string(),
    };
    black_box(&empty);
    // 发射器（1 String）
    let dispense = ModelDispense {
        item_name: key.to_string(),
    };
    black_box(&dispense);
    fp = fp.wrapping_mul(31).wrapping_add(4);
    // 活塞周期（extend + retract，2 String）
    if round.is_multiple_of(PISTON_EVERY) {
        let extend = ModelPistonEvent {
            direction: format!("{facing:?}"),
        };
        black_box(&extend);
        let retract = ModelPistonEvent {
            direction: format!("{facing:?}"),
        };
        black_box(&retract);
        fp = fp.wrapping_mul(31).wrapping_add(2);
    }
    // 龙相位（2 String）+ 苦力怕充能（1 String）
    if round.is_multiple_of(RARE_EVERY) {
        let phase = PHASES[((round / RARE_EVERY) % PHASES.len() as u64) as usize];
        let dragon = ModelDragonPhase {
            current_phase: format!("{phase:?}"),
            new_phase: format!("{phase:?}"),
        };
        black_box(&dragon);
        let creeper = ModelCreeperPower {
            power_source: "LIGHTNING".to_string(),
        };
        black_box(&creeper);
        fp = fp.wrapping_mul(31).wrapping_add(2);
    }
    // 鸡下蛋（1 String）
    if round.is_multiple_of(EGG_EVERY) {
        let egg = ModelEggDrop {
            item_name: "minecraft:egg".to_string(),
        };
        black_box(&egg);
        fp = fp.wrapping_mul(31).wrapping_add(1);
    }
    fp
}

/// 一轮交互负载的事件面（新臂：订阅者先行门，无监听零构造）。
fn round_new(round: u64, fp: u64) -> u64 {
    let has_handlers = false;
    let mut fp = fp;
    if has_handlers {
        // 与旧臂同构（镜像生产改造：有监听时构造与派发逐字不变）
        fp = round_old(round, fp);
    } else {
        // 无监听：跳过全部构造，仅折叠成功计数语义（与旧臂逐次
        // 折叠同构）
        let command = COMMANDS[(round % COMMANDS.len() as u64) as usize];
        black_box(command);
        fp = fp.wrapping_mul(31).wrapping_add(4); // 签名+桶装+桶倒+发射
        if round.is_multiple_of(PISTON_EVERY) {
            fp = fp.wrapping_mul(31).wrapping_add(2); // 活塞伸缩成功
        }
        if round.is_multiple_of(RARE_EVERY) {
            fp = fp.wrapping_mul(31).wrapping_add(2); // 相位切换+充能
        }
        if round.is_multiple_of(EGG_EVERY) {
            fp = fp.wrapping_mul(31).wrapping_add(1); // 下蛋成功
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

    println!("== 轮次 35 双臂基准：交互与世界行为路径事件构造订阅者先行门 ==");
    println!(
        "负载：{N_ROUNDS} 混合轮（签名+桶×2+发射器；每 {PISTON_EVERY} 轮活塞；每 {RARE_EVERY} 轮龙相位+充能；每 {EGG_EVERY} 轮下蛋），无订阅者模型\n"
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
