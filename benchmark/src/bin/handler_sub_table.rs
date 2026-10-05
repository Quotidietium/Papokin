//! 轮次 25 基准：事件订阅表形态（整表 COW → 单事件 `ArcSwap` + 注册时排序）。
//!
//! 对照形态（`PluginManager::handlers` 的驻留表示与注册/派发路径）：
//! - 整表 COW（现状）：`ArcSwap<HashMap<&'static str, Vec<Arc<H>>>>`，
//!   每次订阅/退订 rcu 克隆全表（367+ 事件键 × 各键向量），派发时
//!   持整表代际跨 await，且每次 fire 现排 `order_handlers`；
//! - 单事件 ArcSwap（收编）：外层索引只在事件键首次出现时写，
//!   订阅只重写该事件向量并在注册时排序，派发只钉住单事件代际、
//!   零排序零分配。语义等价由「逐键派发序指纹全等」闸门证明。
//!
//! 负载模型（40 插件 × 25 订阅 = 1000 次注册，367 事件键）：
//! - 注册阶段：统计两臂分配次数/字节（旧臂每次注册克隆全表）；
//! - fire 钉住阶段：热键 fire 持代际跨 25 次并发注册，量净驻留
//!   增量（旧臂每次 rcu 钉住一整表代际，新臂只钉单向量）；
//! - 稳态 fire：热键 10k 次派发统计派发路径分配次数
//!   （旧臂每次现排 `order_handlers` 产一份排序 Vec）；
//! - 卸载阶段：卸载半数插件（按 source 退订）统计分配。
//!
//! 观测：分配计数器（次数+净驻留字节）、逐键派发序指纹全等。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::HashMap;
use std::io::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use arc_swap::ArcSwap;

// ---------------------------------------------------------------------------
// 分配计数器（次数 + 毛字节 + 释放字节；净驻留 = 毛 - 释放）
// ---------------------------------------------------------------------------

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);
static FREED_BYTES: AtomicUsize = AtomicUsize::new(0);

struct Counting;

// SAFETY: 仅转发 System 并计数，不改动布局与返回值语义。
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: 与文档约定一致，直接转发。
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        FREED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: 与文档约定一致，直接转发。
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn counters() -> (usize, usize, usize) {
    (
        ALLOCS.load(Ordering::Relaxed),
        ALLOC_BYTES.load(Ordering::Relaxed),
        FREED_BYTES.load(Ordering::Relaxed),
    )
}

/// 净驻留字节 = 累计分配 - 累计释放
fn retained() -> usize {
    let (_, alloc, freed) = counters();
    alloc.saturating_sub(freed)
}

// ---------------------------------------------------------------------------
// 负载模型
// ---------------------------------------------------------------------------

const EVENT_KEYS: usize = 367;
const PLUGINS: usize = 40;
const SUBS_PER_PLUGIN: usize = 25;
const FIRE_HOLD_EVERY: usize = 50;
const FIRE_HOLD_SPAN: usize = 25;
const STEADY_FIRES: usize = 10_000;

/// 模拟处理器：注册序号（定序决胜键）、优先级（0=Highest … 4=Lowest，
/// 与生产 derive 序一致）、来源插件。
struct ModelHandler {
    seq: u64,
    priority: u8,
    source: u16,
}

/// 确定性 Lcg（常数取自 Knuth MMIX）
struct Rng(u64);

impl Rng {
    const fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 11
    }
}

/// 事件键表：进程级泄漏的静态字符串（模型全程存活）
fn event_keys() -> Vec<&'static str> {
    (0..EVENT_KEYS)
        .map(|i| {
            let s: &'static str = Box::leak(format!("papokin:event_{i:03}").into_boxed_str());
            s
        })
        .collect()
}

