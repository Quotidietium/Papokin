use papokin_data::chunk::{Biome, BiomeTree, NETHER_BIOME_SOURCE, OVERWORLD_BIOME_SOURCE};
use papokin_data::dimension::Dimension;
use papokin_util::math::position::BlockPos;
use rustc_hash::FxHashSet;

use crate::biome::{BiomeSupplier, MultiNoiseBiomeSupplier, end::TheEndBiomeSupplier};
use crate::generation::biome_coords;
use crate::generation::noise::router::multi_noise_sampler::MultiNoiseSampler;

use super::{VanillaGenerator, WorldGenerator};

/// 查找采样生物群系 ID 包含在其中的最近位置
/// `targets`，与原版的 `BiomeSource.findClosestBiome3d` 一致。
///
/// 搜索从 `origin` 开始逐环向外螺旋进行，步长为
/// 以 `horizontal_step` 为步长最多推进到 `horizontal_radius`，在
/// 从原点 y 开始、以 `vertical_step` 为步长向外扩展的 y 层级
/// (不超出该维度的建筑高度上限)。第一个匹配的生效，
/// 会像原版那样近似得到最近的一个。
///
///返回采样点的方块位置以及具体的生物群系
/// 未在该处找到时。
#[must_use]
#[expect(clippy::implicit_hasher)]
pub fn find_closest_biome_3d(
    world_gen: &WorldGenerator,
    origin: BlockPos,
    targets: &FxHashSet<u8>,
    horizontal_radius: i32,
    horizontal_step: i32,
    vertical_step: i32,
) -> Option<(BlockPos, &'static Biome)> {
    match world_gen {
        WorldGenerator::Flat(flat) => {
            // 超平坦世界处处使用单一固定生物群系，与
            // `FlatGenerator::step_to_biomes` 中的解析。
            let name = flat.biome.strip_prefix("minecraft:").unwrap_or(&flat.biome);
            let biome = Biome::from_name(name).unwrap_or(&Biome::PLAINS);
            targets.contains(&biome.id).then_some((origin, biome))
        }
        WorldGenerator::Noise(generator) => find_in_noise_world(
            generator,
            origin,
            targets,
            horizontal_radius,
            horizontal_step,
            vertical_step,
        ),
        // 插件生成器只能通过生成整个区块来暴露生物群系，因此
        // 没有可供搜索的采样器。
        WorldGenerator::Custom(_) => None,
    }
}

fn find_in_noise_world(
    generator: &VanillaGenerator,
    origin: BlockPos,
    targets: &FxHashSet<u8>,
    horizontal_radius: i32,
    horizontal_step: i32,
    vertical_step: i32,
) -> Option<(BlockPos, &'static Biome)> {
    // 原版先将请求与 `BiomeSource#possibleBiomes` 求交集，
    // 因此查询无法在该维度生成的生物群系时会返回
    // 立即处理，而不是扫描整个搜索半径。
    if targets.is_disjoint(&possible_biomes(&generator.dimension)) {
        return None;
    }

    let supplier: &dyn BiomeSupplier = if generator.dimension == Dimension::THE_END {
        &TheEndBiomeSupplier
    } else if generator.dimension == Dimension::THE_NETHER {
        &MultiNoiseBiomeSupplier::NETHER
    } else {
        &MultiNoiseBiomeSupplier::OVERWORLD
    };

    let mut sampler = MultiNoiseSampler::generate(&generator.base_router.multi_noise);

    // 原版中是从 `level.getMinY() + 1` 到 `level.getMaxY() + 1`。
    let min_y = i32::from(generator.settings.shape.min_y) + 1;
    let max_y =
        i32::from(generator.settings.shape.min_y) + i32::from(generator.settings.shape.height);
    let ys = out_from_origin(origin.0.y, min_y, max_y, vertical_step);

    let check_column = |x: i32, z: i32, sampler: &mut MultiNoiseSampler| {
        let biome_x = biome_coords::from_block(x);
        let biome_z = biome_coords::from_block(z);
        for &y in &ys {
            let biome = supplier.biome(biome_x, biome_coords::from_block(y), biome_z, sampler);
            if targets.contains(&biome.id) {
                return Some((BlockPos::new(x, y, z), biome));
            }
        }
        None
    };

    let rings = horizontal_radius / horizontal_step;
    for radius in 0..=rings {
        if radius == 0 {
            if let Some(found) = check_column(origin.0.x, origin.0.z, &mut sampler) {
                return Some(found);
            }
            continue;
        }

        // 切比雪夫距离为 `radius` 的方形环的周长。
        for dx in -radius..=radius {
            for dz in [-radius, radius] {
                let x = origin.0.x + dx * horizontal_step;
                let z = origin.0.z + dz * horizontal_step;
                if let Some(found) = check_column(x, z, &mut sampler) {
                    return Some(found);
                }
            }
        }
        for dz in (1 - radius)..radius {
            for dx in [-radius, radius] {
                let x = origin.0.x + dx * horizontal_step;
                let z = origin.0.z + dz * horizontal_step;
                if let Some(found) = check_column(x, z, &mut sampler) {
                    return Some(found);
                }
            }
        }
    }

    None
}

