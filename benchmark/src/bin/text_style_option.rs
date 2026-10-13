//! 轮次 51 基准：文本组件空样式盒去化。
//!
//! 生产改动：`TextComponentBase.style` 从无条件物化的 `Box<Style>`
//! （168 B 堆块 + 一次分配）改为 `Option<Box<Style>>`，`None` 表示未
//! 设置任何样式；解码边界（serde 反序列化）把空样式折叠回 `None`，
//! 相等/哈希按规范化两态等价实现。
//!
//! 双臂口径：
//! - 旧臂：忠实复刻改动前的结构（`style: Box<Style>` 恒物化，构造/
//!   克隆逐节点深拷贝样式盒），内容变体尺寸与生产一致；
//! - 新臂：生产 `TextComponent`（`text()`/`translate()` 构造、
//!   `.color_named()` 置样式、`style()` 读视图）。
//!
//! 电池：
//! 1. 构造驻留（主判据）：纯文本组件每实例存活字节与分配数；
//! 2. 带样式构造（对照）：置色组件不得劣化；
//! 3. 克隆广播：混合树（纯根 + 2 带样式子 + 1 纯子）整体克隆；
//! 4. 译码边界：map 形 JSON 反序列化后未设样式的内存表示；
//! 5. 指纹：逻辑等价树的 JSON 序列化值两臂全等；
//! 6. 时间：构造 / 克隆 / 读三组耗时比。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::borrow::Cow;
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use papokin_util::text::TextComponent;
use papokin_util::text::color::NamedColor;
use papokin_util::text::style::Style;

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

struct CountingAlloc;

// SAFETY: 仅统计后原样转发系统分配器，不改变任何分配语义。
unsafe impl GlobalAlloc for CountingAlloc {
    // SAFETY: 转发给系统分配器；仅在成功时记账。
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: 转发给系统分配器；仅在成功时记账。
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            LIVE_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        ptr
    }

    // SAFETY: 转发系统分配器，ptr 来自本分配器的 alloc。
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        // SAFETY: 转发系统分配器。
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

const N: usize = 2000;
const CLONES: usize = 1000;
const READ_ROUNDS: usize = 200;

/// 旧臂内容变体：与生产 `TextContent` 的 Text/Translate 两变体同形
/// （`Cow` + `Vec` 参数）。
// repr(C) 防止 niche 折叠把枚举压到 48 B：生产 TextContent 实测 56 B，
// 旧臂内容盒须同尺寸才公平
#[repr(C)]
#[derive(Clone)]
enum OldContent {
    Text(Cow<'static, str>),
    /// 保真复刻：生产枚举含翻译变体，撑起同尺寸（56 B）内联
    #[allow(dead_code)]
    Translate {
        translate: Cow<'static, str>,
        with: Vec<OldBase>,
    },
}

/// 旧臂基础结构：`style: Box<Style>` 无条件物化（改动前形态）。
#[derive(Clone)]
struct OldBase {
    content: Box<OldContent>,
    style: Box<Style>,
    extra: Vec<Self>,
}

impl OldBase {
    /// 复刻旧 `TextComponent::text`：内容盒 + 空样式盒 + 空 Vec。
    fn text(plain: &str) -> Self {
        Self {
            content: Box::new(OldContent::Text(Cow::Owned(plain.to_string()))),
            style: Box::default(),
            extra: vec![],
        }
    }

