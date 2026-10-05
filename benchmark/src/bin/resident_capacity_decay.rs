//! 轮次 20 基准：驻留缓冲容量衰减治理（负载尖峰后的保留容量回收）。
//!
//! 对照形态：
//! - 无衰减（现状）：驻留 `Vec`/`HashMap` 每 tick `clear()` 重填，
//!   容量永久保留历史峰值；
//! - 衰减（收编）：清理时记录上轮长度，容量超过
//!   `max(4×上轮长度, 地板)` 则收缩到「上轮长度 + 地板」。
//!
//! 负载模型：600 tick；前 10 tick 为尖峰时代（爆炸级方块变更 +
//! 传送级区块批次），其后为稳态时代。观测：两臂在尖峰末与收尾的
//! 保留容量（字节）、稳态时代分配次数（收缩事件数上界）、以及
//! 处理指纹全等（衰减不改变任何投递语义）。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::HashMap;
use std::io::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};

// ---------------------------------------------------------------------------
// 分配计数器
// ---------------------------------------------------------------------------

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);

struct Counting;

// SAFETY: 仅统计后原样转发系统分配器，不改变任何分配语义
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
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

fn counters() -> (usize, usize) {
    (
        ALLOCS.load(Ordering::Relaxed),
        ALLOC_BYTES.load(Ordering::Relaxed),
    )
}

// ---------------------------------------------------------------------------
// 衰减策略：直接调用生产原语 `papokin_util::capacity`（基准包已依赖
// papokin-util），测的即是线上同一份实现
// ---------------------------------------------------------------------------

use papokin_util::capacity::{decay_clear_map, decay_clear_vec};

// ---------------------------------------------------------------------------
// 负载与结构模型
// ---------------------------------------------------------------------------

const TICKS: usize = 600;
const BURST_TICKS: usize = 10;
const PLAYERS: usize = 50;

/// 尖峰/稳态两时代的每 tick 负载
struct Load {
    changes: usize,
    per_section: usize,
    events: usize,
    batch: usize,
}

const fn load_at(tick: usize) -> Load {
    if tick < BURST_TICKS {
        Load {
            changes: 10_000,
            per_section: 50,
            events: 400,
            batch: 500,
        }
    } else {
        Load {
            changes: 400,
            per_section: 7,
            events: 80,
            batch: 24,
        }
    }
}

/// 冲刷暂存模型：对应 `BlockUpdateFlushScratch` + 事件缓冲对
#[derive(Default)]
struct FlushModel {
    producer_changes: HashMap<u64, u16>,
    changes: HashMap<u64, u16>,
    sections: HashMap<u64, Vec<(u64, u16)>>,
    producer_events: Vec<[u64; 2]>,
    events: Vec<[u64; 2]>,
}

impl FlushModel {
    fn retained_bytes(&self) -> usize {
        let map_pair =
            (self.producer_changes.capacity() + self.changes.capacity()) * size_of::<(u64, u16)>();
        let sections_buckets = self.sections.capacity() * size_of::<(u64, Vec<(u64, u16)>)>();
        let sections_inner: usize = self
            .sections
            .values()
            .map(|v| v.capacity() * size_of::<(u64, u16)>())
            .sum();
        let events_pair =
            (self.producer_events.capacity() + self.events.capacity()) * size_of::<[u64; 2]>();
        map_pair + sections_buckets + sections_inner + events_pair
    }
}

/// 区块批次暂存模型：对应玩家侧 `ChunkBatchScratch` 四 `Vec`
#[derive(Default)]
struct BatchModel {
    prepared: Vec<[u64; 2]>,
    encoded_results: Vec<Option<([u64; 11], bool)>>,
    encoded: Vec<[u64; 11]>,
    dispatched: Vec<u64>,
}

impl BatchModel {
    const fn retained_bytes(&self) -> usize {
        self.prepared.capacity() * size_of::<[u64; 2]>()
            + self.encoded_results.capacity() * size_of::<Option<([u64; 11], bool)>>()
            + self.encoded.capacity() * size_of::<[u64; 11]>()
            + self.dispatched.capacity() * size_of::<u64>()
    }
}

// ---------------------------------------------------------------------------
// 指纹（交换律混合：与迭代顺序无关，两臂处理语义必须全等）
// ---------------------------------------------------------------------------

const fn fp_mix(h: u64, v: u64) -> u64 {
    let mut x = h ^ v;
    x = x.wrapping_mul(0x0000_0100_0000_01B3);
    x.rotate_left(17)
}

// ---------------------------------------------------------------------------
// 单趟 tick 模拟
// ---------------------------------------------------------------------------

