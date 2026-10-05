//! 轮次 22 基准：实体/计划刻/方块实体三张世界表峰值桶驻留回收。
//!
//! 对照形态：
//! - 无收缩（现状）：移除路径只删条目，`DashMap`/`HashMap` 桶数组
//!   永久保留历史峰值；
//! - 收缩（收编）：`entity_map` 在 `update_all` 尾按 4× 滞回收缩
//!   （同轮次 21 参数），`chunks_with_scheduled_ticks` 与
//!   `block_entities` 在 100 tick 周期的 `clean_memory` 路径按
//!   既有 4096 槽位余量规则收缩。
//!
//! 负载模型（对齐「事件高峰 → 回落 → 小幅再波动」节奏）：
//! 1. 高峰填充：实体 5 万（刷怪塔/袭击事件）、计划刻 2 万区块
//!    （流体扩散）、方块实体 3 万区块（大型机器区加载）；
//! 2. 高峰回落：实体消失 90%、计划刻消化 95%（留长尾刻）、
//!    方块实体随区块卸载 90%——桶容量远超滞回带，收缩臂在此回收；
//! 3. 小幅再波动：三张表各回涨约 10%（检验滞回/余量防振荡）。
//!
//! 观测：回落后与收尾的桶驻留（字节）、全程分配次数、收缩事件
//! 计数（再波动阶段必须为 0）、存活条目指纹全等（逐出语义不变）。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::{HashMap, HashSet};
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
// 三表模型：尺寸对齐生产条目（entity_map 值 Arc 8B、计划刻零尺寸值、
// block_entities 内联 FxHashMap 24B 起步）
// ---------------------------------------------------------------------------

/// 实体事件高峰（刷怪塔/袭击量级）
const ENTITY_PEAK: i32 = 50_000;
/// 计划刻高峰区块数（流体/岩浆扩散量级）
const SCHED_PEAK: u64 = 20_000;
/// 方块实体高峰区块数（大型机器区）
const BE_PEAK: u64 = 30_000;
/// `clean_memory` 既有槽位余量规则（level.rs 三张姊妹表同阈）
const CLEAN_SLACK: usize = 4096;

/// 生产侧 `entity_map` 滞回收缩判定（逐字同构轮次 22 插入点）：
/// 容量超 `len × 4 + 64` 才收缩
fn hysteresis_shrink<K: Eq + std::hash::Hash, V>(
    map: &mut HashMap<K, V>,
    enabled: bool,
    events: &mut usize,
) {
    if enabled && map.capacity() > map.len() * 4 + 64 {
        map.shrink_to_fit();
        *events += 1;
    }
}

/// 生产侧 `clean_memory` 槽位余量收缩判定（逐字同构既有三表规则）：
/// 空余槽位 ≥ 4096 才收缩
fn slack_shrink<K: Eq + std::hash::Hash, V>(
    map: &mut HashMap<K, V>,
    enabled: bool,
    events: &mut usize,
) {
    if enabled && map.capacity() - map.len() >= CLEAN_SLACK {
        map.shrink_to_fit();
        *events += 1;
    }
}

/// `slack_shrink` 的集合变体（计划刻区块集）
fn slack_shrink_set<T: Eq + std::hash::Hash>(
    set: &mut HashSet<T>,
    enabled: bool,
    events: &mut usize,
) {
    if enabled && set.capacity() - set.len() >= CLEAN_SLACK {
        set.shrink_to_fit();
        *events += 1;
    }
}

struct ChurnModel {
    /// 实体追踪表：键 `entity_id`（4B），值占位 24B（Arc + 控制）
    entity_map: HashMap<i32, [u64; 3]>,
    /// 计划刻区块集：键区块坐标（8B）
    scheduled_ticks: HashSet<u64>,
    /// 方块实体分块表：键区块坐标（8B），值占位 32B（内联 `FxHashMap`）
    block_entities: HashMap<u64, [u64; 4]>,
    shrink_events: usize,
    /// 再波动阶段的收缩计数（闸门：必须为 0）
    shrink_after_rechurn: usize,
}

impl ChurnModel {
    fn new() -> Self {
        Self {
            entity_map: HashMap::new(),
            scheduled_ticks: HashSet::new(),
            block_entities: HashMap::new(),
            shrink_events: 0,
            shrink_after_rechurn: 0,
        }
    }

    fn retained_bytes(&self) -> (usize, usize, usize) {
        (
            self.entity_map.capacity() * size_of::<(i32, [u64; 3])>(),
            self.scheduled_ticks.capacity() * size_of::<u64>(),
            self.block_entities.capacity() * size_of::<(u64, [u64; 4])>(),
        )
    }
}

// ---------------------------------------------------------------------------
// 指纹（交换律折叠：先以固定偏移基底吸收键再折叠，避免自消退化）
// ---------------------------------------------------------------------------

