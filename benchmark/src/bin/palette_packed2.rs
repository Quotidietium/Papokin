//! 轮次 49 基准：调色板第三层——≤4 态 2 bit 索引存储。
//!
//! 对照形态（轮次 5 半字节层的直系续作）：
//! - 旧态：异构区段调色板 ≤16 态一律半字节索引（4 bit/格，
//!   16³ 区段 2048B）；2-4 态的低多样性区段（空气+洞穴贯通、
//!   水+砂砾、石头+深板岩过渡等）与 16 态区段同价；
//! - 新态：≤4 态改 2 bit 索引（每字节四格，1024B，恰为半字节
//!   层一半），5-16 态维持半字节，17+ 维持 u8 索引/稠密；
//!   升级链 Packed2 → Nibble → Indexed → Dense 单调完备。
//!
//! 负载：2400 区段按自然地形调色板尺寸分布（55% 同质、17%
//! 2-4 态、8% 5-8 态、12% 9-16 态、6% 17-64 态、2% 65-300 态）
//! 逐次变异构建 + 全量迭代折叠 + 网络三元组折叠 + 每区段
//! 64 步变异电池。
//!
//! 语义指纹：迭代值与网络序列化三元组（位宽/调色板/打包字）
//! 逐区段折叠全等（双臂共用脚本）。旧臂忠实复刻改动前语义，
//! 含同质升级的 O(VOLUME) 展平成本与 Indexed→Dense 升级链。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use papokin_data::BlockStateId;
use papokin_world::chunk::palette::{BlockPalette, NetworkPalette};

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

struct CountingAlloc;

// SAFETY: 仅统计后原样转发系统分配器，不改变任何分配语义。
unsafe impl GlobalAlloc for CountingAlloc {
    // SAFETY: 转发给系统分配器；仅在成功时记账。
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: 转发给系统分配器；仅在成功时记账。
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            LIVE_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
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

// ---------------------------------------------------------------------------
// 语料：确定性 PRNG 与变异流（与轮次 5 基准同构）
// ---------------------------------------------------------------------------

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

    const fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

const DIM: usize = 16;
const VOLUME: usize = DIM * DIM * DIM;

fn state_pool(n: usize) -> Vec<u16> {
    (0..n)
        .map(|i| BlockStateId::new_or_air(i as u16).as_u16())
        .collect()
}

fn mutation_stream(rng: &mut Rng, pool: &[u16]) -> Vec<(usize, usize, usize, u16)> {
    let mut ops = Vec::with_capacity(VOLUME + 512);
    for i in 0..VOLUME {
        let y = i / (DIM * DIM);
        let z = (i / DIM) % DIM;
        let x = i % DIM;
        let value = if i < pool.len() {
            pool[i]
        } else {
            pool[rng.below(pool.len())]
        };
        ops.push((x, y, z, value));
    }
    for _ in 0..512 {
        let x = rng.below(DIM);
        let y = rng.below(DIM);
        let z = rng.below(DIM);
        let value = if rng.below(4) == 0 {
            pool[0]
        } else {
            pool[rng.below(pool.len())]
        };
        ops.push((x, y, z, value));
    }
    ops
}

struct SectionSpec {
    palette_size: usize,
    seed: u64,
}

fn workload() -> Vec<SectionSpec> {
    // 模拟自然地形 24 区段/列的调色板尺寸分布；2-4 态带单列
    // （洞穴贯通的空气段、水体、岩层过渡），是本轮的受益子集。
    let mut specs = Vec::new();
    let mut rng = Rng(0x5EED_5EED_5EED_5EED);
    for i in 0..2400u64 {
        let roll = rng.below(100);
        let palette_size = if roll < 55 {
            1
        } else if roll < 72 {
            2 + rng.below(3) // 2-4：本轮受益带
        } else if roll < 80 {
            5 + rng.below(4) // 5-8
        } else if roll < 92 {
            9 + rng.below(8) // 9-16
        } else if roll < 98 {
            17 + rng.below(48) // 17-64
        } else {
            65 + rng.below(236) // 65-300
        };
        specs.push(SectionSpec {
            palette_size,
            seed: 0xB10C_0000 + i,
        });
    }
    specs
}

// ---------------------------------------------------------------------------
// 旧态复刻（改动前语义：Homogeneous / Nibble ≤16 / Indexed ≤256 / Dense）
// ---------------------------------------------------------------------------

enum LegacyStorage {
    Dense(Box<[[[u16; DIM]; DIM]; DIM]>),
    Indexed(Box<[[[u8; DIM]; DIM]; DIM]>),
    Nibble(Box<[u8]>),
}

struct LegacyHetero {
    storage: LegacyStorage,
    palette: Vec<u16>,
    counts: Vec<u16>,
}

enum LegacyPalette {
    Homogeneous(u16),
    Heterogeneous(Box<LegacyHetero>),
}

impl LegacyHetero {
    const fn linear(x: usize, y: usize, z: usize) -> usize {
        (y * DIM + z) * DIM + x
    }

