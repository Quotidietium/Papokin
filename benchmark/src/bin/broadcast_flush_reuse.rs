//! 轮次 18 基准：广播扇出分组与方块冲刷路径逐调用分配流失的量化
//! 与收编对照。
//!
//! ## 侦察结论（生产可达性已验证）
//!
//! 每次广播（`broadcast_to_chunk` / `broadcast_packet_all` /
//! `send_to_tracking_players` 一族，全仓 121 处调用点）经
//! `collect_java_recipients_by_version` 新建 `BTreeMap<版本, Vec<&客户端>>`
//! 以及每版本一个 Vec，仅为了让同版本收件人共享一份序列化字节——分组
//! 结构是纯瞬态的。热点：`TrackedEntity::send_to_tracking_players`
//! 每个移动实体每 tick 至少一次（移动/旋转/属性/移除包）。
//!
//! 世界冲刷（每世界每 tick 无条件执行）：
//!
//! - `flush_block_updates`：每 tick 新建分组 `HashMap<节, Vec<变更>>`
//!   以及每节一个 Vec；`mem::take` 取走变更队列后其容量随本 tick 结束
//!   丢弃，下一 tick 生产者推入时逐级重分配；
//! - `flush_synced_block_events`：同样 `mem::take` 丢容量。
//!
//! 收编形态：① 广播改单趟内联版本槽（栈上 4 槽懒序列化缓存，分组
//! 结构零分配；语义字节每版本一份保持不变）；② 冲刷改驻留交换缓冲
//! （`swap` 接替 `mem::take`，队列容量跨 tick 保留）+ 分组图驻留
//! （清值留桶）。本基准量化两形态分配账，并以投递指纹作语义等价硬
//! 闸门。
//!
//! 硬闸门：两形态投递多重集指纹（交换律混合，跨客户端顺序本来
//! 不可观测）与投递总数全等；复用形态稳态分配次数与字节严格小于
//! 逐调用新建。
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::io::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};

// ---------------------------------------------------------------------------
// 分配计数
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
// 负载参数
// ---------------------------------------------------------------------------

/// 模拟 tick 数
const TICKS: usize = 600;
/// 每 tick 广播次数：实体追踪移动/旋转/属性包约 600（300 移动实体）+
/// 单变更多节广播/方块事件/声音粒子约 340
const BROADCASTS_PER_TICK: usize = 940;
/// 单次广播收件人数（注视同区块/追踪同实体的玩家常态）
const RECIPIENTS: usize = 12;
/// 双版本广播占比（每 10 次 1 次，ViaVersion 混合服常态；其余单版本）
const MIXED_VERSION_MODULO: usize = 10;
/// 每 tick 方块变更数（红石农场 + 玩家挖掘 + 流体）
const FLUSH_CHANGES: usize = 400;
/// 变更分布的节数
const FLUSH_SECTIONS: u64 = 60;
/// 每 tick 同步方块事件数（活塞链等）
const FLUSH_EVENTS: usize = 80;

// ---------------------------------------------------------------------------
// 负载模型
// ---------------------------------------------------------------------------

/// 收件人占位：客户端令牌 + 协议版本
#[derive(Clone, Copy)]
struct Recipient {
    token: u64,
    version: u8,
}

/// 一次方块变更占位：位置 + 状态
#[derive(Clone, Copy)]
struct Change {
    pos: u64,
    state: u64,
}

/// 混合指纹： fnv 式折叠
const fn fp_mix(fp: u64, v: u64) -> u64 {
    (fp ^ v).wrapping_mul(0x0000_0100_0000_01B3)
}

