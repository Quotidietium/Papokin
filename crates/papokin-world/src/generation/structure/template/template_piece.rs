//! 基于模板的结构部件，对应 Minecraft 原版 `TemplateStructurePiece`。
//!
//! 此模块提供 `TemplatePiece`，它实现了 `StructurePieceBase`
//! 用于将已加载结构模板中的方块放置到世界中。

use std::sync::Arc;

use papokin_data::{Mirror, Rotation};
use papokin_util::{
    math::{block_box::BlockBox, vector3::Vector3},
    random::RandomGenerator,
};

use crate::{
    ProtoChunk,
    generation::structure::{
        piece::StructurePieceType,
        structures::{StructurePiece, StructurePieceBase},
    },
    world::WorldPortalExt,
};

use super::{BlockStateResolver, PaletteEntry, StructurePlaceSettings, StructureTemplate};

/// 一种结构部件，从 NBT 模板放置方块，与原版 `TemplateStructurePiece` 匹配。
///
/// 此部分处理：
/// - 从 `StructureTemplate` 加载方块
/// - 应用 `StructurePlaceSettings` 中的旋转、镜像和轴点变换
/// - 在世界坐标处放置方块
/// - 用 `final_state` 替换拼图方块
/// - 跳过结构空位方块
pub struct TemplatePiece {
    /// 底层结构部件的元数据。
    pub piece: StructurePiece,

    /// 模板标识符/名称。
    pub template_name: String,

    /// 要放置的模板。
    pub template: Arc<StructureTemplate>,

    /// 放置设置（旋转、镜像、轴点、处理器等）。
    pub place_settings: StructurePlaceSettings,

    /// 模板在世界中的起始位置。
    pub template_position: Vector3<i32>,
}

impl TemplatePiece {
    /// 使用旧版旋转和镜像，在 `origin` 处根据给定的 `template` 创建一个新的模板部件。
    #[must_use]
    pub fn new(
        template: Arc<StructureTemplate>,
        rotation: Rotation,
        mirror: Mirror,
        origin: Vector3<i32>,
        piece_type: StructurePieceType,
    ) -> Self {
        let place_settings = StructurePlaceSettings::new()
            .set_rotation(rotation)
            .set_mirror(mirror);
        let bounding_box = template.get_bounding_box(&place_settings, origin);

        Self {
            piece: StructurePiece::new(piece_type, bounding_box, 0),
            template_name: String::new(),
            template,
            place_settings,
            template_position: origin,
        }
    }

    /// 创建一个带有完整 `StructurePlaceSettings`、对应原版 `TemplateStructurePiece` 的新模板部件。
    #[must_use]
    pub fn from_settings(
        template: Arc<StructureTemplate>,
        template_name: String,
        place_settings: StructurePlaceSettings,
        origin: Vector3<i32>,
        piece_type: StructurePieceType,
    ) -> Self {
        let bounding_box = template.get_bounding_box(&place_settings, origin);

        Self {
            piece: StructurePiece::new(piece_type, bounding_box, 0),
            template_name,
            template,
            place_settings,
            template_position: origin,
        }
    }

    /// 创建一个具有指定链长度的模板部件。
    #[must_use]
    pub fn with_chain_length(
        template: Arc<StructureTemplate>,
        rotation: Rotation,
        mirror: Mirror,
        origin: Vector3<i32>,
        piece_type: StructurePieceType,
        chain_length: u32,
    ) -> Self {
        let mut piece = Self::new(template, rotation, mirror, origin, piece_type);
        piece.piece.chain_length = chain_length;
        piece
    }

    /// 返回应用旋转后的模板尺寸。
    #[must_use]
    pub fn rotated_size(&self) -> Vector3<i32> {
        self.place_settings
            .get_rotation()
            .transform_size(self.template.size)
    }

    /// 获取放置设置中配置的旋转。
    #[must_use]
    pub const fn get_rotation(&self) -> Rotation {
        self.place_settings.get_rotation()
    }

    /// 获取已加载模板的引用。
    #[must_use]
    pub const fn template(&self) -> &Arc<StructureTemplate> {
        &self.template
    }

    /// 获取世界模板位置。
    #[must_use]
    pub const fn template_position(&self) -> Vector3<i32> {
        self.template_position
    }

    /// 获取结构放置设置的引用。
    #[must_use]
    pub const fn place_settings(&self) -> &StructurePlaceSettings {
        &self.place_settings
    }

    /// 获取结构放置设置的可变引用。
    pub const fn place_settings_mut(&mut self) -> &mut StructurePlaceSettings {
        &mut self.place_settings
    }

    /// 将模板相对位置变换为世界坐标。
    #[must_use]
    pub fn transform_pos(&self, local_pos: Vector3<i32>) -> Vector3<i32> {
        StructureTemplate::transform_block_pos(
            local_pos,
            self.place_settings.get_mirror(),
            self.place_settings.get_rotation(),
            self.place_settings.get_rotation_pivot(),
        ) + self.template_position
    }