    /// 复刻旧 builder：置色后样式盒内含颜色。
    fn color_named(self, color: NamedColor) -> Self {
        Self {
            content: self.content,
            style: Box::new(Style::default().color_named(color)),
            extra: self.extra,
        }
    }
}

/// 新臂等价构造：直接走生产 API。
fn new_text(plain: &str) -> TextComponent {
    TextComponent::text(plain.to_string())
}

/// 电池统计。
#[derive(Default, Clone, Copy)]
struct Stats {
    live_after: usize,
    allocs: usize,
}

fn snapshot() -> (usize, usize) {
    (
        LIVE_BYTES.load(Ordering::Relaxed),
        ALLOCS.load(Ordering::Relaxed),
    )
}

fn run_battery<T>(build: impl Fn(&str) -> T) -> Stats {
    // 语料在测量窗口外生成：format! 的容量策略随内联上下文漂移，
    // 两臂须消费同一批精确容量字符串，差分才纯粹是结构差异
    let corpus: Vec<String> = (0..N).map(|i| format!("消息编号{i}")).collect();
    let (b0, a0) = snapshot();
    let held: Vec<T> = corpus.iter().map(|s| build(s)).collect();
    let (b1, a1) = snapshot();
    std::mem::forget(held);
    Stats {
        live_after: b1 - b0,
        allocs: a1 - a0,
    }
}

/// 构造 + 克隆广播电池：驻留含原始树与克隆树。
fn run_clone_battery<T>(build_tree: impl Fn() -> T, clone_one: impl Fn(&T) -> T) -> Stats {
    let tree = build_tree();
    let (b0, a0) = snapshot();
    let held: Vec<T> = (0..CLONES).map(|_| clone_one(&tree)).collect();
    let (b1, a1) = snapshot();
    std::mem::forget(held);
    std::mem::forget(tree);
    Stats {
        live_after: b1 - b0,
        allocs: a1 - a0,
    }
}

/// 时间电池：返回每轮总耗时（ms）。
fn time_battery<T>(rounds: usize, mut step: impl FnMut() -> T) -> f64 {
    // 预热
    let warm = step();
    let start = Instant::now();
    for _ in 0..rounds {
        let v = step();
        std::mem::forget(v);
    }
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    std::mem::forget(warm);
    ms
}

/// 旧臂树的 JSON 值视图：与生产 flatten 序列化语义对齐。
fn old_to_value(base: &OldBase) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    match &*base.content {
        OldContent::Text(text) => {
            map.insert("text".into(), serde_json::Value::String(text.to_string()));
        }
        OldContent::Translate { translate, with } => {
            map.insert(
                "translate".into(),
                serde_json::Value::String(translate.to_string()),
            );
            map.insert(
                "with".into(),
                serde_json::Value::Array(with.iter().map(old_to_value).collect()),
            );
        }
    }
    if let Some(color) = &base.style.color {
        let name = match color {
            papokin_util::text::color::Color::Named(named) => named.to_string().to_lowercase(),
            other => format!("{other:?}"),
        };
        map.insert("color".into(), serde_json::Value::String(name));
    }
    if !base.extra.is_empty() {
        map.insert(
            "extra".into(),
            serde_json::Value::Array(base.extra.iter().map(old_to_value).collect()),
        );
    }
    serde_json::Value::Object(map)
}

/// 指纹：两臂各建「纯根 + 带样式子 + 纯子（带纯孙）」树，序列化为 JSON 值。
fn old_fingerprint() -> serde_json::Value {
    let mut with_child = OldBase::text("根消息");
    with_child
        .extra
        .push(OldBase::text("带色子").color_named(NamedColor::Red));
    let mut plain_child = OldBase::text("纯子");
    plain_child.extra.push(OldBase::text("纯孙"));
    with_child.extra.push(plain_child);
    old_to_value(&with_child)
}

fn new_fingerprint() -> serde_json::Value {
    let mut with_child = TextComponent::text("根消息");
    with_child
        .0
        .extra
        .push(TextComponent::text("带色子").color_named(NamedColor::Red).0);
    let mut plain_child = TextComponent::text("纯子");
    plain_child.0.extra.push(TextComponent::text("纯孙").0);
    with_child.0.extra.push(plain_child.0);
    serde_json::to_value(&with_child).unwrap_or_default()
}

/// 译码边界电池：map 形无样式键 JSON → 生产反序列化。
/// 返回（每实例存活字节, 分配数, 全部未物化样式盒）。
fn decode_battery() -> (f64, f64, bool) {
    let payload = serde_json::json!({ "text": "译码纯文本" });
    let (b0, a0) = snapshot();
    let held: Vec<TextComponent> = (0..N)
        .map(|_| {
            serde_json::from_value(payload.clone()).unwrap_or_else(|_| TextComponent::text(""))
        })
        .collect();
    let (b1, a1) = snapshot();
    let all_none = held.iter().all(|comp| comp.0.style.is_none());
    std::mem::forget(held);
    (
        (b1 - b0) as f64 / N as f64,
        (a1 - a0) as f64 / N as f64,
        all_none,
    )
}

/// 驻留电池组结果。
struct Residency {
    old_plain: Stats,
    new_plain: Stats,
    old_styled: Stats,
    new_styled: Stats,
    old_clone: Stats,
    new_clone: Stats,
    decode_bytes: f64,
    decode_allocs: f64,
    decode_all_none: bool,
}

