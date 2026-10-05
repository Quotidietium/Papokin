//! 轮次 23 基准：线程局部暂存的用后容量衰减。
//!
//! 对照形态：
//! - 无衰减（现状）：`BLOCK_COLLISION_SCRATCH`/`ENTITY_BOX_SCRATCH`/
//!   `PLAYER_BOX_SCRATCH` 等 `thread_local` Vec 只清不重缩，任何一次
//!   尖峰填充（复杂地形碰撞、密集区按盒查询）的容量随线程永久驻留；
//! - 衰减（收编）：消费闭包返回后按本轮填充量 `decay_clear_vec`
//!   （轮次 20 原语：容量超 max(4×本轮长度, 64) 才收缩到 长度+64）。
//!
//! 负载模型（对齐「多线程 × 稳态小填充 + 偶发尖峰」的真实形态）：
//! - 8 个模拟线程（rayon 池量级）× 600 tick；
//! - 每线程每 tick 两次调用：碰撞收集（双 Vec：24B 形状项 +
//!   16B 位置项）稳态 32 项、按盒查询（8B 项）稳态 48 项；
//! - 第 10 tick 每线程各一次尖峰：碰撞 8192 项、按盒 4096 项
//!   （复杂地形实体移动 / 密集区大盒查询）。
//!
//! 观测：尖峰后一拍与收尾的每线程驻留（字节）、全程分配次数、
//! 收缩事件计数（稳态段必须为 0）、消费内容指纹全等（`f` 所见
//! 切片不因衰减改变）。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};

// ---------------------------------------------------------------------------
// 分配计数器
// ---------------------------------------------------------------------------

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

struct Counting;

// SAFETY: 仅统计后原样转发系统分配器，不改变任何分配语义
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        // SAFETY: 转发给系统分配器。
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: 转发给系统分配器。
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

// ---------------------------------------------------------------------------
// 模型：每线程三条暂存 Vec（尺寸对齐生产条目）
// ---------------------------------------------------------------------------

const THREADS: usize = 8;
const TICKS: u32 = 600;
const SPIKE_TICK: u32 = 10;
const COLLISION_STEADY: usize = 32;
const COLLISION_SPIKE: usize = 8192;
const BOX_STEADY: usize = 48;
const BOX_SPIKE: usize = 4096;

/// 生产侧衰减原语逐字同构（`papokin-util::capacity::decay_clear_vec`）
fn decay_clear_vec<T>(vec: &mut Vec<T>) {
    let last_len = vec.len();
    vec.clear();
    if vec.capacity() > last_len.saturating_mul(4).max(64) {
        vec.shrink_to(last_len + 64);
    }
}

/// 生产侧填充语义：`_into` 入口 `clear` 重填
fn fill_with<T>(vec: &mut Vec<T>, count: usize, tick: u32, lane: u64, mk: impl Fn(u64) -> T) {
    vec.clear();
    for i in 0..count as u64 {
        vec.push(mk(lane ^ u64::from(tick) << 32 | i));
    }
}

struct ThreadScratch {
    /// 碰撞形状暂存：24B 项（生产 `BoundingBox`）
    collisions: Vec<[u64; 3]>,
    /// 位置映射暂存：16B 项（生产 `(usize, BlockPos)`）
    positions: Vec<[u64; 2]>,
    /// 按盒查询暂存：8B 项（生产 `Arc`）
    box_query: Vec<u64>,
}

impl ThreadScratch {
    const fn retained_bytes(&self) -> usize {
        self.collisions.capacity() * size_of::<[u64; 3]>()
            + self.positions.capacity() * size_of::<[u64; 2]>()
            + self.box_query.capacity() * size_of::<u64>()
    }
}

// ---------------------------------------------------------------------------
// 指纹（逐调用交换律折叠 + 跨调用顺序链，同轮次 20 范式）
// ---------------------------------------------------------------------------

const fn fp_mix(h: u64, v: u64) -> u64 {
    let mut x = h ^ v;
    x = x.wrapping_mul(0x0000_0100_0000_01B3);
    x.rotate_left(17)
}

/// 消费闭包 `f` 的观测折叠：对本调用所见切片求交换律和
fn consume_fp<T>(slice: &[T], bits: impl Fn(&T) -> u64) -> u64 {
    let mut fp = 0u64;
    for (i, v) in slice.iter().enumerate() {
        fp ^= fp_mix(bits(v), i as u64).rotate_left((i & 31) as u32);
    }
    fp
}

// ---------------------------------------------------------------------------
// 臂驱动
// ---------------------------------------------------------------------------

