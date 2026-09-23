use std::{
    collections::{HashMap, HashSet},
    sync::LazyLock,
};

use papokin_util::{math::position::BlockPos, random::RandomGenerator};

use super::features::{
    bamboo::BambooFeature,
    basalt_columns::BasaltColumnsFeature,
    basalt_pillar::BasaltPillarFeature,
    block_column::BlockColumnFeature,
    block_pile::BlockPileFeature,
    blue_ice::BlueIceFeature,
    bonus_chest::BonusChestFeature,
    chorus_plant::ChorusPlantFeature,
    coral::{
        coral_claw::CoralClawFeature, coral_mushroom::CoralMushroomFeature,
        coral_tree::CoralTreeFeature,
    },
    delta_feature::DeltaFeatureFeature,
    desert_well::DesertWellFeature,
    disk::DiskFeature,
    drip_stone::{
        cluster::DripstoneClusterFeature, large::LargeDripstoneFeature,
        small::SmallDripstoneFeature,
    },
    end_gateway::EndGatewayFeature,
    end_island::EndIslandFeature,
    end_platform::EndPlatformFeature,
    end_podium::EndPodiumFeature,
    end_spike::EndSpikeFeature,
    fallen_tree::FallenTreeFeature,
    fill_layer::FillLayerFeature,
    forest_rock::ForestRockFeature,
    fossil::FossilFeature,
    freeze_top_layer::FreezeTopLayerFeature,
    geode::GeodeFeature,
    glowstone_blob::GlowstoneBlobFeature,
    huge_brown_mushroom::HugeBrownMushroomFeature,
    huge_fungus::HugeFungusFeature,
    huge_red_mushroom::HugeRedMushroomFeature,
    ice_spike::IceSpikeFeature,
    iceberg::IcebergFeature,
    kelp::KelpFeature,
    lake::LakeFeature,
    monster_room::DungeonFeature,
    multiface_growth::MultifaceGrowthFeature,
    nether_forest_vegetation::NetherForestVegetationFeature,
    netherrack_replace_blobs::ReplaceBlobsFeature,
    ore::OreFeature,
    random_boolean_selector::RandomBooleanFeature,
    random_patch::RandomPatchFeature,
    random_selector::RandomFeature,
    replace_single_block::ReplaceSingleBlockFeature,
    root_system::RootSystemFeature,
    scattered_ore::ScatteredOreFeature,
    sculk_patch::SculkPatchFeature,
    sea_pickle::SeaPickleFeature,
    seagrass::SeagrassFeature,
    simple_block::SimpleBlockFeature,
    simple_random_selector::SimpleRandomFeature,
    spring_feature::SpringFeatureFeature,
    tree::TreeFeature,
    twisting_vines::TwistingVinesFeature,
    underwater_magma::UnderwaterMagmaFeature,
    vegetation_patch,
    vegetation_patch::VegetationPatchFeature,
    vines::VinesFeature,
    void_start_platform::VoidStartPlatformFeature,
    waterlogged_vegetation_patch,
    waterlogged_vegetation_patch::WaterloggedVegetationPatchFeature,
    weeping_vines::WeepingVinesFeature,
    weighted_random_selector::WeightedRandomFeature,
};
use crate::generation::proto_chunk::GenerationCache;
use crate::world::WorldPortalExt;

pub static CONFIGURED_FEATURES: LazyLock<
    HashMap<papokin_data::configured_feature::ConfiguredFeature, ConfiguredFeature>,
> = LazyLock::new(build_configured_features);

pub static BONE_MEAL_FEATURES: LazyLock<
    HashSet<papokin_data::configured_feature::ConfiguredFeature>,
> = LazyLock::new(|| {
    papokin_data::tag::get_tag_values(
        papokin_data::tag::RegistryKey::WorldgenConfiguredFeature,
        "minecraft:can_spawn_from_bone_meal",
    )
    .into_iter()
    .flatten()
    .filter_map(|name| papokin_data::configured_feature::ConfiguredFeature::from_name(name))
    .collect()
});

