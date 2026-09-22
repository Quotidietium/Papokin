use crate::population_seed_fn;

use super::{
    RandomDeriver, RandomDeriverImpl, RandomGenerator, RandomImpl, gaussian::GaussianGenerator,
    hash_block_pos,
};

/// Xoroshiro128+ 随机数生成器实现。
///
/// 这是一种现代、快速的随机数生成器，用于较新的 Minecraft 版本。
/// 它实现了 Xoroshiro128+ 算法，该算法具有更好的统计特性
/// 和性能都优于旧版 LCG 实现。
///
/// 该生成器维护 128 位状态，拆分为两个 64 位字（`lo` 和 `hi`）
/// 并使用异或、旋转和移位运算来生成高质量随机数。
pub struct Xoroshiro {
    /// 生成器状态的低 64 位。
    lo: u64,
    /// 生成器状态的高 64 位。
    hi: u64,
    /// 用于下一次高斯生成所存储的高斯值。
    internal_next_gaussian: Option<f64>,
}

impl Xoroshiro {
    population_seed_fn!();

    /// 根据给定种子创建新的 Xoroshiro 生成器。
    ///
    /// 种子使用 Stafford 13 混合函数进行混合，以确保
    /// 初始状态的良好分布。
    ///
    /// # Arguments
    /// - `seed` – 初始种子值。
    ///
    /// # Returns
    /// 一个新的 `Xoroshiro` 实例。
    #[must_use]
    pub const fn from_seed(seed: u64) -> Self {
        let (lo, hi) = Self::mix_u64(seed);
        let lo = mix_stafford_13(lo);
        let hi = mix_stafford_13(hi);
        Self::new(lo, hi)
    }

    /// 使用给定状态字创建新的 Xoroshiro 生成器。
    ///
    /// 若两个状态字均为零，则将其替换为默认值
    /// 以避免对算法无效的全零状态。
    ///
    /// # Arguments
    /// - `lo` – 较低的 64 位状态字。
    /// - `hi` – 较高的 64 位状态字。
    ///
    /// # Returns
    /// 一个新的 `Xoroshiro` 实例。
    const fn new(lo: u64, hi: u64) -> Self {
        let (lo, hi) = if (lo | hi) == 0 {
            (0x9E3779B97F4A7C15, 0x6A09E667F3BCC909)
        } else {
            (lo, hi)
        };
        Self {
            lo,
            hi,
            internal_next_gaussian: None,
        }
    }

    /// 将一个 64 位种子混入两个 64 位状态字。
    ///
    /// 此方法使用黄金比例常数产生两个不同的
    /// 从单一种子派生的初始值。
    ///
    /// # Arguments
    /// - `seed` – 种子值。
    ///
    /// # Returns
    /// 一个包含两个 64 位值、用于初始化状态的元组。
    const fn mix_u64(seed: u64) -> (u64, u64) {
        let l = seed ^ 0x6A09E667F3BCC909;
        let m = l.wrapping_add(0x9E3779B97F4A7C15);
        (l, m)
    }

    /// 创建不混合种子的新 Xoroshiro 生成器。
    ///
    /// 这用于测试，以及种子已经正确混合的情况。
    ///
    /// # Arguments
    /// - `seed` – 种子值（将使用标准 mix 函数进行混合）。
    ///
    /// # Returns
    /// 一个新的 `Xoroshiro` 实例。
    #[must_use]
    pub const fn from_seed_unmixed(seed: u64) -> Self {
        let (lo, hi) = Self::mix_u64(seed);
        Self::new(lo, hi)
    }

    /// 生成具有指定位数的随机值。
    ///
    /// # Arguments
    /// - `bits` – 要提取的位数（0-64）。
    ///
    /// # Returns
    /// 具有给定位数的随机值。
    pub(super) const fn next(&mut self, bits: u64) -> u64 {
        self.next_random() >> (64 - bits)
    }

