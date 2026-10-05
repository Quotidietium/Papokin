//! 轮次 24 基准：区块保留字段的序列化驻留（parsed → blob）。
//!
//! 对照形态（`PreservedChunkData` 的驻留表示）：
//! - 解析驻留（现状）：加载时把外来字段 `clone` 成 `NbtCompound`
//!   常驻内存（HashMap + Box<str> + 逐标签枚举堆开销），落盘时
//!   深克隆并入根；
//! - blob 驻留（收编）：加载时多走一步序列化，仅保留原始 NBT
//!   字节（精确贴合的 boxed slice，每区块一份连续缓冲），落盘时
//!   解析回复合标签再并入根。语义等价由「解析输出 canonical
//!   指纹全等」闸门证明。
//!
//! 负载模型（合成 vanilla 导入世界语料，2000 区块）：
//! - `structures`：starts 空 + References 8 类结构 × 0-20 长整数组；
//! - `LastUpdate` 长整；未建模高度图 2 组 × 37 长整；
//! - 30% 区块带 `blending_data`；20% 带外来 mod 小字段。
//!
//! 观测：语料驻留（分配器 分配-释放 净额）、落盘路径耗时
//! （克隆+序列化 vs 解析+序列化）、全程分配次数、canonical
//! 输出指纹全等。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use papokin_nbt::compound::NbtCompound;
use papokin_nbt::tag::NbtTag;

// ---------------------------------------------------------------------------
// 分配计数器（次数 + 字节）
// ---------------------------------------------------------------------------

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);
static FREED_BYTES: AtomicUsize = AtomicUsize::new(0);

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
        FREED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: 转发给系统分配器。
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// （分配次数， 净驻留字节 = 累计分配 - 累计释放）
fn counters() -> (usize, usize) {
    (
        ALLOCS.load(Ordering::Relaxed),
        ALLOC_BYTES.load(Ordering::Relaxed) - FREED_BYTES.load(Ordering::Relaxed),
    )
}

// ---------------------------------------------------------------------------
// 语料：合成 vanilla 导入区块的外来字段
// ---------------------------------------------------------------------------

const CHUNKS: usize = 2000;

/// 简单确定性伪随机（xorshift64*），语料形状逐区块可复现
struct Rng(u64);

