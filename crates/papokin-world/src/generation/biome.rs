use papokin_util::math::{square_f64, vector3::Vector3};

use super::biome_coords;

// 这会混合生物群系边界，并根据种子返回应使用哪个生物群系来填充地表
pub fn get_biome_blend(
    bottom_y: i8,
    height: u16,
    seed: i64,
    x: i32,
    y: i32,
    z: i32,
) -> Vector3<i32> {
    // 这是生物群系边界的“左侧”
    let offset_x = x - 2;
    let offset_y = y - 2;
    let offset_z = z - 2;
    let biome_x = biome_coords::from_block(offset_x);
    let biome_y = biome_coords::from_block(offset_y);
    let biome_z = biome_coords::from_block(offset_z);
    // 对 3 取 & 得到 0-3 的值，它也是转换为生物群系坐标时被移除的数据
    // 这实际上相当于对生物群系做“四等分”
    // 原本是 "/ 4.0"，但我们改用 "* 0.25"，因为乘法可能更快
    let quarters_x = (offset_x & 0b11) as f64 * 0.25;
    let quarters_y = (offset_y & 0b11) as f64 * 0.25;
    let quarters_z = (offset_z & 0b11) as f64 * 0.25;

    let mut best_permutation = 0;
    let mut best_score = f64::INFINITY;
    for permutation in 0..8 {
        let should_maintain_x = (permutation & 0b100) == 0;
        let should_maintain_y = (permutation & 0b010) == 0;
        let should_maintain_z = (permutation & 0b001) == 0;

        // 如果在偏移，向生物群系坐标加 1
        let shifted_biome_x = if should_maintain_x {
            biome_x
        } else {
            biome_x + 1
        };
        let shifted_biome_y = if should_maintain_y {
            biome_y
        } else {
            biome_y + 1
        };
        let shifted_biome_z = if should_maintain_z {
            biome_z
        } else {
            biome_z + 1
        };

        // 并沿位移翻转 "quarters"
        let shifted_quarters_x = if should_maintain_x {
            quarters_x
        } else {
            quarters_x - 1.0
        };
        let shifted_quarters_y = if should_maintain_y {
            quarters_y
        } else {
            quarters_y - 1.0
        };
        let shifted_quarters_z = if should_maintain_z {
            quarters_z
        } else {
            quarters_z - 1.0
        };

        let permutation_score = score_permutation(
            seed,
            shifted_biome_x,
            shifted_biome_y,
            shifted_biome_z,
            shifted_quarters_x,
            shifted_quarters_y,
            shifted_quarters_z,
        );

        if best_score > permutation_score {
            best_score = permutation_score;
            best_permutation = permutation;
        }
    }

    // 现在检查我们要使用“左”侧还是“右”侧
    let biome_x = if (best_permutation & 0b100) == 0 {
        biome_x
    } else {
        biome_x + 1
    };
    let biome_y = if (best_permutation & 0b010) == 0 {
        biome_y
    } else {
        biome_y + 1
    };
    let biome_z = if (best_permutation & 0b001) == 0 {
        biome_z
    } else {
        biome_z + 1
    };

    // Java 的 `getBiomeForNoiseGen`
    let bottom_y = bottom_y as i32;
    let biome_bottom = biome_coords::from_block(bottom_y);
    let biome_top = biome_bottom + biome_coords::from_block(height as i32) - 1;
    let biome_y = biome_y.clamp(biome_bottom, biome_top);

    Vector3::new(biome_x, biome_y, biome_z)
}

// 这实际上是为生物群系位置的四分点加上一个随机偏移（约 +/- 0.0-0.8），并且
// 返回各部分的斜边平方 + 偏移量
const fn score_permutation(
    seed: i64,
    x: i32,
    y: i32,
    z: i32,
    x_part: f64,
    y_part: f64,
    z_part: f64,
) -> f64 {
    let mix = salt_mix(seed, x as i64);
    let mix = salt_mix(mix, y as i64);
    let mix = salt_mix(mix, z as i64);
    let mix = salt_mix(mix, x as i64);
    let mix = salt_mix(mix, y as i64);
    let mix = salt_mix(mix, z as i64);
    let offset_x = scale_mix(mix);
    let mix = salt_mix(mix, seed);
    let offset_y = scale_mix(mix);
    let mix = salt_mix(mix, seed);
    let offset_z = scale_mix(mix);

    square_f64(z_part + offset_z) + square_f64(y_part + offset_y) + square_f64(x_part + offset_x)
}

#[inline]
pub const fn scale_mix(l: i64) -> f64 {
    // 先移位，再用 1023（1024 - 1）做掩码
    // 这在数学上与 floor_mod(l >> 24, 1024) 完全等价
    // 但可在单个 CPU 周期内执行。
    let d = ((l >> 24) & 1023) as f64 / 1024.0;

    (d - 0.5) * 0.9
}

#[inline]
const fn salt_mix(seed: i64, salt: i64) -> i64 {
    let mixed_seed = seed.wrapping_mul(
        seed.wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407),
    );
    mixed_seed.wrapping_add(salt)
}

#[cfg(test)]
mod test {
    use papokin_util::{math::vector3::Vector3, read_data_from_file};

    use crate::{
        biome::hash_seed,
        generation::biome::{get_biome_blend, scale_mix, score_permutation},
    };

    use super::salt_mix;

    #[test]
    fn mix_seed() {
        let seed = salt_mix(12345678, 12345678);
        assert_eq!(seed, 2937271135939595220);
    }

    #[test]
    fn permutation() {
        let seed = hash_seed(0);
        let score = score_permutation(seed, 123, 456, 456, 0.25, 0.5, 0.75);
        assert_eq!(score, 1.276986312866211);
    }

    #[test]
    fn biome_blend() {
        let biome_pos = get_biome_blend(-64, 384, 1234567890, 123, 123, 123);
        assert_eq!(biome_pos, Vector3::new(31, 30, 30));
    }

    #[test]
    fn scale() {
        let seed = scale_mix(12345678);
        assert_eq!(seed, -0.45);
    }

    #[test]
    fn chunk_wide_blend() {
        let data: Vec<(i32, i32, i32, i32, i32, i32)> =
            read_data_from_file!("../../../../assets/tests/biome_mixer.json");

        let seed = hash_seed((-777i64) as u64);
        for (i, (x, y, z, result_x, result_y, result_z)) in data.into_iter().enumerate() {
            let result = get_biome_blend(i8::MIN, u16::MAX, seed, x, y, z);
            let expected = Vector3::new(result_x, result_y, result_z);
            assert_eq!(
                result, expected,
                "Expected: {expected:?}, was: {result:?} ({i})"
            );
        }
    }
}