pub enum ConfiguredFeature {
    NoOp,
    Tree(Box<TreeFeature>),
    FallenTree(FallenTreeFeature),
    Flower(RandomPatchFeature),
    NoBonemealFlower(RandomPatchFeature),
    RandomPatch(RandomPatchFeature),
    BlockPile(BlockPileFeature),
    SpringFeature(SpringFeatureFeature),
    ChorusPlant(ChorusPlantFeature),
    ReplaceSingleBlock(ReplaceSingleBlockFeature),
    VoidStartPlatform(VoidStartPlatformFeature),
    DesertWell(DesertWellFeature),
    Fossil(FossilFeature),
    HugeRedMushroom(HugeRedMushroomFeature),
    HugeBrownMushroom(HugeBrownMushroomFeature),
    IceSpike(IceSpikeFeature),
    GlowstoneBlob(GlowstoneBlobFeature),
    FreezeTopLayer(FreezeTopLayerFeature),
    Vines(VinesFeature),
    BlockColumn(BlockColumnFeature),
    VegetationPatch(VegetationPatchFeature),
    WaterloggedVegetationPatch(WaterloggedVegetationPatchFeature),
    RootSystem(RootSystemFeature),
    MultifaceGrowth(MultifaceGrowthFeature),
    UnderwaterMagma(UnderwaterMagmaFeature),
    MonsterRoom(DungeonFeature),
    BlueIce(BlueIceFeature),
    Iceberg(IcebergFeature),
    ForestRock(ForestRockFeature),
    Disk(DiskFeature),
    Lake(LakeFeature),
    Ore(OreFeature),
    EndPlatform(EndPlatformFeature),
    EndPodium(EndPodiumFeature),
    EndSpike(EndSpikeFeature),
    EndIsland(EndIslandFeature),
    EndGateway(EndGatewayFeature),
    Seagrass(SeagrassFeature),
    Kelp(KelpFeature),
    CoralTree(CoralTreeFeature),
    CoralMushroom(CoralMushroomFeature),
    CoralClaw(CoralClawFeature),
    SeaPickle(SeaPickleFeature),
    SimpleBlock(SimpleBlockFeature),
    Bamboo(BambooFeature),
    HugeFungus(HugeFungusFeature),
    NetherForestVegetation(NetherForestVegetationFeature),
    WeepingVines(WeepingVinesFeature),
    TwistingVines(TwistingVinesFeature),
    BasaltColumns(BasaltColumnsFeature),
    DeltaFeature(DeltaFeatureFeature),
    NetherrackReplaceBlobs(ReplaceBlobsFeature),
    FillLayer(FillLayerFeature),
    BonusChest(BonusChestFeature),
    BasaltPillar(BasaltPillarFeature),
    ScatteredOre(ScatteredOreFeature),
    RandomSelector(RandomFeature),
    SimpleRandomSelector(SimpleRandomFeature),
    WeightedRandomSelector(WeightedRandomFeature),
    RandomBooleanSelector(RandomBooleanFeature),
    Geode(Box<GeodeFeature>),
    DripstoneCluster(DripstoneClusterFeature),
    LargeDripstone(LargeDripstoneFeature),
    PointedDripstone(SmallDripstoneFeature),
    SculkPatch(SculkPatchFeature),
}

// 是的，这看起来可能很丑，你会疑惑为什么要硬编码，但硬编码是有道理的，因为我们必须在代码中为这些情况添加逻辑

