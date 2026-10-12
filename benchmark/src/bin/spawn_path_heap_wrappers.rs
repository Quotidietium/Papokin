//! 轮次 44 基准：实体 spawn 路径堆包装消除。
//!
//! 对照形态（每实体构造的三个堆分配点）：
//! - 旧态：`current_biome: ArcSwap<&'static Biome>` 构造时
//!   `Arc::new(&'static)`（8B 盒/实体）、`move_control:
//!   Mutex<Box<dyn MoveControlTrait>>`（控制盒/实体）、
//!   `navigator: Mutex<Navigator{Box<dyn>}>`（导航盒/实体，
//!   `GroundPathNavigation` 约 1.2KB）；
//! - 新态：群系 Arc 盒按群系共享缓存（构造/切换零分配）、
//!   `AnyMoveControl` 枚举内联地面默认、`NavigatorInner`
//!   混合枚举内联地面导航（Deref 单点分派，能力面不变）。
//!
//! 负载：20000 实体构造 + 20 轮使用电池（控制 tick/导航
//! tick+`is_idle`+`move_to`/群系读取）+ 10% 实体群系切换 ×2 轮。
//! 语义指纹：使用电池与切换路径的读值折叠全等（双臂共用脚本）。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use papokin_data::biome::Biome;
use rustc_hash::FxHashMap;

const ENTITY_COUNT: usize = 20000;
const TICK_ROUNDS: usize = 20;
const SWITCH_ROUNDS: usize = 2;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);
static FREED_BYTES: AtomicUsize = AtomicUsize::new(0);

struct Counting;

// SAFETY: 仅在系统分配器外包裹计数，转发语义与系统分配器一致。
unsafe impl GlobalAlloc for Counting {
    // SAFETY: 转发系统分配器，契约由分配器 trait 保证。
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: 转发系统分配器。
        unsafe { System.alloc(layout) }
    }

    // SAFETY: 转发系统分配器，ptr 来自本分配器的 alloc。
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        FREED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: 转发系统分配器。
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn resident() -> usize {
    ALLOC_BYTES.load(Ordering::Relaxed) - FREED_BYTES.load(Ordering::Relaxed)
}