/// 生成注册计划：(事件键, 处理器) 序列，确定性
fn registration_plan(keys: &[&'static str]) -> Vec<(&'static str, Arc<ModelHandler>)> {
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    let mut plan = Vec::with_capacity(PLUGINS * SUBS_PER_PLUGIN);
    let mut seq = 0u64;
    for plugin in 0..PLUGINS {
        for _ in 0..SUBS_PER_PLUGIN {
            let key = keys[(rng.next() as usize) % keys.len()];
            let handler = Arc::new(ModelHandler {
                seq,
                priority: (rng.next() % 5) as u8,
                source: plugin as u16,
            });
            seq += 1;
            plan.push((key, handler));
        }
    }
    plan
}

// ---------------------------------------------------------------------------
// 旧臂：整表 COW
// ---------------------------------------------------------------------------

type OldMap = HashMap<&'static str, Vec<Arc<ModelHandler>>>;

struct OldArm {
    map: ArcSwap<OldMap>,
}

impl OldArm {
    fn new() -> Self {
        Self {
            map: ArcSwap::from_pointee(HashMap::new()),
        }
    }

    fn register(&self, key: &'static str, handler: &Arc<ModelHandler>) {
        self.map.rcu(|old| {
            let mut new = (**old).clone();
            new.entry(key).or_default().push(Arc::clone(handler));
            Arc::new(new)
        });
    }

    /// 派发（生产 `fire` 同构）：持整表代际，`order_handlers` 现排
    /// 产一份排序 Vec，再折叠序号（模拟逐处理器调用）
    fn fire_fold(&self, key: &'static str) -> u64 {
        let guard = self.map.load();
        let mut ordered: Vec<&Arc<ModelHandler>> = guard
            .get(key)
            .map(|v| v.iter().collect())
            .unwrap_or_default();
        ordered.sort_by_key(|h| std::cmp::Reverse(h.priority));
        ordered.iter().fold(0u64, |acc, h| fp_mix(acc, h.seq))
    }

    /// fire 钉住：加载整表代际并保持 `span` 次注册（模拟跨 await）
    fn fire_hold_across(&self, key: &'static str, pending: &[(&'static str, Arc<ModelHandler>)]) {
        let guard = self.map.load();
        std::hint::black_box(guard.get(key));
        for (k, h) in pending {
            self.register(k, h);
        }
        // 代际在此处才释放（钉住期间的注册各产一整表新代际）
    }

    fn unregister_source(&self, source: u16) {
        self.map.rcu(|old| {
            let mut new = (**old).clone();
            new.retain(|_, v| {
                v.retain(|h| h.source != source);
                !v.is_empty()
            });
            Arc::new(new)
        });
    }

    /// 终态派发序（与生产 fire 同：现排）
    fn final_order(&self, key: &'static str) -> Vec<(u64, u8)> {
        let guard = self.map.load();
        let mut ordered: Vec<(u64, u8)> = guard
            .get(key)
            .map(|v| v.iter().map(|h| (h.seq, h.priority)).collect())
            .unwrap_or_default();
        ordered.sort_by_key(|h| std::cmp::Reverse(h.1));
        ordered
    }
}

// ---------------------------------------------------------------------------
// 新臂：单事件 ArcSwap + 注册时排序
// ---------------------------------------------------------------------------

type NewVec = ArcSwap<Vec<Arc<ModelHandler>>>;

struct NewArm {
    map: RwLock<HashMap<&'static str, Arc<NewVec>>>,
}

impl NewArm {
    fn new() -> Self {
        Self {
            map: RwLock::new(HashMap::new()),
        }
    }

    fn handlers_for(&self, key: &'static str) -> Option<Arc<NewVec>> {
        self.map.read().ok()?.get(key).cloned()
    }

    fn register(&self, key: &'static str, handler: &Arc<ModelHandler>) {
        let vec = self.handlers_for(key).unwrap_or_else(|| {
            let mut map = self
                .map
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            Arc::clone(
                map.entry(key)
                    .or_insert_with(|| Arc::new(ArcSwap::from_pointee(Vec::new()))),
            )
        });
        vec.rcu(|old| {
            let mut new = (**old).clone();
            new.push(Arc::clone(handler));
            new.sort_by_key(|h| std::cmp::Reverse(h.priority));
            Arc::new(new)
        });
    }

    /// 派发（收编同构）：只钉单事件代际，向量已预排，零分配
    fn fire_fold(&self, key: &'static str) -> u64 {
        let Some(vec) = self.handlers_for(key) else {
            return 0;
        };
        let guard = vec.load();
        guard.iter().fold(0u64, |acc, h| fp_mix(acc, h.seq))
    }

    fn fire_hold_across(&self, key: &'static str, pending: &[(&'static str, Arc<ModelHandler>)]) {
        let Some(vec) = self.handlers_for(key) else {
            return;
        };
        let guard = vec.load();
        std::hint::black_box(guard.is_empty());
        for (k, h) in pending {
            self.register(k, h);
        }
    }

    fn unregister_source(&self, source: u16) {
        let vecs: Vec<Arc<NewVec>> = self
            .map
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
            .cloned()
            .collect();
        for vec in vecs {
            if !vec.load().iter().any(|h| h.source == source) {
                continue;
            }
            vec.rcu(|old| {
                let mut new = (**old).clone();
                new.retain(|h| h.source != source);
                Arc::new(new)
            });
        }
    }

    fn final_order(&self, key: &'static str) -> Vec<(u64, u8)> {
        let Some(vec) = self.handlers_for(key) else {
            return Vec::new();
        };
        let guard = vec.load();
        guard.iter().map(|h| (h.seq, h.priority)).collect()
    }
}

