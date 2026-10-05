//! 轮次 21 基准：区块编码缓存逐出后的桶容量回收。
//!
//! 对照形态：
//! - 无收缩（现状）：`prune_if_over_budget` 只 `remove` 条目，桶数组
//!   永久保留历史峰值（探索尖峰后驻留）；
//! - 收缩（收编）：逐出结束、CAS 释放前，若
//!   `capacity > len × 4 + 64` 则 `shrink_to_fit`（4× 滞回防振荡）。
//!
//! 负载模型（对齐真实「探索 → 停探卸载 → 再探索」节奏）：
//! 1. 探索期 8 周期 × 2000 条新编码条目（1.6 万条，桶数组冲至峰值）；
//! 2. 停探卸载：90% 条目标记弱引用死亡，触发预算逐出（死条目清扫），
//!    存活仅约 1.6 千条——桶容量远超滞回带，收缩臂在此回收；
//! 3. 再探索期 4 周期 × 2000 条（检验回涨摊开销）。
//!
//! 观测：卸载后与收尾的桶容量驻留（字节）、全程分配次数（收缩 +
//! 回涨的额外开销上界）、存活条目指纹全等（逐出语义不变）。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::{HashMap, HashSet};
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
// 缓存模型：键 = （协议版本， 区块坐标），值 = 88B 编码条目
// （元素尺寸对齐生产 `EncodedChunk`；负载字节用权重模拟，只用于预算记账）
// ---------------------------------------------------------------------------

const EXPLORE_CYCLES: usize = 8;
const REEXPLORE_CYCLES: usize = 4;
const INSERT_PER_CYCLE: usize = 2000;
/// 每条模拟负载字节（10-40KB 真实区块包以权重代表，只用于预算记账）
const PAYLOAD_BYTES: usize = 24 * 1024;
/// 预算：大于探索期总量（探索期不逐出），卸载后 stale 记账触发逐出
const BUDGET_BYTES: usize = EXPLORE_CYCLES * INSERT_PER_CYCLE * PAYLOAD_BYTES * 3 / 2;

struct CacheModel {
    map: HashMap<(u8, u64), [u64; 11]>,
    bytes: usize,
}

impl CacheModel {
    fn insert(&mut self, key: (u8, u64), entry: [u64; 11]) {
        self.map.insert(key, entry);
        self.bytes += PAYLOAD_BYTES;
    }

    /// 预算逐出：先清扫 `dead` 中的死键（弱引用失效模拟），仍超标再按
    /// 坐标模 97 降序距离逐出至 80%；返回逐出条数（对齐生产两阶段）。
    /// 注：生产侧清扫只在超预算 prune 时顺带发生（卸载后 stale 记账撑住
    /// 账面，下次超预算即触发）；本模型以 dead 非空直接触发与之等效
    fn prune_if_over_budget(&mut self, dead: &HashSet<u64>) -> usize {
        if self.bytes <= BUDGET_BYTES && dead.is_empty() {
            return 0;
        }
        let target = BUDGET_BYTES / 5 * 4;
        let mut evicted = 0usize;

        // 阶段一：死条目清扫
        for key in dead {
            if self.map.remove(&(0, *key)).is_some() {
                self.bytes -= PAYLOAD_BYTES;
                evicted += 1;
            }
        }

        // 阶段二：距离逐出（确定性替代「距注视中心最远」；
        // 同距按坐标升序决胜，驱逐选择与 map 迭代序无关）
        if self.bytes > target {
            let mut farthest: Vec<(u64, u64, (u8, u64))> = self
                .map
                .keys()
                .map(|key| (key.1 % 97, key.1, *key))
                .collect();
            farthest.sort_unstable_by_key(|(dist, ord, _)| (std::cmp::Reverse(*dist), *ord));
            for (_, _, key) in farthest {
                if self.bytes <= target {
                    break;
                }
                if self.map.remove(&key).is_some() {
                    self.bytes -= PAYLOAD_BYTES;
                    evicted += 1;
                }
            }
        }
        evicted
    }

    fn retained_bytes(&self) -> usize {
        self.map.capacity() * size_of::<((u8, u64), [u64; 11])>()
    }
}

