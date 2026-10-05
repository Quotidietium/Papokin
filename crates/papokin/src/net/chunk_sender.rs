use bytes::Bytes;
use rayon::prelude::*;
use rustc_hash::FxHashSet;
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

/// 轮次 19：批次借用调用方驻留候选缓冲（原自有 `Vec`，每批次新建），
/// 跨 tick 复用容量
pub struct PreparedBatch<'a> {
    pub chunks: &'a [PreparedChunk],
    pub epoch_snapshot: u32,
    pub target_version: JavaMinecraftVersion,
}

/// 区块批次发送驻留暂存（轮次 19）：候选/编码结果/输出/派发四缓冲，
/// 由 `Player::tick` 持锁横跨「准备→并行编码→提交」全程重填。
/// `Player::tick` 逐玩家单线程执行，持锁无竞争；批次借用与编码
/// 出参为互不相交字段借用。
#[derive(Default)]
pub struct ChunkBatchScratch {
    /// 批次候选（`prepare_batch` 重填）
    pub prepared: Vec<PreparedChunk>,
    /// 并行编码结果（`encode_batch` 经 `collect_into_vec` 复用容量收集）
    pub encoded_results: Vec<Option<(EncodedChunk, bool)>>,
    /// 编码输出（`encode_batch` 重填）
    pub encoded: Vec<EncodedChunk>,
    /// 已派发位置（`commit_batch` 重填，供实体配对）
    pub dispatched: Vec<Vector2<i32>>,
}

#[derive(Clone)]
pub struct EncodedChunk {
    pub position: Vector2<i32>,
    pub payload: Bytes,
    pub light_payload: Option<Bytes>,
    pub chunk_ref: Weak<ChunkData>,
    /// 编码时刻捕获的区块内容改动代数；区块之后任何内容变异
    ///（方块/光照/方块实体……都会联动 `mark_modified`）都会使
    /// 代数领先于本值，缓存条目即判过期。
    pub generation: u64,
}

impl EncodedChunk {
    #[must_use]
    pub fn is_fresh_for(&self, candidate: &PreparedChunk) -> bool {
        let Some(held) = self.chunk_ref.upgrade() else {
            return false;
        };

        self.position == candidate.position
            && Arc::ptr_eq(&held, &candidate.chunk)
            && self.generation == held.modification_generation()
    }

    /// 条目的线上字节数（编码缓存预算记账用）
    #[must_use]
    pub fn encoded_bytes(&self) -> usize {
        self.payload.len() + self.light_payload.as_ref().map_or(0, Bytes::len)
    }
}

/// 按世界共享的区块编码缓存（0.3.20 起替代每玩家私有缓存）。
///
/// 编码产物（区块包 + 旧版光照包）只取决于区块内容与目标协议版本，
/// 与接收玩家无关，因此同一世界、同一版本的所有玩家天然共享同一份
/// 缓存：N 个玩家注视同一区块只编码一次、驻留一份。失效语义由
/// `EncodedChunk::is_fresh_for` 保证：弱引用存活（未卸载）+ Arc 同一
///（未重载）+ 内容代数一致（未变异）。
pub struct SharedChunkEncodeCache {
    map: dashmap::DashMap<(JavaMinecraftVersion, Vector2<i32>), EncodedChunk>,
    /// 缓存条目线上字节总量（近似记账：插入加、替换/逐出减）
    bytes: std::sync::atomic::AtomicUsize,
    /// 逐出序列化守卫：CAS 抢占，失败者本轮跳过（下一 tick 再试）
    pruning: std::sync::atomic::AtomicBool,
}