const fn fp_mix(h: u64, v: u64) -> u64 {
    let mut x = h ^ v;
    x = x.wrapping_mul(0x0000_0100_0000_01B3);
    x.rotate_left(17)
}

const BASIS: u64 = 0xCBF2_9CE4_8422_2325;

fn keys_fp<K, V, F: Fn(&K) -> u64>(map: &HashMap<K, V>, key_bits: F) -> u64 {
    let mut fp = 0u64;
    for key in map.keys() {
        let bits = key_bits(key);
        fp ^= fp_mix(BASIS, bits).rotate_left((bits & 31) as u32);
    }
    fp
}

fn set_fp<T, F: Fn(&T) -> u64>(set: &HashSet<T>, key_bits: F) -> u64 {
    let mut fp = 0u64;
    for key in set {
        let bits = key_bits(key);
        fp ^= fp_mix(BASIS, bits).rotate_left((bits & 31) as u32);
    }
    fp
}

fn model_fps(model: &ChurnModel) -> (u64, u64, u64) {
    (
        keys_fp(&model.entity_map, |k| *k as u64),
        set_fp(&model.scheduled_ticks, |k| *k),
        keys_fp(&model.block_entities, |k| *k),
    )
}

// ---------------------------------------------------------------------------
// 臂驱动
// ---------------------------------------------------------------------------

struct ArmReport {
    fp_mid: (u64, u64, u64),
    fp_final: (u64, u64, u64),
    retained_mid: (usize, usize, usize),
    retained_final: (usize, usize, usize),
    allocs_total: usize,
    shrink_events: usize,
    shrink_after_rechurn: usize,
}

fn run_arm(shrink: bool) -> ArmReport {
    let mut model = ChurnModel::new();

    // 预热（对齐既往轮次惯例）：小负载插入后清空
    for i in 0..64 {
        model.entity_map.insert(i, [0; 3]);
        model.scheduled_ticks.insert(i as u64);
        model.block_entities.insert(i as u64, [0; 4]);
    }
    model.entity_map.clear();
    model.scheduled_ticks.clear();
    model.block_entities.clear();

    let a0 = ALLOCS.load(Ordering::Relaxed);

    // 阶段 1：高峰填充
    for id in 0..ENTITY_PEAK {
        model.entity_map.insert(id, [id as u64; 3]);
    }
    for pos in 0..SCHED_PEAK {
        model.scheduled_ticks.insert(pos);
    }
    for pos in 0..BE_PEAK {
        model.block_entities.insert(pos, [pos; 4]);
    }

    // 阶段 2：高峰回落——实体消失 90%、计划刻消化 95%（留 5%
    // 长尾刻，贴近生产且避免空集指纹退化为 0x0 的空采样）、
    // 方块实体随区块卸载 90%（remove 语义逐字同构生产移除路径）
    model.entity_map.retain(|id, _| id % 10 == 0);
    model.scheduled_ticks.retain(|pos| pos % 20 == 0);
    model.block_entities.retain(|pos, _| pos % 10 == 0);

    // 生产时序：实体表在 update_all 尾逐 tick 检查；
    // 另两表在 100 tick 周期的 clean_memory 检查
    hysteresis_shrink(&mut model.entity_map, shrink, &mut model.shrink_events);
    slack_shrink_set(&mut model.scheduled_ticks, shrink, &mut model.shrink_events);
    slack_shrink(&mut model.block_entities, shrink, &mut model.shrink_events);

    let retained_mid = model.retained_bytes();
    let fp_mid = model_fps(&model);

    // 阶段 3：小幅再波动（各回涨约 10%）——滞回/余量必须压住振荡
    let events_before = model.shrink_events;
    for id in ENTITY_PEAK..ENTITY_PEAK + 5_000 {
        model.entity_map.insert(id, [id as u64; 3]);
    }
    for pos in SCHED_PEAK..SCHED_PEAK + 2_000 {
        model.scheduled_ticks.insert(pos);
    }
    for pos in BE_PEAK..BE_PEAK + 3_000 {
        model.block_entities.insert(pos, [pos; 4]);
    }
    hysteresis_shrink(&mut model.entity_map, shrink, &mut model.shrink_events);
    slack_shrink_set(&mut model.scheduled_ticks, shrink, &mut model.shrink_events);
    slack_shrink(&mut model.block_entities, shrink, &mut model.shrink_events);
    model.shrink_after_rechurn = model.shrink_events - events_before;

    let retained_final = model.retained_bytes();
    let fp_final = model_fps(&model);
    let a1 = ALLOCS.load(Ordering::Relaxed);

    ArmReport {
        fp_mid,
        fp_final,
        retained_mid,
        retained_final,
        allocs_total: a1 - a0,
        shrink_events: model.shrink_events,
        shrink_after_rechurn: model.shrink_after_rechurn,
    }
}

