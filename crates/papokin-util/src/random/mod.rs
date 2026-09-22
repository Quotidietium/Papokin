use std::{
    sync::atomic::{AtomicU64, Ordering},
    time,
};

use legacy_rand::{LegacyRand, LegacySplitter};
use worldgen_random::WorldgenRandom;
use xoroshiro128::{Xoroshiro, XoroshiroSplitter};

mod gaussian;
pub mod legacy_rand;
pub mod worldgen_random;
pub mod xoroshiro128;

/// 全局种子唯一化器，用于基于时间生成唯一种子。
static SEED_UNIQUIFIER: AtomicU64 = AtomicU64::new(8682522807148012u64);

pub fn get_seed() -> u64 {
    let seed = SEED_UNIQUIFIER
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |val| {
            Some(val.wrapping_mul(1181783497276652981u64))
        })
        .unwrap_or(0);

    let nanos = time::SystemTime::now()
        .duration_since(time::SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());

    let nano_upper = (nanos >> 8) as u64;
    let nano_lower = nanos as u64;
    seed ^ nano_upper ^ nano_lower
}

pub enum RandomGenerator {
    /// Xoroshiro128+ 随机数生成器（现代、快速的实现）。
    Xoroshiro(Xoroshiro),
    /// 用 Java `WorldgenRandom` 位源语义包装的 Xoroshiro。
    Worldgen(WorldgenRandom),
    /// 旧版随机数生成器（与旧版 Minecraft 兼容）。
    Legacy(LegacyRand),
}

/// 用于创建子随机数生成器的统一随机数派生枚举。
pub enum RandomDeriver {
    /// Xoroshiro 分裂器实现。
    Xoroshiro(XoroshiroSplitter),
    /// 旧版拆分器（splitter）实现。
    Legacy(LegacySplitter),
}

// TODO: 为此编写单元测试
#[macro_export]
macro_rules! population_seed_fn {
    () => {
        /// 生成用于结构放置的种群种子。
        ///
        /// 种群种子用于确定结构的放置
        /// 例如村庄、神殿和其他地物。它们将世界种子
        /// 与方块坐标结合，为每个区块创建唯一的种子。
        ///
        /// # Arguments
        /// - `world_seed` – 基础世界种子。
        /// - `block_x` – X 方块坐标。
        /// - `block_z` – Z 方块坐标。
        ///
        /// # Returns
        /// 给定位置的种群种子。
        pub fn get_population_seed(world_seed: u64, block_x: i32, block_z: i32) -> u64 {
            let mut rand = Self::from_seed(world_seed);
            let l = rand.next_i64() | 1;
            let m = rand.next_i64() | 1;
            let base = (block_x as i64)
                .wrapping_mul(l)
                .wrapping_add((block_z as i64).wrapping_mul(m));
            (base as u64) ^ world_seed
        }
    };
}

/// 生成用于地物放置的装饰器种子。
///
/// 装饰器种子用于放置树木等单个地物，
/// 花朵以及区块内的矿石。
///
/// # Arguments
/// - `population_seed` – 该区域的基础布置种子。
/// - `index` – 修饰器的索引。
/// - `step` – 装饰阶段编号。
///
/// # Returns
/// 给定参数对应的装饰器种子。
// TODO: 为此编写单元测试
#[inline]
#[must_use]
pub const fn get_decorator_seed(population_seed: u64, index: u64, step: u64) -> u64 {
    population_seed
        .wrapping_add(index)
        .wrapping_add(10_000u64.wrapping_mul(step))
}

/// 生成用于大范围地物放置的区域种子。
///
/// 区域种子用于跨越多个区块的地物，例如
/// 史莱姆区块或特定生物群系放置。
///
/// # Arguments
/// - `world_seed` – 基础世界种子。
/// - `region_x` – 区域的 X 坐标。
/// - `region_z` – 区域的 Z 坐标。
/// - `salt` – 用于让种子对特定地物保持唯一的盐值。
///
/// # Returns
/// 给定位置的区域种子。
#[inline]
#[must_use]
pub fn get_region_seed(world_seed: u64, region_x: i32, region_z: i32, salt: u32) -> u64 {
    let x_part = i64::from(region_x).wrapping_mul(341873128712) as u64;
    let z_part = i64::from(region_z).wrapping_mul(132897987541) as u64;

    world_seed
        .wrapping_add(x_part)
        .wrapping_add(z_part)
        .wrapping_add(i64::from(salt) as u64)
}