/// 给定维度的生物群系源能产出的所有生物群系的 ID。
#[must_use]
pub fn possible_biomes(dimension: &Dimension) -> FxHashSet<u8> {
    let mut out = FxHashSet::default();
    if *dimension == Dimension::THE_END {
        // 与 `TheEndBiomeSupplier` 中的固定集合一致。
        for biome in [
            &Biome::THE_END,
            &Biome::END_HIGHLANDS,
            &Biome::END_MIDLANDS,
            &Biome::SMALL_END_ISLANDS,
            &Biome::END_BARRENS,
        ] {
            out.insert(biome.id);
        }
    } else if *dimension == Dimension::THE_NETHER {
        collect_tree_biomes(&NETHER_BIOME_SOURCE, &mut out);
    } else {
        collect_tree_biomes(&OVERWORLD_BIOME_SOURCE, &mut out);
    }
    out
}

fn collect_tree_biomes(tree: &'static BiomeTree, out: &mut FxHashSet<u8>) {
    match tree {
        BiomeTree::Leaf { biome, .. } => {
            out.insert(biome.id);
        }
        BiomeTree::Branch { nodes, .. } => {
            for node in *nodes {
                collect_tree_biomes(node, out);
            }
        }
    }
}

/// 要探测的 Y 层级，按从 `origin` 向外的顺序排列（先向上），
/// 被钳制在 `[min, max]` 之间，与原版的 `Mth.outFromOrigin` 一致。
fn out_from_origin(origin: i32, min: i32, max: i32, step: i32) -> Vec<i32> {
    let start = origin.clamp(min, max);
    let mut ys = vec![start];
    let mut distance = step;
    loop {
        let up = start + distance;
        let down = start - distance;
        if up > max && down < min {
            break;
        }
        if up <= max {
            ys.push(up);
        }
        if down >= min {
            ys.push(down);
        }
        distance += step;
    }
    ys
}

#[cfg(test)]
mod test {
    use papokin_data::chunk::Biome;
    use papokin_data::dimension::Dimension;
    use papokin_util::math::position::BlockPos;
    use papokin_util::world_seed::Seed;
    use rustc_hash::FxHashSet;

    use super::super::flat::FlatGenerator;
    use super::super::{GeneratorInit, VanillaGenerator, WorldGenerator};
    use super::{find_closest_biome_3d, out_from_origin};

    fn targets(biomes: &[&Biome]) -> FxHashSet<u8> {
        biomes.iter().map(|biome| biome.id).collect()
    }

    #[test]
    fn y_levels_spread_outwards() {
        assert_eq!(
            out_from_origin(64, -63, 320, 64),
            vec![64, 128, 0, 192, 256, 320]
        );
        // 越界的原点会先被钳制。
        assert_eq!(out_from_origin(-500, -63, 320, 200), vec![-63, 137]);
    }

    #[test]
    fn finds_known_biome() {
        // 种子 13579 在方块 (-96, 4, 32) 附近有一片沙漠；见
        // `biome::test::biome_desert`.
        let world_gen = WorldGenerator::Noise(Box::new(VanillaGenerator::new(
            Seed(13579),
            Dimension::OVERWORLD,
        )));

        let (pos, biome) = find_closest_biome_3d(
            &world_gen,
            BlockPos::new(-96, 4, 32),
            &targets(&[&Biome::DESERT]),
            6400,
            32,
            64,
        )
        .expect("沙漠应在范围内");
        assert_eq!(biome.id, Biome::DESERT.id);
        assert_eq!(pos, BlockPos::new(-96, 4, 32));
    }

    #[test]
    fn short_circuits_impossible_dimension_biomes() {
        let world_gen = WorldGenerator::Noise(Box::new(VanillaGenerator::new(
            Seed(13579),
            Dimension::OVERWORLD,
        )));

        // 下界生物群系绝不可能在主世界生成；这必须
        // 返回，而不必扫描整个搜索半径。
        assert!(
            find_closest_biome_3d(
                &world_gen,
                BlockPos::new(0, 64, 0),
                &targets(&[&Biome::CRIMSON_FOREST]),
                6400,
                32,
                64,
            )
            .is_none()
        );
    }

    #[test]
    fn flat_world_has_a_single_fixed_biome() {
        let world_gen = WorldGenerator::Flat(Box::new(FlatGenerator::new(
            Seed(0),
            Dimension::OVERWORLD,
            Vec::new(),
            "minecraft:plains".to_string(),
        )));

        let origin = BlockPos::new(17, 64, -3);
        let (pos, biome) = find_closest_biome_3d(
            &world_gen,
            origin,
            &targets(&[&Biome::PLAINS]),
            6400,
            32,
            64,
        )
        .expect("平坦生物群系无处不在");
        assert_eq!(biome.id, Biome::PLAINS.id);
        assert_eq!(pos, origin);

        assert!(
            find_closest_biome_3d(
                &world_gen,
                origin,
                &targets(&[&Biome::DESERT]),
                6400,
                32,
                64
            )
            .is_none()
        );
    }
}
