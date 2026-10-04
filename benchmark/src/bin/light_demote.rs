//! 轮次 6：光照容器均质归一基准
//!
//! 对比旧行为（落载时在场的 2048 B 光照数组一律 `Full` 常驻）与新行为
//! （均质数组降级为 1 B 的 `Empty(v)`：天空光仅均质 15 可降级，方块光任意
//! 均质值可降级）在同一合成区块负载下的存活字节、构建耗时、落盘字节与
//! 网络字节；两模式做三道一致性硬闸门：
//!
//! 1. 内存形态逐格解码值完全相等（降级不得改变任何格子的光照读数）；
//! 2. 落盘形态按自家读取规则解码后逐格相等（允许字节数不同：均质 0
//!    方块光旧版写 2048 B 零数组、新版省略，两者解码同为全 0）；
//! 3. 网络形态按客户端规则解码后逐格相等（缺失天空光解码为 15、缺失
//!    方块光解码为 0——此闸门专门兜底「均质 0 天空光误降级」这类漏光
//!    回归）。
//!
//! 旧行为以基准内复刻实现，语义对齐改动前
//! `papokin-world/src/chunk/format/mod.rs` 的落载逻辑
//! （`map_or(Empty(0), Full)`）；新行为的降级直接使用生产函数
//! `demote_block_light` / `demote_sky_light`。落盘/网络编码规则在基准内
//! 按生产实现逐行镜像（生产侧由单测 `loaded_uniform_light_demoted_and_round_trips`
//! 覆盖）。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use papokin_world::chunk::format::{LightContainer, demote_block_light, demote_sky_light};

// ---------------------------------------------------------------------------
// 计数分配器：精确测量两批光照容器的存活字节差
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
// 负载模型：主世界风格的光照区段组成
// ---------------------------------------------------------------------------

/// 每个区块的区段数（1.21.11 主世界 -64..320，共 24 个 16 格高区段）
const SECTIONS_PER_CHUNK: usize = 24;
/// 模拟加载的区块数
const CHUNKS: usize = 1200;
const ARRAY_SIZE: usize = LightContainer::ARRAY_SIZE;

/// 单个光照数组的「磁盘形态」类别
#[derive(Clone, Copy)]
enum DiskClass {
    /// 数组缺失（原版 NBT 中无此键）
    Absent,
    /// 在场且逐格均质（参数为均质值 0..=15）
    Uniform(u8),
    /// 在场且非均质
    Mixed,
}

/// 天空光组成（贴近主世界实测直觉）：
/// 15% 缺失（深埋区段）、45% 均质 15（高空全亮）、10% 均质 0（洞穴封顶
/// 区段）、30% 非均质（地表附近明暗过渡）
const fn sky_class(rng: &mut Rng) -> DiskClass {
    match rng.below(100) {
        0..15 => DiskClass::Absent,
        15..60 => DiskClass::Uniform(15),
        60..70 => DiskClass::Uniform(0),
        _ => DiskClass::Mixed,
    }
}

/// 方块光组成：80% 缺失（无光源区段）、8% 均质 0、2% 均质非零
/// （如岩浆层均亮）、10% 非均质
const fn block_class(rng: &mut Rng) -> DiskClass {
    match rng.below(100) {
        0..80 => DiskClass::Absent,
        80..88 => DiskClass::Uniform(0),
        88..90 => DiskClass::Uniform(7),
        _ => DiskClass::Mixed,
    }
}

/// 由磁盘类别生成原始数组（即落载时从 NBT 读出的字节）
fn gen_disk_array(rng: &mut Rng, class: DiskClass) -> Option<Box<[u8]>> {
    match class {
        DiskClass::Absent => None,
        DiskClass::Uniform(value) => Some(vec![value << 4 | value; ARRAY_SIZE].into_boxed_slice()),
        DiskClass::Mixed => {
            let mut arr = vec![0u8; ARRAY_SIZE];
            for byte in &mut arr {
                *byte = rng.next() as u8;
            }
            // 保证非均质：首字节强制与其余不同
            arr[0] = arr[ARRAY_SIZE / 2].wrapping_add(1);
            Some(arr.into_boxed_slice())
        }
    }
}

// ---------------------------------------------------------------------------
// 两种构建路径：旧版落载（复刻） vs 新版落载（生产降级函数）
// ---------------------------------------------------------------------------

/// 旧版语义：在场数组一律 `Full`（改动前 `map_or(Empty(0), Full)`）
fn legacy_build(disk: Option<Box<[u8]>>) -> LightContainer {
    disk.map_or(LightContainer::Empty(0), LightContainer::Full)
}

/// 新版语义：在场数组按类别降级（生产函数）
fn new_build(disk: Option<Box<[u8]>>, sky: bool) -> LightContainer {
    disk.map_or(LightContainer::Empty(0), |data| {
        if sky {
            demote_sky_light(LightContainer::Full(data))
        } else {
            demote_block_light(LightContainer::Full(data))
        }
    })
}

/// 一套区块负载的光照容器集合（天空 + 方块各 `CHUNKS * SECTIONS` 个）
struct LightWorld {
    sky: Vec<LightContainer>,
    block: Vec<LightContainer>,
}