/// 生成用于判定史莱姆区块的种子。
///
/// 这遵循 Minecraft 的特定公式来判断某个区块是否
/// 是“史莱姆区块”，史莱姆可在其中无视光照等级生成。
///
/// # Arguments
/// - `x` – 区块的 X 坐标。
/// - `z` – 区块的 Z 坐标。
/// - `seed` – 世界种子。
/// - `salt` – 盐值（默认为 987234911）。
///
/// # Returns
/// 用于旧版随机生成器的种子值。
#[inline]
#[must_use]
pub const fn seed_slime_chunk(x: i32, z: i32, seed: u64, salt: u64) -> u64 {
    (seed
        .wrapping_add((x.wrapping_mul(x).wrapping_mul(4_987_142)) as i64 as u64)
        .wrapping_add((x.wrapping_mul(5_947_611)) as i64 as u64)
        .wrapping_add((z.wrapping_mul(z) as i64).wrapping_mul(4_392_871) as u64)
        .wrapping_add((z.wrapping_mul(389_711)) as i64 as u64))
        ^ salt
}

/// 生成用于洞穴和峡谷生成的雕刻器种子。
///
/// 雕刻器种子用于洞穴和峡谷等地形雕刻特性。
///
/// # Arguments
/// - `world_seed` – 基础世界种子（加上雕刻器索引）。
/// - `chunk_x` – X 区块坐标。
/// - `chunk_z` – Z 区块坐标。
///
/// # Returns
/// 给定区块的雕刻器种子。
#[inline]
#[must_use]
pub fn get_carver_seed(world_seed: u64, chunk_x: i32, chunk_z: i32) -> u64 {
    let mut random = LegacyRand::from_seed(world_seed);
    let l = random.next_i64() | 1;
    let m = random.next_i64() | 1;
    ((chunk_x as i64)
        .wrapping_mul(l)
        .wrapping_add((chunk_z as i64).wrapping_mul(m)) as u64)
        ^ world_seed
}

/// 生成用于结构选择与放置的大型地物种子。
///
/// 对应原版 Minecraft 的 `WorldgenRandom.setLargeFeatureSeed`。
///
/// # Arguments
/// - `world_seed` – 基础世界种子。
/// - `chunk_x` – X 区块坐标。
/// - `chunk_z` – Z 区块坐标。
///
/// # Returns
/// 给定区块的大型特征种子。
#[inline]
#[must_use]
pub fn get_large_feature_seed(world_seed: u64, chunk_x: i32, chunk_z: i32) -> u64 {
    let mut random = LegacyRand::from_seed(world_seed);
    let x_scale = random.next_i64();
    let z_scale = random.next_i64();
    ((chunk_x as i64).wrapping_mul(x_scale)
        ^ (chunk_z as i64).wrapping_mul(z_scale)
        ^ (world_seed as i64)) as u64
}

#[expect(clippy::return_self_not_must_use)]
pub trait RandomImpl {
    fn split(&mut self) -> Self;

    fn next_splitter(&mut self) -> RandomDeriver;

    fn next_i32(&mut self) -> i32;

    fn next_bounded_i32(&mut self, bound: i32) -> i32;

    fn next_inbetween_i32(&mut self, min: i32, max: i32) -> i32 {
        if min >= max {
            return min;
        }
        self.next_bounded_i32(max - min + 1) + min
    }

    fn next_inbetween_f32(&mut self, min: f32, max: f32) -> f32 {
        self.next_f32() * (max - min) + min
    }

    fn next_i64(&mut self) -> i64;

    fn next_bool(&mut self) -> bool;

    fn next_f32(&mut self) -> f32;

    fn next_f64(&mut self) -> f64;

    fn next_gaussian(&mut self) -> f64;

    #[allow(clippy::suboptimal_flops)]
    fn next_triangular(&mut self, mode: f64, deviation: f64) -> f64 {
        mode + deviation * (self.next_f64() - self.next_f64())
    }

    fn skip(&mut self, count: i32) {
        for _ in 0..count {
            self.next_i64();
        }
    }

    fn next_inbetween_i32_exclusive(&mut self, min: i32, max: i32) -> i32 {
        if min >= max {
            return min;
        }
        min + self.next_bounded_i32(max - min)
    }
}

impl RandomImpl for RandomGenerator {
    #[inline]
    fn split(&mut self) -> Self {
        match self {
            Self::Xoroshiro(x) => Self::Xoroshiro(x.split()),
            Self::Worldgen(x) => Self::Worldgen(x.split()),
            Self::Legacy(l) => Self::Legacy(l.split()),
        }
    }

    #[inline]
    fn next_splitter(&mut self) -> RandomDeriver {
        match self {
            Self::Xoroshiro(x) => RandomDeriver::Xoroshiro(x.next_splitter()),
            Self::Worldgen(x) => x.next_splitter(),
            Self::Legacy(l) => l.next_splitter(),
        }
    }

    #[inline]
    fn next_i32(&mut self) -> i32 {
        match self {
            Self::Xoroshiro(x) => x.next_i32(),
            Self::Worldgen(x) => x.next_i32(),
            Self::Legacy(l) => l.next_i32(),
        }
    }

