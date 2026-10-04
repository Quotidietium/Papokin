//! 轮次 7：区块编码缓存按世界共享 + 改动代数失效基准
//!
//! 对比旧行为（每玩家私有编码缓存，各 32 MiB 预算，新鲜度=弱引用+
//! Arc 同一）与新行为（按世界共享缓存，64 MiB 全局预算，新鲜度再加
//! 内容改动代数）在同一多玩家旅途负载下的缓存驻留峰值、总编码次数
//! 与陈旧发送数。
//!
//! 负载：4 名玩家视距 12，从同一点出发向三个方向远征（另一人留守），
//! 每 tick 边界抖动抽 20 个视距内区块模拟离开/重回发送集，并对全体
//! 注视区块随机施加 25 次内容变异。旧行为为 0.3.19 语义复刻（变异
//! 不失效缓存——重回的玩家会收到变异前的旧编码）；新行为按生产
//! `SharedChunkEncodeCache` 语义复刻（变异经代数校验失效，重回必然
//! 重编码，绝不发陈旧内容）。缓存只记尺寸不驻留真实负载（尺寸为坐标
//! 的确定性函数，两模式面对同一世界）。
//!
//! 闸门：A）无变异旅途下两模式的逐玩家出站位置流哈希必须全等
//! （缓存策略变化不得改变玩家收到的内容与时机——红线 2）；
//! B）变异旅途下新模式陈旧发送数必须为 0（代数失效正确性）。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use serde_json::json;

/// 玩家数（同世界、同协议版本）
const PLAYERS: usize = 4;
/// 旅途长度（tick）
const TICKS: i32 = 2_400;
/// 视距（生产默认量级）
const VIEW_DISTANCE: i32 = 12;
/// 每 tick 每玩家边界抖动抽样数（离开/重回发送集的视距内区块）
const WOBBLE_PER_TICK: usize = 20;
/// 每 tick 对全体注视区块的内容变异次数上限（密/疏两档场景）
const MUTATIONS_PER_TICK: usize = 25;
/// 旧行为：每玩家私有缓存字节预算（0.3.19 常量复刻）
const LEGACY_PLAYER_BUDGET: usize = 32 * 1024 * 1024;
/// 新行为：共享缓存全局字节预算（生产常量）
const SHARED_BUDGET: usize = 64 * 1024 * 1024;

/// 区块坐标打包为 map 键
const fn key(x: i32, z: i32) -> i64 {
    ((x as i64) << 32) | (z as i64 & 0xFFFF_FFFF)
}

const fn key_x(k: i64) -> i32 {
    (k >> 32) as i32
}