/// 确定性生成第 b 次广播的第 r 个收件人
const fn recipient(b: usize, tick: usize, r: usize) -> Recipient {
    let token = (b as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((tick as u64) << 16)
        .wrapping_add(r as u64);
    // 每 MIXED_VERSION_MODULO 次广播一次双版本：前半收件人 v1、后半 v2
    let version = if b.is_multiple_of(MIXED_VERSION_MODULO) && r >= RECIPIENTS / 2 {
        2
    } else {
        1
    };
    Recipient { token, version }
}

/// 语义负载：每（广播, 版本）一份，两形态完全相同（对账时相互抵消），
/// 长度确定性派生
fn serialize_payload(b: usize, tick: usize, version: u8) -> Vec<u8> {
    let len = 24 + (b * 7 + tick + usize::from(version)) % 40;
    let mut payload = vec![0u8; len];
    payload[0] = u8::try_from(b & 0xFF).unwrap_or_default();
    payload[1] = version;
    payload
}

/// 投递入账（交换律混合：跨客户端投递顺序本来不可观测，等价性
/// 按投递多重集判定）：混合（收件人令牌、版本、负载指纹）
fn deliver(fp: &mut u64, r: &Recipient, payload: &[u8]) {
    let mut h = r.token ^ u64::from(r.version) ^ ((payload.len() as u64) << 32);
    for (i, b) in payload.iter().enumerate() {
        h = fp_mix(h, u64::from(*b).wrapping_add(i as u64));
    }
    *fp ^= h.rotate_left((r.token & 31) as u32);
}

// ---------------------------------------------------------------------------
// 广播：生产现状（BTreeMap 分组） vs 单趟内联版本槽
// ---------------------------------------------------------------------------

/// 生产现状：先建 `BTreeMap<版本, Vec<收件人>>`，再逐版本序列化投递
fn broadcast_fresh(b: usize, tick: usize, fp: &mut u64) {
    let mut by_version: BTreeMap<u8, Vec<Recipient>> = BTreeMap::new();
    for r in 0..RECIPIENTS {
        let rcp = recipient(b, tick, r);
        by_version.entry(rcp.version).or_default().push(rcp);
    }
    for (version, recipients) in by_version {
        let payload = serialize_payload(b, tick, version);
        for rcp in recipients {
            deliver(fp, &rcp, &payload);
        }
    }
}

/// 收编形态：栈上内联版本槽，逐收件人懒序列化共享，分组零分配
fn broadcast_single_pass(b: usize, tick: usize, fp: &mut u64) {
    let mut slots: [Option<(u8, Vec<u8>)>; 4] = [None, None, None, None];
    'recipients: for r in 0..RECIPIENTS {
        let rcp = recipient(b, tick, r);
        for slot in &mut slots {
            match slot {
                Some((v, payload)) if *v == rcp.version => {
                    deliver(fp, &rcp, payload);
                    continue 'recipients;
                }
                None => {
                    let payload = serialize_payload(b, tick, rcp.version);
                    deliver(fp, &rcp, &payload);
                    *slot = Some((rcp.version, payload));
                    continue 'recipients;
                }
                _ => {}
            }
        }
        // 槽满兜底：直接序列化投递（本基准负载不会到达）
        let payload = serialize_payload(b, tick, rcp.version);
        deliver(fp, &rcp, &payload);
    }
}

// ---------------------------------------------------------------------------
// 冲刷：生产现状（每 tick 新建分组 + take 丢容量） vs 驻留交换
// ---------------------------------------------------------------------------

const fn change(tick: usize, i: usize) -> Change {
    let pos = (tick as u64)
        .wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        .wrapping_add(i as u64);
    Change {
        pos,
        state: pos ^ 0x0005_DEEC_E66D,
    }
}

/// 节键：变更确定性散布到 `FLUSH_SECTIONS` 个节
const fn section_key(c: &Change) -> u64 {
    c.pos % FLUSH_SECTIONS
}

/// 节广播入账（交换律混合）：节键 + 各变更
fn deliver_section(fp: &mut u64, section: u64, updates: &[Change]) {
    let mut h = section ^ ((updates.len() as u64) << 32);
    for u in updates {
        h = fp_mix(h, u.pos ^ u.state);
    }
    *fp ^= h.rotate_left((section & 31) as u32);
}

