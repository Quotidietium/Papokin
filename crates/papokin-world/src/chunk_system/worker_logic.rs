use super::chunk_state::{Chunk, StagedChunkEnum};
use super::generation_cache::Cache;
use super::{ChunkPos, IOLock};
use crate::ProtoChunk;
use crate::chunk::format::LightContainer;
use crate::chunk::io::LoadedData::Loaded;
use crate::chunk::io::{FileIO, LoadedData, run_blocking};
use crate::level::Level;
use papokin_config::lighting::LightingEngineConfig;
use papokin_data::chunk::ChunkStatus;
use std::collections::hash_map::Entry;
use std::sync::Arc;
use std::sync::atomic::Ordering::Relaxed;
use tracing::{debug, error, warn};

pub enum RecvChunk {
    IO(Chunk),
    Generation(Cache),
    GenerationFailure {
        pos: ChunkPos,
        stage: StagedChunkEnum,
        error: String,
    },
}

/// 根据当前光照配置检查区块是否需要重新计算光照
/// 若区块具有均匀光照（来自全亮/全暗模式），但服务器
/// 现在正以默认模式运行（需要正确的光照计算）
fn needs_relighting(chunk: &crate::chunk::ChunkData, config: LightingEngineConfig) -> bool {
    if config != LightingEngineConfig::Default {
        return false;
    }

    // 如果区块声称已点亮，就相信它。
    if chunk.light_populated.load(Relaxed) {
        return false;
    }

    let engine = chunk
        .light_engine
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    // 扫描任何复杂光照数据
    let has_complex_light = engine.sky_light.iter().any(|lc| match lc {
        LightContainer::Full(data) => data.iter().any(|&b| b != 0x00 && b != 0xFF),
        LightContainer::Empty(val) => *val != 0 && *val != 15,
    }) || engine.block_light.iter().any(|lc| match lc {
        LightContainer::Full(data) => data.iter().any(|&b| b != 0x00 && b != 0xFF),
        LightContainer::Empty(val) => *val != 0 && *val != 15,
    });

    // 如果它拥有复杂光照，我们无需重新点亮。
    !has_complex_light
}

fn load_proto_chunk(chunk: &crate::chunk::ChunkData, level: &Level) -> ProtoChunk {
    ProtoChunk::from_chunk_data(chunk, &level.world_gen.load())
}

fn process_loaded_chunk(chunk: Arc<crate::chunk::ChunkData>, level: &Level) -> Chunk {
    let pos = ChunkPos::new(chunk.x, chunk.z);
    if chunk.status == ChunkStatus::Full {
        let needs_relight = needs_relighting(&chunk, level.lighting_config);
        if needs_relight {
            debug!("区块 {pos:?} 的光照为均匀填充，降级到 Features 阶段以重新计算光照");

            let mut proto = load_proto_chunk(&chunk, level);

            // 清除所有光照数据
            let section_count = proto.light.sky_light.len();
            proto.light.sky_light = (0..section_count)
                .map(|_| LightContainer::new_empty(15))
                .collect();
            proto.light.block_light = (0..section_count)
                .map(|_| LightContainer::new_empty(0))
                .collect();
            proto.stage = StagedChunkEnum::Features;
            Chunk::Proto(Box::new(proto))
        } else {
            Chunk::Level(chunk)
        }
    } else {
        let proto = load_proto_chunk(&chunk, level);
        Chunk::Proto(Box::new(proto))
    }
}