    fn index_at(&self, linear: usize) -> usize {
        let y = linear / (DIM * DIM);
        let z = (linear / DIM) % DIM;
        let x = linear % DIM;
        match &self.storage {
            LegacyStorage::Dense(_) => 0,
            LegacyStorage::Indexed(indices) => indices[y][z][x] as usize,
            LegacyStorage::Nibble(buf) => {
                let byte = buf[linear / 2];
                if linear.is_multiple_of(2) {
                    (byte & 0x0F) as usize
                } else {
                    (byte >> 4) as usize
                }
            }
        }
    }

    fn get(&self, x: usize, y: usize, z: usize) -> u16 {
        match &self.storage {
            LegacyStorage::Dense(cube) => cube[y][z][x],
            _ => self.palette[self.index_at(Self::linear(x, y, z))],
        }
    }

    /// 写入新调色板索引，越层时按生产升级链升级存储。
    fn write_or_upgrade(&mut self, x: usize, y: usize, z: usize, value: u16, new_index: usize) {
        let linear = Self::linear(x, y, z);
        match &mut self.storage {
            LegacyStorage::Dense(cube) => cube[y][z][x] = value,
            LegacyStorage::Indexed(indices) => {
                if new_index <= 255 {
                    indices[y][z][x] = new_index as u8;
                } else {
                    // 复刻生产：调色板超 256 态升稠密值存储。
                    let mut cube = Box::new([[[0u16; DIM]; DIM]; DIM]);
                    for y2 in 0..DIM {
                        for z2 in 0..DIM {
                            for x2 in 0..DIM {
                                cube[y2][z2][x2] = self.palette[indices[y2][z2][x2] as usize];
                            }
                        }
                    }
                    cube[y][z][x] = value;
                    self.storage = LegacyStorage::Dense(cube);
                }
            }
            LegacyStorage::Nibble(buf) => {
                if new_index <= 15 {
                    nibble_write(buf, linear, new_index as u8);
                } else {
                    // 复刻生产：调色板超 16 态升 u8 索引存储。
                    let mut indices = Box::new([[[0u8; DIM]; DIM]; DIM]);
                    for i in 0..VOLUME {
                        let byte = buf[i / 2];
                        let idx = if i.is_multiple_of(2) {
                            byte & 0x0F
                        } else {
                            byte >> 4
                        };
                        indices[i / (DIM * DIM)][(i / DIM) % DIM][i % DIM] = idx;
                    }
                    indices[y][z][x] = new_index as u8;
                    self.storage = LegacyStorage::Indexed(indices);
                }
            }
        }
    }
}

/// 忠实复刻生产 `from_cube`：全量展平迭代做调色板去重与计数
/// （O(VOLUME)，与生产同价），按首现序填充半字节索引（旧态
/// 2 态落 Nibble 层）。
fn legacy_hetero_from_homogeneous(
    original: u16,
    value: u16,
    (x, y, z): (usize, usize, usize),
) -> LegacyPalette {
    let mut palette: Vec<u16> = Vec::new();
    let mut counts: Vec<u16> = Vec::new();
    let mut buf = vec![0u8; VOLUME / 2].into_boxed_slice();
    let target = LegacyHetero::linear(x, y, z);
    for i in 0..VOLUME {
        let v = if i == target { value } else { original };
        if let Some(idx) = palette.iter().position(|p| *p == v) {
            counts[idx] += 1;
            nibble_write(&mut buf, i, idx as u8);
        } else {
            palette.push(v);
            counts.push(1);
            nibble_write(&mut buf, i, (palette.len() - 1) as u8);
        }
    }
    LegacyPalette::Heterogeneous(Box::new(LegacyHetero {
        storage: LegacyStorage::Nibble(buf),
        palette,
        counts,
    }))
}

impl LegacyPalette {
    const fn new() -> Self {
        Self::Homogeneous(0)
    }