impl ConfiguredFeature {
    #[expect(clippy::too_many_arguments)]
    #[expect(clippy::too_many_lines)]
    pub fn generate<T: GenerationCache>(
        &self,
        chunk: &mut T,
        block_registry: &dyn WorldPortalExt,
        min_y: i8,
        height: u16,
        feature_name: papokin_data::placed_feature::PlacedFeature, // 此放置的地物
        random: &mut RandomGenerator,
        pos: BlockPos,
    ) -> bool {
        match self {
            Self::NetherrackReplaceBlobs(feature) => feature.generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::NetherForestVegetation(feature) => feature.generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::VegetationPatch(feature) => feature.generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::WaterloggedVegetationPatch(feature) => feature.generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::PointedDripstone(feature) => feature.generate(chunk, random, pos),
            Self::CoralMushroom(_feature) => CoralMushroomFeature::generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::CoralTree(_feature) => CoralTreeFeature::generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::CoralClaw(_feature) => {
                CoralClawFeature::generate(chunk, block_registry, random, pos)
            }
            Self::EndPlatform(_feature) => EndPlatformFeature::generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::EndPodium(feature) => feature.generate(chunk, pos),
            Self::EndSpike(feature) => feature.generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::SpringFeature(feature) => feature.generate(block_registry, chunk, random, pos),
            Self::SimpleBlock(feature) => feature.generate(block_registry, chunk, random, pos),
            Self::RandomPatch(feature)
            | Self::Flower(feature)
            | Self::NoBonemealFlower(feature) => feature.generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::DesertWell(_feature) => {
                DesertWellFeature::generate(chunk, min_y, height, feature_name, random, pos)
            }
            Self::Fossil(feature) => feature.generate(chunk, min_y, height, random, pos),
            Self::Bamboo(feature) => feature.generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::BlockColumn(feature) => feature.generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::RandomBooleanSelector(feature) => feature.generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::Tree(feature) => feature.generate(block_registry, chunk, random, pos),
            Self::RandomSelector(feature) => feature.generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::SimpleRandomSelector(feature) => feature.generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::WeightedRandomSelector(feature) => feature.generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::Vines(_feature) => VinesFeature::generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::Seagrass(feature) => {
                feature.generate(chunk, min_y, height, feature_name, random, pos)
            }
            Self::TwistingVines(feature) => {
                feature.generate(chunk, min_y, height, feature_name, random, pos)
            }
            Self::UnderwaterMagma(feature) => {
                feature.generate(chunk, min_y, height, feature_name, random, pos)
            }
            Self::SeaPickle(feature) => {
                feature.generate(chunk, min_y, height, feature_name, random, pos)
            }
            Self::Geode(feature) => feature.generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::Kelp(_feature) => {
                KelpFeature::generate(chunk, min_y, height, feature_name, random, pos)
            }
            Self::Ore(feature) => feature.generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::ScatteredOre(feature) => feature.generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::MonsterRoom(_feature) => {
                DungeonFeature::generate(chunk, min_y, height, feature_name, random, pos)
            }
            Self::BlueIce(_feature) => BlueIceFeature::generate(chunk, random, pos),
            Self::GlowstoneBlob(_feature) => {
                GlowstoneBlobFeature::generate(chunk, min_y, height, feature_name, random, pos)
            }
            Self::Disk(feature) => feature.generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::Lake(feature) => feature.generate(block_registry, chunk, random, pos),
            Self::BasaltColumns(feature) => feature.generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::BasaltPillar(_feature) => BasaltPillarFeature::generate(chunk, random, pos),
            Self::ForestRock(feature) => feature.generate(chunk, random, pos),
            Self::FreezeTopLayer(_feature) => {
                FreezeTopLayerFeature::generate(chunk, min_y, height, feature_name, random, pos)
            }
            Self::IceSpike(_feature) => {
                IceSpikeFeature::generate(chunk, min_y, height, feature_name, random, pos)
            }
            Self::Iceberg(feature) => {
                feature.generate(chunk, min_y, height, feature_name, random, pos)
            }
            Self::ChorusPlant(_feature) => {
                ChorusPlantFeature::generate(chunk, min_y, height, feature_name, random, pos)
            }
            Self::EndIsland(_feature) => {
                EndIslandFeature::generate(chunk, min_y, height, feature_name, random, pos)
            }
            Self::SculkPatch(feature) => feature.generate(block_registry, chunk, random, pos),
            Self::RootSystem(feature) => feature.generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::BonusChest(_feature) => {
                BonusChestFeature::generate(chunk, min_y, height, feature_name, random, pos)
            }
            Self::DeltaFeature(feature) => {
                feature.generate(chunk, min_y, height, feature_name, random, pos)
            }
            Self::BlockPile(feature) => feature.generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::DripstoneCluster(feature) => feature.generate(chunk, pos),
            Self::LargeDripstone(feature) => feature.generate(chunk, random, pos),
            Self::EndGateway(feature) => feature.generate(chunk, pos),
            Self::FillLayer(feature) => {
                feature.generate(chunk, min_y, height, feature_name, random, pos)
            }
            Self::FallenTree(feature) => feature.generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::HugeBrownMushroom(feature) => {
                feature.generate(chunk, min_y, height, feature_name, random, pos)
            }
            Self::HugeFungus(feature) => feature.generate(
                chunk,
                block_registry,
                min_y,
                height,
                feature_name,
                random,
                pos,
            ),
            Self::HugeRedMushroom(feature) => {
                feature.generate(chunk, min_y, height, feature_name, random, pos)
            }
            Self::MultifaceGrowth(feature) => {
                feature.generate(chunk, min_y, height, feature_name, random, pos)
            }
            Self::ReplaceSingleBlock(feature) => {
                feature.generate(chunk, min_y, height, feature_name, random, pos)
            }
            Self::VoidStartPlatform(feature) => {
                feature.generate(chunk, min_y, height, feature_name, random, pos)
            }
            Self::WeepingVines(feature) => {
                feature.generate(chunk, min_y, height, feature_name, random, pos)
            }
            Self::NoOp => false,
        }
    }
}

