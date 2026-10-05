//! 轮次 19 基准：区块批次发送缓冲与脏实体元数据版本集逐调用
//! 分配流失的量化与收编对照。
//!
//! ## 侦察结论（生产可达性已验证）
//!
//! **区块批次发送**（`ChunkSender`，每玩家移动突发期每 tick 一批）：
//! 每批次新建 ① 候选 `Vec<PreparedChunk>`（`collect_sorted_candidates`
//! 带容量新建）；② 小 pending 排序 `Vec`（pending ≤ 16 时）；③ 并行
//! 编码结果 `Vec`（rayon collect）；④ 输出 `Vec::with_capacity`；
//! ⑤ 已派发位置 `Vec::with_capacity`（`commit_batch` 返回）。玩家
//! 传送/加入/跨图时区块流式发送持续数十 tick，正常行走跨区块边界
//! 亦逐 tick 触发。
//!
//! **脏实体元数据**（`send_dirty_entity_data`，每脏实体每 tick）：
//! 每次调用新建 ① 收件人 `Vec<&Player>`；② `BTreeMap<版本, Vec>`
//! 分组 + 每版本 Vec；③ 版本列表 `Vec`（供 `pack_dirty_for_versions`
//! 取版本集）。水下生物/玩家空气值逐 tick 变脏、潜行/游泳/着火
//! 等状态切换均走此路径。
//!
//! 收编形态：① 批次五缓冲驻留玩家侧暂存（准备/编码/提交全流程
//! 重填，rayon `collect_into_vec` 复用容量）；② 版本集改栈内联槽
//! 两趟过滤（收件人 Vec/BTreeMap/版本 Vec 全消）。本基准量化
//! 两主题分配账，并以投递指纹作语义等价硬闸门。
//!
//! 硬闸门：两形态投递内容（编码块、派发位置、元数据字节与
//! 收件人）指纹全等；复用形态稳态分配严格小于逐调用新建。
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::BTreeMap;
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
/// 流式发送中的玩家数（传送/加入/移动突发，每玩家每 tick 一批）
const STREAMING_PLAYERS: usize = 200;
/// 每批次区块数（自适应配额常态 8-24）
const MAX_BATCH_CHUNKS: usize = 24;
/// 小 pending 批次占比（触发排序 Vec 分支）
const SMALL_PENDING_MODULO: usize = 3;
/// 每 tick 脏元数据实体数（水下空气/姿态切换/着火等）
const DIRTY_ENTITIES: usize = 150;
/// 元数据广播收件人数
const META_RECIPIENTS: usize = 10;

// ---------------------------------------------------------------------------
// 公共负载模型
// ---------------------------------------------------------------------------

/// 混合指纹：fnv 式折叠
const fn fp_mix(fp: u64, v: u64) -> u64 {
    (fp ^ v).wrapping_mul(0x0000_0100_0000_01B3)
}

// ---------------------------------------------------------------------------
// 主题一：区块批次发送缓冲
// ---------------------------------------------------------------------------

/// 候选区块占位：位置 + 内容令牌
#[derive(Clone, Copy)]
struct PreparedChunk {
    pos: u64,
    token: u64,
}

/// 编码块占位：位置 + 负载（语义字节，两形态同源）
struct EncodedChunk {
    pos: u64,
    payload: Vec<u8>,
}

/// 确定性生成批次规模（8-24）
const fn batch_len(player: usize, tick: usize) -> usize {
    8 + (player * 5 + tick * 3) % (MAX_BATCH_CHUNKS - 7)
}

const fn prepared(player: usize, tick: usize, i: usize) -> PreparedChunk {
    let pos = (player as u64)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add((tick as u64) << 8)
        .wrapping_add(i as u64);
    PreparedChunk {
        pos,
        token: pos ^ 0x0005_DEEC_E66D,
    }
}

/// 语义负载：每区块一份编码字节（两形态同源，对账抵消）
fn encode_payload(c: &PreparedChunk) -> Vec<u8> {
    let len = 32 + (c.pos % 96) as usize;
    let mut payload = vec![0u8; len];
    payload[0] = u8::try_from(c.pos & 0xFF).unwrap_or_default();
    payload[1] = u8::try_from(c.token & 0xFF).unwrap_or_default();
    payload
}

