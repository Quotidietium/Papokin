#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use criterion::{Criterion, criterion_group, criterion_main};
use papokin_data::{BlockStateId, dimension::Dimension};
use papokin_util::{math::vector2::Vector2, world_seed::Seed};
use papokin_world::{
    chunk::{ChunkData, format::anvil::SingleChunkDataSerializer},
    chunk_system::{Chunk, StagedChunkEnum, generate_single_chunk},
    generation::get_world_gen,
    world::WorldPortalExt,
};
use std::hint::black_box;

struct BlockRegistry;

impl WorldPortalExt for BlockRegistry {
    fn can_place_at(
        &self,
        _block: &papokin_data::Block,
        _state: &papokin_data::BlockState,
        _block_accessor: &dyn papokin_world::world::BlockAccessor,
        _block_pos: &papokin_util::math::position::BlockPos,
    ) -> bool {
        true
    }

    fn mirror(
        &self,
        block: &papokin_data::Block,
        state_id: BlockStateId,
        mirror: papokin_data::Mirror,
    ) -> &'static papokin_data::BlockState {
        block.mirror(state_id, mirror)
    }

    fn rotate(
        &self,
        block: &papokin_data::Block,
        state_id: BlockStateId,
        rotation: papokin_data::Rotation,
    ) -> &'static papokin_data::BlockState {
        block.rotate(state_id, rotation)
    }

    fn spawn_mobs_for_chunk_generation(
        &self,
        _cache: &mut dyn papokin_world::generation::proto_chunk::GenerationCache,
        _biome: &'static papokin_data::chunk::Biome,
        _chunk_x: i32,
        _chunk_z: i32,
    ) {
    }
}

fn bench_chunk_deserialization(c: &mut Criterion) {
    let dimension = Dimension::OVERWORLD;
    let world_gen = get_world_gen(Seed(42), dimension, false, Vec::new(), String::new());
    let chunk = generate_single_chunk(&world_gen, &BlockRegistry, 0, 0, StagedChunkEnum::Full);
    let Chunk::Level(chunk) = chunk else {
        panic!("完整生成必须返回 level chunk");
    };
    let bytes = chunk.to_bytes().expect("序列化基准区块失败");
    let position = Vector2::new(chunk.x, chunk.z);

    c.bench_function("chunk_nbt_deserialization", |b| {
        b.iter(|| {
            black_box(
                ChunkData::from_bytes(black_box(&bytes), position).expect("反序列化基准区块失败"),
            );
        });
    });
}

criterion_group!(benches, bench_chunk_deserialization);
criterion_main!(benches);