// 生成的代码现在与其他代码生成产物放在一起
// 放在 `src/generated` 中，以免把它深埋在 `generation/feature` 之下。
include!("../../../../papokin-data/src/generated/configured_features_generated.rs");

#[cfg(test)]
mod tests {
    use super::{BONE_MEAL_FEATURES, CONFIGURED_FEATURES, ConfiguredFeature};
    use crate::chunk_system::chunk_state::StagedChunkEnum;
    use crate::generation::{
        generator::WorldGenerator,
        get_world_gen,
        proto_chunk::{GenerationCache, ProtoChunk},
    };
    use crate::world::WorldPortalExt;
    use papokin_data::{
        Block, BlockState, BlockStateId, Mirror, Rotation,
        configured_feature::ConfiguredFeature as FeatureId, dimension::Dimension,
    };
    use papokin_util::world_seed::Seed;
    use papokin_util::{
        math::position::BlockPos,
        random::{RandomGenerator, xoroshiro128::Xoroshiro},
    };

    /// 测试用 `WorldPortalExt` 桩：允许任意方块放置，镜像/旋转直接委托给方块定义。
    struct Registry;

    impl WorldPortalExt for Registry {
        fn can_place_at(
            &self,
            _block: &Block,
            _state: &BlockState,
            _block_accessor: &dyn crate::world::BlockAccessor,
            _block_pos: &BlockPos,
        ) -> bool {
            true
        }

        fn mirror(
            &self,
            block: &Block,
            state_id: BlockStateId,
            mirror: Mirror,
        ) -> &'static BlockState {
            block.mirror(state_id, mirror)
        }