/// 生产侧 shrink 判定（逐字同构 `chunk_sender.rs` 的轮次 21 插入点）：
/// 有逐出且容量超 `len × 4 + 64` 才收缩
fn maybe_shrink(model: &mut CacheModel, evicted: usize, shrink: bool) {
    if shrink && evicted > 0 && model.map.capacity() > model.map.len() * 4 + 64 {
        model.map.shrink_to_fit();
    }
}

// ---------------------------------------------------------------------------
// 指纹（交换律折叠：存活条目集合与迭代顺序无关）
// ---------------------------------------------------------------------------

const fn fp_mix(h: u64, v: u64) -> u64 {
    let mut x = h ^ v;
    x = x.wrapping_mul(0x0000_0100_0000_01B3);
    x.rotate_left(17)
}

fn survivors_fp(model: &CacheModel) -> u64 {
    // 注意：entry 载荷镜像键值，直接 fp_mix(key, entry) 会异或自消
    // 退化为 0x0；先以固定偏移基底吸收键再折叠载荷，保证灵敏度
    const BASIS: u64 = 0xCBF2_9CE4_8422_2325;
    let mut fp = 0u64;
    for (key, entry) in &model.map {
        let term = fp_mix(fp_mix(BASIS, key.1), entry[0]);
        fp ^= term.rotate_left((key.1 & 31) as u32);
    }
    fp
}

// ---------------------------------------------------------------------------
// 臂驱动
// ---------------------------------------------------------------------------

struct ArmReport {
    fp_unload: u64,
    fp_final: u64,
    retained_after_unload: usize,
    retained_final: usize,
    allocs_total: usize,
    shrink_events: usize,
}

fn run_arm(shrink: bool) -> ArmReport {
    let mut model = CacheModel {
        map: HashMap::new(),
        bytes: 0,
    };
    let mut shrink_events = 0usize;

    // 预热（对齐既往轮次惯例）：小负载插入后清空
    for i in 0..64u64 {
        model.insert((0, i), [i; 11]);
    }
    model.map.clear();
    model.bytes = 0;

    let (a0, _) = counters();

    // 阶段 1：探索期——持续插入全新键，无逐出压力
    for cycle in 0..EXPLORE_CYCLES {
        let base = (cycle as u64 + 1) << 32;
        for i in 0..INSERT_PER_CYCLE as u64 {
            let key = base | i;
            model.insert((0, key), [key; 11]);
        }
    }

    // 阶段 2：停探卸载——90% 键的「区块」卸载（弱引用死亡），
    // stale 记账超预算触发逐出
    let mut dead = HashSet::new();
    for cycle in 0..EXPLORE_CYCLES {
        let base = (cycle as u64 + 1) << 32;
        for i in 0..INSERT_PER_CYCLE as u64 {
            if i % 10 != 0 {
                dead.insert(base | i);
            }
        }
    }
    let evicted = model.prune_if_over_budget(&dead);
    let cap_before = model.map.capacity();
    maybe_shrink(&mut model, evicted, shrink);
    shrink_events += usize::from(shrink && model.map.capacity() < cap_before);
    let retained_after_unload = model.retained_bytes();
    let fp_unload = survivors_fp(&model);

    // 阶段 3：再探索期——检验桶回涨的摊开销
    for cycle in 0..REEXPLORE_CYCLES {
        let base = (cycle as u64 + 100) << 32;
        for i in 0..INSERT_PER_CYCLE as u64 {
            let key = base | i;
            model.insert((0, key), [key; 11]);
        }
        let evicted = model.prune_if_over_budget(&HashSet::new());
        let cap_before = model.map.capacity();
        maybe_shrink(&mut model, evicted, shrink);
        shrink_events += usize::from(shrink && model.map.capacity() < cap_before);
    }

    let retained_final = model.retained_bytes();
    let fp_final = survivors_fp(&model);
    let (a1, _) = counters();

    ArmReport {
        fp_unload,
        fp_final,
        retained_after_unload,
        retained_final,
        allocs_total: a1 - a0,
        shrink_events,
    }
}

// ---------------------------------------------------------------------------
// 闸门与报告
// ---------------------------------------------------------------------------

