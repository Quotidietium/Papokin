use dashmap::DashMap;
use papokin_util::math::vector2::Vector2;
use std::sync::{Arc, Mutex, Weak};

/// 按区块分桶的实体空间索引，供小范围盒查询（`get_entities_at_box`）
/// 替代对全服实体表的线性扫描——后者在高实体量、高频调用（漏斗每
/// 8 刻、投掷物每刻、压力板/绊线每刻）下是每 tick 的主要 CPU 与
/// 分配热点。
///
/// ### 正确性模型
///
/// * 桶内存放 `Weak`：实体移除（`Arc` 全部释放）后升级失败，查询期
///   惰性剔除——移除路径无需通知索引，杜绝漏移除导致的查找缺席。
/// * 跨区块移动只需向**新桶**插入：旧桶中的过期项会被查询侧的精确
///   AABB 过滤排除，不会产生错误命中；同一实体可能短暂出现在多个
///   桶中，查询结果按 `Arc::ptr_eq` 去重。
/// * 查询跨度超过上限时返回 `None`，由调用方回退全表线性扫描，
///   保证任意大盒子仍然正确。
///
/// ### 并发模型
///
/// 每个桶是 `Mutex<Vec<Weak<V>>>`，桶表为 `DashMap`。锁序恒为
/// 「`DashMap` 分片锁 → 桶内 `Mutex`」，不存在反向获取路径；删除空桶
/// 用 `remove_if` 复检空性，与并发插入（先建桶或使桶非空）串行化，
/// 不会误删仍含活项的桶。
pub struct ChunkedEntityIndex<V: ?Sized + Send + Sync + 'static> {
    buckets: DashMap<Vector2<i32>, Mutex<Vec<Weak<V>>>>,
}

impl<V: ?Sized + Send + Sync + 'static> ChunkedEntityIndex<V> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            buckets: DashMap::new(),
        }
    }

    /// 将实体插入其当前所在区块的桶。
    pub fn insert(&self, chunk: Vector2<i32>, entity: &Arc<V>) {
        let weak = Arc::downgrade(entity);
        let mut bucket = self
            .buckets
            .entry(chunk)
            .or_insert_with(|| Mutex::new(Vec::new()));
        bucket
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(weak);
    }

    /// 收集 `min_chunk..=max_chunk`（含）范围内的存活实体，去重并
    /// 剔除已死亡的弱引用。跨度槽数超过 `max_buckets` 时返回 `None`
    /// （调用方回退线性扫描）。
    ///
    /// 去重依据：同一实体跨桶移动后会短暂存在于多个桶中。
    #[must_use]
    pub fn query(
        &self,
        min_chunk: Vector2<i32>,
        max_chunk: Vector2<i32>,
        max_buckets: usize,
    ) -> Option<Vec<Arc<V>>> {
        // 反向区间（min > max）的 span 钳为 0，下方逐桶循环自然为空；
        // 负差值经 as usize 会回绕成巨值，先钳非负再经 u32 转换
        // （区块坐标量级约 ±1.9M，i32 减法与 u32 范围均安全）。
        let span_x = (max_chunk.x - min_chunk.x).max(0) as u32 as usize;
        let span_z = (max_chunk.y - min_chunk.y).max(0) as u32 as usize;
        let cells = (span_x + 1).checked_mul(span_z + 1)?;
        if cells > max_buckets {
            return None;
        }

        let mut result: Vec<Arc<V>> = Vec::new();
        let mut emptied: Vec<Vector2<i32>> = Vec::new();

        for cx in min_chunk.x..=max_chunk.x {
            for cz in min_chunk.y..=max_chunk.y {
                let Some(bucket) = self.buckets.get(&Vector2::new(cx, cz)) else {
                    continue;
                };
                let mut items = bucket
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                items.retain(|weak| {
                    weak.upgrade().is_some_and(|entity| {
                        if !result.iter().any(|existing| Arc::ptr_eq(existing, &entity)) {
                            result.push(entity);
                        }
                        // 保留活引用：桶内位置仍有效，跨桶项由查询结果
                        // 去重排除
                        true
                    })
                });
                if items.is_empty() {
                    emptied.push(Vector2::new(cx, cz));
                }
            }
        }

        // 释放所有分片守卫后再移除空桶，避免同分片重入死锁；
        // remove_if 复检空性，与并发插入串行化。
        for chunk in emptied {
            self.buckets.remove_if(&chunk, |_, bucket| {
                bucket
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .is_empty()
            });
        }

        Some(result)
    }
}

impl<V: ?Sized + Send + Sync + 'static> Default for ChunkedEntityIndex<V> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Dummy {
        id: u32,
    }

    fn chunk(x: i32, z: i32) -> Vector2<i32> {
        Vector2::new(x, z)
    }

    #[test]
    fn inserted_entity_is_found_by_bucket_query() {
        let index = ChunkedEntityIndex::<Dummy>::new();
        let entity = Arc::new(Dummy { id: 1 });
        index.insert(chunk(0, 0), &entity);

        let found = index.query(chunk(0, 0), chunk(0, 0), 64).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, 1);
    }

    #[test]
    fn query_only_returns_requested_buckets() {
        let index = ChunkedEntityIndex::<Dummy>::new();
        let near = Arc::new(Dummy { id: 1 });
        let far = Arc::new(Dummy { id: 2 });
        index.insert(chunk(0, 0), &near);
        index.insert(chunk(9, 9), &far);

        let found = index.query(chunk(0, 0), chunk(1, 1), 64).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, 1, "远处的实体不得被错误命中");
    }

    #[test]
    fn dropped_entities_are_pruned_and_bucket_removed() {
        let index = ChunkedEntityIndex::<Dummy>::new();
        let entity = Arc::new(Dummy { id: 1 });
        index.insert(chunk(3, -2), &entity);
        drop(entity);

        let found = index.query(chunk(3, -2), chunk(3, -2), 64).unwrap();
        assert!(found.is_empty(), "已释放实体的弱引用必须被剔除");
        assert!(
            !index.buckets.contains_key(&chunk(3, -2)),
            "空桶应在查询后清理"
        );
    }

    /// 跨桶移动后同一实体短暂存在于两个桶：联合查询必须去重。
    #[test]
    fn entity_in_two_buckets_is_deduplicated() {
        let index = ChunkedEntityIndex::<Dummy>::new();
        let entity = Arc::new(Dummy { id: 7 });
        index.insert(chunk(0, 0), &entity);
        // 跨块移动：只插新桶（旧桶留有过期项）
        index.insert(chunk(1, 0), &entity);

        let found = index.query(chunk(0, 0), chunk(1, 0), 64).unwrap();
        assert_eq!(found.len(), 1, "同一实体跨桶必须去重");
        assert_eq!(found[0].id, 7);
    }

    /// 跨度超过上限必须回退（返回 None），由调用方线性扫描。
    #[test]
    fn oversized_span_falls_back_to_none() {
        let index = ChunkedEntityIndex::<Dummy>::new();
        let entity = Arc::new(Dummy { id: 1 });
        index.insert(chunk(0, 0), &entity);

        assert!(index.query(chunk(0, 0), chunk(100, 100), 64).is_none());
    }

    #[test]
    fn inverted_or_empty_ranges_yield_no_candidates() {
        let index = ChunkedEntityIndex::<Dummy>::new();
        let entity = Arc::new(Dummy { id: 1 });
        index.insert(chunk(5, 5), &entity);

        // min > max：范围为空，不命中任何桶
        let found = index.query(chunk(6, 6), chunk(5, 5), 64).unwrap();
        assert!(found.is_empty());
    }
}