impl Rng {
    const fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

/// 单个区块的外来字段（形状对齐 vanilla 1.21 世界）
fn make_foreign_fields(chunk_idx: usize) -> NbtCompound {
    let mut rng = Rng(chunk_idx as u64 + 1);
    let mut root = NbtCompound::new();

    root.put_long("LastUpdate", rng.next() as i64);

    let mut structures = NbtCompound::new();
    structures.put_compound("starts", NbtCompound::new());
    let mut references = NbtCompound::new();
    for name in [
        "minecraft:village",
        "minecraft:mineshaft",
        "minecraft:fortress",
        "minecraft:stronghold",
        "minecraft:monument",
        "minecraft:mansion",
        "minecraft:pillager_outpost",
        "minecraft:trail_ruins",
    ] {
        let count = (rng.next() % 21) as usize;
        let arr: Vec<i64> = (0..count).map(|_| rng.next() as i64).collect();
        references.put(name, NbtTag::LongArray(arr));
    }
    structures.put_compound("References", references);
    root.put_compound("structures", structures);

    // 未建模高度图（Papokin 只管理 3 组，其余驻留保留字段）
    let mut heightmaps = NbtCompound::new();
    let wg: Vec<i64> = (0..37).map(|_| rng.next() as i64).collect();
    let floor: Vec<i64> = (0..37).map(|_| rng.next() as i64).collect();
    heightmaps.put("WORLD_SURFACE_WG", NbtTag::LongArray(wg));
    heightmaps.put("OCEAN_FLOOR_WG", NbtTag::LongArray(floor));
    root.put_compound("Heightmaps", heightmaps);

    if chunk_idx % 10 < 3 {
        let mut blending = NbtCompound::new();
        blending.put_int("min_section_x", chunk_idx as i32);
        blending.put_int("min_section_z", chunk_idx as i32 + 1);
        root.put_compound("blending_data", blending);
    }
    if chunk_idx.is_multiple_of(5) {
        let mut forge = NbtCompound::new();
        forge.put_string("modid", format!("example_mod_{}", chunk_idx % 7));
        forge.put_int("revision", (rng.next() % 100) as i32);
        root.put_compound("ForgeData", forge);
    }

    root
}

// ---------------------------------------------------------------------------
// canonical 指纹：复合标签键序无关、列表序敏感的递归折叠
// ---------------------------------------------------------------------------

const fn fp_mix(h: u64, v: u64) -> u64 {
    let mut x = h ^ v;
    x = x.wrapping_mul(0x0000_0100_0000_01B3);
    x.rotate_left(17)
}

fn str_fp(s: &str) -> u64 {
    let mut fp = 0xCBF2_9CE4_8422_2325u64;
    for (i, b) in s.bytes().enumerate() {
        fp = fp_mix(fp, u64::from(b) ^ ((i as u64) << 8));
    }
    fp
}

fn tag_fp(tag: &NbtTag) -> u64 {
    match tag {
        // End 只作为列表终止标记出现，不会驻留于复合标签值
        NbtTag::End => 0,
        NbtTag::Byte(v) => fp_mix(1, *v as u64),
        NbtTag::Short(v) => fp_mix(2, *v as u64),
        NbtTag::Int(v) => fp_mix(3, *v as u64),
        NbtTag::Long(v) => fp_mix(4, *v as u64),
        NbtTag::Float(v) => fp_mix(5, u64::from(v.to_bits())),
        NbtTag::Double(v) => fp_mix(6, v.to_bits()),
        NbtTag::ByteArray(arr) => arr_fp(7, arr.iter().map(|v| *v as u64)),
        NbtTag::String(s) => fp_mix(8, str_fp(s)),
        NbtTag::List(list) => {
            let mut fp = 9u64;
            for (i, item) in list.iter().enumerate() {
                fp = fp_mix(fp, tag_fp(item) ^ (i as u64));
            }
            fp
        }
        NbtTag::Compound(compound) => fp_mix(10, compound_fp(compound)),
        NbtTag::IntArray(arr) => arr_fp(11, arr.iter().map(|v| *v as u64)),
        NbtTag::LongArray(arr) => arr_fp(12, arr.iter().map(|v| *v as u64)),
    }
}

fn arr_fp(domain: u64, iter: impl Iterator<Item = u64>) -> u64 {
    let mut fp = domain;
    for (i, v) in iter.enumerate() {
        fp = fp_mix(fp, v ^ (i as u64));
    }
    fp
}

/// 复合标签 canonical 指纹：子键顺序无关（XOR 折叠）
fn compound_fp(compound: &NbtCompound) -> u64 {
    let mut fp = 0u64;
    for (key, tag) in &compound.child_tags {
        fp ^= fp_mix(str_fp(key), tag_fp(tag));
    }
    fp
}

// ---------------------------------------------------------------------------
// 两臂：驻留构建 + 落盘路径
// ---------------------------------------------------------------------------

struct ArmReport {
    retained_bytes: usize,
    save_ns: u128,
    save_allocs: usize,
    output_fp: u64,
}

/// 自有序列化字节的回读：失败即基准自身 bug，按房规以
/// match + 退出码处理而非 expect
fn parse_own_blob_or_exit(blob: &[u8]) -> NbtCompound {
    let mut cursor = std::io::Cursor::new(blob);
    let mut reader = papokin_nbt::deserializer::NbtReadHelperJava::new(
        papokin_nbt::deserializer::NbtStreamReader(&mut cursor),
    );
    match papokin_nbt::Nbt::read_unnamed(&mut reader) {
        Ok(nbt) => nbt.root_tag,
        Err(err) => {
            eprintln!("自有序列化回读失败（基准内部不变式破坏）: {err}");
            std::process::exit(1);
        }
    }
}

/// 序列化 → 重新解析 → canonical 指纹（模拟落盘并验证输出）
fn serialize_and_fp(compound: &NbtCompound) -> u64 {
    let bytes = papokin_nbt::Nbt::from(compound.clone()).write_unnamed();
    compound_fp(&parse_own_blob_or_exit(bytes.as_ref()))
}

fn run_arm_parsed(fields: &[NbtCompound]) -> ArmReport {
    let (a0, b0) = counters();
    // 驻留：克隆语料（模拟加载路径的 tag.clone() 驻留）
    let resident: Vec<NbtCompound> = fields.to_vec();
    let (_, b1) = counters();

    let t0 = Instant::now();
    let mut fp = 0u64;
    for compound in &resident {
        // 落盘：深克隆并入根（生产现状）后序列化
        let mut root = compound.clone();
        root.put_int("DataVersion", 4440);
        fp = fp_mix(fp, serialize_and_fp(&root));
    }
    let save_ns = t0.elapsed().as_nanos();
    let (a2, _) = counters();

    ArmReport {
        retained_bytes: b1 - b0,
        save_ns,
        save_allocs: a2 - a0,
        output_fp: fp,
    }
}

fn run_arm_blob(fields: &[NbtCompound]) -> ArmReport {
    let (a0, b0) = counters();
    // 驻留：加载时多走一步序列化，仅留字节（生产收编形态）
    // 驻留：加载时多走一步序列化，仅留字节（生产收编形态）。
    // 精确贴合：`write_unnamed` 的 Vec 增长摊余最多 2× 冗余，
    // 先转 boxed slice 压实再驻留（否则量到的是摊余而非字节本体）
    let resident: Vec<Box<[u8]>> = fields
        .iter()
        .map(|compound| {
            papokin_nbt::Nbt::from(compound.clone())
                .write_unnamed()
                .to_vec()
                .into_boxed_slice()
        })
        .collect();
    let (_, b1) = counters();

    let t0 = Instant::now();
    let mut fp = 0u64;
    for blob in &resident {
        // 落盘：解析回复合标签并入根（生产收编形态）后序列化
        let mut root = parse_own_blob_or_exit(blob);
        root.put_int("DataVersion", 4440);
        fp = fp_mix(fp, serialize_and_fp(&root));
    }
    let save_ns = t0.elapsed().as_nanos();
    let (a2, _) = counters();

    ArmReport {
        retained_bytes: b1 - b0,
        save_ns,
        save_allocs: a2 - a0,
        output_fp: fp,
    }
}

// ---------------------------------------------------------------------------
// 闸门与报告
// ---------------------------------------------------------------------------

fn evaluate_gates(parsed: &ArmReport, blob: &ArmReport) -> Vec<(&'static str, bool, String)> {
    let mut gates: Vec<(&str, bool, String)> = Vec::new();

    // 闸门 1：落盘输出 canonical 指纹全等（语义等价硬证明）
    gates.push((
        "输出指纹全等",
        parsed.output_fp == blob.output_fp && blob.output_fp != 0,
        format!(
            "解析驻留 {:#018x} vs blob 驻留 {:#018x}",
            parsed.output_fp, blob.output_fp
        ),
    ));

    // 闸门 2：驻留字节 ≥3× 压缩
    gates.push((
        "驻留 ≥3× 压缩",
        blob.retained_bytes * 3 <= parsed.retained_bytes,
        format!(
            "解析驻留 {} B vs blob {} B（{:.1}×）",
            parsed.retained_bytes,
            blob.retained_bytes,
            parsed.retained_bytes as f64 / blob.retained_bytes.max(1) as f64
        ),
    ));

    // 闸门 3：落盘路径耗时 blob 臂 ≤ 解析臂 3×（克隆换解析，
    // 实测常常更快；3× 为反退化护栏）
    gates.push((
        "落盘耗时 ≤3×",
        blob.save_ns <= parsed.save_ns.saturating_mul(3).max(1),
        format!(
            "解析臂 {:.2} ms vs blob 臂 {:.2} ms（{:.2}×）",
            parsed.save_ns as f64 / 1e6,
            blob.save_ns as f64 / 1e6,
            blob.save_ns as f64 / parsed.save_ns.max(1) as f64
        ),
    ));

    // 闸门 4：落盘路径分配次数 blob 臂 ≤ 解析臂 2×
    gates.push((
        "落盘分配 ≤2×",
        blob.save_allocs <= parsed.save_allocs.saturating_mul(2),
        format!(
            "解析臂 {} 次 vs blob 臂 {} 次",
            parsed.save_allocs, blob.save_allocs
        ),
    ));

    gates
}

fn main() {
    let fields: Vec<NbtCompound> = (0..CHUNKS).map(make_foreign_fields).collect();

    let parsed = run_arm_parsed(&fields);
    let blob = run_arm_blob(&fields);
    let gates = evaluate_gates(&parsed, &blob);

    let pass = gates.iter().all(|(_, ok, _)| *ok);

    let report = serde_json::json!({
        "round": 24,
        "theme": "区块保留字段序列化驻留（parsed → blob）",
        "workload": {
            "chunks": CHUNKS,
            "corpus": "合成 vanilla 导入：structures/LastUpdate/未建模高度图/blending_data/mod 字段",
        },
        "parsed": {
            "retained_bytes": parsed.retained_bytes,
            "save_ns": parsed.save_ns,
            "save_allocs": parsed.save_allocs,
            "output_fingerprint": format!("{:#018x}", parsed.output_fp),
        },
        "blob": {
            "retained_bytes": blob.retained_bytes,
            "save_ns": blob.save_ns,
            "save_allocs": blob.save_allocs,
            "output_fingerprint": format!("{:#018x}", blob.output_fp),
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

    let path = "note/report/perf/round24-preserved-fields-blob.json";
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

    println!("=== 轮次 24：区块保留字段序列化驻留 ===");
    println!(
        "驻留：解析 {} B vs blob {} B；落盘耗时 {:.2} ms vs {:.2} ms",
        parsed.retained_bytes,
        blob.retained_bytes,
        parsed.save_ns as f64 / 1e6,
        blob.save_ns as f64 / 1e6
    );
    for (name, ok, detail) in &gates {
        println!("[{}] {name}: {detail}", if *ok { "PASS" } else { "FAIL" });
    }
    if !pass {
        std::process::exit(2);
    }
}
