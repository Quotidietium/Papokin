//! region 序列化器缓存内存基准（性能优化轮次 1，见 note/report/perf/）。
//!
//! 工作负载模拟运行中服务器的真实缓存行为：区块全部处于
//! watched（加载）状态，经历「初始地形保存 → 多轮增量保存 +
//! 强制落盘（自动保存）→ 抽样读回校验」。读回阶段逐字节校验
//! 负载，作为数据完整性闸门——驱逐逻辑若丢数据此处必现。
//!
//! 两种模式：
//! - 子进程模式 `--child --budget-mib N`：跑一遍工作负载，
//!   标准输出最后一行打印单次结果 JSON。
//! - 父进程模式（默认）：以 0 / 32 / 8 MiB 预算各派生一个
//!   子进程，汇总对比表并写出对比 JSON。
//!
//! 0 MiB 预算等价于优化前（0.3.14）的无界缓存行为，因此
//! 「0 vs 32/8」即优化前后的同工作负载对比。

#![allow(clippy::print_stdout)]

use std::{
    error::Error,
    path::PathBuf,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use bytes::Bytes;
use papokin_config::chunk::AnvilChunkConfig;
use papokin_util::math::vector2::Vector2;
use papokin_world::{
    chunk::{
        ChunkReadingError, ChunkSerializingError, ChunkWritingError,
        format::anvil::{AnvilChunkFile, SingleChunkDataSerializer},
        io::{Dirtiable, FileIO, LoadedData, file_manager::PathFromLevelFolder},
    },
    level::LevelFolder,
};
use serde_json::{Value, json};
use tokio::sync::mpsc;

/// 基准负载载体：坐标 + 定长伪随机负载，压缩后体积≈原文。
struct PayloadChunk {
    x: i32,
    z: i32,
    payload: Vec<u8>,
    dirty: AtomicBool,
}

impl PayloadChunk {
    fn new(x: i32, z: i32, payload: Vec<u8>) -> Arc<Self> {
        Arc::new(Self {
            x,
            z,
            payload,
            dirty: AtomicBool::new(true),
        })
    }
}

impl Dirtiable for PayloadChunk {
    fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Relaxed)
    }
    fn mark_dirty(&self, flag: bool) {
        self.dirty.store(flag, Ordering::Relaxed);
    }
}

impl SingleChunkDataSerializer for PayloadChunk {
    fn to_bytes(&self) -> Result<Bytes, ChunkSerializingError> {
        Ok(Bytes::copy_from_slice(&self.payload))
    }

    fn from_bytes(bytes: &Bytes, pos: Vector2<i32>) -> Result<Self, ChunkReadingError> {
        Ok(Self {
            x: pos.x,
            z: pos.y,
            payload: bytes.to_vec(),
            dirty: AtomicBool::new(false),
        })
    }

    fn position(&self) -> (i32, i32) {
        (self.x, self.z)
    }
}

impl PathFromLevelFolder for PayloadChunk {
    fn file_path(folder: &LevelFolder, file_name: &str) -> PathBuf {
        folder.region_folder.join(file_name)
    }
}

type Manager =
    papokin_world::chunk::io::file_manager::ChunkFileManager<AnvilChunkFile<PayloadChunk>>;

/// xorshift 伪随机字节：不可压缩，保证缓存账完全由负载长度决定。
fn pseudo_random_bytes(len: usize, seed: u64) -> Vec<u8> {
    let mut state = seed | 1;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 32) as u8
        })
        .collect()
}

fn rss_bytes() -> u64 {
    let mut sys = sysinfo::System::new();
    let pid = sysinfo::Pid::from_u32(std::process::id());
    let _ = sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    sys.process(pid).map_or(0, sysinfo::Process::memory)
}

struct Args {
    child: bool,
    budget_mib: u64,
    regions: i32,
    chunks_per_region: i32,
    payload_kib: usize,
    rounds: u32,
    out: PathBuf,
}

fn parse_args() -> Result<Args, Box<dyn Error>> {
    let mut args = Args {
        child: false,
        budget_mib: 0,
        regions: 128,
        chunks_per_region: 64,
        payload_kib: 8,
        rounds: 3,
        out: PathBuf::from("note/report/perf/round1-region-cache.json"),
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--child" => args.child = true,
            "--budget-mib" => {
                args.budget_mib = it
                    .next()
                    .ok_or("--budget-mib 缺值")?
                    .parse()
                    .map_err(|_| "--budget-mib 需为非负整数")?;
            }
            "--regions" => {
                args.regions = it
                    .next()
                    .ok_or("--regions 缺值")?
                    .parse()
                    .map_err(|_| "--regions 需为整数")?;
            }
            "--chunks" => {
                args.chunks_per_region = it
                    .next()
                    .ok_or("--chunks 缺值")?
                    .parse()
                    .map_err(|_| "--chunks 需为整数")?;
            }
            "--payload-kib" => {
                args.payload_kib = it
                    .next()
                    .ok_or("--payload-kib 缺值")?
                    .parse()
                    .map_err(|_| "--payload-kib 需为整数")?;
            }
            "--rounds" => {
                args.rounds = it
                    .next()
                    .ok_or("--rounds 缺值")?
                    .parse()
                    .map_err(|_| "--rounds 需为整数")?;
            }
            "--out" => args.out = PathBuf::from(it.next().ok_or("--out 缺值")?),
            other => return Err(format!("未知参数 {other}").into()),
        }
    }
    Ok(args)
}

