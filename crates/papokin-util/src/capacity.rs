//! 驻留缓冲容量治理：负载尖峰回落后回收超额保留容量。
//!
//! 驻留缓冲（每 tick 清理重填的 `Vec`/`HashMap`）天然保留历史峰值容量，
//! 尖峰（爆炸、传送、玩家涌入）过后这些容量永不释放，抬高常驻内存。
//! 本模块提供统一的衰减策略：清理时记录上一轮长度，若容量超过
//! `max(DECAY_FACTOR × 上轮长度, 容量地板)` 则收缩到「上轮长度 + 地板」，
//! 既给下一轮重填留出免重分配的余量，又靠 4 倍滞回保证稳态负载下
//! 绝不触发收缩（摊倍增长的容量恒小于 4× 长度）。

use std::collections::HashMap;
use std::hash::{BuildHasher, Hash};

/// `Vec` 容量地板（元素数）：低于此规模收缩无意义
pub const VEC_DECAY_FLOOR: usize = 64;
/// `HashMap` 容量地板（桶数）：低于此规模收缩无意义
pub const MAP_DECAY_FLOOR: usize = 64;
/// 滞回倍数：容量超过上轮长度 × 此倍数才收缩
pub const DECAY_FACTOR: usize = 4;

/// 清空 `Vec` 并按衰减策略回收超额容量。
///
/// 替代驻留缓冲重填前的 `clear()`：以上一轮长度为负载估计，
/// 容量超出滞回带时收缩到「上轮长度 + [`VEC_DECAY_FLOOR`]」。
#[inline]
pub fn decay_clear_vec<T>(vec: &mut Vec<T>) {
    let last_len = vec.len();
    vec.clear();
    if vec.capacity() > last_len.saturating_mul(DECAY_FACTOR).max(VEC_DECAY_FLOOR) {
        vec.shrink_to(last_len + VEC_DECAY_FLOOR);
    }
}

/// 清空 `HashMap` 并按衰减策略回收超额桶容量，语义同 [`decay_clear_vec`]。
#[inline]
pub fn decay_clear_map<K: Eq + Hash, V, S: BuildHasher>(map: &mut HashMap<K, V, S>) {
    let last_len = map.len();
    map.clear();
    if map.capacity() > last_len.saturating_mul(DECAY_FACTOR).max(MAP_DECAY_FLOOR) {
        map.shrink_to(last_len + MAP_DECAY_FLOOR);
    }
}

#[cfg(test)]
mod tests {
    use super::{DECAY_FACTOR, MAP_DECAY_FLOOR, VEC_DECAY_FLOOR, decay_clear_map, decay_clear_vec};
    use std::collections::HashMap;

    #[test]
    fn vec_below_floor_never_shrinks() {
        let mut v: Vec<u8> = Vec::with_capacity(VEC_DECAY_FLOOR);
        v.push(1);
        decay_clear_vec(&mut v);
        assert_eq!(v.capacity(), VEC_DECAY_FLOOR, "地板之下不得收缩");
    }

    #[test]
    fn vec_within_hysteresis_never_shrinks() {
        // 容量 = 3× 长度 < 4× 滞回带：稳态增长曲线上的容量不得收缩
        let mut v: Vec<u8> = Vec::with_capacity(3 * 1024);
        for i in 0..1024u16 {
            v.push(i as u8);
        }
        decay_clear_vec(&mut v);
        assert_eq!(v.capacity(), 3 * 1024, "滞回带内不得收缩");
    }

    #[test]
    fn vec_over_hysteresis_shrinks_to_last_len_plus_floor() {
        let spike = 4096usize;
        let steady = 100usize;
        let mut v: Vec<u8> = Vec::new();
        for round in 0..4 {
            for i in 0..spike {
                v.push(i as u8);
            }
            decay_clear_vec(&mut v);
            assert!(v.capacity() >= spike, "尖峰负载期不得收缩: round {round}");
        }
        assert!(v.capacity() >= spike);
        // 负载回落：容量超过 4×100 触发收缩，落点 = 100 + 地板
        for i in 0..steady {
            v.push(i as u8);
        }
        decay_clear_vec(&mut v);
        assert!(
            v.capacity() <= steady + VEC_DECAY_FLOOR,
            "收缩后容量应不超过上轮长度 + 地板: {}",
            v.capacity()
        );
        assert!(v.capacity() >= steady, "收缩不得跌破负载需求");
        // 收缩后同负载重填不触发任何重分配
        let cap = v.capacity();
        for i in 0..steady {
            v.push(i as u8);
        }
        assert_eq!(v.capacity(), cap, "余量内重填不得重分配");
    }

    #[test]
    fn vec_decay_follows_stepwise_decline() {
        let mut v: Vec<u8> = Vec::new();
        for era in [1000usize, 100, 10] {
            for i in 0..era {
                v.push(i as u8);
            }
            decay_clear_vec(&mut v);
            let cap = v.capacity();
            assert!(
                cap <= era.max(VEC_DECAY_FLOOR) + VEC_DECAY_FLOOR + era * (DECAY_FACTOR - 1),
                "时代 {era}：容量 {cap} 应贴近负载"
            );
        }
        assert!(v.capacity() <= 10 + VEC_DECAY_FLOOR);
    }

    #[test]
    fn vec_decay_preserves_clear_semantics() {
        let mut v: Vec<u8> = Vec::with_capacity(8192);
        v.extend(0..10u8);
        decay_clear_vec(&mut v);
        assert!(v.is_empty(), "衰减清理后必须为空");
        assert!(v.capacity() < 8192);
    }

    #[test]
    fn map_over_hysteresis_shrinks() {
        let mut m: HashMap<u64, u16> = HashMap::new();
        for i in 0..4096u64 {
            m.insert(i, i as u16);
        }
        decay_clear_map(&mut m);
        assert!(m.capacity() >= 4096, "满负载清理不得收缩");
        for i in 0..50u64 {
            m.insert(i, i as u16);
        }
        decay_clear_map(&mut m);
        assert!(
            m.capacity() <= 50 + MAP_DECAY_FLOOR + 50 * (DECAY_FACTOR - 1),
            "回落后桶容量应贴近负载: {}",
            m.capacity()
        );
        assert!(m.is_empty());
    }

    #[test]
    fn map_within_hysteresis_never_shrinks() {
        let mut m: HashMap<u64, u16> = HashMap::with_capacity(128);
        for i in 0..64u64 {
            m.insert(i, i as u16);
        }
        let cap = m.capacity();
        decay_clear_map(&mut m);
        assert_eq!(m.capacity(), cap, "滞回带内不得收缩");
    }
}