    #[inline]
    fn next_bounded_i32(&mut self, bound: i32) -> i32 {
        match self {
            Self::Xoroshiro(x) => x.next_bounded_i32(bound),
            Self::Worldgen(x) => x.next_bounded_i32(bound),
            Self::Legacy(l) => l.next_bounded_i32(bound),
        }
    }

    #[inline]
    fn next_i64(&mut self) -> i64 {
        match self {
            Self::Xoroshiro(x) => x.next_i64(),
            Self::Worldgen(x) => x.next_i64(),
            Self::Legacy(l) => l.next_i64(),
        }
    }

    #[inline]
    fn next_bool(&mut self) -> bool {
        match self {
            Self::Xoroshiro(x) => x.next_bool(),
            Self::Worldgen(x) => x.next_bool(),
            Self::Legacy(l) => l.next_bool(),
        }
    }

    #[inline]
    fn next_f32(&mut self) -> f32 {
        match self {
            Self::Xoroshiro(x) => x.next_f32(),
            Self::Worldgen(x) => x.next_f32(),
            Self::Legacy(l) => l.next_f32(),
        }
    }

    #[inline]
    fn next_f64(&mut self) -> f64 {
        match self {
            Self::Xoroshiro(x) => x.next_f64(),
            Self::Worldgen(x) => x.next_f64(),
            Self::Legacy(l) => l.next_f64(),
        }
    }

    #[inline]
    fn next_gaussian(&mut self) -> f64 {
        match self {
            Self::Xoroshiro(x) => x.next_gaussian(),
            Self::Worldgen(x) => x.next_gaussian(),
            Self::Legacy(l) => l.next_gaussian(),
        }
    }

    #[inline]
    fn skip(&mut self, count: i32) {
        match self {
            Self::Xoroshiro(x) => x.skip(count),
            Self::Worldgen(x) => x.skip(count),
            Self::Legacy(l) => l.skip(count),
        }
    }
}

pub trait RandomDeriverImpl {
    fn split_string(&self, seed: &str) -> RandomGenerator;

    fn split_u64(&self, seed: u64) -> RandomGenerator;

    fn split_pos(&self, x: i32, y: i32, z: i32) -> RandomGenerator;
}

impl RandomDeriverImpl for RandomDeriver {
    #[inline]
    fn split_string(&self, seed: &str) -> RandomGenerator {
        match self {
            Self::Xoroshiro(x) => RandomGenerator::Xoroshiro(x.split_string(seed)),
            Self::Legacy(l) => l.split_string(seed),
        }
    }

    #[inline]
    fn split_u64(&self, seed: u64) -> RandomGenerator {
        match self {
            Self::Xoroshiro(x) => x.split_u64(seed),
            Self::Legacy(l) => l.split_u64(seed),
        }
    }

    #[inline]
    fn split_pos(&self, x: i32, y: i32, z: i32) -> RandomGenerator {
        match self {
            Self::Xoroshiro(s) => RandomGenerator::Xoroshiro(s.split_pos(x, y, z)),
            Self::Legacy(s) => s.split_pos(x, y, z),
        }
    }
}

/// 将方块位置散列为 64 位值，用于随机数生成器播种。
///
/// This hash function is designed to produce well-distributed values for
/// 用作位置相关随机生成的种子。
///
/// # Arguments
/// - `x` – X 坐标。
/// - `y` – Y 坐标。
/// - `z` – Z 坐标。
///
/// # Returns
/// 64 位哈希值。
#[must_use]
pub const fn hash_block_pos(x: i32, y: i32, z: i32) -> i64 {
    let l =
        ((x.wrapping_mul(3129871)) as i64) ^ ((z as i64).wrapping_mul(116129781i64)) ^ (y as i64);

    let l = l
        .wrapping_mul(l)
        .wrapping_mul(42317861i64)
        .wrapping_add(l.wrapping_mul(11i64));

    l >> 16
}

#[cfg(test)]
mod tests {
    use crate::random::get_region_seed;

    use super::hash_block_pos;

    #[test]
    fn region_seed() {
        let seed = get_region_seed(12345612, 1, 1, 14357620);
        assert_eq!(seed, 474797819485);
    }

    #[test]
    fn block_position_hash() {
        let values: [((i32, i32, i32), i64); 8] = [
            ((0, 0, 0), 0),
            ((1, 1, 1), 60311958971344),
            ((4, 4, 4), 120566413180880),
            ((25, 25, 25), 111753446486209),
            ((676, 676, 676), 75210837988243),
            ((458329, 458329, 458329), -43764888250),
            ((-387008604, -387008604, -387008604), 8437923733503),
            ((176771161, 176771161, 176771161), 18421337580760),
        ];

        for ((x, y, z), value) in values {
            assert_eq!(hash_block_pos(x, y, z), value);
        }
    }
}
