//! 轮次 10 基准：组帧缓冲全局池化（`frame_scratch` 常驻 → 全局有界池检出）。
//!
//! 模型与轮次 9 同款：W 个工作线程静态瓜分 128 条模拟连接，每连接依次
//! 经历登录期（含一次 1 MiB 大包突发）与稳态游玩期。三策略对比：
//!
//! - `legacy`（优化前）：每条连接的编码器常驻一块组帧 scratch（含历史
//!   最大包撑大后的收缩治理），连接存续期内驻留 ≈ 连接数 × ≤256 KiB；
//!   批路径每批 `Vec::new()`（分配流失，不驻留）。
//! - `global`（优化后/生产语义）：全局单池 `Mutex<Vec<Vec<u8>>>`，逐次
//!   组帧检出/归还，驻留封顶 16 份，归还时逾 2×256 KiB 收缩回 256 KiB。
//!
//! 硬闸门：两策略产出的帧字节流必须逐字节全等（FNV-1a 滚动哈希 +
//! 字节数双重校验）。
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::io::Write as _;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

// ---------------------------------------------------------------------------
// 计数分配器（live_bytes 差分 = 策略驻留账，与轮次 8/9 同款）
// ---------------------------------------------------------------------------

mod counting {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicUsize, Ordering};

    pub struct Counting;

    static LIVE: AtomicUsize = AtomicUsize::new(0);

    // SAFETY: 仅统计后原样转发系统分配器，不改变任何分配语义
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            LIVE.fetch_add(layout.size(), Ordering::Relaxed);
            // SAFETY: 转发给系统分配器。
            unsafe { System.alloc(layout) }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
            // SAFETY: 转发给系统分配器。
            unsafe { System.dealloc(ptr, layout) }
        }
    }

    pub fn live_bytes() -> usize {
        LIVE.load(Ordering::Relaxed)
    }
}

#[global_allocator]
static GLOBAL: counting::Counting = counting::Counting;

use counting::live_bytes;

// ---------------------------------------------------------------------------
// 确定性伪随机（xorshift64*，与轮次 8/9 一致）
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
// 负载模型
// ---------------------------------------------------------------------------

/// 模拟并发连接数
const CONNECTIONS: usize = 128;
/// 每连接登录期数据包数
const LOGIN_PACKETS: usize = 40;
/// 每连接稳态游玩期数据包数（总包数 = `CONNECTIONS` × (`LOGIN_PACKETS` + `PLAY_PACKETS`)）
const PLAY_PACKETS: usize = 260;
/// 登录突发大包尺寸（配方/标签同步级）
const BIG_PACKET: usize = 1024 * 1024;
/// 组帧缓冲驻留上限（与生产 `MAX_RETAINED_SCRATCH` 一致）
const MAX_RETAINED_SCRATCH: usize = 256 * 1024;
/// 全局池驻留上限（与生产 `MAX_POOLED_FRAME_BUFFERS` 一致）
const GLOBAL_POOL_CAP: usize = 16;

/// 数据包尺寸分布：登录期偏大（配置/注册表同步），游玩期以中小包为主
const fn packet_size(login_phase: bool, rng: &mut Rng) -> usize {
    if login_phase {
        match rng.below(100) {
            0..30 => 512 + rng.below(1536),
            30..80 => 2048 + rng.below(30 * 1024),
            _ => 32 * 1024 + rng.below(96 * 1024),
        }
    } else {
        match rng.below(100) {
            0..60 => 200 + rng.below(1848),
            60..90 => 2048 + rng.below(14 * 1024),
            _ => 16 * 1024 + rng.below(112 * 1024),
        }
    }
}

