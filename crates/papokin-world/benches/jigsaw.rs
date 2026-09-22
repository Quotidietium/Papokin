#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use criterion::{Criterion, criterion_group, criterion_main};
use papokin_data::Rotation;
use papokin_util::math::position::BlockPos;
use papokin_util::random::{RandomGenerator, xoroshiro128::Xoroshiro};
use papokin_world::generation::structure::structures::jigsaw::TemplatePool;
use std::hint::black_box;

/// 用于驱动大型深层拼图结构的池，因此每个元素的模板
/// 工作（拼图查找、边界框、高度）都会像在生产环境中一样被执行。
const POOLS: [&str; 2] = [
    "minecraft:ancient_city/city_center",
    "minecraft:pillager_outpost/base_plates",
];

fn bench_jigsaw_pool_elements(c: &mut Criterion) {
    let offset = BlockPos::new(0, 0, 0);

    for id in POOLS {
        let pool = TemplatePool::discover(id).expect("连接池应能加载");
        let kind = &pool.elements[0].kind;

        c.bench_function(&format!("jigsaw/get_shuffled_blocks/{id}"), |b| {
            b.iter(|| {
                let mut random = RandomGenerator::Xoroshiro(Xoroshiro::from_seed(black_box(0)));
                black_box(kind.get_shuffled_jigsaw_blocks(
                    black_box(offset),
                    Rotation::None,
                    &mut random,
                ));
            });
        });

        c.bench_function(&format!("jigsaw/get_y_size/{id}"), |b| {
            b.iter(|| black_box(kind.get_y_size()));
        });

        c.bench_function(&format!("jigsaw/get_bounding_box/{id}"), |b| {
            b.iter(|| black_box(kind.get_bounding_box(black_box(offset), Rotation::None)));
        });
    }
}

criterion_group!(benches, bench_jigsaw_pool_elements);
criterion_main!(benches);