    /// 将模板位置和边界框移动 `(dx, dy, dz)`。
    pub fn move_piece(&mut self, dx: i32, dy: i32, dz: i32) {
        self.piece.translate(dx, dy, dz);
        self.template_position += Vector3::new(dx, dy, dz);
    }

    /// 检查方块名称是否为结构空位（不应被放置）。
    fn is_structure_void(name: &str) -> bool {
        name == "minecraft:structure_void" || name == "structure_void"
    }

    /// 将模板中的所有方块放入区块，与原版 `postProcess` 行为一致。
    fn place_blocks(&mut self, chunk: &mut ProtoChunk, chunk_box: &BlockBox) {
        self.place_settings = self
            .place_settings
            .clone()
            .set_bounding_box(Some(*chunk_box));
        self.piece.bounding_box = self
            .template
            .get_bounding_box(&self.place_settings, self.template_position);
        let box_limit = self.piece.bounding_box;

        for block in &self.template.blocks {
            let palette_entry = &self.template.palette[block.state as usize];

            // 结构方块是数据标记
            if palette_entry.name == "minecraft:structure_block" {
                continue;
            }

            let mut placed_entry = palette_entry.clone();
            let mut block_entity_nbt = block.nbt.clone();

            // 拼图方块被替换为 final_state
            if palette_entry.name == "minecraft:jigsaw" {
                let final_state = block_entity_nbt
                    .as_ref()
                    .and_then(|nbt| nbt.get_string("final_state"))
                    .unwrap_or("minecraft:air");
                placed_entry = PaletteEntry::from_string(final_state);
                block_entity_nbt = None;
            }

            // 跳过结构空位方块
            if Self::is_structure_void(&placed_entry.name) {
                continue;
            }

            // 解析应用旋转/镜像后的方块状态
            let Some(mut state) = BlockStateResolver::resolve(
                &placed_entry,
                self.place_settings.get_rotation(),
                self.place_settings.get_mirror(),
            ) else {
                continue;
            };

            // 将位置变换到世界坐标
            let world_pos = self.transform_pos(block.pos);

            // 对照结构片段包围盒与当前区块盒两者检查边界
            if !box_limit.contains_pos(&world_pos) || !chunk_box.contains_pos(&world_pos) {
                continue;
            }

            // 如果启用则处理含水状态
            if self.place_settings.should_apply_waterlogging()
                && chunk.get_block_state(&world_pos).to_block_id() == papokin_data::Block::WATER.id
                && let Some((_, waterlogged)) = placed_entry
                    .properties
                    .iter_mut()
                    .find(|(name, _)| name == "waterlogged")
            {
                *waterlogged = "true".to_string();
                if let Some(waterlogged_state) = BlockStateResolver::resolve(
                    &placed_entry,
                    self.place_settings.get_rotation(),
                    self.place_settings.get_mirror(),
                ) {
                    state = waterlogged_state;
                }
            }

            // 放置方块
            chunk.set_block_state(world_pos.x, world_pos.y, world_pos.z, state);

            let final_block = papokin_data::Block::from_id(state.id.to_block_id());
            let block_entity_id = super::get_block_entity_id(final_block.name);
            if block_entity_nbt.is_some() || block_entity_id.is_some() {
                let fallback_id = block_entity_id.unwrap_or(final_block.name);
                let mut placed_nbt = papokin_nbt::compound::NbtCompound::new();

                placed_nbt.put_string("id", fallback_id.to_string());
                placed_nbt.put_int("x", world_pos.x);
                placed_nbt.put_int("y", world_pos.y);
                placed_nbt.put_int("z", world_pos.z);

                if let Some(template_nbt) = &block_entity_nbt {
                    for (key, value) in &template_nbt.child_tags {
                        if key.as_ref() != "x"
                            && key.as_ref() != "y"
                            && key.as_ref() != "z"
                            && key.as_ref() != "id"
                        {
                            placed_nbt.child_tags.insert(key.clone(), value.clone());
                        }
                    }
                }

                if placed_nbt.get_string("LootTable").is_some()
                    && placed_nbt.get_long("LootTableSeed").is_none()
                {
                    use papokin_util::random::{
                        RandomImpl, hash_block_pos, legacy_rand::LegacyRand,
                    };
                    let mut random =
                        LegacyRand::from_seed(
                            hash_block_pos(world_pos.x, world_pos.y, world_pos.z) as u64,
                        );
                    placed_nbt.put_long("LootTableSeed", random.next_i64());
                }

                chunk.add_block_entity(placed_nbt);
            }
        }
    }
}

impl StructurePieceBase for TemplatePiece {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn place(
        &mut self,
        chunk: &mut ProtoChunk,
        _block_registry: &dyn WorldPortalExt,
        _random: &mut RandomGenerator,
        _seed: i64,
        chunk_box: &BlockBox,
    ) {
        self.place_blocks(chunk, chunk_box);
    }

    fn translate(&mut self, x: i32, y: i32, z: i32) {
        self.move_piece(x, y, z);
    }

    fn get_structure_piece(&self) -> &StructurePiece {
        &self.piece
    }

    fn get_structure_piece_mut(&mut self) -> &mut StructurePiece {
        &mut self.piece
    }
}
