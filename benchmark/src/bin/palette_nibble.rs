//! 轮次 5：区块调色板半字节索引存储基准
//!
//! 对比旧行为（调色板 ≤256 态一律 u8 索引，4096 B/区段）与新行为
//! （≤16 态改 4 bit 半字节索引，2048 B/区段）在同一合成地形负载下的
//! 存活字节与变异耗时；两模式逐区段做「全量迭代值 + 网络序列化字节」
//! 一致性硬闸门。
//!
//! 旧行为以基准内复刻实现，语义对齐改动前
//! `papokin-world/src/chunk/palette.rs`；新行为直接使用生产类型
//! `PalettedContainer`，变异流两侧逐字节相同。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use papokin_data::BlockStateId;
use papokin_world::chunk::palette::{BlockPalette, NetworkPalette};

// ---------------------------------------------------------------------------
// 计数分配器：精确测量两批区段容器的存活字节差
// ---------------------------------------------------------------------------

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

struct CountingAlloc;

// SAFETY: 仅统计后原样转发系统分配器，不改变任何分配语义
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: 转发给系统分配器；仅在成功时记账。
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            LIVE_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        // SAFETY: 转发给系统分配器。
        unsafe { System.dealloc(ptr, layout) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: 转发给系统分配器；按新旧尺寸差记账。
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            if new_size > layout.size() {
                LIVE_BYTES.fetch_add(new_size - layout.size(), Ordering::Relaxed);
            } else {
                LIVE_BYTES.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        new_ptr
    }
}

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc;

// ---------------------------------------------------------------------------
// 确定性随机数（xorshift64*），两模式共享同一流
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

// ---------------------------------------------------------------------------
// 旧版调色板容器复刻（改动前语义：Homogeneous / Indexed u8 / Dense）
// ---------------------------------------------------------------------------

const DIM: usize = 16;
const VOLUME: usize = DIM * DIM * DIM;

enum LegacyStorage {
    Dense(Box<[[[u16; DIM]; DIM]; DIM]>),
    Indexed(Box<[[[u8; DIM]; DIM]; DIM]>),
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
    fn get(&self, x: usize, y: usize, z: usize) -> u16 {
        match &self.storage {
            LegacyStorage::Dense(cube) => cube[y][z][x],
            LegacyStorage::Indexed(indices) => self.palette[indices[y][z][x] as usize],
        }
    }

    fn set(&mut self, x: usize, y: usize, z: usize, value: u16) {
        let original = self.get(x, y, z);
        if original == value {
            return;
        }

        let original_index = self
            .palette
            .iter()
            .position(|v| *v == original)
            .unwrap_or(0);

        let new_index = if let Some(idx) = self.palette.iter().position(|v| *v == value) {
            self.counts[idx] += 1;
            idx
        } else {
            self.palette.push(value);
            self.counts.push(1);
            self.palette.len() - 1
        };

        let mut upgraded = false;
        match &mut self.storage {
            LegacyStorage::Dense(cube) => cube[y][z][x] = value,
            LegacyStorage::Indexed(indices) => {
                if new_index <= 255 {
                    indices[y][z][x] = new_index as u8;
                } else {
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
                    upgraded = true;
                }
            }
        }

        self.counts[original_index] -= 1;

        if self.counts[original_index] == 0 {
            let last_index = self.palette.len() - 1;
            self.palette.swap_remove(original_index);
            self.counts.swap_remove(original_index);

            if !upgraded && let LegacyStorage::Indexed(indices) = &mut self.storage {
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
        }
    }
}

type NetworkTriplet = (u8, Vec<u16>, Vec<i64>);

impl LegacyPalette {
    const fn new() -> Self {
        Self::Homogeneous(0)
    }

    fn set(&mut self, x: usize, y: usize, z: usize, value: u16) {
        match self {
            Self::Homogeneous(original) => {
                let original_value = *original;
                if value != original_value {
                    let mut cube = Box::new([[[original_value; DIM]; DIM]; DIM]);
                    cube[y][z][x] = value;
                    *self = Self::from_cube(&cube);
                }
            }
            Self::Heterogeneous(data) => {
                data.set(x, y, z, value);
                if data.counts.len() == 1 {
                    *self = Self::Homogeneous(data.palette[0]);
                }
            }
        }
    }