    fn set(&mut self, x: usize, y: usize, z: usize, value: u16) {
        match self {
            Self::Homogeneous(original) => {
                let original = *original;
                if value == original {
                    return;
                }
                *self = legacy_hetero_from_homogeneous(original, value, (x, y, z));
            }
            Self::Heterogeneous(data) => {
                let original = data.get(x, y, z);
                if original == value {
                    return;
                }
                let original_index = data
                    .palette
                    .iter()
                    .position(|v| *v == original)
                    .unwrap_or(0);
                let new_index = if let Some(idx) = data.palette.iter().position(|v| *v == value) {
                    data.counts[idx] += 1;
                    idx
                } else {
                    data.palette.push(value);
                    data.counts.push(1);
                    data.palette.len() - 1
                };

                data.write_or_upgrade(x, y, z, value, new_index);

                data.counts[original_index] -= 1;
                if data.counts[original_index] == 0 {
                    let last_index = data.palette.len() - 1;
                    data.palette.swap_remove(original_index);
                    data.counts.swap_remove(original_index);
                    match &mut data.storage {
                        LegacyStorage::Dense(_) => {}
                        LegacyStorage::Indexed(indices) => {
                            for row in indices.iter_mut() {
                                for col in row.iter_mut() {
                                    for idx in col.iter_mut() {
                                        if *idx as usize == last_index {
                                            *idx = original_index as u8;
                                        }
                                    }
                                }
                            }
                        }
                        LegacyStorage::Nibble(buf) => {
                            for i in 0..VOLUME {
                                let byte = buf[i / 2];
                                let idx = if i.is_multiple_of(2) {
                                    byte & 0x0F
                                } else {
                                    byte >> 4
                                };
                                if idx as usize == last_index {
                                    nibble_write(buf, i, original_index as u8);
                                }
                            }
                        }
                    }
                }
                if data.counts.len() == 1 {
                    let only = data.palette[0];
                    *self = Self::Homogeneous(only);
                }
            }
        }
    }

    fn dump(&self) -> Vec<u16> {
        match self {
            Self::Homogeneous(v) => vec![*v; VOLUME],
            Self::Heterogeneous(data) => (0..VOLUME)
                .map(|i| {
                    let x = i % DIM;
                    let z = (i / DIM) % DIM;
                    let y = i / (DIM * DIM);
                    data.get(x, y, z)
                })
                .collect(),
        }
    }