// ---------------------------------------------------------------------------
// 闸门与报告
// ---------------------------------------------------------------------------

fn evaluate_gates(no_shrink: &ArmReport, shrunk: &ArmReport) -> Vec<(&'static str, bool, String)> {
    let mut gates: Vec<(&str, bool, String)> = Vec::new();

    // 闸门 1：三表存活指纹全等（回落后与收尾双采样）
    gates.push((
        "指纹全等",
        no_shrink.fp_mid == shrunk.fp_mid && no_shrink.fp_final == shrunk.fp_final,
        format!(
            "实体 {:#018x}、计划刻 {:#018x}、方块实体 {:#018x}（收尾一致：{}）",
            shrunk.fp_mid.0,
            shrunk.fp_mid.1,
            shrunk.fp_mid.2,
            no_shrink.fp_final == shrunk.fp_final
        ),
    ));

    // 闸门 2：实体表回落后桶驻留 ≥4× 回收
    gates.push((
        "实体表桶驻留 ≥4× 回收",
        shrunk.retained_mid.0 * 4 <= no_shrink.retained_mid.0,
        format!(
            "无收缩 {} B vs 收缩 {} B（{:.1}×）",
            no_shrink.retained_mid.0,
            shrunk.retained_mid.0,
            no_shrink.retained_mid.0 as f64 / shrunk.retained_mid.0.max(1) as f64
        ),
    ));

    // 闸门 3：计划刻 + 方块实体两表回落后桶驻留 ≥4× 回收
    let other_no = no_shrink.retained_mid.1 + no_shrink.retained_mid.2;
    let other_shrunk = shrunk.retained_mid.1 + shrunk.retained_mid.2;
    gates.push((
        "计划刻+方块实体桶驻留 ≥4× 回收",
        other_shrunk * 4 <= other_no,
        format!(
            "无收缩 {} B vs 收缩 {} B（{:.1}×）",
            other_no,
            other_shrunk,
            other_no as f64 / other_shrunk.max(1) as f64
        ),
    ));

    // 闸门 4：收缩事件有界（每表至多一次）且再波动阶段零振荡
    gates.push((
        "收缩事件 ≤ 3 且再波动零收缩",
        shrunk.shrink_events <= 3 && shrunk.shrink_after_rechurn == 0,
        format!(
            "总 {} 次，再波动 {} 次",
            shrunk.shrink_events, shrunk.shrink_after_rechurn
        ),
    ));

    // 闸门 5：收缩臂额外分配有界（上界 = 3 次收缩重哈希 + 排空表
    // 自零回涨的 log2 步进 ≈ 15；路径性反复扩缩会在闸门 4 先爆）
    let extra = shrunk.allocs_total as isize - no_shrink.allocs_total as isize;
    gates.push((
        "额外分配 ≤ 32",
        extra <= 32,
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
        "round": 22,
        "theme": "实体/计划刻/方块实体三表峰值桶驻留回收",
        "workload": {
            "entity_peak": ENTITY_PEAK,
            "sched_peak": SCHED_PEAK,
            "block_entities_peak": BE_PEAK,
            "clean_slack": CLEAN_SLACK,
            "entity_survivor_ratio": 0.1,
            "be_survivor_ratio": 0.1,
        },
        "no_shrink": {
            "retained_mid_bytes": no_shrink.retained_mid,
            "retained_final_bytes": no_shrink.retained_final,
            "allocs_total": no_shrink.allocs_total,
            "fingerprint_mid": format!("{:?}", no_shrink.fp_mid),
            "fingerprint_final": format!("{:?}", no_shrink.fp_final),
        },
        "shrunk": {
            "retained_mid_bytes": shrunk.retained_mid,
            "retained_final_bytes": shrunk.retained_final,
            "allocs_total": shrunk.allocs_total,
            "shrink_events": shrunk.shrink_events,
            "shrink_after_rechurn": shrunk.shrink_after_rechurn,
            "fingerprint_mid": format!("{:?}", shrunk.fp_mid),
            "fingerprint_final": format!("{:?}", shrunk.fp_final),
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

    let path = "note/report/perf/round22-world-tables-bucket-reclaim.json";
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

    println!("=== 轮次 22：世界三表峰值桶驻留回收 ===");
    println!(
        "回落后驻留：实体 {}→{} B、计划刻 {}→{} B、方块实体 {}→{} B",
        no_shrink.retained_mid.0,
        shrunk.retained_mid.0,
        no_shrink.retained_mid.1,
        shrunk.retained_mid.1,
        no_shrink.retained_mid.2,
        shrunk.retained_mid.2
    );
    for (name, ok, detail) in &gates {
        println!("[{}] {name}: {detail}", if *ok { "PASS" } else { "FAIL" });
    }
    if !pass {
        std::process::exit(2);
    }
}