/// 事件广播入账（顺序无关混合）
const fn deliver_event(fp: &mut u64, e: u64) {
    *fp ^= fp_mix(0x9E37_79B9_7F4A_7C15, e).rotate_left((e & 31) as u32);
}

/// 生产现状冲刷：变更队列 take 后容量随处理结束丢弃；每 tick 新建
/// 分组 `HashMap` 与每节 `Vec`；事件队列同样 take 丢容量
fn flush_fresh(tick: usize, fp: &mut u64) {
    // 生产者：队列容量每 tick 从零逐级重分配
    let mut queue: Vec<Change> = Vec::new();
    for i in 0..FLUSH_CHANGES {
        queue.push(change(tick, i));
    }
    let changes = std::mem::take(&mut queue);

    let mut by_section: HashMap<u64, Vec<Change>> = HashMap::new();
    for c in changes {
        by_section.entry(section_key(&c)).or_default().push(c);
    }
    for (section, updates) in by_section {
        deliver_section(fp, section, &updates);
    }

    let mut events: Vec<u64> = Vec::new();
    for i in 0..FLUSH_EVENTS {
        events.push(((tick as u64) << 32) | i as u64);
    }
    for e in std::mem::take(&mut events) {
        deliver_event(fp, e);
    }
}

/// 冲刷驻留缓冲（轮次 18 收编形态）：变更/事件交换缓冲 + 分组图驻留
#[derive(Default)]
struct FlushScratch {
    changes: Vec<Change>,
    by_section: HashMap<u64, Vec<Change>>,
    events: Vec<u64>,
}

impl FlushScratch {
    /// 收编形态冲刷：`swap` 接替 `mem::take`（队列容量跨 tick 保留），
    /// 分组图清值留桶、重填零分配
    fn flush(&mut self, queue: &mut Vec<Change>, event_queue: &mut Vec<u64>, fp: &mut u64) {
        self.changes.clear();
        std::mem::swap(queue, &mut self.changes);

        for updates in self.by_section.values_mut() {
            updates.clear();
        }
        for c in self.changes.iter().copied() {
            self.by_section.entry(section_key(&c)).or_default().push(c);
        }
        for (section, updates) in &self.by_section {
            if updates.is_empty() {
                continue;
            }
            deliver_section(fp, *section, updates);
        }

        self.events.clear();
        std::mem::swap(event_queue, &mut self.events);
        for e in self.events.iter().copied() {
            deliver_event(fp, e);
        }
    }
}

// ---------------------------------------------------------------------------
// 每 tick 模拟
// ---------------------------------------------------------------------------

fn per_tick_fresh(tick: usize) -> u64 {
    let mut fp = 0xcbf2_9ce4_8422_2325u64;
    for b in 0..BROADCASTS_PER_TICK {
        broadcast_fresh(b, tick, &mut fp);
    }
    flush_fresh(tick, &mut fp);
    fp
}

fn per_tick_reuse(
    tick: usize,
    scratch: &mut FlushScratch,
    queues: &mut (Vec<Change>, Vec<u64>),
) -> u64 {
    let mut fp = 0xcbf2_9ce4_8422_2325u64;
    for b in 0..BROADCASTS_PER_TICK {
        broadcast_single_pass(b, tick, &mut fp);
    }
    for i in 0..FLUSH_CHANGES {
        queues.0.push(change(tick, i));
    }
    for i in 0..FLUSH_EVENTS {
        queues.1.push(((tick as u64) << 32) | i as u64);
    }
    scratch.flush(&mut queues.0, &mut queues.1, &mut fp);
    fp
}

// ---------------------------------------------------------------------------
// 主流程
// ---------------------------------------------------------------------------

struct Counts {
    allocs: usize,
    alloc_bytes: usize,
}

fn snapshot() -> Counts {
    let (allocs, alloc_bytes) = counters();
    Counts {
        allocs,
        alloc_bytes,
    }
}