fn build_world(new_rules: bool) -> (LightWorld, u128) {
    // 类别流与数组字节流必须与 RNG 种子一一对应，保证两模式负载逐字节相同
    let mut rng = Rng(0x5EED_0611_7E6D_0006);
    let total = CHUNKS * SECTIONS_PER_CHUNK;
    let mut sky = Vec::with_capacity(total);
    let mut block = Vec::with_capacity(total);
    let start = Instant::now();
    for _ in 0..total {
        let sky_kind = sky_class(&mut rng);
        let sky_disk = gen_disk_array(&mut rng, sky_kind);
        let block_kind = block_class(&mut rng);
        let block_disk = gen_disk_array(&mut rng, block_kind);
        if new_rules {
            sky.push(new_build(sky_disk, true));
            block.push(new_build(block_disk, false));
        } else {
            sky.push(legacy_build(sky_disk));
            block.push(legacy_build(block_disk));
        }
    }
    (LightWorld { sky, block }, start.elapsed().as_nanos())
}

// ---------------------------------------------------------------------------
// 三道闸门
// ---------------------------------------------------------------------------

/// 内存形态：逐格解码值的位置敏感折叠校验和
fn memory_checksum(world: &LightWorld) -> u64 {
    let mut acc = 0u64;
    for container in world.sky.iter().chain(world.block.iter()) {
        for linear in 0..ARRAY_SIZE * 2 {
            let value = container.get(linear % 16, (linear / 256) % 16, (linear / 16) % 16);
            acc = acc
                .rotate_left(5)
                .wrapping_add(u64::from(value))
                .wrapping_add(linear as u64);
        }
    }
    acc
}

/// 落盘形态编码（镜像生产 `extract_light_ref` 规则）：
/// `Empty(0)` 省略；非零 `Empty` 物化为均质数组；`Full` 原样引用
fn save_form(container: &LightContainer) -> Option<Box<[u8]>> {
    match container {
        LightContainer::Empty(0) => None,
        LightContainer::Empty(value) => {
            Some(vec![value << 4 | value; ARRAY_SIZE].into_boxed_slice())
        }
        LightContainer::Full(data) => Some(data.clone()),
    }
}

/// 旧版落盘规则（复刻改动前语义）：`Empty` 一律省略，`Full` 原样写出
fn legacy_save_form(container: &LightContainer) -> Option<Box<[u8]>> {
    match container {
        LightContainer::Empty(_) => None,
        LightContainer::Full(data) => Some(data.clone()),
    }
}

/// 网络形态编码（镜像生产 `chunk_data/light.rs` 规则）：与落盘同形——
/// 非零 `Empty` 物化为均质数组随掩码下发，`Empty(0)` 置空掩码
fn wire_form(container: &LightContainer) -> Option<Box<[u8]>> {
    save_form(container)
}

/// 把「可能缺失的数组」展开为逐格值流并折叠。无论数组在场与否都走
/// 统一的 4096 格流（半字节序与 `LightContainer::get` 一致：偶数格取低
/// 半字节），保证「解码值相同 ⇒ 校验和相同」
fn decode_fold(form: Option<&[u8]>, missing_default: u8, acc: &mut u64) {
    for i in 0..ARRAY_SIZE * 2 {
        let value = form.map_or(missing_default, |data| {
            let byte = data[i >> 1];
            if i.is_multiple_of(2) {
                byte & 0x0F
            } else {
                byte >> 4
            }
        });
        *acc = acc
            .rotate_left(5)
            .wrapping_add(u64::from(value))
            .wrapping_add(i as u64);
    }
}

/// 闸门 2/3 的汇总结果：解码校验和 + 编码总字节
struct FormStats {
    checksum: u64,
    total_bytes: usize,
    present_arrays: usize,
}

/// 落盘视角：缺失一律解码为 0（自家加载器对天空/方块缺失数组的现行默认）
fn save_stats(world: &LightWorld, legacy: bool) -> FormStats {
    let mut stats = FormStats {
        checksum: 0,
        total_bytes: 0,
        present_arrays: 0,
    };
    for container in world.sky.iter().chain(world.block.iter()) {
        let form = if legacy {
            legacy_save_form(container)
        } else {
            save_form(container)
        };
        if let Some(data) = &form {
            stats.total_bytes += data.len();
            stats.present_arrays += 1;
        }
        decode_fold(form.as_deref(), 0, &mut stats.checksum);
    }
    stats
}

/// 网络视角：缺失天空光解码为 15、缺失方块光解码为 0（客户端规则）
fn wire_stats(world: &LightWorld) -> FormStats {
    let mut stats = FormStats {
        checksum: 0,
        total_bytes: 0,
        present_arrays: 0,
    };
    for container in &world.sky {
        let form = wire_form(container);
        if let Some(data) = &form {
            stats.total_bytes += data.len();
            stats.present_arrays += 1;
        }
        decode_fold(form.as_deref(), 15, &mut stats.checksum);
    }
    for container in &world.block {
        let form = wire_form(container);
        if let Some(data) = &form {
            stats.total_bytes += data.len();
            stats.present_arrays += 1;
        }
        decode_fold(form.as_deref(), 0, &mut stats.checksum);
    }
    stats
}