/// 生产现状批次：候选/排序/编码结果/输出/派发五 Vec 逐批新建
fn batch_fresh(player: usize, tick: usize, fp: &mut u64) {
    let quota = batch_len(player, tick);
    // ① 候选 Vec（带容量新建）
    let mut ready: Vec<PreparedChunk> = Vec::with_capacity(quota);
    // ② 小 pending 排序 Vec（每 SMALL_PENDING_MODULO 批一次）
    if (player + tick).is_multiple_of(SMALL_PENDING_MODULO) {
        let mut sorted: Vec<u64> = (0..quota).map(|i| prepared(player, tick, i).pos).collect();
        sorted.sort_unstable();
        for pos in sorted {
            ready.push(PreparedChunk {
                pos,
                token: pos ^ 0x0005_DEEC_E66D,
            });
        }
    } else {
        for i in 0..quota {
            ready.push(prepared(player, tick, i));
        }
    }

    // ③ 并行编码结果 Vec（生产为 rayon collect，分配账相同）
    let encoded_results: Vec<Option<EncodedChunk>> = ready
        .iter()
        .map(|c| {
            Some(EncodedChunk {
                pos: c.pos,
                payload: encode_payload(c),
            })
        })
        .collect();
    // ④ 输出 Vec::with_capacity
    let mut output: Vec<EncodedChunk> = Vec::with_capacity(encoded_results.len());
    for encoded in encoded_results.into_iter().flatten() {
        *fp = fp_mix(*fp, encoded.pos ^ (encoded.payload.len() as u64));
        output.push(encoded);
    }

    // ⑤ 已派发位置 Vec::with_capacity（返回供实体配对）
    let mut dispatched: Vec<u64> = Vec::with_capacity(output.len());
    for chunk in &output {
        dispatched.push(chunk.pos);
    }
    for pos in dispatched {
        *fp = fp_mix(*fp, pos);
    }
}

/// 批次发送驻留暂存（轮次 19 收编形态）：五缓冲全驻留
#[derive(Default)]
struct BatchScratch {
    ready: Vec<PreparedChunk>,
    sorted: Vec<u64>,
    encoded_results: Vec<Option<EncodedChunk>>,
    output: Vec<EncodedChunk>,
    dispatched: Vec<u64>,
}

impl BatchScratch {
    /// 收编形态批次：全流程重填，稳态零结构分配
    fn batch(&mut self, player: usize, tick: usize, fp: &mut u64) {
        let quota = batch_len(player, tick);
        self.ready.clear();
        if (player + tick).is_multiple_of(SMALL_PENDING_MODULO) {
            self.sorted.clear();
            self.sorted
                .extend((0..quota).map(|i| prepared(player, tick, i).pos));
            self.sorted.sort_unstable();
            for pos in self.sorted.iter().copied() {
                self.ready.push(PreparedChunk {
                    pos,
                    token: pos ^ 0x0005_DEEC_E66D,
                });
            }
        } else {
            self.ready
                .extend((0..quota).map(|i| prepared(player, tick, i)));
        }

        // 生产收编形态为 rayon collect_into_vec 复用容量；分配账相同
        self.encoded_results.clear();
        self.encoded_results.extend(self.ready.iter().map(|c| {
            Some(EncodedChunk {
                pos: c.pos,
                payload: encode_payload(c),
            })
        }));
        self.output.clear();
        for encoded in self.encoded_results.drain(..).flatten() {
            *fp = fp_mix(*fp, encoded.pos ^ (encoded.payload.len() as u64));
            self.output.push(encoded);
        }

        self.dispatched.clear();
        self.dispatched.extend(self.output.iter().map(|c| c.pos));
        for pos in self.dispatched.iter().copied() {
            *fp = fp_mix(*fp, pos);
        }
    }
}

// ---------------------------------------------------------------------------
// 主题二：脏实体元数据版本集
// ---------------------------------------------------------------------------

/// 收件人占位：客户端令牌 + 协议版本（10% 双版本混合）
#[derive(Clone, Copy)]
struct MetaRecipient {
    token: u64,
    version: u8,
}

const fn meta_recipient(entity: usize, tick: usize, r: usize) -> MetaRecipient {
    let token = (entity as u64)
        .wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        .wrapping_add((tick as u64) << 16)
        .wrapping_add(r as u64);
    let version = if entity.is_multiple_of(10) && r >= META_RECIPIENTS / 2 {
        2
    } else {
        1
    };
    MetaRecipient { token, version }
}