// ---------------------------------------------------------------------------
// 指纹与驱动
// ---------------------------------------------------------------------------

const BASIS: u64 = 0xCBF2_9CE4_8422_2325;

const fn fp_mix(acc: u64, v: u64) -> u64 {
    let mut x = acc ^ v;
    x = x.wrapping_mul(0x0000_0100_0000_01B3);
    x.rotate_left(13)
}

/// 逐键派发序指纹：键序按事件表顺序，键内按派发序，
/// 决胜键 = 注册序号（插入序全等 ⟺ 派发序全等）
fn fingerprint_orders(
    keys: &[&'static str],
    arm_final: &dyn Fn(&'static str) -> Vec<(u64, u8)>,
) -> u64 {
    let mut fp = BASIS;
    for key in keys {
        for (seq, prio) in arm_final(key) {
            fp = fp_mix(fp, seq);
            fp = fp_mix(fp, u64::from(prio));
        }
        fp = fp_mix(fp, 0x9E37_79B9_7F4A_7C15);
    }
    fp
}

struct ArmReport {
    load_allocs: usize,
    load_alloc_bytes: usize,
    hold_retained_growth: usize,
    steady_fire_allocs: usize,
    unload_allocs: usize,
    fp: u64,
}

/// 通用驱动：注册阶段每 `FIRE_HOLD_EVERY` 次插一次钉住 fire，
/// 钉住跨 `FIRE_HOLD_SPAN` 次注册；收尾稳态 fire + 半数卸载。
#[allow(clippy::too_many_arguments)]
fn drive<A>(
    arm: &A,
    keys: &[&'static str],
    plan: &[(&'static str, Arc<ModelHandler>)],
    register: impl Fn(&A, &'static str, &Arc<ModelHandler>),
    fire_hold: impl Fn(&A, &'static str, &[(&'static str, Arc<ModelHandler>)]),
    fire_fold: impl Fn(&A, &'static str) -> u64,
    unregister: impl Fn(&A, u16),
    final_order: impl Fn(&'static str) -> Vec<(u64, u8)>,
) -> ArmReport {
    let hot_key = keys[0];
    let (a0, ab0, _) = counters();

    // 阶段 1：注册（夹钉住 fire）
    let mut hold_growth_max = 0usize;
    let mut i = 0usize;
    while i < plan.len() {
        let (k, h) = &plan[i];
        register(arm, k, h);
        i += 1;
        if i.is_multiple_of(FIRE_HOLD_EVERY) && i + FIRE_HOLD_SPAN <= plan.len() {
            let before = retained();
            fire_hold(arm, hot_key, &plan[i..i + FIRE_HOLD_SPAN]);
            let growth = retained().saturating_sub(before);
            if growth > hold_growth_max {
                hold_growth_max = growth;
            }
            i += FIRE_HOLD_SPAN;
        }
    }
    let (a1, ab1, _) = counters();
    let load_allocs = a1 - a0;
    let load_alloc_bytes = ab1 - ab0;

    // 阶段 2：稳态 fire（只数派发路径分配）
    let (a2, _, _) = counters();
    let mut sink = 0u64;
    for _ in 0..STEADY_FIRES {
        sink ^= fire_fold(arm, hot_key);
    }
    std::hint::black_box(sink);
    let (a3, _, _) = counters();
    let steady_fire_allocs = a3 - a2;

    // 阶段 3：卸载半数插件
    let (a4, _, _) = counters();
    for plugin in 0..PLUGINS / 2 {
        unregister(arm, plugin as u16);
    }
    let (a5, _, _) = counters();
    let unload_allocs = a5 - a4;

    // 阶段 4：终态指纹
    let fp = fingerprint_orders(keys, &final_order);

    ArmReport {
        load_allocs,
        load_alloc_bytes,
        hold_retained_growth: hold_growth_max,
        steady_fire_allocs,
        unload_allocs,
        fp,
    }
}

// ---------------------------------------------------------------------------
// 闸门与报告
// ---------------------------------------------------------------------------

fn evaluate_gates(old: &ArmReport, new: &ArmReport) -> Vec<(&'static str, bool, String)> {
    let mut gates: Vec<(&str, bool, String)> = Vec::new();

    gates.push((
        "G1 逐键派发序指纹全等",
        old.fp == new.fp && old.fp != 0,
        format!("old={:#018x} new={:#018x}", old.fp, new.fp),
    ));
    gates.push((
        "G2 加载分配字节 ≤ 旧臂 1/4",
        new.load_alloc_bytes * 4 <= old.load_alloc_bytes,
        format!("old={} new={}", old.load_alloc_bytes, new.load_alloc_bytes),
    ));
    gates.push((
        "G3 钉住驻留增量 ≤ 旧臂 1/4",
        new.hold_retained_growth * 4 <= old.hold_retained_growth,
        format!(
            "old={} new={}",
            old.hold_retained_growth, new.hold_retained_growth
        ),
    ));
    gates.push((
        "G4 稳态 fire 零分配（新臂）",
        new.steady_fire_allocs == 0 && old.steady_fire_allocs >= STEADY_FIRES,
        format!(
            "old={} new={}（{} 次 fire）",
            old.steady_fire_allocs, new.steady_fire_allocs, STEADY_FIRES
        ),
    ));
    gates.push((
        "G5 卸载分配 ≤ 旧臂 1/4",
        new.unload_allocs * 4 <= old.unload_allocs,
        format!("old={} new={}", old.unload_allocs, new.unload_allocs),
    ));
    gates
}

fn main() {
    let keys = event_keys();
    let plan = registration_plan(&keys);

    let old_arm = OldArm::new();
    let old = drive(
        &old_arm,
        &keys,
        &plan,
        OldArm::register,
        OldArm::fire_hold_across,
        OldArm::fire_fold,
        OldArm::unregister_source,
        |k| old_arm.final_order(k),
    );

    let new_arm = NewArm::new();
    let new = drive(
        &new_arm,
        &keys,
        &plan,
        NewArm::register,
        NewArm::fire_hold_across,
        NewArm::fire_fold,
        NewArm::unregister_source,
        |k| new_arm.final_order(k),
    );

    let gates = evaluate_gates(&old, &new);
    let mut pass = true;
    println!("轮次25 事件订阅表形态对照");
    for (name, ok, detail) in &gates {
        println!(
            "  [{}] {name} — {detail}",
            if *ok { "PASS" } else { "FAIL" }
        );
        pass &= ok;
    }
    println!(
        "加载分配 old={} 次/{} B → new={} 次/{} B",
        old.load_allocs, old.load_alloc_bytes, new.load_allocs, new.load_alloc_bytes
    );

    // JSON 结果（单次 format! 拼装，规避 format_push_string）
    let gates_json = gates
        .iter()
        .enumerate()
        .map(|(i, (name, ok, detail))| {
            format!(
                "    {{\"name\": \"{name}\", \"pass\": {ok}, \"detail\": \"{}\"}}{}",
                detail.replace('"', "'"),
                if i + 1 == gates.len() { "" } else { "," }
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let json = format!(
        "{{\n  \"round\": 25,\n  \"results\": {{\n    \"old\": {{\"load_allocs\": {}, \"load_alloc_bytes\": {}, \"hold_retained_growth\": {}, \"steady_fire_allocs\": {}, \"unload_allocs\": {}, \"fp\": \"{:#018x}\"}},\n    \"new\": {{\"load_allocs\": {}, \"load_alloc_bytes\": {}, \"hold_retained_growth\": {}, \"steady_fire_allocs\": {}, \"unload_allocs\": {}, \"fp\": \"{:#018x}\"}}\n  }},\n  \"gates\": [\n{gates_json}\n  ]\n}}\n",
        old.load_allocs,
        old.load_alloc_bytes,
        old.hold_retained_growth,
        old.steady_fire_allocs,
        old.unload_allocs,
        old.fp,
        new.load_allocs,
        new.load_alloc_bytes,
        new.hold_retained_growth,
        new.steady_fire_allocs,
        new.unload_allocs,
        new.fp,
    );

    let out_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../note/report/perf");
    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        eprintln!("创建报告目录失败：{e}");
        std::process::exit(1);
    }
    let path = out_dir.join("round25-handler-sub-table.json");
    match std::fs::File::create(&path) {
        Ok(mut f) => {
            if let Err(e) = f.write_all(json.as_bytes()) {
                eprintln!("写结果文件失败：{e}");
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("建结果文件失败 {}：{e}", path.display());
            std::process::exit(1);
        }
    }
    println!("结果已写入 {}", path.display());
    if !pass {
        eprintln!("存在未过闸门");
        std::process::exit(1);
    }
}