struct ArmReport {
    fp: u64,
    retained_after_spike: usize,
    retained_final: usize,
    allocs_total: usize,
    shrink_events: usize,
    shrink_after_tick100: usize,
}

/// 单个调用点的「填充 → 消费 → 用后清理」全序，两臂共享
#[allow(clippy::too_many_arguments)]
fn drive_call<T>(
    vec: &mut Vec<T>,
    count: usize,
    tick: u32,
    lane: u64,
    mk: impl Fn(u64) -> T,
    bits: impl Fn(&T) -> u64,
    decay: bool,
    events: &mut usize,
    late_events: &mut usize,
) -> u64 {
    fill_with(vec, count, tick, lane, mk);
    let call_fp = consume_fp(vec, bits);
    let cap_before = vec.capacity();
    if decay {
        decay_clear_vec(vec);
    } else {
        vec.clear();
    }
    *events += usize::from(vec.capacity() < cap_before);
    *late_events += usize::from(tick > 100 && vec.capacity() < cap_before);
    call_fp
}

fn run_arm(decay: bool) -> ArmReport {
    let mut scratches: Vec<ThreadScratch> = (0..THREADS)
        .map(|_| ThreadScratch {
            collisions: Vec::new(),
            positions: Vec::new(),
            box_query: Vec::new(),
        })
        .collect();
    let mut fp = 0u64;
    let mut shrink_events = 0usize;
    let mut shrink_after_tick100 = 0usize;

    // 预热（对齐既往轮次惯例）
    for (t, scratch) in scratches.iter_mut().enumerate() {
        fill_with(&mut scratch.collisions, 64, 0, t as u64, |v| [v; 3]);
        fill_with(&mut scratch.box_query, 64, 0, t as u64, |v| v);
        scratch.collisions.clear();
        scratch.box_query.clear();
    }

    let a0 = ALLOCS.load(Ordering::Relaxed);
    let mut retained_after_spike = 0usize;

    for tick in 1..=TICKS {
        for (t, scratch) in scratches.iter_mut().enumerate() {
            let lane = t as u64;
            let collision_count = if tick == SPIKE_TICK {
                COLLISION_SPIKE
            } else {
                COLLISION_STEADY
            };
            let box_count = if tick == SPIKE_TICK {
                BOX_SPIKE
            } else {
                BOX_STEADY
            };

            // 调用点 1：碰撞收集（形状 + 位置双暂存，生产同点双衰减）
            let mut call_fp = drive_call(
                &mut scratch.collisions,
                collision_count,
                tick,
                lane,
                |v| [v; 3],
                |v| v[0],
                decay,
                &mut shrink_events,
                &mut shrink_after_tick100,
            );
            call_fp ^= drive_call(
                &mut scratch.positions,
                collision_count,
                tick,
                lane | 0xA500,
                |v| [v; 2],
                |v| v[0],
                decay,
                &mut shrink_events,
                &mut shrink_after_tick100,
            )
            .rotate_left(11);

            // 调用点 2：按盒查询（同构）
            call_fp ^= drive_call(
                &mut scratch.box_query,
                box_count,
                tick,
                lane | 0x5A00,
                |v| v,
                |v| *v,
                decay,
                &mut shrink_events,
                &mut shrink_after_tick100,
            )
            .rotate_left(7);

            // 跨调用顺序链（tick × 线程交织序敏感）
            fp = fp_mix(fp, call_fp ^ (u64::from(tick) << 8 | lane));
        }
        if tick == SPIKE_TICK + 1 {
            retained_after_spike = scratches.iter().map(ThreadScratch::retained_bytes).sum();
        }
    }

    let retained_final = scratches.iter().map(ThreadScratch::retained_bytes).sum();
    let a1 = ALLOCS.load(Ordering::Relaxed);

    ArmReport {
        fp,
        retained_after_spike,
        retained_final,
        allocs_total: a1 - a0,
        shrink_events,
        shrink_after_tick100,
    }
}

// ---------------------------------------------------------------------------
// 闸门与报告
// ---------------------------------------------------------------------------