/// 冲刷路径：swap 进出 + 分节重填 + 投递折叠；`decay` 决定是否启用衰减清理
fn flush_tick(model: &mut FlushModel, tick: usize, load: &Load, decay: bool, fp: &mut u64) {
    // 生产侧本 tick 新增变更：位置取自固定宇宙（真实服务器同一批
    // 世界区域反复变更，节键跨 tick 复用），仅状态值随 tick 变化
    for i in 0..load.changes as u64 {
        let state = ((i as usize + tick) & 0xF) as u16;
        model.producer_changes.insert(i, state);
    }
    for i in 0..load.events as u64 {
        model.producer_events.push([tick as u64, i]);
    }

    // 清理 + swap（衰减臂在清理时回收超额容量）
    if decay {
        decay_clear_map(&mut model.changes);
        decay_clear_vec(&mut model.events);
    } else {
        model.changes.clear();
        model.events.clear();
    }
    std::mem::swap(&mut model.producer_changes, &mut model.changes);
    std::mem::swap(&mut model.producer_events, &mut model.events);

    if model.changes.is_empty() {
        return;
    }

    // 清值留桶后重填分节
    for updates in model.sections.values_mut() {
        if decay {
            decay_clear_vec(updates);
        } else {
            updates.clear();
        }
    }
    for (pos, state) in &model.changes {
        let section = pos / load.per_section as u64;
        model
            .sections
            .entry(section)
            .or_default()
            .push((*pos, *state));
    }

    // 投递（内外两层均为交换律折叠：与 map 迭代序、vec 元素序皆无关）
    for (section, updates) in &model.sections {
        let mut h = *section;
        for (pos, state) in updates {
            h ^= fp_mix(*pos, u64::from(*state)).rotate_left((*pos & 31) as u32);
        }
        *fp ^= h.rotate_left((section & 31) as u32);
    }
    for event in &model.events {
        *fp ^= fp_mix(event[0], event[1]).rotate_left((event[1] & 31) as u32);
    }

    // 分节数超限的压实护栏（对应 `FLUSH_SECTIONS_MAX`）
    if model.sections.len() > 4096 {
        model.sections.clear();
    }
}

/// 批次路径：四 `Vec` 清理重填一轮
fn batch_tick(model: &mut BatchModel, load: &Load, decay: bool, fp: &mut u64) {
    if decay {
        decay_clear_vec(&mut model.prepared);
        decay_clear_vec(&mut model.encoded_results);
        decay_clear_vec(&mut model.encoded);
        decay_clear_vec(&mut model.dispatched);
    } else {
        model.prepared.clear();
        model.encoded_results.clear();
        model.encoded.clear();
        model.dispatched.clear();
    }
    for i in 0..load.batch as u64 {
        model.prepared.push([i, i ^ 0x5DEE]);
        model.encoded_results.push(Some(([i; 11], true)));
        model.encoded.push([i.wrapping_mul(31); 11]);
        model.dispatched.push(i.rotate_left(7));
    }
    for d in &model.dispatched {
        *fp ^= fp_mix(*d, model.encoded[*d as usize % model.encoded.len()][0]);
    }
}

// ---------------------------------------------------------------------------
// 臂驱动
// ---------------------------------------------------------------------------

struct ArmReport {
    fp: u64,
    retained_after_burst: usize,
    retained_final: usize,
    allocs_total: usize,
    bytes_total: usize,
    steady_allocs: usize,
    late_allocs: usize,
}

fn run_arm(decay: bool) -> ArmReport {
    let mut flush_models: Vec<FlushModel> = (0..4).map(|_| FlushModel::default()).collect();
    let mut batch_models: Vec<BatchModel> = (0..PLAYERS).map(|_| BatchModel::default()).collect();
    let mut fp = 0u64;

    // 预热 3 tick 稳态负载（对齐既往轮次惯例）
    for _ in 0..3 {
        let load = load_at(BURST_TICKS);
        for m in &mut flush_models {
            flush_tick(m, 0, &load, decay, &mut fp);
        }
        for m in &mut batch_models {
            batch_tick(m, &load, decay, &mut fp);
        }
    }
    fp = 0;

    let (a0, b0) = counters();
    let mut retained_after_burst = 0usize;
    let mut steady_start = (0usize, 0usize);
    let mut late_start = (0usize, 0usize);

    for tick in 0..TICKS {
        let load = load_at(tick);
        // tick 内交换律折叠（与迭代序无关，两臂必等），tick 间顺序链式
        // 混合——杜绝全 XOR 汇流在构造性负载下整体抵消退化为零
        let mut tick_fp = 0u64;
        for m in &mut flush_models {
            flush_tick(m, tick, &load, decay, &mut tick_fp);
        }
        for m in &mut batch_models {
            batch_tick(m, &load, decay, &mut tick_fp);
        }
        fp = fp_mix(fp, tick_fp ^ tick as u64);
        if tick == BURST_TICKS - 1 {
            retained_after_burst = flush_models
                .iter()
                .map(FlushModel::retained_bytes)
                .sum::<usize>()
                + batch_models
                    .iter()
                    .map(BatchModel::retained_bytes)
                    .sum::<usize>();
        }
        if tick == BURST_TICKS {
            steady_start = counters();
        }
        // 收缩波早已结束的晚段稳态观测点（一次性收缩波限于前几十 tick）
        if tick == 100 {
            late_start = counters();
        }
    }

    let retained_final = flush_models
        .iter()
        .map(FlushModel::retained_bytes)
        .sum::<usize>()
        + batch_models
            .iter()
            .map(BatchModel::retained_bytes)
            .sum::<usize>();
    let (a1, b1) = counters();

    ArmReport {
        fp,
        retained_after_burst,
        retained_final,
        allocs_total: a1 - a0,
        bytes_total: b1 - b0,
        steady_allocs: a1 - steady_start.0,
        late_allocs: a1 - late_start.0,
    }
}