const fn key_z(k: i64) -> i32 {
    k as i32
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

    const fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// 区块包尺寸：坐标确定性采样（两策略面对同一世界），
/// 剖面同轮次 3/4（70% 3-15 KiB / 25% 15-50 KiB / 5% 50-200 KiB）
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

/// 玩家路径：三人从原点分头远征（x+/z+/对角），一人留守原点
const fn player_center(player: usize, tick: i32) -> (i32, i32) {
    let step = tick / 4;
    match player {
        0 => (step, 0),
        1 => (0, step),
        2 => (-step, -step),
        _ => (0, 0),
    }
}

/// 缓存条目：线上尺寸 + 编码时刻捕获的内容版本
#[derive(Clone, Copy)]
struct CacheEntry {
    size: usize,
    version: u64,
}

/// 单模式运行结果
struct SimResult {
    peak_bytes: usize,
    final_bytes: usize,
    total_encodes: u64,
    total_encode_bytes: u64,
    max_tick_encodes: u32,
    stale_sends: u64,
    prune_events: u32,
    stream_hash: u64,
    wall_ms: u128,
}

/// 世界状态：全体区块的内容版本（两模式共享同一变异流）
struct WorldState {
    content_version: HashMap<i64, u64>,
}

impl WorldState {
    fn version_of(&self, k: i64) -> u64 {
        self.content_version.get(&k).copied().unwrap_or(0)
    }
}

/// 模式参数
struct Mode {
    /// true=共享缓存+代数失效；false=每玩家私有缓存+无代数
    shared: bool,
    /// 每 tick 变异次数（0=洁净旅途，gate A 用）
    mutations_per_tick: usize,
}

#[allow(clippy::too_many_lines)]
fn simulate(mode: &Mode) -> SimResult {
    let offsets = view_offsets();
    let start = Instant::now();

    // 缓存：legacy=每玩家一张表；shared=全玩家一张表
    let mut private_caches: Vec<HashMap<i64, CacheEntry>> =
        (0..PLAYERS).map(|_| HashMap::new()).collect();
    let mut private_bytes: Vec<usize> = vec![0; PLAYERS];
    let mut shared_cache: HashMap<i64, CacheEntry> = HashMap::new();
    let mut shared_bytes = 0usize;

    let mut world = WorldState {
        content_version: HashMap::new(),
    };
    let mut watched: Vec<HashSet<i64>> = (0..PLAYERS).map(|_| HashSet::new()).collect();
    let mut wobble_rng = Rng(0xB0BB_1E5A_5EED_0007 | 1);
    let mut mutate_rng = Rng(0x1E55_1E55_5EED_0007 | 1);

    let mut peak_bytes = 0usize;
    let mut total_encodes = 0u64;
    let mut total_encode_bytes = 0u64;
    let mut max_tick_encodes = 0u32;
    let mut stale_sends = 0u64;
    let mut prune_events = 0u32;
    let mut stream_hash = 0u64;

    for tick in 0..TICKS {
        let centers: Vec<(i32, i32)> = (0..PLAYERS).map(|p| player_center(p, tick)).collect();
        let mut tick_encodes = 0u32;

        for (p, &center) in centers.iter().enumerate() {
            // 注视集推进：前沿进入 + 越界离开。前沿顺序必须确定性
            // （按 offsets 固定序过滤，而非哈希集差集迭代——后者的
            // RandomState 会让两次运行的出站事件序漂移）
            let current: HashSet<i64> = offsets
                .iter()
                .map(|&(dx, dz)| key(center.0 + dx, center.1 + dz))
                .collect();
            let frontier: Vec<i64> = offsets
                .iter()
                .map(|&(dx, dz)| key(center.0 + dx, center.1 + dz))
                .filter(|k| !watched[p].contains(k))
                .collect();
            watched[p] = current;

            // 边界抖动：抽 WOBBLE 个视距内区块立即离开并重回发送集
            let mut events = frontier;
            for _ in 0..WOBBLE_PER_TICK {
                let (dx, dz) = offsets[wobble_rng.below(offsets.len())];
                events.push(key(center.0 + dx, center.1 + dz));
            }

            for k in events {
                let x = key_x(k);
                let z = key_z(k);
                let current_version = world.version_of(k);
                let sent_version = if mode.shared {
                    let fresh = shared_cache
                        .get(&k)
                        .is_some_and(|e| e.version == current_version);
                    if !fresh {
                        // 未缓存或代数落后：重编码并替换
                        let size = chunk_size(x, z);
                        if let Some(old) = shared_cache.insert(
                            k,
                            CacheEntry {
                                size,
                                version: current_version,
                            },
                        ) {
                            shared_bytes -= old.size;
                        }
                        shared_bytes += size;
                        total_encodes += 1;
                        total_encode_bytes += size as u64;
                        tick_encodes += 1;
                    }
                    current_version
                } else {
                    // legacy：无代数校验，任何残留条目都视为新鲜
                    let hit_version = private_caches[p].get(&k).map(|e| e.version);
                    hit_version.unwrap_or_else(|| {
                        let size = chunk_size(x, z);
                        private_caches[p].insert(
                            k,
                            CacheEntry {
                                size,
                                version: current_version,
                            },
                        );
                        private_bytes[p] += size;
                        total_encodes += 1;
                        total_encode_bytes += size as u64;
                        tick_encodes += 1;
                        current_version
                    })
                };

                if sent_version < current_version {
                    stale_sends += 1;
                }
                // 出站流折叠（两模式同一事件序列；版本体现内容新旧）
                stream_hash = stream_hash
                    .rotate_left(7)
                    .wrapping_add(k as u64)
                    .wrapping_add(sent_version << 32)
                    .wrapping_add((p as u64) << 56);
            }
        }
        max_tick_encodes = max_tick_encodes.max(tick_encodes);

        // 内容变异流：对全体注视区块随机施加（两模式同一流）。
        // 并集排序去重保证跨运行确定性（哈希集迭代序不可作据）
        if mode.mutations_per_tick > 0 {
            let mut union: Vec<i64> = watched.iter().flat_map(|w| w.iter().copied()).collect();
            union.sort_unstable();
            union.dedup();
            for _ in 0..mode.mutations_per_tick.min(union.len()) {
                let k = union[mutate_rng.below(union.len())];
                *world.content_version.entry(k).or_insert(0) += 1;
            }
        }

        // 逐出：legacy 各管各预算；shared 全局预算按距最近玩家最远优先
        if mode.shared {
            if shared_bytes > SHARED_BUDGET {
                prune_events += 1;
                let target = SHARED_BUDGET / 5 * 4;
                let mut farthest: Vec<(i64, i64)> = shared_cache
                    .keys()
                    .map(|&k| {
                        let min_dist = centers
                            .iter()
                            .map(|c| {
                                let dx = i64::from(key_x(k) - c.0);
                                let dz = i64::from(key_z(k) - c.1);
                                dx * dx + dz * dz
                            })
                            .min()
                            .unwrap_or(i64::MAX);
                        (min_dist, k)
                    })
                    .collect();
                farthest.sort_unstable_by_key(|&(d, _)| std::cmp::Reverse(d));
                for (_, k) in farthest {
                    if shared_bytes <= target {
                        break;
                    }
                    if let Some(removed) = shared_cache.remove(&k) {
                        shared_bytes -= removed.size;
                    }
                }
            }
            peak_bytes = peak_bytes.max(shared_bytes);
        } else {
            let mut total = 0usize;
            for (p, cache) in private_caches.iter_mut().enumerate() {
                if private_bytes[p] > LEGACY_PLAYER_BUDGET {
                    prune_events += 1;
                    let target = LEGACY_PLAYER_BUDGET / 5 * 4;
                    let center = centers[p];
                    let mut farthest: Vec<(i64, i64)> = cache
                        .keys()
                        .map(|&k| {
                            let dx = i64::from(key_x(k) - center.0);
                            let dz = i64::from(key_z(k) - center.1);
                            (dx * dx + dz * dz, k)
                        })
                        .collect();
                    farthest.sort_unstable_by_key(|&(d, _)| std::cmp::Reverse(d));
                    for (_, k) in farthest {
                        if private_bytes[p] <= target {
                            break;
                        }
                        if let Some(removed) = cache.remove(&k) {
                            private_bytes[p] -= removed.size;
                        }
                    }
                }
                total += private_bytes[p];
            }
            peak_bytes = peak_bytes.max(total);
        }
    }

    let final_bytes = if mode.shared {
        shared_bytes
    } else {
        private_bytes.iter().sum()
    };

    SimResult {
        peak_bytes,
        final_bytes,
        total_encodes,
        total_encode_bytes,
        max_tick_encodes,
        stale_sends,
        prune_events,
        stream_hash,
        wall_ms: start.elapsed().as_millis(),
    }
}

fn mib(bytes: usize) -> f64 {
    bytes as f64 / 1024.0 / 1024.0
}

fn mode_json(r: &SimResult, policy: &str) -> serde_json::Value {
    json!({
        "policy": policy,
        "peak_bytes": r.peak_bytes,
        "final_bytes": r.final_bytes,
        "total_encodes": r.total_encodes,
        "total_encode_bytes": r.total_encode_bytes,
        "max_tick_encodes": r.max_tick_encodes,
        "stale_sends": r.stale_sends,
        "prune_events": r.prune_events,
        "wall_ms": r.wall_ms,
    })
}

#[allow(clippy::too_many_lines)]
fn main() {
    // 闸门 A：洁净旅途（无变异）两模式出站流必须逐哈希一致；
    // 洁净旅途同时隔离「共享去重」的纯编码收益（无变异重编码噪声）
    let clean_legacy = simulate(&Mode {
        shared: false,
        mutations_per_tick: 0,
    });
    let clean_shared = simulate(&Mode {
        shared: true,
        mutations_per_tick: 0,
    });
    assert_eq!(
        clean_legacy.stream_hash, clean_shared.stream_hash,
        "闸门 A 失败：无变异旅途下两模式出站流不一致（红线 2）"
    );
    assert_eq!(
        clean_legacy.stale_sends, 0,
        "洁净旅途 legacy 不应有陈旧发送"
    );
    assert_eq!(
        clean_shared.stale_sends, 0,
        "洁净旅途 shared 不应有陈旧发送"
    );

    // 稀疏变异（2 次/tick 全世界）：贴近真实服务器的安静运行点
    let sparse_legacy = simulate(&Mode {
        shared: false,
        mutations_per_tick: 2,
    });
    let sparse_shared = simulate(&Mode {
        shared: true,
        mutations_per_tick: 2,
    });
    assert_eq!(
        sparse_shared.stale_sends, 0,
        "闸门 B 失败：稀疏场景出现陈旧发送"
    );

    // 稠密变异（25 次/tick）：压力场景，量化代数失效的重编码上界
    let dense_legacy = simulate(&Mode {
        shared: false,
        mutations_per_tick: MUTATIONS_PER_TICK,
    });
    let dense_shared = simulate(&Mode {
        shared: true,
        mutations_per_tick: MUTATIONS_PER_TICK,
    });
    assert_eq!(
        dense_shared.stale_sends, 0,
        "闸门 B 失败：稠密场景出现陈旧发送"
    );

    println!("| 场景 | 模式 | 缓存峰值 | 收尾驻留 | 总编码 | 单 tick 峰值 | 陈旧发送 |");
    println!("|---|---|---:|---:|---:|---:|---:|");
    for (scene, legacy, shared) in [
        ("洁净（隔离去重收益）", &clean_legacy, &clean_shared),
        (
            "稀疏变异 2/tick（真实运行点）",
            &sparse_legacy,
            &sparse_shared,
        ),
        ("稠密变异 25/tick（压力上界）", &dense_legacy, &dense_shared),
    ] {
        println!(
            "| {scene} | legacy 私有 | {:.1} MiB | {:.1} MiB | {} | {} | {} |",
            mib(legacy.peak_bytes),
            mib(legacy.final_bytes),
            legacy.total_encodes,
            legacy.max_tick_encodes,
            legacy.stale_sends,
        );
        println!(
            "| {scene} | shared 共享+代数 | {:.1} MiB | {:.1} MiB | {} | {} | {} |",
            mib(shared.peak_bytes),
            mib(shared.final_bytes),
            shared.total_encodes,
            shared.max_tick_encodes,
            shared.stale_sends,
        );
    }
    println!();
    println!(
        "洁净旅途：编码次数 {} → {}（共享去重 -{:.1}%），出站流哈希一致（闸门 A 通过）",
        clean_legacy.total_encodes,
        clean_shared.total_encodes,
        (clean_legacy.total_encodes - clean_shared.total_encodes) as f64
            / clean_legacy.total_encodes as f64
            * 100.0,
    );
    // 校正口径：legacy 若也要做到零陈旧，须为每次陈旧发送补一次重编码
    let adjusted = dense_legacy.total_encodes + dense_legacy.stale_sends;
    println!(
        "稠密旅途：shared 陈旧 0（闸门 B 通过）；legacy 陈旧 {} ——校正同正确性口径编码数 {} → {}（-{:.1}%）",
        dense_legacy.stale_sends,
        adjusted,
        dense_shared.total_encodes,
        (adjusted - dense_shared.total_encodes) as f64 / adjusted as f64 * 100.0,
    );
    println!(
        "缓存峰值（稠密）：{:.1} → {:.1} MiB（-{:.1}%）",
        mib(dense_legacy.peak_bytes),
        mib(dense_shared.peak_bytes),
        (dense_legacy.peak_bytes - dense_shared.peak_bytes) as f64 / dense_legacy.peak_bytes as f64
            * 100.0,
    );

    let json = json!({
        "round": 7,
        "subject": "区块编码缓存按世界共享 + 内容改动代数失效",
        "gates": {
            "clean_journey_stream_hash_equal": true,
            "shared_mode_stale_sends_zero_all_scenarios": true,
        },
        "workload": {
            "players": PLAYERS,
            "ticks": TICKS,
            "view_distance": VIEW_DISTANCE,
            "wobble_per_tick": WOBBLE_PER_TICK,
            "scenarios": "洁净 0 / 稀疏 2 / 稠密 25 次变异每 tick",
            "note": "缓存只记尺寸（坐标的确定性函数），两模式面对同一世界与同一变异流",
        },
        "clean": {
            "legacy": mode_json(&clean_legacy, "每玩家私有 32 MiB，新鲜度=弱引用+Arc（0.3.19 语义复刻）"),
            "shared": mode_json(&clean_shared, "世界共享 64 MiB + 代数失效（生产语义复刻）"),
            "stream_hash_legacy": clean_legacy.stream_hash,
            "stream_hash_shared": clean_shared.stream_hash,
        },
        "sparse": {
            "legacy": mode_json(&sparse_legacy, "同上"),
            "shared": mode_json(&sparse_shared, "同上"),
        },
        "dense": {
            "legacy": mode_json(&dense_legacy, "同上"),
            "shared": mode_json(&dense_shared, "同上"),
            "correctness_adjusted_legacy_encodes": adjusted,
        },
        "date": "2026-10-04",
    });

    let path = std::path::Path::new("note/report/perf/round7-shared-chunk-cache.json");
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
}