    fn from_cube(cube: &[[[u16; DIM]; DIM]; DIM]) -> Self {
        let mut palette: Vec<u16> = Vec::new();
        let mut counts: Vec<u16> = Vec::new();
        for row in cube {
            for col in row {
                for val in col {
                    if let Some(index) = palette.iter().position(|v| v == val) {
                        counts[index] += 1;
                    } else {
                        palette.push(*val);
                        counts.push(1);
                    }
                }
            }
        }

        if palette.len() == 1 {
            return Self::Homogeneous(palette[0]);
        }

        if palette.len() <= 256 {
            let mut indices = Box::new([[[0u8; DIM]; DIM]; DIM]);
            for y in 0..DIM {
                for z in 0..DIM {
                    for x in 0..DIM {
                        let idx = palette
                            .iter()
                            .position(|p| *p == cube[y][z][x])
                            .unwrap_or(0);
                        indices[y][z][x] = idx as u8;
                    }
                }
            }
            Self::Heterogeneous(Box::new(LegacyHetero {
                storage: LegacyStorage::Indexed(indices),
                palette,
                counts,
            }))
        } else {
            Self::Heterogeneous(Box::new(LegacyHetero {
                storage: LegacyStorage::Dense(Box::new(*cube)),
                palette,
                counts,
            }))
        }
    }

    fn dump(&self) -> Vec<u16> {
        let mut out = Vec::with_capacity(VOLUME);
        for y in 0..DIM {
            for z in 0..DIM {
                for x in 0..DIM {
                    out.push(match self {
                        Self::Homogeneous(v) => *v,
                        Self::Heterogeneous(data) => data.get(x, y, z),
                    });
                }
            }
        }
        out
    }