impl SharedChunkEncodeCache {
    #[must_use]
    pub fn new() -> Self {
        Self {
            map: dashmap::DashMap::new(),
            bytes: std::sync::atomic::AtomicUsize::new(0),
            pruning: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// 当前缓存的线上字节总量（近似值）
    #[must_use]
    pub fn total_bytes(&self) -> usize {
        self.bytes.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 当前缓存条目数（测试与诊断用）
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// 缓存是否为空（测试与诊断用）
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// 取命中且新鲜的条目；过期条目顺手移除并记账
    fn get_fresh(
        &self,
        version: JavaMinecraftVersion,
        candidate: &PreparedChunk,
    ) -> Option<EncodedChunk> {
        let key = (version, candidate.position);
        let fresh = self
            .map
            .get(&key)
            .and_then(|entry| entry.is_fresh_for(candidate).then(|| entry.value().clone()));
        if fresh.is_some() {
            return fresh;
        }
        // 不存在或已过期：仅当占位条目确属过期时才移除（避免删掉
        // 并发刚插入的新鲜条目），移除成功则同步字节记账
        if let Some((_, removed)) = self.map.remove_if(&key, |_, v| !v.is_fresh_for(candidate)) {
            self.bytes.fetch_sub(
                removed.encoded_bytes(),
                std::sync::atomic::Ordering::Relaxed,
            );
        }
        None
    }

    /// 插入新编码条目；同键旧条目（必然已过期）按差额调整记账
    fn insert(&self, encoded: EncodedChunk, version: JavaMinecraftVersion) {
        let key = (version, encoded.position);
        let new_bytes = encoded.encoded_bytes();
        match self.map.insert(key, encoded) {
            Some(old) => {
                let old_bytes = old.encoded_bytes();
                if new_bytes >= old_bytes {
                    self.bytes
                        .fetch_add(new_bytes - old_bytes, std::sync::atomic::Ordering::Relaxed);
                } else {
                    self.bytes
                        .fetch_sub(old_bytes - new_bytes, std::sync::atomic::Ordering::Relaxed);
                }
            }
            None => {
                self.bytes
                    .fetch_add(new_bytes, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }

    /// 超出预算时按「距所有注视中心最远优先」逐出至预算的八成，
    /// 并顺带清扫弱引用已死的条目（区块已卸载）。返回逐出条数。
    /// 并发触发由 CAS 序列化：未抢到守卫的调用直接返回 0。
    pub fn prune_if_over_budget(&self, centers: &[Vector2<i32>], max_bytes: usize) -> usize {
        use std::sync::atomic::Ordering::Relaxed;

        if self.bytes.load(Relaxed) <= max_bytes {
            return 0;
        }
        if self
            .pruning
            .compare_exchange(false, true, Relaxed, Relaxed)
            .is_err()
        {
            return 0;
        }

        let target = max_bytes / 5 * 4;
        let mut evicted = 0usize;

        // 先扫死条目（区块已卸载/重载：弱引用无法升级）
        let dead: Vec<(JavaMinecraftVersion, Vector2<i32>)> = self
            .map
            .iter()
            .filter(|entry| entry.chunk_ref.upgrade().is_none())
            .map(|entry| *entry.key())
            .collect();
        for key in dead {
            if let Some((_, removed)) = self.map.remove(&key) {
                self.bytes.fetch_sub(removed.encoded_bytes(), Relaxed);
                evicted += 1;
            }
        }

        // 再按距最近注视中心的距离平方降序逐出，直至达标
        if self.bytes.load(Relaxed) > target {
            let mut farthest: Vec<(i64, (JavaMinecraftVersion, Vector2<i32>))> = self
                .map
                .iter()
                .map(|entry| {
                    let pos = entry.key().1;
                    let min_dist = centers
                        .iter()
                        .map(|c| {
                            let dx = i64::from(pos.x - c.x);
                            let dz = i64::from(pos.y - c.y);
                            dx * dx + dz * dz
                        })
                        .min()
                        .unwrap_or(i64::MAX);
                    (min_dist, *entry.key())
                })
                .collect();
            farthest.sort_unstable_by_key(|(dist, _)| std::cmp::Reverse(*dist));
            for (_, key) in farthest {
                if self.bytes.load(Relaxed) <= target {
                    break;
                }
                if let Some((_, removed)) = self.map.remove(&key) {
                    self.bytes.fetch_sub(removed.encoded_bytes(), Relaxed);
                    evicted += 1;
                }
            }
        }

        self.pruning.store(false, Relaxed);
        evicted
    }
}

impl Default for SharedChunkEncodeCache {
    fn default() -> Self {
        Self::new()
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
    /// 小 pending 排序驻留缓冲（轮次 19）：仅 `prepare_batch` 持
    /// `&mut self` 锁内使用，无跨锁借用
    sort_scratch: Vec<Vector2<i32>>,
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
            sort_scratch: Vec::new(),
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

    /// 轮次 19：重填调用方驻留候选缓冲（原带配额容量新建 `Vec` 返回）；
    /// 小 pending 排序复用自有 `sort_scratch`
    fn collect_sorted_candidates_into(
        &mut self,
        level: &Level,
        center: Vector2<i32>,
        view_distance: NonZero<u8>,
        out: &mut Vec<PreparedChunk>,
    ) {
        let quota_limit = self.send_quota.floor() as usize;
        out.clear();

        // 如果 pending_chunks 很小，直接排序可以避免扫描偏移量。
        if self.pending_chunks.len() <= 16 {
            self.sort_scratch.clear();
            self.sort_scratch
                .extend(self.pending_chunks.iter().copied());
            self.sort_scratch.sort_unstable_by_key(|pos| {
                let dx = (pos.x - center.x).unsigned_abs() as u64;
                let dz = (pos.y - center.y).unsigned_abs() as u64;
                dx * dx + dz * dz
            });

            for pos in &self.sort_scratch {
                if out.len() >= quota_limit {
                    break;
                }

                if let Some(chunk) = level.loaded_chunks.get(pos) {
                    out.push(PreparedChunk {
                        position: *pos,
                        chunk: chunk.value().clone(),
                    });
                }
            }
        } else {
            // 复用预编译的圆柱形区块视野查找表（已按从中心向外的顺序排序）。
            let offsets = Cylindrical::get_offsets(view_distance.get());
            for &(dx, dy) in offsets {
                if out.len() >= quota_limit {
                    break;
                }

                let pos = Vector2::new(center.x + i32::from(dx), center.y + i32::from(dy));
                if self.pending_chunks.contains(&pos)
                    && let Some(chunk) = level.loaded_chunks.get(&pos)
                {
                    out.push(PreparedChunk {
                        position: pos,
                        chunk: chunk.value().clone(),
                    });
                }
            }

            // 针对预计算表之外任何待处理区块的回退
            if out.is_empty() {
                for pos in &self.pending_chunks {
                    if out.len() >= quota_limit {
                        break;
                    }
                    if let Some(chunk) = level.loaded_chunks.get(pos) {
                        out.push(PreparedChunk {
                            position: *pos,
                            chunk: chunk.value().clone(),
                        });
                    }
                }
            }
        }
    }

    /// 轮次 19：`candidates` 出参重填（原内部新建 `Vec` 随批次返回），
    /// 返回的批次借用该缓冲，容量跨 tick 保留
    pub fn prepare_batch<'a>(
        &mut self,
        level: &Level,
        player_chunk: Vector2<i32>,
        view_distance: NonZero<u8>,
        epoch: u32,
        version: JavaMinecraftVersion,
        candidates: &'a mut Vec<PreparedChunk>,
    ) -> Option<PreparedBatch<'a>> {
        if version >= JavaMinecraftVersion::V_1_20_2 && self.in_flight_batches >= self.max_in_flight
        {
            return None;
        }

        let max_batch = self.desired_rate.max(1.0);
        self.send_quota = (self.send_quota + self.desired_rate).min(max_batch);

        if self.send_quota < 1.0 || self.pending_chunks.is_empty() {
            return None;
        }

        self.collect_sorted_candidates_into(level, player_chunk, view_distance, candidates);
        if candidates.is_empty() {
            return None;
        }

        Some(PreparedBatch {
            chunks: candidates,
            epoch_snapshot: epoch,
            target_version: version,
        })
    }

    /// 轮次 19：双出参重填（原内部 `collect` 结果 Vec + 输出
    /// `Vec::with_capacity` 双分配）；结果经 `collect_into_vec`
    /// 复用容量收集，再就地重填输出
    pub fn encode_batch(
        batch: &PreparedBatch,
        cache: &SharedChunkEncodeCache,
        results: &mut Vec<Option<(EncodedChunk, bool)>>,
        output: &mut Vec<EncodedChunk>,
    ) {
        let version = batch.target_version;

        // 二元组第二元标记是否为本轮新编码（缓存命中不重复插入，
        // 否则共享缓存的字节记账会因重复插入虚增）
        results.clear();
        batch
            .chunks
            .par_iter()
            .map(|candidate| {
                let pos = candidate.position;
                if let Some(cached) = cache.get_fresh(version, candidate) {
                    return Some((cached, false));
                }

                let chunk = &candidate.chunk;
                // 代数必须在编码前捕获：若编码期间并发变异，条目记下
                // 的是旧代数，下一轮发送会因代数落后而重编码——宁可
                // 多编一次，绝不把变异前的内容标成最新
                let generation = chunk.modification_generation();
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

                // 负载会随编码缓存长期驻留（共享缓存按世界设字节预算），
                // 收缩掉 32 KiB 预分配的多余容量再移交 Bytes——
                // Bytes::from(Vec) 原样接管底层分配，不收缩则
                // 每条缓存按预分配容量而非实际包长占内存。
                chunk_buf.shrink_to_fit();
                Some((
                    EncodedChunk {
                        position: pos,
                        payload: Bytes::from(chunk_buf),
                        light_payload,
                        chunk_ref: Arc::downgrade(chunk),
                        generation,
                    },
                    true,
                ))
            })
            .collect_into_vec(results);

        output.clear();
        for (encoded, is_new) in results.drain(..).flatten() {
            if is_new {
                cache.insert(encoded.clone(), version);
            }
            output.push(encoded);
        }
    }

    /// 轮次 19：`dispatched` 出参重填（原 `Vec::with_capacity` 返回）；
    /// epoch 失配或无编码块时重填结果为空，语义与原返回空 Vec 一致
    pub fn commit_batch(
        &mut self,
        batch: &PreparedBatch,
        encoded_chunks: &[EncodedChunk],
        client: &JavaClient,
        current_epoch: u32,
        dispatched: &mut Vec<Vector2<i32>>,
    ) {
        dispatched.clear();
        if current_epoch != batch.epoch_snapshot || encoded_chunks.is_empty() {
            return;
        }

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
            dispatched.push(chunk.position);
        }

        let sent_count = dispatched.len();
        if sent_count > 0 {
            if version >= JavaMinecraftVersion::V_1_20_2 {
                client.try_send_packet(&CChunkBatchEnd::new(sent_count as u16));
                self.in_flight_batches = self.in_flight_batches.saturating_add(1);
            }

            self.send_quota -= sent_count as f32;
        }
    }
}

impl Default for ChunkSender {
    fn default() -> Self {
        Self::new()
    }
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

    fn prepared_chunks(positions: &[(i32, i32)]) -> Vec<PreparedChunk> {
        positions
            .iter()
            .map(|&(x, z)| PreparedChunk {
                position: Vector2::new(x, z),
                chunk: populated_chunk(8),
            })
            .collect()
    }

    fn batch_of(chunks: &[PreparedChunk], version: JavaMinecraftVersion) -> PreparedBatch<'_> {
        PreparedBatch {
            chunks,
            epoch_snapshot: 1,
            target_version: version,
        }
    }

    /// 测试便利封装：驻留出参由局部 Vec 充当
    fn encode(batch: &PreparedBatch, cache: &SharedChunkEncodeCache) -> Vec<EncodedChunk> {
        let mut results = Vec::new();
        let mut output = Vec::new();
        ChunkSender::encode_batch(batch, cache, &mut results, &mut output);
        output
    }

    #[test]
    fn encode_batch_payload_matches_direct_serialization_and_cache_reuses() {
        let version = JavaMinecraftVersion::V_1_21_11;
        let chunks = prepared_chunks(&[(0, 0), (1, 0), (2, 0)]);
        let batch = batch_of(&chunks, version);

        let cache = SharedChunkEncodeCache::new();
        let encoded = encode(&batch, &cache);
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
        assert_eq!(cache.len(), 3, "三条新编码应全部入缓存");
        let accounted = cache.total_bytes();
        let actual: usize = encoded.iter().map(EncodedChunk::encoded_bytes).sum();
        assert_eq!(accounted, actual, "字节记账应与实际一致");

        // 缓存命中路径：同批再次编码应复用同一分配而非重新序列化
        let encoded2 = encode(&batch, &cache);
        assert_eq!(encoded2.len(), 3);
        for (first, second) in encoded.iter().zip(&encoded2) {
            assert_eq!(
                first.payload.as_ptr(),
                second.payload.as_ptr(),
                "缓存命中应共享同一分配而非重新序列化"
            );
        }
        assert_eq!(cache.len(), 3, "命中不新增条目");
        assert_eq!(cache.total_bytes(), accounted, "命中不虚增记账");
    }

    #[test]
    fn mutation_invalidates_cached_encoding_via_generation() {
        let version = JavaMinecraftVersion::V_1_21_11;
        let chunks = prepared_chunks(&[(0, 0)]);
        let batch = batch_of(&chunks, version);
        let cache = SharedChunkEncodeCache::new();

        let first = encode(&batch, &cache);
        assert_eq!(first.len(), 1);

        // 内容变异（方块/光照/方块实体写路径统一经 mark_modified 收口）
        batch.chunks[0].chunk.mark_modified();

        let second = encode(&batch, &cache);
        assert_eq!(second.len(), 1);
        assert_ne!(
            first[0].payload.as_ptr(),
            second[0].payload.as_ptr(),
            "变异后必须重编码而非复用旧条目"
        );
        assert!(
            second[0].generation > first[0].generation,
            "新条目的代数应随变异递增"
        );
        assert_eq!(cache.len(), 1, "过期条目应被替换而非堆积");

        // 未再变异时第三次编码恢复命中
        let third = encode(&batch, &cache);
        assert_eq!(
            second[0].payload.as_ptr(),
            third[0].payload.as_ptr(),
            "无变异应命中缓存"
        );
    }

    #[test]
    fn prune_sweeps_dead_entries_before_distance_eviction() {
        let version = JavaMinecraftVersion::V_1_21_11;
        let center = Vector2::new(0, 0);
        let cache = SharedChunkEncodeCache::new();

        let chunks = prepared_chunks(&[(1, 0), (0, 2), (2, 1)]);
        let batch = batch_of(&chunks, version);
        let encoded = encode(&batch, &cache);
        assert_eq!(encoded.len(), 3);
        // batch 仅借用 chunks（NLL 在上方最后一次使用后即结束借用），
        // drop 持有方即可释放区块强引用，弱引用随之失效
        // （encoded 只持 Weak 与 Bytes，不影响区块存活）
        drop(encoded);
        drop(chunks);

        // 预算 0 必触发；死条目在距离逐出之前被清扫
        let evicted = cache.prune_if_over_budget(&[center], 0);
        assert_eq!(evicted, 3, "三条死条目应全部清扫: {evicted}");
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.total_bytes(), 0, "清扫后记账应归零");
    }

    #[test]
    fn prune_evicts_farthest_from_all_centers_within_budget() {
        let version = JavaMinecraftVersion::V_1_21_11;
        let center = Vector2::new(0, 0);
        // 3 条近（视距内量级）+ 3 条远（跨世界/远征量级）
        let chunks = prepared_chunks(&[(1, 0), (0, 2), (2, 1), (40, 0), (0, 41), (39, 40)]);
        let batch = batch_of(&chunks, version);
        let cache = SharedChunkEncodeCache::new();
        let encoded = encode(&batch, &cache);
        assert_eq!(encoded.len(), 6);

        let total = cache.total_bytes();
        // 令目标落在条目数一半处：max = 3T/4 触发，target = 3T/5
        let max = total * 3 / 4;
        let target = max / 5 * 4;

        let evicted = cache.prune_if_over_budget(&[center], max);
        assert_eq!(evicted, 3, "等尺寸条目减半须逐出 3 条: {evicted}");

        // 近处三条必须保留，远处三条必须逐出
        let remaining: Vec<Vector2<i32>> = cache.map.iter().map(|e| e.key().1).collect();
        for pos in [Vector2::new(1, 0), Vector2::new(0, 2), Vector2::new(2, 1)] {
            assert!(remaining.contains(&pos), "近处条目应保留: {pos:?}");
        }
        for pos in [
            Vector2::new(40, 0),
            Vector2::new(0, 41),
            Vector2::new(39, 40),
        ] {
            assert!(!remaining.contains(&pos), "远处条目应逐出: {pos:?}");
        }
        assert!(
            cache.total_bytes() <= target,
            "逐出后应降至目标以内: {} <= {target}",
            cache.total_bytes()
        );

        // 预算已足时再次调用应为无操作
        assert_eq!(cache.prune_if_over_budget(&[center], max), 0);

        // 保留条目的线上字节仍与直接序列化一致（逐出不触碰内容）
        let retained = cache.map.get(&(version, Vector2::new(1, 0))).unwrap();
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