fn region_coords(region_x: i32, count: i32) -> Vec<Vector2<i32>> {
    (0..count)
        .map(|i| Vector2::new(region_x * 32 + i % 32, i / 32))
        .collect()
}

async fn save_region(
    manager: &Manager,
    folder: &Arc<LevelFolder>,
    region_x: i32,
    count: i32,
    payload_len: usize,
    seed_base: u64,
    forced: bool,
) -> Result<(), ChunkWritingError> {
    let chunks: Vec<(Vector2<i32>, Arc<PayloadChunk>)> = region_coords(region_x, count)
        .into_iter()
        .map(|at| {
            let seed = seed_base
                .wrapping_mul(1_000_003)
                .wrapping_add(u64::try_from(at.x).unwrap_or(0))
                .wrapping_add(u64::try_from(at.y).unwrap_or(0) << 32);
            (
                at,
                PayloadChunk::new(at.x, at.y, pseudo_random_bytes(payload_len, seed)),
            )
        })
        .collect();
    if forced {
        manager.save_chunks_forced(folder, chunks).await
    } else {
        manager.save_chunks(folder, chunks).await
    }
}

/// 读回指定坐标并逐字节校验负载；返回校验是否全部通过。
async fn verify_chunks(
    manager: &Manager,
    folder: &Arc<LevelFolder>,
    coords: &[Vector2<i32>],
    payload_len: usize,
    seed_base: u64,
) -> Result<bool, Box<dyn Error>> {
    let (send, mut recv) = mpsc::channel(64);
    manager.fetch_chunks(folder, coords, send).await;
    let mut ok = true;
    while let Some(data) = recv.recv().await {
        match data {
            LoadedData::Loaded(chunk) => {
                let seed = seed_base
                    .wrapping_mul(1_000_003)
                    .wrapping_add(u64::try_from(chunk.x).unwrap_or(0))
                    .wrapping_add(u64::try_from(chunk.z).unwrap_or(0) << 32);
                if chunk.payload != pseudo_random_bytes(payload_len, seed) {
                    ok = false;
                }
            }
            _ => ok = false,
        }
    }
    Ok(ok)
}