/// 由 (conn, seq) 确定性派生一个数据包负载（不落语料库，两策略
/// 各自现算、逐包一致）
fn gen_packet(conn: usize, seq: usize, login_phase: bool) -> Vec<u8> {
    let phase_bit = usize::from(!login_phase);
    let seed = ((conn as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ ((seq as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) | 1))
        .wrapping_add(phase_bit as u64);
    let mut rng = Rng(seed);
    let size = packet_size(login_phase, &mut rng);
    let mut data = Vec::with_capacity(size);
    for _ in 0..size {
        data.push((rng.next() & 0xFF) as u8);
    }
    data
}

/// 生成登录期那次 1 MiB 突发大包（同样确定性）
fn gen_big_packet(conn: usize) -> Vec<u8> {
    let mut rng = Rng((conn as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93) | 7);
    let mut data = Vec::with_capacity(BIG_PACKET);
    for _ in 0..BIG_PACKET {
        data.push((rng.next() & 0xFF) as u8);
    }
    data
}

// ---------------------------------------------------------------------------
// 组帧原语（对齐生产 frame_packet 的非压缩字节布局：
// VarInt 总长 + VarInt 数据长(0 标记) + 原文，头部 ≤ 10 字节）
// ---------------------------------------------------------------------------

fn write_var_int(value: usize, out: &mut Vec<u8>) {
    let mut value = value;
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

const fn var_int_size(mut value: usize) -> usize {
    let mut size = 0;
    loop {
        value >>= 7;
        size += 1;
        if value == 0 {
            return size;
        }
    }
}

/// 组一帧（阈值 0 的 `data_length=0` 标记布局），追加进 `frame`，
/// 返回本帧的 FNV-1a 哈希供线上字节门滚动累积。
fn frame_one(packet: &[u8], frame: &mut Vec<u8>) -> u64 {
    let data_len_var = 1usize; // VarInt(0) 恰 1 字节
    let full_len = data_len_var + packet.len();
    let mut header = [0u8; 10];
    // 头部先写入栈上小缓冲再整体拷入，与生产一致
    let header_len = {
        let mut cursor = Vec::new();
        write_var_int(full_len, &mut cursor);
        write_var_int(0, &mut cursor);
        header[..cursor.len()].copy_from_slice(&cursor);
        cursor.len()
    };
    debug_assert_eq!(header_len, var_int_size(full_len) + 1);
    frame.reserve(header_len + packet.len());
    frame.extend_from_slice(&header[..header_len]);
    frame.extend_from_slice(packet);

    // FNV-1a 对本帧新增字节段滚动
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for &byte in &frame[frame.len() - header_len - packet.len()..] {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

/// 归还治理（与生产 `give_back_frame_buffer` 同款）：逾 2 倍
/// 留存上限收缩回上限
fn give_back_governance(buffer: &mut Vec<u8>) {
    buffer.clear();
    if buffer.capacity() > 2 * MAX_RETAINED_SCRATCH {
        buffer.shrink_to(MAX_RETAINED_SCRATCH);
    }
}

// ---------------------------------------------------------------------------
// 连接剧本：登录期（含大包突发）+ 稳态游玩期，逐包组帧
// ---------------------------------------------------------------------------

/// legacy：每连接常驻组帧 scratch（含轮次 2 收缩治理），返回
/// 该连接的滚动哈希/字节数与最终常驻缓冲（驻留账载体）。
fn run_conn_legacy(conn: usize) -> (u64, usize, Vec<u8>) {
    let mut scratch: Vec<u8> = Vec::new();
    let mut rolling: u64 = 0xcbf2_9ce4_8422_2325;
    let mut bytes = 0usize;

    for seq in 0..LOGIN_PACKETS {
        let packet = if seq == LOGIN_PACKETS / 2 {
            gen_big_packet(conn)
        } else {
            gen_packet(conn, seq, true)
        };
        bytes += packet.len();
        scratch.clear();
        // 轮次 2 收缩治理：容量逾留存上限且本包明显更小时收缩
        let hint = packet.len() + 10;
        let capacity = scratch.capacity();
        if capacity > MAX_RETAINED_SCRATCH && hint < capacity / 2 {
            scratch.shrink_to(MAX_RETAINED_SCRATCH.max(hint));
        }
        let hash = frame_one(&packet, &mut scratch);
        rolling = rolling.wrapping_mul(1_099_511_628_211) ^ hash;
    }
    for seq in 0..PLAY_PACKETS {
        let packet = gen_packet(conn, LOGIN_PACKETS + seq, false);
        bytes += packet.len();
        scratch.clear();
        let hint = packet.len() + 10;
        let capacity = scratch.capacity();
        if capacity > MAX_RETAINED_SCRATCH && hint < capacity / 2 {
            scratch.shrink_to(MAX_RETAINED_SCRATCH.max(hint));
        }
        let hash = frame_one(&packet, &mut scratch);
        rolling = rolling.wrapping_mul(1_099_511_628_211) ^ hash;
    }
    (rolling, bytes, scratch)
}

/// global：逐次组帧自共享池检出/归还（生产轮次 10 语义）。
fn run_conn_global(conn: usize, pool: &Mutex<Vec<Vec<u8>>>, created: &AtomicUsize) -> (u64, usize) {
    let mut rolling: u64 = 0xcbf2_9ce4_8422_2325;
    let mut bytes = 0usize;

    let frame_once = |packet: &[u8]| {
        let mut buffer = {
            let mut guard = pool.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            guard.pop()
        }
        .unwrap_or_else(|| {
            created.fetch_add(1, Ordering::Relaxed);
            Vec::new()
        });
        buffer.clear();
        let hash = frame_one(packet, &mut buffer);
        give_back_governance(&mut buffer);
        {
            let mut guard = pool.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if guard.len() < GLOBAL_POOL_CAP {
                guard.push(buffer);
            }
        }
        hash
    };

    for seq in 0..LOGIN_PACKETS {
        let packet = if seq == LOGIN_PACKETS / 2 {
            gen_big_packet(conn)
        } else {
            gen_packet(conn, seq, true)
        };
        bytes += packet.len();
        let hash = frame_once(&packet);
        rolling = rolling.wrapping_mul(1_099_511_628_211) ^ hash;
    }
    for seq in 0..PLAY_PACKETS {
        let packet = gen_packet(conn, LOGIN_PACKETS + seq, false);
        bytes += packet.len();
        let hash = frame_once(&packet);
        rolling = rolling.wrapping_mul(1_099_511_628_211) ^ hash;
    }
    (rolling, bytes)
}

// ---------------------------------------------------------------------------
// 策略执行与度量
// ---------------------------------------------------------------------------

struct ModeResult {
    standing_bytes: usize,
    buffers_created: usize,
    wire_hash: u64,
    wire_bytes: u64,
    wall_ms: f64,
}

fn run_legacy(workers: usize) -> ModeResult {
    let baseline = live_bytes();
    let start = Instant::now();
    let wire_bytes = AtomicUsize::new(0);
    let wire_hash = AtomicUsize::new(0);

    let retained: Vec<Vec<u8>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|worker| {
                let wire_bytes = &wire_bytes;
                let wire_hash = &wire_hash;
                scope.spawn(move || {
                    let conns: Vec<usize> =
                        (0..CONNECTIONS).filter(|c| c % workers == worker).collect();
                    let mut kept = Vec::new();
                    for conn in conns {
                        let (rolling, bytes, scratch) = run_conn_legacy(conn);
                        wire_bytes.fetch_add(bytes, Ordering::Relaxed);
                        wire_hash.fetch_add(rolling as usize, Ordering::Relaxed);
                        kept.push(scratch);
                    }
                    kept
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap_or_default())
            .collect()
    });

    let standing = live_bytes().saturating_sub(baseline);
    let result = ModeResult {
        standing_bytes: standing,
        buffers_created: CONNECTIONS,
        wire_hash: wire_hash.load(Ordering::Relaxed) as u64,
        wire_bytes: wire_bytes.load(Ordering::Relaxed) as u64,
        wall_ms: start.elapsed().as_secs_f64() * 1000.0,
    };
    drop(retained);
    result
}

fn run_global(workers: usize) -> ModeResult {
    let baseline = live_bytes();
    let start = Instant::now();
    let created = AtomicUsize::new(0);
    let wire_bytes = AtomicUsize::new(0);
    let wire_hash = AtomicUsize::new(0);
    let pool: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());

    std::thread::scope(|scope| {
        for worker in 0..workers {
            let pool = &pool;
            let created = &created;
            let wire_bytes = &wire_bytes;
            let wire_hash = &wire_hash;
            scope.spawn(move || {
                let conns: Vec<usize> =
                    (0..CONNECTIONS).filter(|c| c % workers == worker).collect();
                for conn in conns {
                    let (rolling, bytes) = run_conn_global(conn, pool, created);
                    wire_bytes.fetch_add(bytes, Ordering::Relaxed);
                    wire_hash.fetch_add(rolling as usize, Ordering::Relaxed);
                }
            });
        }
    });

    let standing = live_bytes().saturating_sub(baseline);
    let result = ModeResult {
        standing_bytes: standing,
        buffers_created: created.load(Ordering::Relaxed),
        wire_hash: wire_hash.load(Ordering::Relaxed) as u64,
        wire_bytes: wire_bytes.load(Ordering::Relaxed) as u64,
        wall_ms: start.elapsed().as_secs_f64() * 1000.0,
    };
    drop(pool);
    result
}

// ---------------------------------------------------------------------------
// 主流程：两策略 × 两种工作线程形态，双硬闸门，JSON 落盘
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_lines)]
fn main() {
    let mut scenarios = serde_json::Map::new();
    let mut all_pass = true;

    for workers in [16usize, 128usize] {
        let legacy = run_legacy(workers);
        let global = run_global(workers);

        let wire_hash_equal = legacy.wire_hash == global.wire_hash;
        let wire_bytes_equal = legacy.wire_bytes == global.wire_bytes;
        all_pass &= wire_hash_equal && wire_bytes_equal;

        eprintln!(
            "W={workers}: legacy standing={}B created={} wall={:.1}ms | global standing={}B created={} wall={:.1}ms | 哈希门={wire_hash_equal} 字节门={wire_bytes_equal}",
            legacy.standing_bytes,
            legacy.buffers_created,
            legacy.wall_ms,
            global.standing_bytes,
            global.buffers_created,
            global.wall_ms,
        );

        scenarios.insert(
            format!("w{workers}"),
            serde_json::json!({
                "workers": workers,
                "legacy": {
                    "standing_bytes": legacy.standing_bytes,
                    "buffers_created": legacy.buffers_created,
                    "wall_ms": legacy.wall_ms,
                },
                "global_pool": {
                    "standing_bytes": global.standing_bytes,
                    "buffers_created": global.buffers_created,
                    "wall_ms": global.wall_ms,
                },
                "gates": {
                    "wire_hash_equal": wire_hash_equal,
                    "wire_bytes_equal": wire_bytes_equal,
                },
            }),
        );
    }

    let report = serde_json::json!({
        "round": 10,
        "topic": "组帧缓冲全局池化（每连接常驻 scratch 与批路径逐批分配的统一池化）",
        "workload": {
            "connections": CONNECTIONS,
            "login_packets": LOGIN_PACKETS,
            "play_packets": PLAY_PACKETS,
            "big_packet_bytes": BIG_PACKET,
            "global_pool_cap": GLOBAL_POOL_CAP,
        },
        "scenarios": scenarios,
        "gates": { "all_pass": all_pass },
    });

    let path = "note/report/perf/round10-frame-buffer-pool.json";
    match std::fs::File::create(path) {
        Ok(mut file) => {
            if let Err(err) = file.write_all(serde_json::to_string_pretty(&report).unwrap_or_default().as_bytes()) {
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
        eprintln!("硬闸门未通过：两策略线上字节不一致");
        std::process::exit(2);
    }
}