    fn palette(&self) -> Vec<u16> {
        match self {
            Self::Homogeneous(v) => vec![*v],
            Self::Heterogeneous(data) => data.palette.clone(),
        }
    }
}

fn nibble_write(buf: &mut Box<[u8]>, linear: usize, value: u8) {
    let byte = &mut buf[linear / 2];
    if linear.is_multiple_of(2) {
        *byte = (*byte & 0xF0) | value;
    } else {
        *byte = (*byte & 0x0F) | (value << 4);
    }
}

// ---------------------------------------------------------------------------
// 双臂与指纹
// ---------------------------------------------------------------------------

const FP_SEED: u64 = 0xcbf2_9ce4_8422_2325;

const fn fp_mix(fp: u64, x: u64) -> u64 {
    (fp ^ x).wrapping_mul(0x1000_0000_01b3)
}

fn fold_u16s(fp: u64, values: &[u16]) -> u64 {
    values.iter().fold(fp, |acc, v| fp_mix(acc, u64::from(*v)))
}

fn fold_i64s(fp: u64, words: &[i64]) -> u64 {
    words.iter().fold(fp, |acc, w| fp_mix(acc, *w as u64))
}

/// 与生产 `encompassing_bits` 同语义：容纳计数值所需的位数。
fn encompassing_bits_local(count: usize) -> u8 {
    if count == 1 {
        1
    } else {
        count.ilog2() as u8 + u8::from(!count.is_power_of_two())
    }
}

/// 网络三元组折叠（位宽 / 调色板值 / 打包字），对齐生产
/// `convert_network` 的三层语义（同质 Single / 间接 Indirect /
/// 越界 Direct），与存储层无关，仅由内容值与调色板内序决定。
fn fold_legacy_network(net_fp: u64, values: &[u16], palette: &[u16]) -> u64 {
    if palette.len() == 1 {
        // Single：位宽 0、无打包字。
        return fp_mix(fp_mix(net_fp, 0), u64::from(palette[0]));
    }
    let raw_bits = encompassing_bits_local(palette.len());
    if raw_bits > 8 {
        // Direct：16 bit 裸值打包，无间接调色板。
        let mut net_fp = fp_mix(net_fp, 16);
        for chunk in values.chunks(4) {
            let mut acc = 0u64;
            for (j, v) in chunk.iter().enumerate() {
                acc |= u64::from(*v) << (16 * j);
            }
            net_fp = fp_mix(net_fp, acc);
        }
        return net_fp;
    }
    // Indirect：位宽 = max(涵盖位, 4) 的调色板索引打包。
    let bits = raw_bits.max(4);
    let per_word = 64 / bits as usize;
    let mut net_fp = fp_mix(net_fp, u64::from(bits));
    net_fp = fold_u16s(net_fp, palette);
    let mut i = 0;
    while i < values.len() {
        let mut acc = 0u64;
        for j in 0..per_word {
            if i + j < values.len() {
                let idx = palette
                    .iter()
                    .position(|p| *p == values[i + j])
                    .unwrap_or(0);
                acc |= (idx as u64) << (bits as usize * j);
            }
        }
        net_fp = fp_mix(net_fp, acc);
        i += per_word;
    }
    net_fp
}

struct ArmStats {
    live_bytes: usize,
    build_ms: f64,
    transition_ms: f64,
    battery_ms: f64,
    qualifying_sections: usize,
}

fn run_legacy(specs: &[SectionSpec]) -> (u64, u64, ArmStats) {
    LIVE_BYTES.store(0, Ordering::Relaxed);
    let start = Instant::now();
    let mut sections = Vec::with_capacity(specs.len());
    for spec in specs {
        let pool = state_pool(spec.palette_size);
        let mut rng = Rng(spec.seed);
        let stream = mutation_stream(&mut rng, &pool);
        let mut section = LegacyPalette::new();
        for &(x, y, z, value) in &stream {
            section.set(x, y, z, value);
        }
        sections.push(section);
    }
    let build_ms = start.elapsed().as_secs_f64() * 1000.0;
    let live_bytes = LIVE_BYTES.load(Ordering::Relaxed);

    let mut fp: u64 = FP_SEED;
    let mut net_fp: u64 = FP_SEED;
    let mut qualifying = 0;
    for section in &sections {
        let values = section.dump();
        let palette = section.palette();
        if (2..=4).contains(&palette.len()) {
            qualifying += 1;
        }
        fp = fold_u16s(fp, &values);
        net_fp = fold_legacy_network(net_fp, &values, &palette);
    }

    // 迁移相：灌入 9 态池触发 Packed2→Nibble 等层级升级，
    // 每区段一次 O(VOLUME)，属设计内成本（如实登记，不作闸门）。
    let transition_start = Instant::now();
    let pool = state_pool(9);
    let mut rng = Rng(0xFA7E_0000_0000_0001);
    for section in &mut sections {
        for _ in 0..64 {
            let x = rng.below(DIM);
            let y = rng.below(DIM);
            let z = rng.below(DIM);
            let v = pool[rng.below(9)];
            section.set(x, y, z, v);
            fp = fp_mix(fp, u64::from(v));
        }
    }
    let transition_ms = transition_start.elapsed().as_secs_f64() * 1000.0;

    // 稳态相：层级已定，纯写路径——时间闸门量纲（4 趟稳测量）。
    let battery_start = Instant::now();
    for _pass in 0..4 {
        for section in &mut sections {
            for _ in 0..64 {
                let x = rng.below(DIM);
                let y = rng.below(DIM);
                let z = rng.below(DIM);
                let v = pool[rng.below(9)];
                section.set(x, y, z, v);
                fp = fp_mix(fp, u64::from(v));
            }
        }
    }
    let battery_ms = battery_start.elapsed().as_secs_f64() * 1000.0;

    (
        fp,
        net_fp,
        ArmStats {
            live_bytes,
            build_ms,
            transition_ms,
            battery_ms,
            qualifying_sections: qualifying,
        },
    )
}

fn run_production(specs: &[SectionSpec]) -> (u64, u64, ArmStats) {
    LIVE_BYTES.store(0, Ordering::Relaxed);
    let start = Instant::now();
    let mut sections = Vec::with_capacity(specs.len());
    for spec in specs {
        let pool = state_pool(spec.palette_size);
        let mut rng = Rng(spec.seed);
        let stream = mutation_stream(&mut rng, &pool);
        let mut section = BlockPalette::default();
        for &(x, y, z, value) in &stream {
            section.set(x, y, z, BlockStateId::new_or_air(value));
        }
        sections.push(section);
    }
    let build_ms = start.elapsed().as_secs_f64() * 1000.0;
    let live_bytes = LIVE_BYTES.load(Ordering::Relaxed);

    let mut fp: u64 = FP_SEED;
    let mut net_fp: u64 = FP_SEED;
    let mut qualifying = 0;
    for section in &sections {
        let values: Vec<u16> = section.iter().map(BlockStateId::as_u16).collect();
        let ser = section.convert_network();
        match &ser.palette {
            NetworkPalette::Single(v) => {
                net_fp = fp_mix(net_fp, u64::from(ser.bits_per_entry));
                net_fp = fp_mix(net_fp, u64::from(*v));
            }
            NetworkPalette::Indirect(palette) => {
                if (2..=4).contains(&palette.len()) {
                    qualifying += 1;
                }
                net_fp = fp_mix(net_fp, u64::from(ser.bits_per_entry));
                net_fp = fold_u16s(net_fp, palette);
                net_fp = fold_i64s(net_fp, &ser.packed_data);
            }
            NetworkPalette::Direct => {
                net_fp = fp_mix(net_fp, u64::from(ser.bits_per_entry));
                net_fp = fold_i64s(net_fp, &ser.packed_data);
            }
        }
        fp = fold_u16s(fp, &values);
    }

    // 迁移相：同旧臂脚本（含 Packed2→Nibble 一次性升级成本）。
    let transition_start = Instant::now();
    let pool: Vec<BlockStateId> = state_pool(9)
        .iter()
        .map(|&v| BlockStateId::new_or_air(v))
        .collect();
    let mut rng = Rng(0xFA7E_0000_0000_0001);
    for section in &mut sections {
        for _ in 0..64 {
            let x = rng.below(DIM);
            let y = rng.below(DIM);
            let z = rng.below(DIM);
            let v = pool[rng.below(9)];
            section.set(x, y, z, v);
            fp = fp_mix(fp, u64::from(v.as_u16()));
        }
    }
    let transition_ms = transition_start.elapsed().as_secs_f64() * 1000.0;

    // 稳态相：同旧臂脚本，纯写路径（4 趟稳测量）。
    let battery_start = Instant::now();
    for _pass in 0..4 {
        for section in &mut sections {
            for _ in 0..64 {
                let x = rng.below(DIM);
                let y = rng.below(DIM);
                let z = rng.below(DIM);
                let v = pool[rng.below(9)];
                section.set(x, y, z, v);
                fp = fp_mix(fp, u64::from(v.as_u16()));
            }
        }
    }
    let battery_ms = battery_start.elapsed().as_secs_f64() * 1000.0;

    (
        fp,
        net_fp,
        ArmStats {
            live_bytes,
            build_ms,
            transition_ms,
            battery_ms,
            qualifying_sections: qualifying,
        },
    )
}

// ---------------------------------------------------------------------------
// 闸门与报告
// ---------------------------------------------------------------------------

fn main() {
    println!("轮次 49 基准：调色板 ≤4 态 2 bit 索引层（轮 5 直系续作）");
    let specs = workload();
    println!(
        "语料：{} 区段（2-4 态带 17%）+ 每区段 64 步变异电池",
        specs.len()
    );
    println!();

    let (old_fp, old_net, old) = run_legacy(&specs);
    let (new_fp, new_net, new) = run_production(&specs);
    let v = evaluate(&old, &new, old_fp, new_fp, old_net, new_net);
    print_gates(&v, &old, &new);
    write_report(&v, &old, &new);
}

struct Verdict {
    semantics_ok: bool,
    network_ok: bool,
    resident_ok: bool,
    battery_ok: bool,
    battery_ratio: f64,
    all_passed: bool,
}

fn evaluate(
    old: &ArmStats,
    new: &ArmStats,
    old_fp: u64,
    new_fp: u64,
    old_net: u64,
    new_net: u64,
) -> Verdict {
    let semantics_ok = old_fp == new_fp;
    let network_ok = old_net == new_net;
    let resident_ok = new.live_bytes < old.live_bytes;
    let battery_ratio = old.battery_ms / new.battery_ms;
    let battery_ok = battery_ratio >= 0.91;
    Verdict {
        semantics_ok,
        network_ok,
        resident_ok,
        battery_ok,
        battery_ratio,
        all_passed: semantics_ok && network_ok && resident_ok && battery_ok,
    }
}

fn print_gates(v: &Verdict, old: &ArmStats, new: &ArmStats) {
    println!("=== 闸门 ===");
    println!(
        "{} 全量迭代+变异指纹全等（主语义闸门）",
        mark(v.semantics_ok)
    );
    println!(
        "{} 网络三元组指纹全等（位宽/调色板/打包字）",
        mark(v.network_ok)
    );
    println!(
        "{} 存活字节下降（主判据）：{} → {} B（每区段 {:.1} → {:.1} B）",
        mark(v.resident_ok),
        old.live_bytes,
        new.live_bytes,
        old.live_bytes as f64 / 2400.0,
        new.live_bytes as f64 / 2400.0
    );
    println!(
        "{} 稳态写电池不劣化（≥0.91×）：{:.2} → {:.2} ms（×{:.3}）",
        mark(v.battery_ok),
        old.battery_ms,
        new.battery_ms,
        v.battery_ratio
    );
    println!(
        "（迁移相如实登记，非闸门：{:.2} → {:.2} ms，含 Packed2→Nibble 一次性升级）",
        old.transition_ms, new.transition_ms
    );
    println!();
    println!("=== 数值 ===");
    println!(
        "构建：{:.2} → {:.2} ms（×{:.2}）",
        old.build_ms,
        new.build_ms,
        old.build_ms / new.build_ms
    );
    println!(
        "2-4 态区段数（受益子集，旧/新口径一致）：{} → {}",
        old.qualifying_sections, new.qualifying_sections
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

fn write_report(v: &Verdict, old: &ArmStats, new: &ArmStats) {
    use std::io::Write as _;

    let report = serde_json::json!({
        "round": 49,
        "topic": "palette_packed2",
        "corpus": {
            "sections": 2400,
            "distribution": {"homogeneous": "55%", "2-4": "17%", "5-8": "8%",
                "9-16": "12%", "17-64": "6%", "65-300": "2%"},
            "battery_ops_per_section": 64,
        },
        "gates": [
            {"name": "全量迭代+变异指纹全等", "passed": v.semantics_ok},
            {"name": "网络三元组指纹全等", "passed": v.network_ok},
            {"name": "存活字节下降（主判据）", "passed": v.resident_ok,
             "detail": format!("{} → {} B", old.live_bytes, new.live_bytes)},
            {"name": "稳态写电池不劣化（≥0.91×）", "passed": v.battery_ok,
             "detail": format!("×{:.3}", v.battery_ratio)},
            {"name": "迁移相登记（非闸门，设计内成本）", "passed": true,
             "detail": format!("{:.2} → {:.2} ms（Packed2→Nibble 一次性升级）",
                 old.transition_ms, new.transition_ms)},
        ],
        "old": {
            "live_bytes": old.live_bytes,
            "build_ms": old.build_ms,
            "transition_ms": old.transition_ms,
            "battery_ms": old.battery_ms,
            "qualifying_sections": old.qualifying_sections,
        },
        "new": {
            "live_bytes": new.live_bytes,
            "build_ms": new.build_ms,
            "transition_ms": new.transition_ms,
            "battery_ms": new.battery_ms,
            "qualifying_sections": new.qualifying_sections,
        },
        "sizes": {
            "nibble_buffer_bytes": VOLUME / 2,
            "packed2_buffer_bytes": VOLUME / 4,
        },
        "outcome": if v.all_passed { "positive" } else { "negative" },
    });

    let path = std::path::Path::new("note/report/perf/round49-palette-packed2.json");
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