async fn run_child(args: &Args) -> Result<Value, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let region_folder = dir.path().join("region");
    std::fs::create_dir_all(&region_folder)?;
    let folder = Arc::new(LevelFolder {
        root_folder: dir.path().to_path_buf(),
        dim_folder: dir.path().to_path_buf(),
        region_folder,
        entities_folder: dir.path().join("entities"),
        poi_folder: dir.path().join("poi"),
    });

    let budget_bytes = usize::try_from(args.budget_mib).unwrap_or(usize::MAX) * 1024 * 1024;
    let manager = Manager::new(AnvilChunkConfig::default(), budget_bytes);
    let payload_len = args.payload_kib * 1024;

    let rss_start = rss_bytes();
    let mut rss_peak = rss_start;

    // 阶段 1：全部区域 watched（模拟玩家加载）+ 初始地形保存
    let t0 = Instant::now();
    for region_x in 0..args.regions {
        manager
            .watch_chunks(&folder, &region_coords(region_x, args.chunks_per_region))
            .await;
        save_region(
            &manager,
            &folder,
            region_x,
            args.chunks_per_region,
            payload_len,
            7,
            true,
        )
        .await?;
        rss_peak = rss_peak.max(rss_bytes());
    }
    let t_populate = t0.elapsed();
    let rss_after_populate = rss_bytes();
    let manager_after_populate = manager.cached_bytes_total().await;

    // 阶段 2：增量保存（产生 pending）+ 强制落盘（转干净），
    // 模拟自动保存节奏下的缓存吞吐
    let t1 = Instant::now();
    for round in 0..args.rounds {
        for region_x in 0..args.regions {
            save_region(
                &manager,
                &folder,
                region_x,
                1,
                payload_len,
                100 + u64::from(round),
                false,
            )
            .await?;
            save_region(&manager, &folder, region_x, 2, payload_len, 7, true).await?;
        }
        rss_peak = rss_peak.max(rss_bytes());
    }
    let t_rounds = t1.elapsed();
    let manager_after_rounds = manager.cached_bytes_total().await;

    // 阶段 3：抽样读回校验（首/中/尾各 2 区块）——数据完整性闸门
    let t2 = Instant::now();
    let mut verify_ok = true;
    for region_x in [0, args.regions / 2, args.regions - 1] {
        let coords = region_coords(region_x, 2);
        if !verify_chunks(&manager, &folder, &coords, payload_len, 7).await? {
            verify_ok = false;
        }
    }
    let t_readback = t2.elapsed();

    let rss_end = rss_bytes();
    let manager_end = manager.cached_bytes_total().await;
    let evicted = manager.evicted_total();

    // 显式落盘收尾，保证临时目录清理前无悬挂写
    manager.block_and_await_ongoing_tasks().await;
    manager.flush_pending_writes().await;

    Ok(json!({
        "budget_mib": args.budget_mib,
        "regions": args.regions,
        "chunks_per_region": args.chunks_per_region,
        "payload_kib": args.payload_kib,
        "rounds": args.rounds,
        "rss_start_mib": rss_start as f64 / 1_048_576.0,
        "rss_after_populate_mib": rss_after_populate as f64 / 1_048_576.0,
        "rss_peak_mib": rss_peak as f64 / 1_048_576.0,
        "rss_end_mib": rss_end as f64 / 1_048_576.0,
        "manager_after_populate_mib": manager_after_populate as f64 / 1_048_576.0,
        "manager_after_rounds_mib": manager_after_rounds as f64 / 1_048_576.0,
        "manager_end_mib": manager_end as f64 / 1_048_576.0,
        "evicted_total": evicted,
        "verify_ok": verify_ok,
        "t_populate_ms": t_populate.as_millis(),
        "t_rounds_ms": t_rounds.as_millis(),
        "t_readback_ms": t_readback.as_millis(),
    }))
}

fn mib(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

fn run_parent(args: &Args) -> Result<(), Box<dyn Error>> {
    let exe = std::env::current_exe()?;
    let budgets = [0u64, 32, 8];
    let mut results = Vec::new();
    for budget in budgets {
        println!("…… 运行子进程：预算 {budget} MiB");
        let output = Command::new(&exe)
            .args([
                "--child",
                "--budget-mib",
                &budget.to_string(),
                "--regions",
                &args.regions.to_string(),
                "--chunks",
                &args.chunks_per_region.to_string(),
                "--payload-kib",
                &args.payload_kib.to_string(),
                "--rounds",
                &args.rounds.to_string(),
            ])
            .output()?;
        if !output.status.success() {
            return Err(format!(
                "子进程（预算 {budget}）失败：{}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }
        let stdout = String::from_utf8(output.stdout)?;
        let last_line = stdout
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .ok_or("子进程无输出")?;
        let value: Value = serde_json::from_str(last_line)?;
        if !value
            .get("verify_ok")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err(format!("子进程（预算 {budget}）数据校验失败").into());
        }
        results.push(value);
    }

    println!();
    println!(
        "| 预算 (MiB) | 峰值 RSS | 结束 RSS | 缓存总账(收尾) | 驱逐数 | 填充耗时 | 轮次耗时 | 读回耗时 |"
    );
    println!("|---:|---:|---:|---:|---:|---:|---:|---:|");
    for r in &results {
        println!(
            "| {} | {:.1} MiB | {:.1} MiB | {:.1} MiB | {} | {} ms | {} ms | {} ms |",
            mib(r, "budget_mib") as u64,
            mib(r, "rss_peak_mib"),
            mib(r, "rss_end_mib"),
            mib(r, "manager_end_mib"),
            r.get("evicted_total").and_then(Value::as_u64).unwrap_or(0),
            r.get("t_populate_ms").and_then(Value::as_u64).unwrap_or(0),
            r.get("t_rounds_ms").and_then(Value::as_u64).unwrap_or(0),
            r.get("t_readback_ms").and_then(Value::as_u64).unwrap_or(0),
        );
    }

    let combined = json!({
        "workload": {
            "regions": args.regions,
            "chunks_per_region": args.chunks_per_region,
            "payload_kib": args.payload_kib,
            "rounds": args.rounds,
        },
        "runs": results,
    });
    if let Some(parent) = args.out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&args.out, serde_json::to_string_pretty(&combined)?)?;
    println!();
    println!("对比 JSON 已写入 {}", args.out.display());
    Ok(())
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = parse_args()?;
    if args.child {
        let value = run_child(&args).await?;
        println!("{value}");
        Ok(())
    } else {
        run_parent(&args)
    }
}