fn run_residency_batteries() -> Residency {
    let old_plain = run_battery(OldBase::text);
    let new_plain = run_battery(new_text);
    let old_styled = run_battery(|s| OldBase::text(s).color_named(NamedColor::Yellow));
    let new_styled = run_battery(|s| new_text(s).color_named(NamedColor::Yellow));
    let old_clone = run_clone_battery(
        || {
            let mut root = OldBase::text("广播根");
            root.extra
                .push(OldBase::text("红子").color_named(NamedColor::Red));
            root.extra
                .push(OldBase::text("绿子").color_named(NamedColor::Green));
            root.extra.push(OldBase::text("纯子"));
            root
        },
        Clone::clone,
    );
    let new_clone = run_clone_battery(
        || {
            let mut root = TextComponent::text("广播根");
            root.0
                .extra
                .push(TextComponent::text("红子").color_named(NamedColor::Red).0);
            root.0
                .extra
                .push(TextComponent::text("绿子").color_named(NamedColor::Green).0);
            root.0.extra.push(TextComponent::text("纯子").0);
            root
        },
        Clone::clone,
    );
    let (decode_bytes, decode_allocs, decode_all_none) = decode_battery();
    Residency {
        old_plain,
        new_plain,
        old_styled,
        new_styled,
        old_clone,
        new_clone,
        decode_bytes,
        decode_allocs,
        decode_all_none,
    }
}

/// 时间电池组结果。
#[derive(Default, Clone, Copy)]
struct Timing {
    construct: f64,
    clone: f64,
    read: f64,
}

fn run_timing_batteries() -> Timing {
    let old_construct_ms = time_battery(N, || OldBase::text("计时消息"));
    let new_construct_ms = time_battery(N, || new_text("计时消息"));

    let old_tree = {
        let mut root = OldBase::text("广播根");
        root.extra
            .push(OldBase::text("红子").color_named(NamedColor::Red));
        root.extra.push(OldBase::text("纯子"));
        root
    };
    let new_tree = {
        let mut root = TextComponent::text("广播根");
        root.0
            .extra
            .push(TextComponent::text("红子").color_named(NamedColor::Red).0);
        root.0.extra.push(TextComponent::text("纯子").0);
        root
    };
    let old_clone_ms = time_battery(CLONES, || old_tree.clone());
    let new_clone_ms = time_battery(CLONES, || new_tree.clone());

    // 读路径：生产编码形态（to_nbt/json/legacy 均「绑定样式、读多字段」；
    // 空态判定旧臂走 11 字段 is_empty，新臂走 style_opt 一次判空）
    let old_read_ms = time_battery(READ_ROUNDS, || {
        let mut hits = 0usize;
        for _ in 0..N {
            let tree = black_box(&old_tree);
            let styled = &*tree.extra[0].style;
            let plain = &*tree.style;
            if styled.color.is_some() {
                hits += 1;
            }
            if plain.is_empty() {
                hits += 1;
            }
            if plain.bold.is_none() && styled.bold.is_none() {
                hits += 1;
            }
        }
        black_box(hits)
    });
    let new_read_ms = time_battery(READ_ROUNDS, || {
        let mut hits = 0usize;
        for _ in 0..N {
            let tree = black_box(&new_tree);
            let styled = tree.0.extra[0].style();
            if styled.color.is_some() {
                hits += 1;
            }
            if tree.0.style_opt().is_none() {
                hits += 1;
            }
            if tree.0.style().bold.is_none() && styled.bold.is_none() {
                hits += 1;
            }
        }
        black_box(hits)
    });

    Timing {
        construct: old_construct_ms / new_construct_ms,
        clone: old_clone_ms / new_clone_ms,
        read: old_read_ms / new_read_ms,
    }
}

struct Verdict {
    fingerprint_ok: bool,
    resident_ok: bool,
    allocs_ok: bool,
    styled_no_regression: bool,
    clone_resident_ok: bool,
    decode_fold_ok: bool,
    read_ratio: f64,
    all_passed: bool,
}

const fn pass(ok: bool) -> &'static str {
    if ok { "通过" } else { "未过" }
}

fn print_measurements(r: &Residency, t: &Timing) {
    println!("=== 实测 ===");
    println!(
        "纯文本构造：旧 {} B · {} alloc → 新 {} B · {} alloc（每实例）",
        r.old_plain.live_after / N,
        r.old_plain.allocs / N,
        r.new_plain.live_after / N,
        r.new_plain.allocs / N
    );
    println!(
        "带样式构造：旧 {} B · {} alloc → 新 {} B · {} alloc（每实例）",
        r.old_styled.live_after / N,
        r.old_styled.allocs / N,
        r.new_styled.live_after / N,
        r.new_styled.allocs / N
    );
    println!(
        "克隆广播（4 节点混合树 ×{CLONES}）：旧 {} B · {} alloc → 新 {} B · {} alloc",
        r.old_clone.live_after, r.old_clone.allocs, r.new_clone.live_after, r.new_clone.allocs
    );
    println!(
        "译码边界：{:.1} B · {:.2} alloc/实例 · 空样式全折叠={}",
        r.decode_bytes, r.decode_allocs, r.decode_all_none
    );
    println!(
        "时间比（旧/新）：构造 ×{:.3} · 克隆 ×{:.3} · 读 ×{:.3}",
        t.construct, t.clone, t.read
    );
}

