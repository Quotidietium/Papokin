use serde::Deserialize;

use crate::random::{RandomGenerator, RandomImpl};

/// 表示用于随机抽样的加权选择池。
#[derive(Deserialize, Clone, Debug)]
pub struct Pool;

impl Pool {
    /// 使用给定的随机生成器从加权分布中选择一个元素。
    ///
    /// # Arguments
    /// * `distribution` – 用于选择的带权重条目切片。
    /// * `random` – 用于随机选取的随机数生成器。
    ///
    /// # Returns
    /// 一个 `Option<E>`，表示被选中的元素；若分布为空则为 `None`。
    pub fn get<'a, E>(
        distribution: &'a [Weighted<E>],
        random: &mut RandomGenerator,
    ) -> Option<&'a E> {
        let mut total_weight = 0;
        for dist in distribution {
            total_weight += dist.weight;
        }

        let mut index = random.next_bounded_i32(total_weight);

        if total_weight < 64 {
            return Some(FlattenedContent::get(index, distribution));
        }

        // WrappedContent
        for dist in distribution {
            index -= dist.weight;
            if index >= 0 {
                continue;
            }
            return Some(&dist.data);
        }
        None
    }
}

/// 池中的一个加权条目。
#[derive(Deserialize, Clone, Debug)]
pub struct Weighted<E> {
    /// 此条目中存储的元素。
    pub data: E,
    /// 此条目用于随机选择的权重。
    pub weight: i32,
}

/// 用于扁平化加权选择的辅助结构体。
struct FlattenedContent;

impl FlattenedContent {
    /// 从加权条目的扁平化表示中选择一个元素。
    ///
    /// # Arguments
    /// * `index` – 要选择的目标索引。
    /// * `entries` – 要展平的带权重条目。
    ///
    /// # Returns
    /// 对应给定索引的元素。
    pub fn get<E>(index: i32, entries: &[Weighted<E>]) -> &E {
        let mut cur_index = 0;

        for entry in entries {
            let weight = entry.weight;
            if index >= cur_index && index < cur_index + weight {
                return &entry.data;
            }
            cur_index += weight;
        }

        &entries[0].data
    }
}
