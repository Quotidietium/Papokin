use papokin_data::noise_router::WrapperType;
use papokin_util::math::vector3::Vector3;
use rustc_hash::FxHashMap;

use crate::generation::{biome_coords, positions::chunk_pos};

use super::{
    chunk_density_function::{Cache, ChunkSpecificNoiseFunctionComponent},
    chunk_noise_router::ChunkNoiseFunctionComponent,
    density_function::{NoiseFunctionComponentRange, PassThrough},
    proto_noise_router::{
        BEARDIFIER_ZERO_CONSTANT, ProtoNoiseFunctionComponent, ProtoSurfaceEstimator,
    },
};

pub struct SurfaceHeightSamplerBuilderOptions {
    // 要检查的最小 y 层级
    minimum_y: i32,
    // 要检查的最大 y 层级
    maximum_y: i32,
    y_level_step_count: usize,
}

impl SurfaceHeightSamplerBuilderOptions {
    #[must_use]
    pub const fn new(minimum_y: i32, maximum_y: i32, y_level_step_count: usize) -> Self {
        Self {
            minimum_y,
            maximum_y,
            y_level_step_count,
        }
    }
}

pub struct SurfaceHeightEstimateSampler<'a> {
    minimum_y: i32,
    maximum_y: i32,
    y_level_step_count: usize,

    component_stack: Box<[ChunkNoiseFunctionComponent<'a>]>,

    // TODO: 这里能改用 flat map 吗？我觉得含水层采样器会在区块之外采样
    cache: FxHashMap<u64, i32>,
}

impl<'a> SurfaceHeightEstimateSampler<'a> {
    pub fn estimate_height(&mut self, block_x: i32, block_z: i32) -> i32 {
        let biome_aligned_x = biome_coords::to_block(biome_coords::from_block(block_x));
        let biome_aligned_z = biome_coords::to_block(biome_coords::from_block(block_z));

        let packed_column = chunk_pos::packed(biome_aligned_x as u64, biome_aligned_z as u64);
        if let Some(&estimate) = self.cache.get(&packed_column) {
            return estimate;
        }

        let estimate = self.calculate_height_estimate(biome_aligned_x, biome_aligned_z);
        self.cache.insert(packed_column, estimate);
        estimate
    }

    fn calculate_height_estimate(&mut self, aligned_x: i32, aligned_z: i32) -> i32 {
        let pos = Vector3::new(aligned_x, 0, aligned_z);

        // 如果最顶层组件是 FindTopSurface，我们要执行精确搜索
        // 而非其内部采用的粗粒度逐格搜索。
        if let Some(ChunkNoiseFunctionComponent::Dependent(
            crate::generation::noise::router::proto_noise_router::DependentProtoNoiseFunctionComponent::FindTopSurface(fts),
        )) = self.component_stack.last()
        {
            let upper = ChunkNoiseFunctionComponent::sample_from_stack(
                &mut self.component_stack[..=fts.upper_bound_index()],
                &pos,
            );

            // 首先找到包含表面的粗粒度单元格
            let cell_height = fts.cell_height();
            let mut y = (upper / cell_height as f32).floor() as i32 * cell_height;
            while y >= self.minimum_y {
                let density = ChunkNoiseFunctionComponent::sample_from_stack(
                    &mut self.component_stack[..=fts.density_index()],
                    &Vector3::new(aligned_x, y, aligned_z),
                );
                if density > 0.0 {
                    return y;
                }
                y -= cell_height;
            }

            return self.minimum_y;
        }

        let surface_y =
            ChunkNoiseFunctionComponent::sample_from_stack(&mut self.component_stack, &pos);
        surface_y.floor() as i32
    }

    #[must_use]
    pub fn generate(
        base: &'a ProtoSurfaceEstimator,
        build_options: &SurfaceHeightSamplerBuilderOptions,
    ) -> Self {
        // TODO: 遍历所有组件（甚至包括用不到的）似乎有点浪费
        // 因为它们用于区块生成），但这是目前我能做到的最好方案。
        // （是否应遍历各函数并更新索引？）
        let mut component_stack =
            Vec::<ChunkNoiseFunctionComponent>::with_capacity(base.full_component_stack.len());
        for base_component in &base.full_component_stack {
            let chunk_component = match base_component {
                ProtoNoiseFunctionComponent::Dependent(dependent) => {
                    ChunkNoiseFunctionComponent::Dependent(dependent)
                }
                ProtoNoiseFunctionComponent::Independent(independent) => {
                    ChunkNoiseFunctionComponent::Independent(independent)
                }
                ProtoNoiseFunctionComponent::PassThrough(pass_through) => {
                    ChunkNoiseFunctionComponent::PassThrough(pass_through.clone())
                }
                ProtoNoiseFunctionComponent::Beardifier(_) => {
                    ChunkNoiseFunctionComponent::Independent(&BEARDIFIER_ZERO_CONSTANT)
                }
                ProtoNoiseFunctionComponent::Wrapper(wrapper) => {
                    //NOTE: 根据我们此前与 proto 函数建立的不变量，可以保证
                    // 被包装的函数已经在栈上
                    let min_value = component_stack[wrapper.input_index].min();
                    let max_value = component_stack[wrapper.input_index].max();

                    match wrapper.wrapper_type {
                        WrapperType::Cache => ChunkNoiseFunctionComponent::Chunk(
                            ChunkSpecificNoiseFunctionComponent::Cache(Cache::new(
                                wrapper.input_index,
                                min_value,
                                max_value,
                            )),
                        ),
                        WrapperType::Interpolated { .. } => {
                            ChunkNoiseFunctionComponent::PassThrough(PassThrough::new(
                                wrapper.input_index,
                                min_value,
                                max_value,
                            ))
                        }
                    }
                }
            };
            component_stack.push(chunk_component);
        }

        Self {
            component_stack: component_stack.into_boxed_slice(),

            maximum_y: build_options.maximum_y,
            minimum_y: build_options.minimum_y,
            y_level_step_count: build_options.y_level_step_count,

            cache: FxHashMap::default(),
        }
    }
}