fn print_gates(v: &Verdict) {
    println!();
    println!("=== 闸门 ===");
    println!("指纹全等（JSON 值）：{}", pass(v.fingerprint_ok));
    println!("驻留字节下降（纯文本）：{}", pass(v.resident_ok));
    println!("构造分配数下降（纯文本）：{}", pass(v.allocs_ok));
    println!("带样式构造不劣化：{}", pass(v.styled_no_regression));
    println!("克隆驻留字节下降：{}", pass(v.clone_resident_ok));
    println!("译码空样式折叠：{}", pass(v.decode_fold_ok));
    println!(
        "读路径耗时比 ≥ 0.91：{}（实际 ×{:.3}）",
        pass(v.read_ratio >= 0.91),
        v.read_ratio
    );
    println!();
    println!(
        "闸门判定：{}",
        if v.all_passed {
            "全部通过，予以收编"
        } else {
            "存在未过闸门项，不予收编"
        }
    );
}

fn write_report(r: &Residency, t: &Timing, v: &Verdict) {
    let report = serde_json::json!({
        "round": 51,
        "topic": "text_style_option",
        "kind": "dual_arm",
        "samples": N,
        "clone_samples": CLONES,
        "old": {
            "plain_bytes_per": r.old_plain.live_after / N,
            "plain_allocs_per": r.old_plain.allocs / N,
            "styled_bytes_per": r.old_styled.live_after / N,
            "styled_allocs_per": r.old_styled.allocs / N,
            "clone_bytes_total": r.old_clone.live_after,
            "clone_allocs_total": r.old_clone.allocs,
        },
        "new": {
            "plain_bytes_per": r.new_plain.live_after / N,
            "plain_allocs_per": r.new_plain.allocs / N,
            "styled_bytes_per": r.new_styled.live_after / N,
            "styled_allocs_per": r.new_styled.allocs / N,
            "clone_bytes_total": r.new_clone.live_after,
            "clone_allocs_total": r.new_clone.allocs,
        },
        "decode_boundary": {
            "bytes_per": r.decode_bytes,
            "allocs_per": r.decode_allocs,
            "all_folded_to_none": r.decode_all_none,
        },
        "gates": {
            "fingerprint_ok": v.fingerprint_ok,
            "resident_ok": v.resident_ok,
            "allocs_ok": v.allocs_ok,
            "styled_no_regression": v.styled_no_regression,
            "clone_resident_ok": v.clone_resident_ok,
            "decode_fold_ok": v.decode_fold_ok,
            "read_ratio": v.read_ratio,
            "construct_ratio": t.construct,
            "clone_ratio": t.clone,
            "all_passed": v.all_passed,
        },
        "outcome": if v.all_passed { "adopted" } else { "rejected" },
    });
    let bytes = serde_json::to_string_pretty(&report).unwrap_or_default();
    if let Err(err) = std::fs::write("note/report/perf/round51-text-style-option.json", bytes) {
        eprintln!("写入报告失败：{err}");
    }
}

fn main() {
    println!("轮次 51 基准：文本组件空样式盒去化（Option<Box<Style>>）");
    println!("语料：{N} 组件/电池 · 克隆广播 {CLONES} 棵 · 读 {READ_ROUNDS} 轮");
    println!();

    let old_fp = old_fingerprint();
    let new_fp = new_fingerprint();

    let r = run_residency_batteries();
    let t = run_timing_batteries();

    let read_ratio = t.read;
    let v = Verdict {
        fingerprint_ok: old_fp == new_fp,
        resident_ok: r.new_plain.live_after < r.old_plain.live_after,
        allocs_ok: r.new_plain.allocs < r.old_plain.allocs,
        styled_no_regression: r.new_styled.live_after <= r.old_styled.live_after
            && r.new_styled.allocs <= r.old_styled.allocs,
        clone_resident_ok: r.new_clone.live_after < r.old_clone.live_after,
        decode_fold_ok: r.decode_all_none,
        read_ratio,
        all_passed: false,
    };
    let all = v.fingerprint_ok
        && v.resident_ok
        && v.allocs_ok
        && v.styled_no_regression
        && v.clone_resident_ok
        && v.decode_fold_ok
        && v.read_ratio >= 0.91;
    let v = Verdict {
        all_passed: all,
        ..v
    };

    print_measurements(&r, &t);
    print_gates(&v);
    write_report(&r, &t, &v);
}