// ---------------------------------------------------------------------------
// 报告与闸门
// ---------------------------------------------------------------------------

fn evaluate_gates(no_decay: &ArmReport, decayed: &ArmReport) -> Vec<(&'static str, bool, String)> {
    let mut gates: Vec<(&str, bool, String)> = Vec::new();

    // 闸门 1：处理语义全等（衰减不改变任何投递内容）
    gates.push((
        "指纹全等",
        no_decay.fp == decayed.fp,
        format!("无衰减 {:#018x} vs 衰减 {:#018x}", no_decay.fp, decayed.fp),
    ));

    // 闸门 2：尖峰末保留容量两臂一致（衰减永不跌破在途负载需求）
    gates.push((
        "尖峰末保留一致",
        no_decay.retained_after_burst == decayed.retained_after_burst,
        format!(
            "无衰减 {} B vs 衰减 {} B",
            no_decay.retained_after_burst, decayed.retained_after_burst
        ),
    ));

    // 闸门 3：收尾保留容量衰减臂至少降至 1/4
    gates.push((
        "收尾保留容量 ≥4× 回收",
        decayed.retained_final * 4 <= no_decay.retained_final,
        format!(
            "无衰减 {} B vs 衰减 {} B（{:.1}×）",
            no_decay.retained_final,
            decayed.retained_final,
            no_decay.retained_final as f64 / decayed.retained_final.max(1) as f64
        ),
    ));

    // 闸门 4：收缩波有界——每次收缩 = 1 次分配，每驻留缓冲至多收缩一次，
    // 上界 = 4 冲刷模型×3（changes/事件/分节图）+ 50 玩家×4 批次 Vec
    let steady_extra = decayed.steady_allocs as isize - no_decay.steady_allocs as isize;
    gates.push((
        "一次性收缩波 ≤ 232",
        steady_extra <= 232,
        format!(
            "稳态时代分配：无衰减 {} vs 衰减 {}（差值 {steady_extra}）",
            no_decay.steady_allocs, decayed.steady_allocs
        ),
    ));

    // 闸门 5：收缩波后的晚段稳态两臂分配完全一致（零抖动）
    let late_diff = decayed.late_allocs as isize - no_decay.late_allocs as isize;
    gates.push((
        "晚段稳态零抖动",
        late_diff == 0,
        format!(
            "第 100 tick 后分配：无衰减 {} vs 衰减 {}（差值 {late_diff}）",
            no_decay.late_allocs, decayed.late_allocs
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
        "round": 20,
        "theme": "驻留缓冲容量衰减治理（负载尖峰后的保留容量回收）",
        "workload": {
            "ticks": TICKS,
            "burst_ticks": BURST_TICKS,
            "players": PLAYERS,
            "burst": "10000变更/200节×50 + 400事件 + 批500×50玩家",
            "steady": "400变更/60节×7 + 80事件 + 批24×50玩家",
        },
        "no_decay": {
            "retained_after_burst_bytes": no_decay.retained_after_burst,
            "retained_final_bytes": no_decay.retained_final,
            "allocs_total": no_decay.allocs_total,
            "bytes_total": no_decay.bytes_total,
            "steady_allocs": no_decay.steady_allocs,
            "late_allocs": no_decay.late_allocs,
            "fingerprint": format!("{:#018x}", no_decay.fp),
        },
        "decayed": {
            "retained_after_burst_bytes": decayed.retained_after_burst,
            "retained_final_bytes": decayed.retained_final,
            "allocs_total": decayed.allocs_total,
            "bytes_total": decayed.bytes_total,
            "steady_allocs": decayed.steady_allocs,
            "late_allocs": decayed.late_allocs,
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

    let path = "note/report/perf/round20-resident-capacity-decay.json";
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

    println!("=== 轮次 20：驻留容量衰减治理 ===");
    println!(
        "保留容量：尖峰末 {} B（两臂一致）→ 收尾 无衰减 {} B / 衰减 {} B",
        no_decay.retained_after_burst, no_decay.retained_final, decayed.retained_final
    );
    for (name, ok, detail) in &gates {
        println!("[{}] {name}: {detail}", if *ok { "PASS" } else { "FAIL" });
    }
    if !pass {
        std::process::exit(2);
    }
}