pub async fn io_read_work(
    recv: Arc<tokio::sync::Mutex<tokio::sync::mpsc::Receiver<Vec<ChunkPos>>>>,
    send: crossbeam::channel::Sender<(ChunkPos, RecvChunk)>,
    level: Arc<Level>,
    lock: IOLock,
) {
    debug!("IO 读取线程启动");

    // 更简洁的循环与异步 recv
    loop {
        let batch = {
            let mut lock_rx = recv.lock().await;
            lock_rx.recv().await
        };
        let Some(batch) = batch else {
            break;
        };
        for pos in &batch {
            // 锁处理
            loop {
                let notified = lock.1.notified();
                if !lock
                    .0
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .contains_key(pos)
                {
                    break;
                }
                notified.await;
            }
        }

        let (t_send, mut t_recv) = tokio::sync::mpsc::channel(1000);

        let batch_len = batch.len();
        let level_clone = level.clone();

        let fetch_task = tokio::spawn(async move {
            level_clone
                .chunk_saver
                .fetch_chunks(&level_clone.level_folder, &batch, t_send)
                .await;
        });

        for _ in 0..batch_len {
            let Some(data) = t_recv.recv().await else {
                break;
            };

            match data {
                Loaded(chunk) => {
                    let pos = ChunkPos::new(chunk.x, chunk.z);
                    let level = level.clone();
                    let result = run_blocking(move || process_loaded_chunk(chunk, &level)).await;
                    let received = match result {
                        Ok(processed) => RecvChunk::IO(processed),
                        Err(err) => RecvChunk::GenerationFailure {
                            pos,
                            stage: StagedChunkEnum::Empty,
                            error: err.to_string(),
                        },
                    };
                    if send.send((pos, received)).is_err() {
                        break;
                    }
                }
                LoadedData::Missing(pos) | LoadedData::Error((pos, _)) => {
                    if send
                        .send((
                            pos,
                            RecvChunk::IO(Chunk::Proto(Box::new(ProtoChunk::new(
                                pos.x,
                                pos.y,
                                &level.world_gen.load(),
                            )))),
                        ))
                        .is_err()
                    {
                        break;
                    }
                }
            }
        }
        let _ = fetch_task.await;
    }
    debug!("IO 读取线程停止");
}

pub async fn io_write_work(
    mut recv: tokio::sync::mpsc::Receiver<Vec<(ChunkPos, Chunk)>>,
    level: Arc<Level>,
    lock: IOLock,
) {
    loop {
        // 不要在此处检查 cancel_token（继续保存区块）
        let Some(data) = recv.recv().await else { break };
        // debug!("io write thread receive chunks size {}", data.len());
        let positions = data.iter().map(|(pos, _)| *pos).collect::<Vec<_>>();
        let level_for_upgrade = level.clone();
        let upgrade_result = run_blocking(move || {
            let mut vec = Vec::with_capacity(data.len());
            for (pos, chunk) in data {
                match chunk {
                    Chunk::Level(chunk) => vec.push((pos, chunk)),
                    Chunk::Proto(chunk) => {
                        let mut temp = Chunk::Proto(chunk);
                        temp.upgrade_to_level_chunk(
                            level_for_upgrade.world_gen.load().dimension(),
                            &level_for_upgrade.lighting_config,
                        );
                        let Chunk::Level(chunk) = temp else { panic!() };
                        vec.push((pos, chunk));
                    }
                }
            }
            vec
        })
        .await;
        let upgrade_failed = match upgrade_result {
            Ok(vec) => {
                if let Err(e) = level
                    .chunk_saver
                    .save_chunks(&level.level_folder, vec)
                    .await
                {
                    error!("保存区块失败：{:?}", e);
                }
                false
            }
            Err(_) => true,
        };

        {
            let mut data = lock
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for i in positions {
                match data.entry(i) {
                    Entry::Occupied(mut entry) => {
                        let rc = entry.get_mut();
                        if *rc <= 1 {
                            entry.remove();
                        } else {
                            *rc -= 1;
                        }
                    }
                    Entry::Vacant(_) => {
                        warn!("io_write: 试图释放不存在的锁条目 {:?}", i);
                    }
                }
            }
        }
        lock.1.notify_waiters();

        if upgrade_failed {
            error!("为保存升级区块失败");
            break;
        }
    }
}

pub fn run_generation(
    pos: ChunkPos,
    mut cache: Cache,
    stage: StagedChunkEnum,
    level: &Level,
) -> RecvChunk {
    let portal = level.world_portal.load_full();
    let Some(portal_ref) = portal.as_deref() else {
        error!("区块生成失败，位置 {pos:?}（{stage:?}）：World portal 未初始化");
        return RecvChunk::GenerationFailure {
            pos,
            stage,
            error: "World portal 未初始化".to_string(),
        };
    };
    // 运行生成并捕获 panic
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cache.advance(
            stage,
            &level.world_gen.load(),
            portal_ref,
            &level.lighting_config,
        );
        cache // 成功时返回缓存
    }));

    match result {
        Ok(cache) => RecvChunk::Generation(cache),
        Err(payload) => {
            let msg = payload
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| {
                    payload
                        .downcast_ref::<String>()
                        .map(std::string::String::as_str)
                })
                .unwrap_or("未知的 panic 负载");

            error!("区块生成失败，位置 {pos:?}（{stage:?}）：{msg}");

            RecvChunk::GenerationFailure {
                pos,
                stage,
                error: msg.to_string(),
            }
        }
    }
}