fn main() {
    // 预热：两形态各跑 3 tick 让驻留容量到位，再进入稳态计量
    let mut warm_fp = 0u64;
    let mut scratch = FlushScratch::default();
    let mut queues: (Vec<Change>, Vec<u64>) = (Vec::new(), Vec::new());
    for tick in 0..3 {
        warm_fp ^= per_tick_fresh(tick);
        warm_fp ^= per_tick_reuse(tick, &mut scratch, &mut queues);
    }
    std::hint::black_box(warm_fp);

    let before_fresh = snapshot();
    let mut fp_fresh = 0u64;
    for tick in 0..TICKS {
        fp_fresh = fp_mix(fp_fresh, per_tick_fresh(tick));
    }
    let after_fresh = snapshot();
    let fresh = Counts {
        allocs: after_fresh.allocs - before_fresh.allocs,
        alloc_bytes: after_fresh.alloc_bytes - before_fresh.alloc_bytes,
    };

    let before_reuse = snapshot();
    let mut fp_reuse = 0u64;
    for tick in 0..TICKS {
        fp_reuse = fp_mix(fp_reuse, per_tick_reuse(tick, &mut scratch, &mut queues));
    }
    let after_reuse = snapshot();
    let reuse = Counts {
        allocs: after_reuse.allocs - before_reuse.allocs,
        alloc_bytes: after_reuse.alloc_bytes - before_reuse.alloc_bytes,
    };

    let content_equal = fp_fresh == fp_reuse;
    let allocs_reduced = reuse.allocs < fresh.allocs;
    let bytes_reduced = reuse.alloc_bytes < fresh.alloc_bytes;
    let all_pass = content_equal && allocs_reduced && bytes_reduced;

    println!(
        "逐调用新建：分配 {} 次 / {} B（每 tick {:.1} 次 / {:.0} B）",
        fresh.allocs,
        fresh.alloc_bytes,
        fresh.allocs as f64 / TICKS as f64,
        fresh.alloc_bytes as f64 / TICKS as f64
    );
    println!(
        "单趟+驻留：分配 {} 次 / {} B（每 tick {:.2} 次 / {:.0} B）",
        reuse.allocs,
        reuse.alloc_bytes,
        reuse.allocs as f64 / TICKS as f64,
        reuse.alloc_bytes as f64 / TICKS as f64
    );
    eprintln!(
        "内容门={content_equal} 分配 -{} 次 / -{} B",
        fresh.allocs.saturating_sub(reuse.allocs),
        fresh.alloc_bytes.saturating_sub(reuse.alloc_bytes),
    );

    let report = serde_json::json!({
        "round": 18,
        "topic": "广播扇出单趟化（栈内联版本槽）+ 方块冲刷驻留交换（swap 接替 mem::take + 分组图驻留）",
        "workload": {
            "ticks": TICKS,
            "broadcasts_per_tick": BROADCASTS_PER_TICK,
            "recipients_per_broadcast": RECIPIENTS,
            "flush_changes_per_tick": FLUSH_CHANGES,
            "flush_sections": FLUSH_SECTIONS,
            "flush_events_per_tick": FLUSH_EVENTS,
        },
        "per_call_fresh": { "allocs": fresh.allocs, "bytes": fresh.alloc_bytes },
        "reused_buffers": { "allocs": reuse.allocs, "bytes": reuse.alloc_bytes },
        "per_tick": {
            "fresh_allocs": fresh.allocs as f64 / TICKS as f64,
            "fresh_bytes": fresh.alloc_bytes as f64 / TICKS as f64,
            "reused_allocs": reuse.allocs as f64 / TICKS as f64,
            "reused_bytes": reuse.alloc_bytes as f64 / TICKS as f64,
        },
        "gates": {
            "content_equal": content_equal,
            "allocations_reduced": allocs_reduced,
            "bytes_reduced": bytes_reduced,
            "all_pass": all_pass,
        },
    });

    let path = "note/report/perf/round18-broadcast-flush-reuse.json";
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
    println!("报告已写入 {path}");
    if !all_pass {
        eprintln!("硬闸门未通过");
        std::process::exit(2);
    }
}
