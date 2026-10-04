use bytes::Bytes;
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use std::num::NonZero;
use std::sync::{Arc, Weak};

use crate::net::java::chunk_data::{CChunkData, ChunkLightExt};
use papokin_protocol::codec::var_int::VarInt;
use papokin_protocol::java::client::play::{
    CChunkBatchEnd, CChunkBatchStart, CLightUpdate, CUnloadChunk,
};
use papokin_protocol::ser::NetworkWriteExt;
use papokin_protocol::{ClientPacket, MultiVersionJavaPacket};
use papokin_util::math::vector2::Vector2;
use papokin_util::version::JavaMinecraftVersion;
use papokin_world::chunk::ChunkData;
use papokin_world::cylindrical_chunk_iterator::Cylindrical;
use papokin_world::level::{Level, SyncChunk};

use crate::net::java::JavaClient;

const MIN_CHUNKS_PER_TICK: f32 = 0.1;
const MAX_CHUNKS_PER_TICK: f32 = 500.0;
const INITIAL_CHUNKS_PER_TICK: f32 = 9.0;
const MAX_CONCURRENT_BATCHES: u16 = 10;

pub struct PreparedChunk {
    pub position: Vector2<i32>,
    pub chunk: SyncChunk,
}

pub struct PreparedBatch {
    pub chunks: Vec<PreparedChunk>,
    pub epoch_snapshot: u32,
    pub target_version: JavaMinecraftVersion,
}

#[derive(Clone)]
pub struct EncodedChunk {
    pub position: Vector2<i32>,
    pub payload: Bytes,
    pub light_payload: Option<Bytes>,
    pub chunk_ref: Weak<ChunkData>,
}

impl EncodedChunk {
    #[must_use]
    pub fn is_fresh_for(&self, candidate: &PreparedChunk) -> bool {
        let Some(held) = self.chunk_ref.upgrade() else {
            return false;
        };

        self.position == candidate.position && Arc::ptr_eq(&held, &candidate.chunk)
    }

    /// 条目的线上字节数（编码缓存预算记账用）
    #[must_use]
    pub fn encoded_bytes(&self) -> usize {
        self.payload.len() + self.light_payload.as_ref().map_or(0, Bytes::len)
    }
}

#[derive(Debug)]
pub struct ChunkSender {
    pub pending_chunks: FxHashSet<Vector2<i32>>,
    sent_chunks: FxHashSet<Vector2<i32>>,
    pub in_flight_batches: u16,
    pub desired_rate: f32,
    pub send_quota: f32,
    pub max_in_flight: u16,
    /// 所属世界，用于触发区块卸载插件事件。
    owner_world: Option<Weak<crate::world::World>>,
    /// 所属玩家，用于触发区块卸载插件事件。
    owner_uuid: Option<uuid::Uuid>,
}