/// 语义负载：每（实体, 版本）一份脏项打包字节（两形态同源）
fn pack_payload(entity: usize, tick: usize, version: u8) -> Vec<u8> {
    let len = 8 + (entity * 3 + tick + usize::from(version)) % 24;
    let mut payload = vec![0u8; len];
    payload[0] = version;
    payload
}

fn deliver_meta(fp: &mut u64, r: &MetaRecipient, payload: &[u8]) {
    let mut h = r.token ^ u64::from(r.version) ^ ((payload.len() as u64) << 32);
    for (i, b) in payload.iter().enumerate() {
        h = fp_mix(h, u64::from(*b).wrapping_add(i as u64));
    }
    *fp ^= h.rotate_left((r.token & 31) as u32);
}

/// 生产现状：收件人 `Vec` + `BTreeMap` 分组 + 版本 `Vec` 逐调用新建
fn meta_fresh(entity: usize, tick: usize, fp: &mut u64) {
    let mut java_recipients: Vec<MetaRecipient> = Vec::new();
    for r in 0..META_RECIPIENTS {
        java_recipients.push(meta_recipient(entity, tick, r));
    }
    if java_recipients.is_empty() {
        return;
    }

    let mut by_version: BTreeMap<u8, Vec<MetaRecipient>> = BTreeMap::new();
    for rcp in java_recipients {
        by_version.entry(rcp.version).or_default().push(rcp);
    }
    let versions: Vec<u8> = by_version.keys().copied().collect();
    for version in versions {
        let payload = pack_payload(entity, tick, version);
        if let Some(recipients) = by_version.get(&version) {
            for rcp in recipients {
                deliver_meta(fp, rcp, &payload);
            }
        }
    }
}

/// 收编形态：栈内联版本槽两趟过滤，分组结构全消
fn meta_single_pass(entity: usize, tick: usize, fp: &mut u64) {
    // 第一趟：内联槽收集不同版本
    let mut slots: [Option<u8>; 4] = [None; 4];
    let mut slot_count = 0usize;
    for r in 0..META_RECIPIENTS {
        let version = meta_recipient(entity, tick, r).version;
        if !slots[..slot_count].contains(&Some(version)) && slot_count < slots.len() {
            slots[slot_count] = Some(version);
            slot_count += 1;
        }
    }
    // 第二趟：逐版本打包并过滤投递
    for slot in &slots[..slot_count] {
        let version = slot.unwrap_or_default();
        let payload = pack_payload(entity, tick, version);
        for r in 0..META_RECIPIENTS {
            let rcp = meta_recipient(entity, tick, r);
            if rcp.version == version {
                deliver_meta(fp, &rcp, &payload);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 每 tick 模拟
// ---------------------------------------------------------------------------

fn per_tick_fresh(tick: usize) -> u64 {
    let mut fp = 0xcbf2_9ce4_8422_2325u64;
    for p in 0..STREAMING_PLAYERS {
        batch_fresh(p, tick, &mut fp);
    }
    for e in 0..DIRTY_ENTITIES {
        meta_fresh(e, tick, &mut fp);
    }
    fp
}

fn per_tick_reuse(tick: usize, scratches: &mut [BatchScratch]) -> u64 {
    let mut fp = 0xcbf2_9ce4_8422_2325u64;
    for (p, scratch) in scratches.iter_mut().enumerate() {
        scratch.batch(p, tick, &mut fp);
    }
    for e in 0..DIRTY_ENTITIES {
        meta_single_pass(e, tick, &mut fp);
    }
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
    let mut scratches: Vec<BatchScratch> = (0..STREAMING_PLAYERS)
        .map(|_| BatchScratch::default())
        .collect();
    for tick in 0..3 {
        warm_fp ^= per_tick_fresh(tick);
        warm_fp ^= per_tick_reuse(tick, &mut scratches);
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
        fp_reuse = fp_mix(fp_reuse, per_tick_reuse(tick, &mut scratches));
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
        "驻留+内联：分配 {} 次 / {} B（每 tick {:.2} 次 / {:.0} B）",
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
        "round": 19,
        "topic": "区块批次发送缓冲驻留（五缓冲重填）+ 脏实体元数据版本集栈内联",
        "workload": {
            "ticks": TICKS,
            "streaming_players": STREAMING_PLAYERS,
            "max_batch_chunks": MAX_BATCH_CHUNKS,
            "dirty_entities_per_tick": DIRTY_ENTITIES,
            "meta_recipients": META_RECIPIENTS,
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

    let path = "note/report/perf/round19-chunk-batch-meta-reuse.json";
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