    /// 与生产 `convert_network` 相同阈值的网络序列化复刻
    /// （方块：间接调色板 4..=8 bit，超出走 16 bit 直接调色板）。
    fn network_bytes(&self) -> NetworkTriplet {
        const MIN_MAP_BITS: u8 = 4;
        const MAX_MAP_BITS: u8 = 8;
        const MAX_BITS: u8 = 16;

        match self {
            Self::Homogeneous(v) => (0, vec![*v], Vec::new()),
            Self::Heterogeneous(data) => {
                let raw_bits = encompassing_bits_local(data.counts.len());
                if raw_bits > MAX_MAP_BITS {
                    let values_per_word = 64 / MAX_BITS as usize;
                    let mut packed = Vec::with_capacity(VOLUME.div_ceil(values_per_word));
                    let mut cell = 0usize;
                    while cell < VOLUME {
                        let mut acc = 0u64;
                        for i in 0..values_per_word {
                            if cell + i < VOLUME {
                                let y = (cell + i) / (DIM * DIM);
                                let z = ((cell + i) / DIM) % DIM;
                                let x = (cell + i) % DIM;
                                let value = data.get(x, y, z);
                                acc |= u64::from(value) << (MAX_BITS as usize * i);
                            }
                        }
                        packed.push(acc as i64);
                        cell += values_per_word;
                    }
                    (MAX_BITS, Vec::new(), packed)
                } else {
                    let bits = raw_bits.max(MIN_MAP_BITS);
                    let values_per_word = 64 / bits as usize;
                    let mut packed = Vec::with_capacity(VOLUME.div_ceil(values_per_word));
                    let mut cell = 0usize;
                    while cell < VOLUME {
                        let mut acc = 0u64;
                        for i in 0..values_per_word {
                            if cell + i < VOLUME {
                                let y = (cell + i) / (DIM * DIM);
                                let z = ((cell + i) / DIM) % DIM;
                                let x = (cell + i) % DIM;
                                let value = data.get(x, y, z);
                                let index =
                                    data.palette.iter().position(|v| *v == value).unwrap_or(0);
                                acc |= (index as u64) << (bits as usize * i);
                            }
                        }
                        packed.push(acc as i64);
                        cell += values_per_word;
                    }
                    (bits, data.palette.clone(), packed)
                }
            }
        }
    }
}

const fn encompassing_bits_local(n: usize) -> u8 {
    let mut bits = 0;
    let mut capacity = 1usize;
    while capacity < n {
        capacity <<= 1;
        bits += 1;
    }
    bits
}

// ---------------------------------------------------------------------------
// 负载：合成地形区段混合
// ---------------------------------------------------------------------------

/// 单个区段的变异流：先铺底（保证池内每个状态至少出现一次），
/// 再做一轮随机编辑（含回填为 0 号状态，触发计数归零与 `swap_remove`）。
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

/// 状态池：取前 `n` 个互不相同的方块状态 id（0 起致密编号保证有效）。
fn state_pool(n: usize) -> Vec<u16> {
    (0..n)
        .map(|i| BlockStateId::new_or_air(i as u16).as_u16())
        .collect()
}

struct SectionSpec {
    palette_size: usize,
    seed: u64,
}

fn workload() -> Vec<SectionSpec> {
    // 模拟自然地形 24 区段/列的调色板尺寸分布
    let mut specs = Vec::new();
    let mut rng = Rng(0x5EED_5EED_5EED_5EED);
    for i in 0..2400u64 {
        let roll = rng.below(100);
        let palette_size = if roll < 55 {
            1
        } else if roll < 80 {
            2 + rng.below(7) // 2-8
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

fn production_network_bytes(palette: &BlockPalette) -> NetworkTriplet {
    let ser = palette.convert_network();
    let palette_values: Vec<u16> = match ser.palette {
        NetworkPalette::Single(v) => vec![v],
        NetworkPalette::Indirect(values) => values.into_vec(),
        NetworkPalette::Direct => Vec::new(),
    };
    (
        ser.bits_per_entry,
        palette_values,
        ser.packed_data.into_vec(),
    )
}

struct BatchResult {
    live_bytes: usize,
    elapsed_ms: u128,
}

/// 闸门数据落盘格式：迭代值（4096×u16 LE）、位宽（u8）、
/// 调色板（u16 长度 + u16 LE 序列）、打包数据（u32 长度 + i64 LE 序列）。
/// 落盘是为了让存活记账只覆盖区段容器本身，闸门产物不占内存。
fn write_gate<W: std::io::Write>(
    w: &mut W,
    dump: &[u16],
    network: &NetworkTriplet,
) -> std::io::Result<()> {
    for value in dump {
        w.write_all(&value.to_le_bytes())?;
    }
    w.write_all(&network.0.to_le_bytes())?;
    w.write_all(&(network.1.len() as u16).to_le_bytes())?;
    for value in &network.1 {
        w.write_all(&value.to_le_bytes())?;
    }
    w.write_all(&(network.2.len() as u32).to_le_bytes())?;
    for value in &network.2 {
        w.write_all(&value.to_le_bytes())?;
    }
    Ok(())
}

/// 与落盘的旧模式闸门数据逐字节比对；任一差异返回 `Ok(false)`。
fn check_gate<R: std::io::Read>(
    r: &mut R,
    dump: &[u16],
    network: &NetworkTriplet,
) -> std::io::Result<bool> {
    let mut u16_buf = [0u8; 2];
    for expected in dump {
        r.read_exact(&mut u16_buf)?;
        if u16::from_le_bytes(u16_buf) != *expected {
            return Ok(false);
        }
    }
    let mut byte = [0u8; 1];
    r.read_exact(&mut byte)?;
    if byte[0] != network.0 {
        return Ok(false);
    }
    r.read_exact(&mut u16_buf)?;
    if u16::from_le_bytes(u16_buf) as usize != network.1.len() {
        return Ok(false);
    }
    for expected in &network.1 {
        r.read_exact(&mut u16_buf)?;
        if u16::from_le_bytes(u16_buf) != *expected {
            return Ok(false);
        }
    }
    let mut u32_buf = [0u8; 4];
    r.read_exact(&mut u32_buf)?;
    if u32::from_le_bytes(u32_buf) as usize != network.2.len() {
        return Ok(false);
    }
    let mut i64_buf = [0u8; 8];
    for expected in &network.2 {
        r.read_exact(&mut i64_buf)?;
        if i64::from_le_bytes(i64_buf) != *expected {
            return Ok(false);
        }
    }
    Ok(true)
}

fn run_legacy(specs: &[SectionSpec], gate: &std::fs::File) -> (BatchResult, bool) {
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
    let elapsed_ms = start.elapsed().as_millis();
    let live_bytes = LIVE_BYTES.load(Ordering::Relaxed);

    let mut gate_ok = true;
    let mut writer = std::io::BufWriter::new(gate);
    'write: {
        for section in &sections {
            let dump = section.dump();
            let network = section.network_bytes();
            if write_gate(&mut writer, &dump, &network).is_err() {
                gate_ok = false;
                break 'write;
            }
        }
        if std::io::Write::flush(&mut writer).is_err() {
            gate_ok = false;
        }
    }
    drop(sections);
    (
        BatchResult {
            live_bytes,
            elapsed_ms,
        },
        gate_ok,
    )
}

fn run_production(specs: &[SectionSpec], gate: &std::fs::File) -> (BatchResult, bool) {
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
    let elapsed_ms = start.elapsed().as_millis();
    let live_bytes = LIVE_BYTES.load(Ordering::Relaxed);

    let mut gate_ok = true;
    let mut reader = std::io::BufReader::new(gate);
    if std::io::Seek::rewind(&mut reader).is_err() {
        gate_ok = false;
    }
    if gate_ok {
        for section in &sections {
            let dump: Vec<u16> = section.iter().map(BlockStateId::as_u16).collect();
            let network = production_network_bytes(section);
            if !matches!(check_gate(&mut reader, &dump, &network), Ok(true)) {
                gate_ok = false;
                break;
            }
        }
    }
    drop(sections);
    (
        BatchResult {
            live_bytes,
            elapsed_ms,
        },
        gate_ok,
    )
}

fn main() {
    let specs = workload();

    let gate_file = match tempfile::tempfile() {
        Ok(file) => file,
        Err(err) => {
            eprintln!("创建闸门临时文件失败：{err}");
            std::process::exit(2);
        }
    };

    let (old, write_ok) = run_legacy(&specs, &gate_file);
    let (new, read_ok) = run_production(&specs, &gate_file);
    let integrity_ok = write_ok && read_ok;

    let saved = old.live_bytes.saturating_sub(new.live_bytes);
    let saved_pct = if old.live_bytes > 0 {
        saved as f64 / old.live_bytes as f64 * 100.0
    } else {
        0.0
    };

    println!();
    println!("| 模式 | 区段容器存活 | 变异+序列化耗时 |");
    println!("|---|---:|---:|");
    println!(
        "| old（u8 索引，≤256 态） | {:.1} MiB | {} ms |",
        old.live_bytes as f64 / 1024.0 / 1024.0,
        old.elapsed_ms
    );
    println!(
        "| new（≤16 态半字节索引） | {:.1} MiB | {} ms |",
        new.live_bytes as f64 / 1024.0 / 1024.0,
        new.elapsed_ms
    );
    println!();
    println!("区段容器存活削减：{saved_pct:.1}%（{saved} 字节）");
    println!(
        "一致性闸门：{}（{} 区段，迭代值+网络序列化逐字节）",
        if integrity_ok { "通过" } else { "失败" },
        specs.len()
    );

    let json = serde_json::json!({
        "round": 5,
        "subject": "区块调色板半字节索引存储：≤16 态区段 u8→4bit",
        "integrity_ok": integrity_ok,
        "workload": {
            "sections": specs.len(),
            "palette_size_distribution": "55% 同质 / 25% 2-8 态 / 12% 9-16 态 / 6% 17-64 态 / 2% 65-300 态",
            "mutations_per_section": VOLUME + 512,
            "live_scope": "仅区段容器（闸门数据落盘比对，不占内存记账）",
        },
        "old": {
            "live_bytes": old.live_bytes,
            "elapsed_ms": old.elapsed_ms as u64,
        },
        "new": {
            "live_bytes": new.live_bytes,
            "elapsed_ms": new.elapsed_ms as u64,
        },
        "saved_bytes": saved,
        "saved_pct": saved_pct,
    });

    let path = std::path::Path::new("note/report/perf/round5-palette-nibble.json");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(
        path,
        serde_json::to_string_pretty(&json).unwrap_or_default(),
    ) {
        Ok(()) => println!("对比 JSON 已写入 {}", path.display()),
        Err(err) => eprintln!("写入 JSON 失败：{err}"),
    }

    if !integrity_ok {
        std::process::exit(1);
    }
}