fn evaluate_gates(no_decay: &ArmReport, decayed: &ArmReport) -> Vec<(&'static str, bool, String)> {
    let mut gates: Vec<(&str, bool, String)> = Vec::new();

    // 闸门 1：消费内容指纹全等（f 所见切片不受衰减影响）
    gates.push((
        "指纹全等",
        no_decay.fp == decayed.fp && decayed.fp != 0,
        format!("无衰减 {:#018x} vs 衰减 {:#018x}", no_decay.fp, decayed.fp),
    ));

    // 闸门 2：尖峰后一拍每线程驻留 ≥4× 回收
    gates.push((
        "尖峰后驻留 ≥4× 回收",
        decayed.retained_after_spike * 4 <= no_decay.retained_after_spike,
        format!(
            "无衰减 {} B vs 衰减 {} B（{:.1}×）",
            no_decay.retained_after_spike,
            decayed.retained_after_spike,
            no_decay.retained_after_spike as f64 / decayed.retained_after_spike.max(1) as f64
        ),
    ));

    // 闸门 3：收尾驻留 ≥4× 回收（尖峰不随线程永久驻留）
    gates.push((
        "收尾驻留 ≥4× 回收",
        decayed.retained_final * 4 <= no_decay.retained_final,
        format!(
            "无衰减 {} B vs 衰减 {} B（{:.1}×）",
            no_decay.retained_final,
            decayed.retained_final,
            no_decay.retained_final as f64 / decayed.retained_final.max(1) as f64
        ),
    ));

    // 闸门 4：收缩事件有界（每线程每缓冲至多尖峰后一拍一次）且
    // 第 100 tick 后零收缩（稳态无抖动）
    gates.push((
        "收缩有界且稳态零收缩",
        decayed.shrink_events <= THREADS * 3 && decayed.shrink_after_tick100 == 0,
        format!(
            "总 {} 次（上界 {}），第 100 tick 后 {} 次",
            decayed.shrink_events,
            THREADS * 3,
            decayed.shrink_after_tick100
        ),
    ));

    // 闸门 5：额外分配有界（每线程每缓冲一次收缩重分配 + 回涨差）
    let extra = decayed.allocs_total as isize - no_decay.allocs_total as isize;
    gates.push((
        "额外分配 ≤ 64",
        extra <= 64,
        format!(
            "全程分配：无衰减 {} vs 衰减 {}（差值 {extra}）",
            no_decay.allocs_total, decayed.allocs_total
        ),
    ));

    gates
}

fn main() {
    let no_decay = run_arm(false);
    let decayed = run_arm(true);
    let gates = evaluate_gates(&no_decay, &decayed);

    let pass = gates.iter().all(|(_, ok, _)| *ok);

    let report = serde_json::json!({
        "round": 23,
        "theme": "线程局部暂存的用后容量衰减",
        "workload": {
            "threads": THREADS,
            "ticks": TICKS,
            "spike_tick": SPIKE_TICK,
            "collision_steady": COLLISION_STEADY,
            "collision_spike": COLLISION_SPIKE,
            "box_steady": BOX_STEADY,
            "box_spike": BOX_SPIKE,
        },
        "no_decay": {
            "retained_after_spike_bytes": no_decay.retained_after_spike,
            "retained_final_bytes": no_decay.retained_final,
            "allocs_total": no_decay.allocs_total,
            "fingerprint": format!("{:#018x}", no_decay.fp),
        },
        "decayed": {
            "retained_after_spike_bytes": decayed.retained_after_spike,
            "retained_final_bytes": decayed.retained_final,
            "allocs_total": decayed.allocs_total,
            "shrink_events": decayed.shrink_events,
            "shrink_after_tick100": decayed.shrink_after_tick100,
            "fingerprint": format!("{:#018x}", decayed.fp),
        },
        "gates": gates
            .iter()
            .map(|(name, ok, detail)| serde_json::json!({
                "name": name,
                "pass": ok,
                "detail": detail,
            }))
            .collect::<Vec<_>>(),
        "pass": pass,
    });

    let path = "note/report/perf/round23-thread-local-scratch-decay.json";
    match std::fs::File::create(path) {
        Ok(mut file) => {
            if let Err(err) = file.write_all(
                serde_json::to_string_pretty(&report)
                    .unwrap_or_default()
                    .as_bytes(),
            ) {
                eprintln!("写入报告失败: {err}");
                std::process::exit(1);
            }
        }
        Err(err) => {
            eprintln!("创建报告文件失败: {err}");
            std::process::exit(1);
        }
    }

    println!("=== 轮次 23：线程局部暂存用后容量衰减 ===");
    println!(
        "驻留：尖峰后 无衰减 {} B / 衰减 {} B；收尾 {} B / {} B",
        no_decay.retained_after_spike,
        decayed.retained_after_spike,
        no_decay.retained_final,
        decayed.retained_final
    );
    for (name, ok, detail) in &gates {
        println!("[{}] {name}: {detail}", if *ok { "PASS" } else { "FAIL" });
    }
    if !pass {
        std::process::exit(2);
    }
}