// ---------------------------------------------------------------------------
// 主流程
// ---------------------------------------------------------------------------

struct ModeResult {
    live_bytes: usize,
    build_ns: u128,
    memory_checksum: u64,
    save: FormStats,
    wire: FormStats,
    empty_containers: usize,
    full_containers: usize,
}

fn run_mode(new_rules: bool) -> ModeResult {
    let baseline = LIVE_BYTES.load(Ordering::Relaxed);
    let (world, build_ns) = build_world(new_rules);
    let live_bytes = LIVE_BYTES.load(Ordering::Relaxed) - baseline;

    let memory_checksum = memory_checksum(&world);
    let save = save_stats(&world, !new_rules);
    let wire = wire_stats(&world);
    let empty_containers = world
        .sky
        .iter()
        .chain(world.block.iter())
        .filter(|c| c.is_empty())
        .count();
    let full_containers = world.sky.len() + world.block.len() - empty_containers;
    drop(world);
    ModeResult {
        live_bytes,
        build_ns,
        memory_checksum,
        save,
        wire,
        empty_containers,
        full_containers,
    }
}

fn main() {
    let legacy = run_mode(false);
    let new = run_mode(true);

    // 三道硬闸门
    assert_eq!(
        legacy.memory_checksum, new.memory_checksum,
        "闸门 1 失败：内存形态逐格解码值不一致"
    );
    assert_eq!(
        legacy.save.checksum, new.save.checksum,
        "闸门 2 失败：落盘形态解码值不一致"
    );
    assert_eq!(
        legacy.wire.checksum, new.wire.checksum,
        "闸门 3 失败：网络形态客户端解码值不一致（漏光回归！）"
    );

    let saved_bytes = legacy.live_bytes as f64 - new.live_bytes as f64;
    let saved_pct = saved_bytes / legacy.live_bytes as f64 * 100.0;
    let wire_saved = legacy.wire.total_bytes as f64 - new.wire.total_bytes as f64;
    let save_saved = legacy.save.total_bytes as f64 - new.save.total_bytes as f64;
    let build_change_pct =
        (new.build_ns as f64 - legacy.build_ns as f64) / legacy.build_ns as f64 * 100.0;

    let json = serde_json::json!({
        "round": 6,
        "subject": "光照容器均质归一：Full(2048B) -> Empty(1B)，天空光仅均质 15 可降级",
        "gates": {
            "memory_decode_equal": true,
            "save_decode_equal": true,
            "wire_client_decode_equal": true,
        },
        "workload": {
            "chunks": CHUNKS,
            "sections_per_chunk": SECTIONS_PER_CHUNK,
            "containers_per_side": CHUNKS * SECTIONS_PER_CHUNK * 2,
            "composition": "sky: 15% absent/45% uniform-15/10% uniform-0/30% mixed; block: 80% absent/8% uniform-0/2% uniform-7/10% mixed",
            "live_scope": "计数分配器仅覆盖两批光照容器本体；校验和无驻留",
        },
        "legacy": {
            "live_bytes": legacy.live_bytes,
            "build_ms": legacy.build_ns as f64 / 1e6,
            "full_containers": legacy.full_containers,
            "empty_containers": legacy.empty_containers,
            "save_bytes": legacy.save.total_bytes,
            "save_arrays": legacy.save.present_arrays,
            "wire_bytes": legacy.wire.total_bytes,
            "wire_arrays": legacy.wire.present_arrays,
        },
        "new": {
            "live_bytes": new.live_bytes,
            "build_ms": new.build_ns as f64 / 1e6,
            "full_containers": new.full_containers,
            "empty_containers": new.empty_containers,
            "save_bytes": new.save.total_bytes,
            "save_arrays": new.save.present_arrays,
            "wire_bytes": new.wire.total_bytes,
            "wire_arrays": new.wire.present_arrays,
        },
        "delta": {
            "live_saved_pct": saved_pct,
            "build_change_pct": build_change_pct,
            "save_saved_pct": save_saved / legacy.save.total_bytes as f64 * 100.0,
            "wire_saved_pct": wire_saved / legacy.wire.total_bytes as f64 * 100.0,
        },
        "date": "2026-10-04",
    });

    let out_path = std::path::Path::new("note/report/perf/round6-light-demote.json");
    if let Some(parent) = out_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(
        out_path,
        serde_json::to_string_pretty(&json).unwrap_or_default(),
    ) {
        Ok(()) => eprintln!("JSON 已写出：{}", out_path.display()),
        Err(err) => eprintln!("写入 JSON 失败：{err}"),
    }

    println!(
        "存活字节：{} -> {}（-{saved_pct:.1}%）；构建耗时变化 {build_change_pct:+.1}%；落盘 {} -> {} B；网络 {} -> {} B",
        legacy.live_bytes,
        new.live_bytes,
        legacy.save.total_bytes,
        new.save.total_bytes,
        legacy.wire.total_bytes,
        new.wire.total_bytes,
    );
}