impl ChunkSender {
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending_chunks: FxHashSet::default(),
            sent_chunks: FxHashSet::default(),
            in_flight_batches: 0,
            desired_rate: INITIAL_CHUNKS_PER_TICK,
            send_quota: 0.0,
            max_in_flight: 1,
            owner_world: None,
            owner_uuid: None,
        }
    }

    /// 将发送者绑定到其所属的玩家与世界，以便区块卸载时能够
    /// 上报给插件。
    pub fn set_owner(&mut self, world: &Arc<crate::world::World>, player_uuid: uuid::Uuid) {
        self.owner_world = Some(Arc::downgrade(world));
        self.owner_uuid = Some(player_uuid);
    }

    pub fn reset(&mut self) {
        self.pending_chunks.clear();
        self.sent_chunks.clear();
        self.in_flight_batches = 0;
        self.send_quota = 0.0;
    }

    #[must_use]
    pub fn is_chunk_sent(&self, pos: &Vector2<i32>) -> bool {
        self.sent_chunks.contains(pos)
    }

    /// 原版 `ChunkMap.isChunkTracked` -> 该玩家的区块数据包已在队列中。
    #[must_use]
    pub fn is_chunk_ready(&self, pos: &Vector2<i32>) -> bool {
        self.sent_chunks.contains(pos)
    }

    #[must_use]
    pub fn sent_chunks_count(&self) -> usize {
        self.sent_chunks.len()
    }

    pub const fn on_batch_acknowledged(&mut self, client_requested_rate: f32) -> bool {
        if self.in_flight_batches == 0 {
            return false;
        }

        self.in_flight_batches = self.in_flight_batches.saturating_sub(1);
        self.desired_rate = if client_requested_rate.is_nan() {
            MIN_CHUNKS_PER_TICK
        } else {
            client_requested_rate.clamp(MIN_CHUNKS_PER_TICK, MAX_CHUNKS_PER_TICK)
        };

        if self.in_flight_batches == 0 {
            self.send_quota = 1.0;
        }

        self.max_in_flight = MAX_CONCURRENT_BATCHES;
        true
    }

    pub fn enqueue_chunk(&mut self, pos: Vector2<i32>) {
        self.sent_chunks.remove(&pos);
        self.pending_chunks.insert(pos);
    }

    pub fn unload_chunk(&mut self, client: &JavaClient, pos: Vector2<i32>) {
        self.pending_chunks.remove(&pos);
        if self.sent_chunks.remove(&pos) && !client.is_closed() {
            client.try_send_packet(&CUnloadChunk::new(pos.x, pos.y));
        }
        // 区块卸载钩子（PlayerChunkUnloadEvent）不在这里触发：
        // 调用方（chunker）持有本 ChunkSender 的 Mutex，锁内
        // fire_blocking 会与触碰同一锁的插件回调互锁。事件改由
        // 调用方在释放锁后统一派发。
    }

    fn collect_sorted_candidates(
        &self,
        level: &Level,
        center: Vector2<i32>,
        view_distance: NonZero<u8>,
    ) -> Vec<PreparedChunk> {
        let quota_limit = self.send_quota.floor() as usize;
        let mut ready = Vec::with_capacity(quota_limit);

        // 如果 pending_chunks 很小，直接排序可以避免扫描偏移量。
        if self.pending_chunks.len() <= 16 {
            let mut sorted: Vec<Vector2<i32>> = self.pending_chunks.iter().copied().collect();
            sorted.sort_unstable_by_key(|pos| {
                let dx = (pos.x - center.x).unsigned_abs() as u64;
                let dz = (pos.y - center.y).unsigned_abs() as u64;
                dx * dx + dz * dz
            });

            for pos in sorted {
                if ready.len() >= quota_limit {
                    break;
                }

                if let Some(chunk) = level.loaded_chunks.get(&pos) {
                    ready.push(PreparedChunk {
                        position: pos,
                        chunk: chunk.value().clone(),
                    });
                }
            }
        } else {
            // 复用预编译的圆柱形区块视野查找表（已按从中心向外的顺序排序）。
            let offsets = Cylindrical::get_offsets(view_distance.get());
            for &(dx, dy) in offsets {
                if ready.len() >= quota_limit {
                    break;
                }

                let pos = Vector2::new(center.x + i32::from(dx), center.y + i32::from(dy));
                if self.pending_chunks.contains(&pos)
                    && let Some(chunk) = level.loaded_chunks.get(&pos)
                {
                    ready.push(PreparedChunk {
                        position: pos,
                        chunk: chunk.value().clone(),
                    });
                }
            }

            // 针对预计算表之外任何待处理区块的回退
            if ready.is_empty() {
                for &pos in &self.pending_chunks {
                    if ready.len() >= quota_limit {
                        break;
                    }
                    if let Some(chunk) = level.loaded_chunks.get(&pos) {
                        ready.push(PreparedChunk {
                            position: pos,
                            chunk: chunk.value().clone(),
                        });
                    }
                }
            }
        }

        ready
    }

    pub fn prepare_batch(
        &mut self,
        level: &Level,
        player_chunk: Vector2<i32>,
        view_distance: NonZero<u8>,
        epoch: u32,
        version: JavaMinecraftVersion,
    ) -> Option<PreparedBatch> {
        if version >= JavaMinecraftVersion::V_1_20_2 && self.in_flight_batches >= self.max_in_flight
        {
            return None;
        }

        let max_batch = self.desired_rate.max(1.0);
        self.send_quota = (self.send_quota + self.desired_rate).min(max_batch);

        if self.send_quota < 1.0 || self.pending_chunks.is_empty() {
            return None;
        }

        let candidates = self.collect_sorted_candidates(level, player_chunk, view_distance);
        if candidates.is_empty() {
            return None;
        }

        Some(PreparedBatch {
            chunks: candidates,
            epoch_snapshot: epoch,
            target_version: version,
        })
    }

    pub fn encode_batch(
        batch: &PreparedBatch,
        cache: &mut FxHashMap<Vector2<i32>, EncodedChunk>,
    ) -> Vec<EncodedChunk> {
        let version = batch.target_version;
        let cached_map = &*cache;

        let encoded_results: Vec<Option<EncodedChunk>> = batch
            .chunks
            .par_iter()
            .map(|candidate| {
                let pos = candidate.position;
                if let Some(cached) = cached_map.get(&pos)
                    && cached.is_fresh_for(candidate)
                {
                    return Some(cached.clone());
                }

                let chunk = &candidate.chunk;
                let mut chunk_buf = Vec::with_capacity(32 * 1024);
                if chunk_buf
                    .write_var_int(&VarInt(CChunkData::to_id(version)))
                    .is_err()
                {
                    return None;
                }
                if CChunkData(chunk)
                    .write_packet_data(&mut chunk_buf, &version)
                    .is_err()
                {
                    return None;
                }

                let light_payload = if version >= JavaMinecraftVersion::V_1_14
                    && version < JavaMinecraftVersion::V_1_18
                {
                    CLightUpdate::from_chunk(chunk, version)
                        .ok()
                        .and_then(|light_packet| {
                            let mut light_buf = Vec::new();
                            (light_buf
                                .write_var_int(&VarInt(CLightUpdate::to_id(version)))
                                .is_ok()
                                && light_packet
                                    .write_packet_data(&mut light_buf, &version)
                                    .is_ok())
                            .then(|| {
                                light_buf.shrink_to_fit();
                                Bytes::from(light_buf)
                            })
                        })
                } else {
                    None
                };

                // 负载会随编码缓存长期驻留（每玩家上限 8192 条），
                // 收缩掉 32 KiB 预分配的多余容量再移交 Bytes——
                // Bytes::from(Vec) 原样接管底层分配，不收缩则
                // 每条缓存按预分配容量而非实际包长占内存。
                chunk_buf.shrink_to_fit();
                Some(EncodedChunk {
                    position: pos,
                    payload: Bytes::from(chunk_buf),
                    light_payload,
                    chunk_ref: Arc::downgrade(chunk),
                })
            })
            .collect();

        let mut output = Vec::with_capacity(encoded_results.len());
        for encoded in encoded_results.into_iter().flatten() {
            cache.insert(encoded.position, encoded.clone());
            output.push(encoded);
        }

        output
    }

    pub fn commit_batch(
        &mut self,
        batch: &PreparedBatch,
        encoded_chunks: &[EncodedChunk],
        client: &JavaClient,
        current_epoch: u32,
    ) -> Vec<Vector2<i32>> {
        if current_epoch != batch.epoch_snapshot || encoded_chunks.is_empty() {
            return Vec::new();
        }

        let mut dispatched_positions = Vec::with_capacity(encoded_chunks.len());
        let version = batch.target_version;

        if version >= JavaMinecraftVersion::V_1_20_2 {
            client.try_send_packet(&CChunkBatchStart);
        }

        for chunk in encoded_chunks {
            if !self.pending_chunks.contains(&chunk.position) {
                continue;
            }

            client.try_enqueue_packet(chunk.payload.clone());
            if let Some(ref light) = chunk.light_payload {
                client.try_enqueue_packet(light.clone());
            }

            self.pending_chunks.remove(&chunk.position);
            self.sent_chunks.insert(chunk.position);
            dispatched_positions.push(chunk.position);
        }

        let sent_count = dispatched_positions.len();
        if sent_count > 0 {
            if version >= JavaMinecraftVersion::V_1_20_2 {
                client.try_send_packet(&CChunkBatchEnd::new(sent_count as u16));
                self.in_flight_batches = self.in_flight_batches.saturating_add(1);
            }

            self.send_quota -= sent_count as f32;
        }

        dispatched_positions
    }
}