        fn rotate(
            &self,
            block: &Block,
            state_id: BlockStateId,
            rotation: Rotation,
        ) -> &'static BlockState {
            block.rotate(state_id, rotation)
        }

        fn spawn_mobs_for_chunk_generation(
            &self,
            _cache: &mut dyn GenerationCache,
            _biome: &'static papokin_data::chunk::Biome,
            _chunk_x: i32,
            _chunk_z: i32,
        ) {
        }
    }

    /// 推进一个主世界 (0,0) 区块到噪声阶段，供特性求值使用。
    fn step_noise_stage(
        world_gen: &WorldGenerator,
        generator: &crate::generation::generator::VanillaGenerator,
    ) -> ProtoChunk {
        let mut chunk = ProtoChunk::new(0, 0, world_gen);
        chunk.step_to_biomes(generator);
        chunk.stage = StagedChunkEnum::StructureReferences;
        chunk.step_to_noise(generator);
        chunk
    }

    #[test]
    fn bonemeal_feature_tag_resolves_to_placeable_blocks() {
        assert_eq!(BONE_MEAL_FEATURES.len(), 8);
        let mut random = RandomGenerator::Xoroshiro(Xoroshiro::from_seed(0));
        for key in BONE_MEAL_FEATURES.iter() {
            let Some(ConfiguredFeature::SimpleBlock(feature)) = CONFIGURED_FEATURES.get(key) else {
                panic!("骨粉特性 {key:?} 必须放置方块");
            };
            assert!(
                feature
                    .to_place
                    .get_for_bonemeal(&mut random, BlockPos::new(0, 64, 0))
                    .is_some()
            );
        }
    }

    #[test]
    fn bonemeal_features_use_tag_output() {
        let tag_values = papokin_data::tag::get_tag_values(
            papokin_data::tag::RegistryKey::WorldgenConfiguredFeature,
            "minecraft:can_spawn_from_bone_meal",
        );
        assert!(tag_values.is_some());
        let values = tag_values.unwrap();
        assert_eq!(values.len(), 8);
        assert!(values.contains(&"flower_default"));
        assert!(values.contains(&"wildflower"));
    }

    /// 回归测试：树木的 `below_trunk_provider` 曾被代码生成固定为 AIR
    /// （26.3 数据把提供器抽成了 `block_state_provider` 注册表资源，
    /// 特性 JSON 里以 id 字符串引用，当时的生成器不识别字符串引用，
    /// 静默回退成 AIR），导致每棵树的树干最下方一格被写成空气。
    /// 修复后规则提供器在任何基面（含被挖空的空气位）上都不得产出空气。
    #[test]
    fn tree_below_trunk_provider_never_yields_air() {
        let world_gen = get_world_gen(
            Seed(42),
            Dimension::OVERWORLD,
            false,
            Vec::new(),
            String::new(),
        );
        let WorldGenerator::Noise(generator) = &*world_gen else {
            unreachable!()
        };
        let mut chunk = step_noise_stage(&world_gen, generator);

        let registry = Registry;
        let mut random = RandomGenerator::Xoroshiro(Xoroshiro::from_seed(0));
        let pos = BlockPos::new(8, 100, 8);
        let trees: Vec<&super::super::features::tree::TreeFeature> = CONFIGURED_FEATURES
            .values()
            .filter_map(|feature| match feature {
                ConfiguredFeature::Tree(tree) => Some(tree.as_ref()),
                _ => None,
            })
            .collect();
        assert!(
            trees.len() >= 40,
            "树木特性数量异常：{}（低于预期，数据是否退化？）",
            trees.len()
        );

        for (block, scenario) in [
            (Block::GRASS_BLOCK, "草方块"),
            (Block::DIRT, "泥土"),
            (Block::AIR, "空气位"),
            (Block::SAND, "沙子"),
        ] {
            chunk.set_block_state(pos.0.x, pos.0.y, pos.0.z, block.default_state);
            for tree in &trees {
                assert!(
                    tree.below_trunk_provider
                        .get_optional(&registry, &chunk, &mut random, pos)
                        .is_none_or(|state| !state.is_air()),
                    "树木下方提供器在{scenario}上产出了空气"
                );
            }
        }
    }

    /// Oak 的精确语义抽查：非土壤基面（草方块、沙子、空气位）替换为泥土，
    /// 土壤基面（`cannot_replace_below_tree_trunk` 标签，如泥土）保持原样。
    #[test]
    fn oak_below_trunk_provider_replaces_non_soil_with_dirt() {
        let world_gen = get_world_gen(
            Seed(42),
            Dimension::OVERWORLD,
            false,
            Vec::new(),
            String::new(),
        );
        let WorldGenerator::Noise(generator) = &*world_gen else {
            unreachable!()
        };
        let mut chunk = step_noise_stage(&world_gen, generator);

        let registry = Registry;
        let mut random = RandomGenerator::Xoroshiro(Xoroshiro::from_seed(0));
        let pos = BlockPos::new(8, 100, 8);
        let Some(ConfiguredFeature::Tree(oak)) = CONFIGURED_FEATURES.get(&FeatureId::Oak) else {
            panic!("Oak 特性缺失");
        };
        for (block, expected, message) in [
            (
                Block::GRASS_BLOCK,
                Some(Block::DIRT.default_state.id),
                "Oak 应把树干下方的草方块替换为泥土",
            ),
            (Block::DIRT, None, "Oak 的下方提供器不应改写泥土"),
            (
                Block::SAND,
                Some(Block::DIRT.default_state.id),
                "Oak 应把树干下方的沙子替换为泥土",
            ),
            (
                Block::AIR,
                Some(Block::DIRT.default_state.id),
                "Oak 应把树干下方的空气位补为泥土",
            ),
        ] {
            chunk.set_block_state(pos.0.x, pos.0.y, pos.0.z, block.default_state);
            assert_eq!(
                oak.below_trunk_provider
                    .get_optional(&registry, &chunk, &mut random, pos)
                    .map(|state| state.id),
                expected,
                "{message}"
            );
        }
    }
}