fn allocs() -> usize {
    ALLOCS.load(Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// 形态复刻（与生产同构的尺寸与分配点）
// ---------------------------------------------------------------------------

/// 地面移动控制复刻（与生产 `MoveControl` 同量级）。
#[derive(Clone, Copy)]
pub struct GroundControl {
    pub wanted_x: f64,
    pub wanted_y: f64,
    pub wanted_z: f64,
    pub speed_modifier: f64,
    pub operation: u8,
    pub strafe_forward: f32,
    pub strafe_right: f32,
    pub pad: [f64; 7],
}

pub trait MoveControlRep {
    fn tick(&mut self);
    fn set_wanted(&mut self, x: f64, y: f64, z: f64, speed: f64);
    fn has_wanted(&self) -> bool;
}

impl MoveControlRep for GroundControl {
    fn tick(&mut self) {
        if self.has_wanted() {
            self.wanted_x += 1.0;
            self.operation = 0;
        }
    }

    fn set_wanted(&mut self, x: f64, y: f64, z: f64, speed: f64) {
        self.wanted_x = x;
        self.wanted_y = y;
        self.wanted_z = z;
        self.speed_modifier = speed;
        self.operation = 1;
    }

    fn has_wanted(&self) -> bool {
        self.operation != 0
    }
}

/// 导航复刻（与生产 `GroundPathNavigation` 同量级，约 1.2KB）。
pub struct GroundNav {
    pub current_goal: Option<(f64, f64, f64)>,
    pub path_nodes: Vec<u64>,
    pub speed_modifier: f64,
    pub tick_count: u32,
    pub stuck_timer: u64,
    pub timeout_limit: f64,
    pub max_distance_to_waypoint: f32,
    pub target_pos: Option<(i32, i32, i32)>,
    pub reach_range: i32,
    pub malus_overrides: FxHashMap<u8, f32>,
    pub mob_width: f32,
    pub mob_height: f32,
    pub flags: u64,
    pub bulk: [u64; 96],
}

impl Default for GroundNav {
    fn default() -> Self {
        Self {
            current_goal: None,
            path_nodes: Vec::new(),
            speed_modifier: 1.0,
            tick_count: 0,
            stuck_timer: 0,
            timeout_limit: 0.0,
            max_distance_to_waypoint: 0.5,
            target_pos: None,
            reach_range: 0,
            malus_overrides: FxHashMap::default(),
            mob_width: 0.6,
            mob_height: 1.9,
            flags: 0,
            bulk: [0; 96],
        }
    }
}

pub trait NavRep {
    fn tick(&mut self);
    fn is_idle(&self) -> bool;
    fn move_to(&mut self, x: f64, y: f64, z: f64, speed: f64) -> bool;
}

impl NavRep for GroundNav {
    fn tick(&mut self) {
        self.tick_count = self.tick_count.wrapping_add(1);
        if self.path_nodes.is_empty() {
            self.flags &= !1;
        }
    }

    fn is_idle(&self) -> bool {
        self.path_nodes.is_empty()
    }

    fn move_to(&mut self, x: f64, y: f64, z: f64, speed: f64) -> bool {
        self.current_goal = Some((x, y, z));
        self.speed_modifier = speed;
        true
    }
}

/// 新态：移动控制混合枚举（内联地面默认，未接线特殊控制仍可装盒）。
pub enum AnyControl {
    Ground(GroundControl),
    Other(Box<dyn MoveControlRep>),
}

impl MoveControlRep for AnyControl {
    fn tick(&mut self) {
        match self {
            Self::Ground(c) => c.tick(),
            Self::Other(c) => c.tick(),
        }
    }

    fn set_wanted(&mut self, x: f64, y: f64, z: f64, speed: f64) {
        match self {
            Self::Ground(c) => c.set_wanted(x, y, z, speed),
            Self::Other(c) => c.set_wanted(x, y, z, speed),
        }
    }

    fn has_wanted(&self) -> bool {
        match self {
            Self::Ground(c) => c.has_wanted(),
            Self::Other(c) => c.has_wanted(),
        }
    }
}

/// 新态：导航混合枚举（内联地面导航，Deref 单点分派）。
pub struct Nav {
    inner: NavInner,
}

#[allow(clippy::large_enum_variant)]
enum NavInner {
    Ground(GroundNav),
    Custom(Box<dyn NavRep>),
}

impl Nav {
    #[must_use]
    pub fn ground() -> Self {
        Self {
            inner: NavInner::Ground(GroundNav::default()),
        }
    }
}

impl Default for Nav {
    fn default() -> Self {
        Self::ground()
    }
}

impl Deref for Nav {
    type Target = dyn NavRep;

    fn deref(&self) -> &Self::Target {
        match &self.inner {
            NavInner::Ground(nav) => nav,
            NavInner::Custom(nav) => nav.as_ref(),
        }
    }
}

impl DerefMut for Nav {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut self.inner {
            NavInner::Ground(nav) => nav,
            NavInner::Custom(nav) => nav.as_mut(),
        }
    }
}

const fn ground_control() -> GroundControl {
    GroundControl {
        wanted_x: 0.0,
        wanted_y: 0.0,
        wanted_z: 0.0,
        speed_modifier: 1.0,
        operation: 0,
        strafe_forward: 0.0,
        strafe_right: 0.0,
        pad: [0.0; 7],
    }
}

/// 新态：群系 Arc 盒共享缓存（与生产 `shared_biome_arc` 同构）。
// `Arc<&'static Biome>` 复刻生产形态（ArcSwap 硬要求），lint 豁免。
#[allow(clippy::redundant_allocation)]
fn shared_biome_arc(biome: &'static Biome) -> Arc<&'static Biome> {
    static CACHE: OnceLock<Mutex<FxHashMap<usize, Arc<&'static Biome>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(FxHashMap::default()));
    let mut guard = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard
        .entry(std::ptr::from_ref(biome) as usize)
        .or_insert_with(|| Arc::new(biome))
        .clone()
}

const BIOME_IDS: [u8; 6] = [0, 1, 4, 5, 7, 9];

fn biome_at(i: usize) -> &'static Biome {
    let id = BIOME_IDS[i % BIOME_IDS.len()];
    Biome::from_id(id)
        .or_else(|| Biome::from_id(0))
        .unwrap_or(&Biome::PLAINS)
}

const fn fp_mix(fp: u64, x: u64) -> u64 {
    (fp ^ x).wrapping_mul(0x1000_0000_01b3)
}

struct ArmStats {
    spawn_allocs: usize,
    spawn_ms: f64,
    tick_ms: f64,
    switch_allocs: usize,
    resident: usize,
}

// ---------------------------------------------------------------------------
// 双臂
// ---------------------------------------------------------------------------

#[allow(clippy::redundant_allocation)]
struct OldEntity {
    biome: Arc<&'static Biome>,
    control: Mutex<Box<dyn MoveControlRep>>,
    nav: Mutex<Box<dyn NavRep>>,
}

#[allow(clippy::redundant_allocation)]
struct NewEntity {
    biome: Arc<&'static Biome>,
    control: Mutex<AnyControl>,
    nav: Mutex<Nav>,
}

fn run_old_arm() -> (u64, ArmStats) {
    let a0 = allocs();
    let start = Instant::now();
    let mut corpus: Vec<OldEntity> = Vec::with_capacity(ENTITY_COUNT);
    for i in 0..ENTITY_COUNT {
        corpus.push(OldEntity {
            biome: Arc::new(biome_at(i)),
            control: Mutex::new(Box::new(ground_control())),
            nav: Mutex::new(Box::new(GroundNav::default())),
        });
    }
    let spawn_allocs = allocs() - a0;
    let spawn_ms = start.elapsed().as_secs_f64() * 1000.0;

    let mut fp: u64 = 0xcbf2_9ce4_8422_2325;
    let tick_start = Instant::now();
    for _ in 0..TICK_ROUNDS {
        for e in &corpus {
            let Ok(mut control) = e.control.lock() else {
                continue;
            };
            control.set_wanted(1.0, 2.0, 3.0, 1.0);
            control.tick();
            fp = fp_mix(fp, u64::from(u8::from(control.has_wanted())));
            let Ok(mut nav) = e.nav.lock() else {
                continue;
            };
            nav.tick();
            fp = fp_mix(fp, u64::from(nav.is_idle()));
            fp = fp_mix(fp, u64::from(nav.move_to(4.0, 5.0, 6.0, 1.2)));
            fp = fp_mix(fp, u64::from(e.biome.id));
        }
    }
    let tick_ms = tick_start.elapsed().as_secs_f64() * 1000.0;
    let after_tick = allocs();

    for round in 1..=SWITCH_ROUNDS {
        for (i, e) in corpus.iter_mut().enumerate() {
            if i.is_multiple_of(10) {
                let target = biome_at(i + round);
                if !std::ptr::eq(*e.biome, target) {
                    e.biome = Arc::new(target);
                }
                fp = fp_mix(fp, u64::from(e.biome.id));
            }
        }
    }
    let switch_allocs = allocs() - after_tick;

    (
        fp,
        ArmStats {
            spawn_allocs,
            spawn_ms,
            tick_ms,
            switch_allocs,
            resident: resident(),
        },
    )
}

fn run_new_arm() -> (u64, ArmStats) {
    let a0 = allocs();
    let start = Instant::now();
    let mut corpus: Vec<NewEntity> = Vec::with_capacity(ENTITY_COUNT);
    for i in 0..ENTITY_COUNT {
        corpus.push(NewEntity {
            biome: shared_biome_arc(biome_at(i)),
            control: Mutex::new(AnyControl::Ground(ground_control())),
            nav: Mutex::new(Nav::ground()),
        });
    }
    let spawn_allocs = allocs() - a0;
    let spawn_ms = start.elapsed().as_secs_f64() * 1000.0;

    let mut fp: u64 = 0xcbf2_9ce4_8422_2325;
    let tick_start = Instant::now();
    for _ in 0..TICK_ROUNDS {
        for e in &corpus {
            let Ok(mut control) = e.control.lock() else {
                continue;
            };
            control.set_wanted(1.0, 2.0, 3.0, 1.0);
            control.tick();
            fp = fp_mix(fp, u64::from(u8::from(control.has_wanted())));
            let Ok(mut nav) = e.nav.lock() else {
                continue;
            };
            nav.tick();
            fp = fp_mix(fp, u64::from(nav.is_idle()));
            fp = fp_mix(fp, u64::from(nav.move_to(4.0, 5.0, 6.0, 1.2)));
            fp = fp_mix(fp, u64::from(e.biome.id));
        }
    }
    let tick_ms = tick_start.elapsed().as_secs_f64() * 1000.0;
    let after_tick = allocs();

    for round in 1..=SWITCH_ROUNDS {
        for (i, e) in corpus.iter_mut().enumerate() {
            if i.is_multiple_of(10) {
                let target = biome_at(i + round);
                if !std::ptr::eq(*e.biome, target) {
                    e.biome = shared_biome_arc(target);
                }
                fp = fp_mix(fp, u64::from(e.biome.id));
            }
        }
    }
    let switch_allocs = allocs() - after_tick;

    (
        fp,
        ArmStats {
            spawn_allocs,
            spawn_ms,
            tick_ms,
            switch_allocs,
            resident: resident(),
        },
    )
}

// ---------------------------------------------------------------------------
// 闸门与报告
// ---------------------------------------------------------------------------

fn main() {
    println!("轮次 44 基准：实体 spawn 路径堆包装消除（群系 Arc/控制盒/导航盒）");
    println!(
        "语料：{ENTITY_COUNT} 实体 · 使用 {TICK_ROUNDS} 轮 · 群系切换 {SWITCH_ROUNDS} 轮（10% 实体）"
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
    tick_ok: bool,
    tick_ratio: f64,
    all_passed: bool,
}

fn evaluate(old: &ArmStats, new: &ArmStats, old_fp: u64, new_fp: u64) -> Verdict {
    let semantics_ok = old_fp == new_fp;
    let resident_ok = new.resident < old.resident;
    let spawn_allocs_ok = new.spawn_allocs < old.spawn_allocs;
    let tick_ratio = old.tick_ms / new.tick_ms;
    let tick_ok = tick_ratio >= 0.91;
    Verdict {
        semantics_ok,
        resident_ok,
        spawn_allocs_ok,
        tick_ok,
        tick_ratio,
        all_passed: semantics_ok && resident_ok && spawn_allocs_ok && tick_ok,
    }
}

fn print_gates(v: &Verdict, old: &ArmStats, new: &ArmStats, old_fp: u64, new_fp: u64) {
    println!("=== 闸门 ===");
    println!(
        "{} 使用电池指纹全等：旧 0x{old_fp:016x} vs 新 0x{new_fp:016x}",
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
        "{} 构造分配次数下降：{} → {} 次（每实体 {:.3} → {:.3}）",
        mark(v.spawn_allocs_ok),
        old.spawn_allocs,
        new.spawn_allocs,
        old.spawn_allocs as f64 / ENTITY_COUNT as f64,
        new.spawn_allocs as f64 / ENTITY_COUNT as f64
    );
    println!(
        "{} 使用耗时不劣化（≥0.91×）：{:.2} → {:.2} ms（×{:.3}）",
        mark(v.tick_ok),
        old.tick_ms,
        new.tick_ms,
        v.tick_ratio
    );
    println!();
    println!("=== 数值 ===");
    println!(
        "构造：{:.2} → {:.2} ms（×{:.2}）",
        old.spawn_ms,
        new.spawn_ms,
        old.spawn_ms / new.spawn_ms
    );
    println!(
        "群系切换分配：{} → {} 次",
        old.switch_allocs, new.switch_allocs
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
        "round": 44,
        "topic": "spawn_path_heap_wrappers",
        "corpus": {
            "entities": ENTITY_COUNT,
            "tick_rounds": TICK_ROUNDS,
            "switch_rounds": SWITCH_ROUNDS,
            "wrapper_points": ["biome_arc", "move_control_box", "navigator_box"],
        },
        "gates": [
            {"name": "使用电池指纹全等", "passed": v.semantics_ok,
             "detail": format!("0x{old_fp:016x} vs 0x{new_fp:016x}")},
            {"name": "驻留字节下降（主判据）", "passed": v.resident_ok,
             "detail": format!("{} → {} B", old.resident, new.resident)},
            {"name": "构造分配次数下降", "passed": v.spawn_allocs_ok,
             "detail": format!("{} → {}", old.spawn_allocs, new.spawn_allocs)},
            {"name": "使用耗时不劣化（≥0.91×）", "passed": v.tick_ok,
             "detail": format!("×{:.3}", v.tick_ratio)},
        ],
        "old": {
            "resident_bytes": old.resident,
            "spawn_allocs": old.spawn_allocs,
            "spawn_ms": old.spawn_ms,
            "tick_ms": old.tick_ms,
            "switch_allocs": old.switch_allocs,
            "content_fingerprint": format!("0x{old_fp:016x}"),
        },
        "new": {
            "resident_bytes": new.resident,
            "spawn_allocs": new.spawn_allocs,
            "spawn_ms": new.spawn_ms,
            "tick_ms": new.tick_ms,
            "switch_allocs": new.switch_allocs,
            "content_fingerprint": format!("0x{new_fp:016x}"),
        },
        "sizes": {
            "ground_nav_inline_bytes": std::mem::size_of::<GroundNav>(),
            "any_control_inline_bytes": std::mem::size_of::<AnyControl>(),
        },
        "outcome": if v.all_passed { "positive" } else { "negative" },
    });

    let path = std::path::Path::new("note/report/perf/round44-spawn-path-heap-wrappers.json");
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