impl Default for ChunkSender {
    fn default() -> Self {
        Self::new()
    }
}

/// 以最远优先逐出编码缓存条目，直到总字节数不超过
/// `target_bytes`，返回逐出条数。距离按区块坐标平方和排序
///（与圆柱视距同一序）：玩家短期内不会回访远处区块，
/// 远者缓存命中率天然最低；近处热条目（当前视距内）
/// 全部保留，杜绝整体清空引发的全量重序列化风暴。
#[must_use]
pub fn prune_encode_cache<S: std::hash::BuildHasher>(
    cache: &mut std::collections::HashMap<Vector2<i32>, EncodedChunk, S>,
    center: Vector2<i32>,
    target_bytes: usize,
) -> usize {
    let mut total: usize = cache.values().map(EncodedChunk::encoded_bytes).sum();
    if total <= target_bytes {
        return 0;
    }
    let mut farthest: Vec<(i64, Vector2<i32>)> = cache
        .keys()
        .map(|pos| {
            let dx = i64::from(pos.x - center.x);
            let dz = i64::from(pos.y - center.y);
            (dx * dx + dz * dz, *pos)
        })
        .collect();
    farthest.sort_unstable_by_key(|entry| std::cmp::Reverse(entry.0));

    let mut evicted = 0;
    for (_, pos) in farthest {
        if total <= target_bytes {
            break;
        }
        if let Some(removed) = cache.remove(&pos) {
            total -= removed.encoded_bytes();
            evicted += 1;
        }
    }
    evicted
}

