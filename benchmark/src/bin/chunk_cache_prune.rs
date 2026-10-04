//! 区块编码缓存逐出策略基准（性能优化轮次 4，见 note/report/perf/）。
//!
//! 背景：0.3.16 之前，每玩家编码缓存在 8192 条计数闸触发时
//! 整体清空——当前视距内的热条目陪葬，随后的视距内重发全部
//! 重序列化（CPU 风暴），且计数制对大尺寸包记账失真。
//! 0.3.17 起改为 32 MiB 字节预算 + 最远优先逐出至八成。
//!
//! 本基准模拟玩家「直线远征 + 视距边界抖动」的旅途：
//! 每 tick 前进 1 区块（视距 12 圆柱的前沿进入发送集），
//! 同时随机抽 40 个视距内区块模拟边界往复（离开/重回发送集）。
//! 两种逐出策略在同一发送序列下对比：
//! - `old`：8192 条计数闸 → 整体清空（0.3.16 行为复刻）
//! - `new`：32 MiB 字节预算 → 最远优先逐出至八成（0.3.17 行为复刻）
//!
//! 指标：缓存峰值字节/条数、总编码次数、单 tick 编码峰值
//!（风暴烈度）、策略触发事件数。完整性闸门：两策略的出站
//! 包流哈希必须一致（缓存只影响重序列化时机，不影响内容）。

#![allow(clippy::print_stdout)]

use std::{collections::HashMap, error::Error};

use serde_json::{Value, json};

/// 旅途长度（tick = 前进区块数）
const JOURNEY_TICKS: i32 = 9_600;
/// 视距（生产默认量级）
const VIEW_DISTANCE: i32 = 12;
/// 每 tick 边界抖动抽样数（离开/重回发送集的视距内区块）
const WOBBLE_PER_TICK: usize = 40;
/// old 策略计数闸（0.3.16 常量复刻）
const OLD_MAX_ENTRIES: usize = 8192;
/// new 策略字节预算与回落目标（0.3.17 常量复刻）
const NEW_MAX_BYTES: usize = 32 * 1024 * 1024;
const NEW_TARGET_BYTES: usize = NEW_MAX_BYTES / 5 * 4;

/// 区块坐标打包为 map 键
const fn key(x: i32, z: i32) -> i64 {
    ((x as i64) << 32) | (z as i64 & 0xFFFF_FFFF)
}

/// xorshift 伪随机数生成器
struct Rng(u64);