fn evaluate_gates(no_shrink: &ArmReport, shrunk: &ArmReport) -> Vec<(&'static str, bool, String)> {
    let mut gates: Vec<(&str, bool, String)> = Vec::new();

    // 闸门 1：存活条目集合全等（逐出语义不变，卸载后与收尾双采样）
    gates.push((
        "指纹全等",
        no_shrink.fp_unload == shrunk.fp_unload && no_shrink.fp_final == shrunk.fp_final,
        format!(
            "卸载后 {:#018x}/{:#018x}，收尾 {:#018x}/{:#018x}",
            no_shrink.fp_unload, shrunk.fp_unload, no_shrink.fp_final, shrunk.fp_final
        ),
    ));

    // 闸门 2：卸载后桶容量驻留至少降至 1/4
    gates.push((
        "卸载后桶驻留 ≥4× 回收",
        shrunk.retained_after_unload * 4 <= no_shrink.retained_after_unload,
        format!(
            "无收缩 {} B vs 收缩 {} B（{:.1}×）",
            no_shrink.retained_after_unload,
            shrunk.retained_after_unload,
            no_shrink.retained_after_unload as f64 / shrunk.retained_after_unload.max(1) as f64
        ),
    ));

    // 闸门 3：收缩事件有界（卸载期一次为预期，再探索期不得振荡）
    gates.push((
        "收缩事件 ≤ 2",
        shrunk.shrink_events <= 2,
        format!("{} 次", shrunk.shrink_events),
    ));

    // 闸门 4：收缩臂额外分配有界（收缩 + 桶回涨的摊开销）
    let extra = shrunk.allocs_total as isize - no_shrink.allocs_total as isize;
    gates.push((
        "额外分配 ≤ 16",
        extra <= 16,
        format!(
            "全程分配：无收缩 {} vs 收缩 {}（差值 {extra}）",
            no_shrink.allocs_total, shrunk.allocs_total
        ),
    ));

    gates
}

fn main() {
    let no_shrink = run_arm(false);
    let shrunk = run_arm(true);
    let gates = evaluate_gates(&no_shrink, &shrunk);

    let pass = gates.iter().all(|(_, ok, _)| *ok);

    let report = serde_json::json!({
        "round": 21,
        "theme": "区块编码缓存逐出后的桶容量回收",
        "workload": {
            "explore_cycles": EXPLORE_CYCLES,
            "reexplore_cycles": REEXPLORE_CYCLES,
            "insert_per_cycle": INSERT_PER_CYCLE,
            "payload_bytes_per_entry": PAYLOAD_BYTES,
            "budget_bytes": BUDGET_BYTES,
            "unload_ratio": 0.9,
        },
        "no_shrink": {
            "retained_after_unload_bytes": no_shrink.retained_after_unload,
            "retained_final_bytes": no_shrink.retained_final,
            "allocs_total": no_shrink.allocs_total,
            "fingerprint_unload": format!("{:#018x}", no_shrink.fp_unload),
            "fingerprint_final": format!("{:#018x}", no_shrink.fp_final),
        },
        "shrunk": {
            "retained_after_unload_bytes": shrunk.retained_after_unload,
            "retained_final_bytes": shrunk.retained_final,
            "allocs_total": shrunk.allocs_total,
            "shrink_events": shrunk.shrink_events,
            "fingerprint_unload": format!("{:#018x}", shrunk.fp_unload),
            "fingerprint_final": format!("{:#018x}", shrunk.fp_final),
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

    let path = "note/report/perf/round21-encode-cache-bucket-reclaim.json";
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

    println!("=== 轮次 21：编码缓存桶容量回收 ===");
    println!(
        "桶驻留：卸载后 无收缩 {} B / 收缩 {} B；收尾 {} B / {} B",
        no_shrink.retained_after_unload,
        shrunk.retained_after_unload,
        no_shrink.retained_final,
        shrunk.retained_final
    );
    for (name, ok, detail) in &gates {
        println!("[{}] {name}: {detail}", if *ok { "PASS" } else { "FAIL" });
    }
    if !pass {
        std::process::exit(2);
    }
}