    /// 生成下一个随机值并推进生成器状态。
    ///
    /// 这实现了核心的 Xoroshiro128+ 算法：
    /// `result = state.lo + state.hi`
    /// 然后通过异或、旋转和移位操作更新状态。
    ///
    /// # Returns
    /// 64 位随机值。
    const fn next_random(&mut self) -> u64 {
        let l = self.lo;
        let m = self.hi;
        let n = l.wrapping_add(m).rotate_left(17).wrapping_add(l);
        let m = m ^ l;
        self.lo = l.rotate_left(49) ^ m ^ (m << 21);
        self.hi = m.rotate_left(28);
        n
    }

    pub const fn next_splitter(&mut self) -> XoroshiroSplitter {
        XoroshiroSplitter {
            lo: self.next_random(),
            hi: self.next_random(),
        }
    }
}

impl GaussianGenerator for Xoroshiro {
    fn stored_next_gaussian(&self) -> Option<f64> {
        self.internal_next_gaussian
    }

    fn set_stored_next_gaussian(&mut self, value: Option<f64>) {
        self.internal_next_gaussian = value;
    }
}

/// 用于初始化种子的 Stafford 13 混合函数。
///
/// 这是一个高质量的整数哈希函数，用于混合初始种子
/// 混入生成器状态以获得更好的分布。
///
/// # Arguments
/// - `z` – 要混合的值。
///
/// # Returns
/// 混合后的值。
const fn mix_stafford_13(z: u64) -> u64 {
    let z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    let z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

impl RandomImpl for Xoroshiro {
    fn split(&mut self) -> Self {
        Self::new(self.next_random(), self.next_random())
    }

    fn next_splitter(&mut self) -> RandomDeriver {
        RandomDeriver::Xoroshiro(self.next_splitter())
    }

    fn next_i32(&mut self) -> i32 {
        self.next_random() as i32
    }

    fn next_bounded_i32(&mut self, bound: i32) -> i32 {
        let mut l = (self.next_i32() as u64) & 0xFFFF_FFFF;
        let mut m = l.wrapping_mul(bound as u64);
        let mut n = m & 0xFFFF_FFFF;
        if n < bound as u64 {
            let i = ((!bound).wrapping_add(1) as u64) % (bound as u64);
            while n < i {
                l = (self.next_i32() as u64) & 0xFFFF_FFFF;
                m = l.wrapping_mul(bound as u64);
                n = m & 0xFFFF_FFFF;
            }
        }
        let o = m >> 32;
        o as i32
    }

    fn next_i64(&mut self) -> i64 {
        self.next_random() as i64
    }

    fn next_bool(&mut self) -> bool {
        (self.next_random() & 1) != 0
    }

    fn next_f32(&mut self) -> f32 {
        self.next(24) as f32 * 5.960_464_5E-8f32
    }

    fn next_f64(&mut self) -> f64 {
        self.next(53) as f64 * f64::from(1.110_223E-16f32)
    }

    fn next_gaussian(&mut self) -> f64 {
        self.calculate_gaussian()
    }
}

/// 用于创建派生 Xoroshiro 随机生成器的分裂器。
///
/// 此结构体允许以确定性的方式创建多个独立的
/// 从单个种子派生多个随机生成器，这对
/// 实现可并行世界生成不可或缺。
pub struct XoroshiroSplitter {
    /// 拆分器状态的低 64 位。
    lo: u64,
    /// 分裂器状态的高 64 位。
    hi: u64,
}

impl XoroshiroSplitter {
    #[must_use]
    pub fn split_string(&self, seed: &str) -> Xoroshiro {
        let bytes = md5::compute(seed.as_bytes());
        let l = u64::from_be_bytes(bytes[0..8].try_into().unwrap_or([0; 8]));
        let m = u64::from_be_bytes(bytes[8..16].try_into().unwrap_or([0; 8]));

        Xoroshiro::new(l ^ self.lo, m ^ self.hi)
    }

    #[must_use]
    pub const fn from_lo_and_hi(&self, lo: u64, hi: u64) -> Xoroshiro {
        Xoroshiro::new(lo ^ self.lo, hi ^ self.hi)
    }

    #[must_use]
    pub const fn split_pos(&self, x: i32, y: i32, z: i32) -> Xoroshiro {
        let l = hash_block_pos(x, y, z) as u64;
        let m = l ^ self.lo;

        Xoroshiro::new(m, self.hi)
    }
}

impl RandomDeriverImpl for XoroshiroSplitter {
    fn split_string(&self, seed: &str) -> RandomGenerator {
        RandomGenerator::Xoroshiro(self.split_string(seed))
    }

    fn split_u64(&self, seed: u64) -> RandomGenerator {
        RandomGenerator::Xoroshiro(Xoroshiro::new(seed ^ self.lo, seed ^ self.hi))
    }

    fn split_pos(&self, x: i32, y: i32, z: i32) -> RandomGenerator {
        RandomGenerator::Xoroshiro(self.split_pos(x, y, z))
    }
}

#[cfg(test)]
mod tests {
    use crate::random::{RandomDeriverImpl, RandomImpl};

    use super::{Xoroshiro, mix_stafford_13};

    // 数值已对照等价 Java 源码的结果进行检查

    #[test]
    fn mix_stafford_13_test() {
        let values: [(u64, i64); 31] = [
            (0, 0),
            (1, 6238072747940578789),
            (64, -8456553050427055661),
            (4096, -1125827887270283392),
            (262144, -120227641678947436),
            (16777216, 6406066033425044679),
            (1073741824, 3143522559155490559),
            (16, -2773008118984693571),
            (1024, 8101005175654470197),
            (65536, -3551754741763842827),
            (4194304, -2737109459693184599),
            (2, -2606959012126976886),
            (128, -5825874238589581082),
            (8192, 1111983794319025228),
            (524288, -7964047577924347155),
            (33554432, -5634612006859462257),
            (2147483648, -1436547171018572641),
            (137438953472, -4514638798598940860),
            (8796093022208, -610572083552328405),
            (562949953421312, -263574021372026223),
            (36028797018963968, 7868130499179604987),
            (253, -4045451768301188906),
            (127, -6873224393826578139),
            (8447, 6670985465942597767),
            (524543, -6228499289678716485),
            (33554687, 2630391896919662492),
            (2147483903, -6879633228472053040),
            (137438953727, -5817997684975131823),
            (8796093022463, 2384436581894988729),
            (562949953421567, -5076179956679497213),
            (36028797018964223, -5993365784811617721),
        ];
        for (input, output) in values {
            assert_eq!(mix_stafford_13(input), output as u64);
        }
    }

    #[test]
    fn next_i32() {
        let values = [
            -160476802,
            781697906,
            653572596,
            1337520923,
            -505875771,
            -47281585,
            342195906,
            1417498593,
            -1478887443,
            1560080270,
        ];

        let mut xoroshiro = Xoroshiro::from_seed(0);
        for value in values {
            assert_eq!(xoroshiro.next_i32(), value);
        }
    }

    #[test]
    fn next_bounded_i32() {
        let mut xoroshiro = Xoroshiro::from_seed(0);

        let values = [9, 1, 1, 3, 8, 9, 0, 3, 6, 3];
        for value in values {
            assert_eq!(xoroshiro.next_bounded_i32(10), value);
        }

        let values = [
            9784805, 470346, 13560642, 7320226, 14949645, 13460529, 2824352, 10938308, 14146127,
            4549185,
        ];
        for value in values {
            assert_eq!(xoroshiro.next_bounded_i32(0xFFFFFF), value);
        }
    }

    #[test]
    fn next_between_i32() {
        let mut xoroshiro = Xoroshiro::from_seed(0);

        let values = [99, 59, 57, 65, 94, 100, 54, 66, 83, 68];
        for value in values {
            assert_eq!(xoroshiro.next_inbetween_i32(50, 100), value);
        }
    }

    #[test]
    fn next_inbetween_exclusive() {
        let mut xoroshiro = Xoroshiro::from_seed(0);

        let values = [98, 59, 57, 65, 94, 99, 53, 66, 82, 68];
        for value in values {
            assert_eq!(xoroshiro.next_inbetween_i32_exclusive(50, 100), value);
        }
    }

    #[test]
    fn next_f64() {
        let mut xoroshiro = Xoroshiro::from_seed(0);

        let values: [f64; 10] = [
            0.16474369376959186,
            0.7997457290026366,
            0.2511961888876212,
            0.11712489470639631,
            0.0997124786680137,
            0.7566797430601416,
            0.7723285712021574,
            0.9420469457586381,
            0.48056202536813664,
            0.6099690583914598,
        ];
        for value in values {
            assert_eq!(xoroshiro.next_f64(), value);
        }
    }

    #[test]
    fn next_f32() {
        let mut xoroshiro = Xoroshiro::from_seed(0);

        let values: [f32; 10] = [
            0.16474366,
            0.7997457,
            0.25119615,
            0.117124856,
            0.09971243,
            0.7566797,
            0.77232856,
            0.94204694,
            0.48056197,
            0.609969,
        ];
        for value in values {
            assert_eq!(xoroshiro.next_f32(), value);
        }
    }

    #[test]
    fn next_i64() {
        let mut xoroshiro = Xoroshiro::from_seed(0);

        let values: [i64; 10] = [
            3038984756725240190,
            -3694039286755638414,
            4633751808701151732,
            2160572957309072155,
            1839370574944072389,
            -4488466507718817201,
            -4199796579929588030,
            -1069045159880208415,
            8864804693509535725,
            -7194800960680693874,
        ];
        for value in values {
            assert_eq!(xoroshiro.next_i64(), value);
        }
    }

    #[test]
    fn next_bool() {
        let mut xoroshiro = Xoroshiro::from_seed(0);

        let values: [bool; 10] = [
            false, false, false, true, true, true, false, true, true, false,
        ];
        for value in values {
            assert_eq!(xoroshiro.next_bool(), value);
        }
    }

    #[test]
    fn next_gaussian() {
        let mut xoroshiro = Xoroshiro::from_seed(0);

        let values: [f64; 10] = [
            -0.48540690699780015,
            0.43399227545320296,
            -0.3283265251019599,
            -0.5052497078202575,
            -0.3772512828630807,
            0.2419080215945433,
            -0.42622066207565135,
            2.411315261138953,
            -1.1419147030553274,
            -0.05849758093810378,
        ];
        for value in values {
            assert_eq!(xoroshiro.next_gaussian(), value);
        }
    }

    #[test]
    fn next_triangular() {
        let mut xoroshiro = Xoroshiro::from_seed(0);

        let values: [f64; 10] = [
            6.824989823834776,
            10.670356470906125,
            6.71516367803936,
            9.151408127217596,
            9.352964834883384,
            8.291618967842293,
            8.954549938640508,
            11.833001837470519,
            10.65851306020791,
            11.684676364031647,
        ];
        for value in values {
            assert_eq!(xoroshiro.next_triangular(10f64, 5f64), value);
        }
    }

    #[test]
    fn split() {
        let mut xoroshiro = Xoroshiro::from_seed(0);

        let mut new_generator = xoroshiro.split();
        assert_eq!(new_generator.next_i32(), 542195535);

        {
            // 让 splitter 离开作用域，以便再次以 `mut` 调用 new_generator
            let splitter = new_generator.next_splitter();
            let mut rand_1 = splitter.split_string("TEST STRING");
            assert_eq!(rand_1.next_i32(), -641435713);

            let mut rand_2 = splitter.split_u64(42069);
            assert_eq!(rand_2.next_i32(), -340700677);

            let mut rand_3 = splitter.split_pos(1337, 80085, -69420);
            assert_eq!(rand_3.next_i32(), 790449132);
        };
        // 验证我们没有改动原始数据
        assert_eq!(xoroshiro.next_i32(), 653572596);
        assert_eq!(new_generator.next_i32(), 435917842);
    }

    #[test]
    fn intersection() {
        let mut xoroshiro = Xoroshiro::new(0, 0);
        assert_eq!(xoroshiro.next_i64(), 6807859099481836695);
    }
}