#[cfg(test)]
mod tests {
    use super::*;
    use papokin_data::Block;

    /// 构造带少量方块的测试区块
    fn populated_chunk(fill: usize) -> SyncChunk {
        let chunk = ChunkData::empty(0, 0);
        for i in 0..fill {
            chunk
                .section
                .set_block_absolute_y(i, 64, i, Block::STONE.default_state.id);
        }
        Arc::new(chunk)
    }

    #[test]
    fn encode_batch_payload_matches_direct_serialization_and_cache_reuses() {
        let version = JavaMinecraftVersion::V_1_21_11;
        let batch = PreparedBatch {
            chunks: (0..3)
                .map(|i| PreparedChunk {
                    position: Vector2::new(i, 0),
                    chunk: populated_chunk(8),
                })
                .collect(),
            epoch_snapshot: 1,
            target_version: version,
        };

        let mut cache = FxHashMap::default();
        let encoded = ChunkSender::encode_batch(&batch, &mut cache);
        assert_eq!(encoded.len(), 3, "全部区块应编码成功");

        // 线上字节与直接序列化完全一致（容量裁剪不得改动内容）
        for (candidate, enc) in batch.chunks.iter().zip(&encoded) {
            let mut expected = Vec::new();
            expected
                .write_var_int(&VarInt(CChunkData::to_id(version)))
                .expect("写入 id 不应失败");
            CChunkData(&candidate.chunk)
                .write_packet_data(&mut expected, &version)
                .expect("直接序列化不应失败");
            assert_eq!(
                &enc.payload[..],
                &expected[..],
                "编码负载应与直接序列化逐字节一致"
            );
            assert!(enc.light_payload.is_none(), "1.21.11 不应有独立光照包");
        }

        // 缓存命中路径：同批再次编码应复用同一分配而非重新序列化
        let encoded2 = ChunkSender::encode_batch(&batch, &mut cache);
        assert_eq!(encoded2.len(), 3);
        for (first, second) in encoded.iter().zip(&encoded2) {
            assert_eq!(
                first.payload.as_ptr(),
                second.payload.as_ptr(),
                "缓存命中应共享同一分配而非重新序列化"
            );
        }
    }
    #[test]
    fn prune_encode_cache_evicts_farthest_first_within_budget() {
        let version = JavaMinecraftVersion::V_1_21_11;
        let center = Vector2::new(0, 0);
        // 3 条近（视距内量级）+ 3 条远（跨世界/远征量级）
        let batch = PreparedBatch {
            chunks: [(1, 0), (0, 2), (2, 1), (40, 0), (0, 41), (39, 40)]
                .iter()
                .map(|&(x, z)| PreparedChunk {
                    position: Vector2::new(x, z),
                    chunk: populated_chunk(8),
                })
                .collect(),
            epoch_snapshot: 1,
            target_version: version,
        };
        let mut cache = FxHashMap::default();
        let encoded = ChunkSender::encode_batch(&batch, &mut cache);
        assert_eq!(encoded.len(), 6);

        let total: usize = cache.values().map(EncodedChunk::encoded_bytes).sum();
        let target = total / 2;

        let evicted = prune_encode_cache(&mut cache, center, target);
        assert_eq!(evicted, 3, "等尺寸条目减半须逐出 3 条: {evicted}");

        // 近处三条必须保留，远处三条必须逐出
        for pos in [Vector2::new(1, 0), Vector2::new(0, 2), Vector2::new(2, 1)] {
            assert!(cache.contains_key(&pos), "近处条目应保留: {pos:?}");
        }
        for pos in [
            Vector2::new(40, 0),
            Vector2::new(0, 41),
            Vector2::new(39, 40),
        ] {
            assert!(!cache.contains_key(&pos), "远处条目应逐出: {pos:?}");
        }

        let after: usize = cache.values().map(EncodedChunk::encoded_bytes).sum();
        assert!(after <= target, "逐出后应降至目标以内: {after} <= {target}");

        // 预算已足时再次调用应为无操作
        assert_eq!(prune_encode_cache(&mut cache, center, target), 0);

        // 保留条目的线上字节仍与直接序列化一致（逐出不触碰内容）
        let retained = &cache[&Vector2::new(1, 0)];
        let mut expected = Vec::new();
        expected
            .write_var_int(&VarInt(CChunkData::to_id(version)))
            .expect("写入 id 不应失败");
        CChunkData(&batch.chunks[0].chunk)
            .write_packet_data(&mut expected, &version)
            .expect("直接序列化不应失败");
        assert_eq!(&retained.payload[..], &expected[..]);
    }
}