impl Rng {
    const fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// 区块包尺寸：坐标确定性采样（两策略面对同一世界），
/// 剖面同轮次 3（70% 3-15 KiB / 25% 15-50 KiB / 5% 50-200 KiB）
const fn chunk_size(x: i32, z: i32) -> usize {
    let mut rng = Rng((key(x, z) as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let roll = rng.next();
    let span = roll as usize;
    match roll % 100 {
        0..=69 => 3 * 1024 + span % (12 * 1024),
        70..=94 => 15 * 1024 + span % (35 * 1024),
        _ => 50 * 1024 + span % (150 * 1024),
    }
}

/// 视距 12 圆柱内的相对偏移（与生产 `Cylindrical` 同口径：
/// 切比雪夫收缩 2 后欧氏距离 < 视距；原点天然满足）
fn view_offsets() -> Vec<(i32, i32)> {
    let mut offsets = Vec::new();
    for dx in -VIEW_DISTANCE..=VIEW_DISTANCE {
        for dz in -VIEW_DISTANCE..=VIEW_DISTANCE {
            let rx = (dx.abs() - 2).max(0) as i64;
            let rz = (dz.abs() - 2).max(0) as i64;
            let vd = i64::from(VIEW_DISTANCE);
            if rx * rx + rz * rz < vd * vd {
                offsets.push((dx, dz));
            }
        }
    }
    offsets
}

/// 策略运行结果
struct SimResult {
    peak_bytes: usize,
    peak_entries: usize,
    total_encodes: u64,
    max_tick_encodes: u32,
    policy_events: u32,
    stream_hash: u64,
}

/// 跑一种策略；`use_new_policy` 选择逐出算法
fn simulate(use_new_policy: bool) -> SimResult {
    let offsets = view_offsets();
    let mut cache: HashMap<i64, usize> = HashMap::new();
    let mut wobble_rng = Rng(0xB0BB_1E5A_A5A5_5A5A | 1);
    let mut stream_hash = 0u64;
    let mut result = SimResult {
        peak_bytes: 0,
        peak_entries: 0,
        total_encodes: 0,
        max_tick_encodes: 0,
        policy_events: 0,
        stream_hash: 0,
    };

    for tick in 0..JOURNEY_TICKS {
        let center = (tick, 0);
        // 本 tick 发送集：视距圆柱 + 边界抖动抽样（可重复）
        let mut pending: Vec<(i32, i32)> = offsets
            .iter()
            .map(|&(dx, dz)| (center.0 + dx, center.1 + dz))
            .collect();
        for _ in 0..WOBBLE_PER_TICK {
            let idx = wobble_rng.next() as usize % offsets.len();
            let (dx, dz) = offsets[idx];
            pending.push((center.0 + dx, center.1 + dz));
        }

        let mut tick_encodes = 0u32;
        for &(x, z) in &pending {
            let k = key(x, z);
            let size = chunk_size(x, z);
            // 出站包流：缓存只改变编码时机，不改变包内容与次序
            stream_hash = stream_hash
                .rotate_left(7)
                .wrapping_add((k as u64).wrapping_mul(31) ^ size as u64);
            // 编码时机：未缓存（含被逐出/清空后的重发）才序列化
            if cache.insert(k, size).is_none() {
                result.total_encodes += 1;
                tick_encodes += 1;
            }
        }
        result.max_tick_encodes = result.max_tick_encodes.max(tick_encodes);

        // 逐出策略（0.3.16 复刻 vs 0.3.17 复刻）
        if use_new_policy {
            let total: usize = cache.values().sum();
            if total > NEW_MAX_BYTES {
                result.policy_events += 1;
                let mut farthest: Vec<(i64, i64)> = cache
                    .keys()
                    .map(|&k| {
                        let x = (k >> 32) as i32;
                        let z = k as i32;
                        let dx = i64::from(x - center.0);
                        let dz = i64::from(z - center.1);
                        (dx * dx + dz * dz, k)
                    })
                    .collect();
                farthest.sort_unstable_by_key(|entry| std::cmp::Reverse(entry.0));
                let mut running = total;
                for (_, k) in farthest {
                    if running <= NEW_TARGET_BYTES {
                        break;
                    }
                    if let Some(removed) = cache.remove(&k) {
                        running -= removed;
                    }
                }
            }
        } else if cache.len() > OLD_MAX_ENTRIES {
            result.policy_events += 1;
            cache.clear();
        }

        let bytes: usize = cache.values().sum();
        result.peak_bytes = result.peak_bytes.max(bytes);
        result.peak_entries = result.peak_entries.max(cache.len());
    }

    result.stream_hash = stream_hash;
    result
}

const fn mib(bytes: usize) -> f64 {
    bytes as f64 / 1024.0 / 1024.0
}

fn main() -> Result<(), Box<dyn Error>> {
    let old = simulate(false);
    let new = simulate(true);

    // 完整性闸门：出站包流必须逐位一致
    assert_eq!(
        old.stream_hash, new.stream_hash,
        "两策略出站包流必须一致（{:x} vs {:x}）",
        old.stream_hash, new.stream_hash
    );

    println!();
    println!("| 策略 | 缓存峰值 | 条目峰值 | 总编码次数 | 单 tick 编码峰值 | 策略触发 |");
    println!("|---|---:|---:|---:|---:|---:|");
    for (name, r) in [("old（计数闸清空）", &old), ("new（字节预算逐出）", &new)] {
        println!(
            "| {name} | {:.1} MiB | {} | {} | {} | {} |",
            mib(r.peak_bytes),
            r.peak_entries,
            r.total_encodes,
            r.max_tick_encodes,
            r.policy_events,
        );
    }
    let saved = 1.0 - new.peak_bytes as f64 / old.peak_bytes as f64;
    println!();
    println!(
        "缓存峰值削减：{:.1}%（{:.1} → {:.1} MiB）",
        saved * 100.0,
        mib(old.peak_bytes),
        mib(new.peak_bytes),
    );

    let report: Value = json!({
        "round": 4,
        "subject": "区块编码缓存逐出策略：字节预算 + 最远优先 vs 计数闸清空",
        "workload": {
            "journey_ticks": JOURNEY_TICKS,
            "view_distance": VIEW_DISTANCE,
            "wobble_per_tick": WOBBLE_PER_TICK,
            "old_max_entries": OLD_MAX_ENTRIES,
            "new_max_bytes": NEW_MAX_BYTES,
        },
        "integrity_ok": true,
        "old": {
            "peak_bytes": old.peak_bytes,
            "peak_entries": old.peak_entries,
            "total_encodes": old.total_encodes,
            "max_tick_encodes": old.max_tick_encodes,
            "policy_events": old.policy_events,
        },
        "new": {
            "peak_bytes": new.peak_bytes,
            "peak_entries": new.peak_entries,
            "total_encodes": new.total_encodes,
            "max_tick_encodes": new.max_tick_encodes,
            "policy_events": new.policy_events,
        },
    });
    let path = "note/report/perf/round4-chunk-cache-prune.json";
    std::fs::write(path, serde_json::to_string_pretty(&report)?)?;
    println!("对比 JSON 已写入 {path}");
    Ok(())
}
