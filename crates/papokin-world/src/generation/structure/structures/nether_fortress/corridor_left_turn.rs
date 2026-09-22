use papokin_data::{Block, block_properties::OakFenceLikeProperties};
use papokin_util::{
    BlockDirection,
    math::block_box::BlockBox,
    random::{RandomGenerator, RandomImpl},
};

use crate::{
    ProtoChunk,
    generation::structure::{
        piece::StructurePieceType,
        structures::{
            StructurePiece, StructurePieceBase, StructurePiecesCollector, WorldPortalExt,
            nether_fortress::{NetherFortressPiece, NetherFortressPieceType, PieceWeight},
        },
    },
};

/// 向左转（西北）的内部走廊，可含一个箱子（5 × 7 × 5）。
pub struct CorridorLeftTurnPiece {
    pub piece: NetherFortressPiece,
    pub contains_chest: bool,
}

impl CorridorLeftTurnPiece {
    pub fn create(
        collector: &mut StructurePiecesCollector,
        random: &mut impl RandomImpl,
        x: i32,
        y: i32,
        z: i32,
        orientation: BlockDirection,
        chain_length: u32,
    ) -> Option<Box<dyn StructurePieceBase>> {
        let bbox = BlockBox::rotated(x, y, z, -1, 0, 0, 5, 7, 5, &orientation);
        if !NetherFortressPiece::is_in_bounds(&bbox) || collector.get_intersecting(&bbox).is_some()
        {
            return None;
        }

        let contains_chest = random.next_bounded_i32(3) == 0;
        let mut piece = NetherFortressPiece::new(
            StructurePieceType::NetherFortressCorridorLeftTurn,
            chain_length,
            bbox,
        );
        piece.piece.set_facing(Some(orientation));
        Some(Box::new(Self {
            piece,
            contains_chest,
        }))
    }
}

impl StructurePieceBase for CorridorLeftTurnPiece {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn get_structure_piece(&self) -> &StructurePiece {
        &self.piece.piece
    }

    fn get_structure_piece_mut(&mut self) -> &mut StructurePiece {
        &mut self.piece.piece
    }

    fn fill_openings_nether(
        &self,
        start: &StructurePiece,
        random: &mut RandomGenerator,
        bridge_pieces: &mut Vec<PieceWeight>,
        corridor_pieces: &mut Vec<PieceWeight>,
        collector: &mut StructurePiecesCollector,
        pieces_to_process: &mut Vec<Box<dyn StructurePieceBase>>,
    ) {
        let mut last: Option<NetherFortressPieceType> = None;
        self.piece.fill_nw_opening(
            start,
            collector,
            random,
            bridge_pieces,
            corridor_pieces,
            &mut last,
            0,
            1,
            true,
            pieces_to_process,
        );
    }

    fn place(
        &mut self,
        chunk: &mut ProtoChunk,
        _block_registry: &dyn WorldPortalExt,
        random: &mut RandomGenerator,
        _seed: i64,
        chunk_box: &BlockBox,
    ) {
        let bb = *chunk_box;
        let p = &self.piece.piece;
        let nb = Block::NETHER_BRICKS.default_state;
        let air = Block::AIR.default_state;

        let fence = |west: bool, east: bool, north: bool, south: bool| {
            let mut props = OakFenceLikeProperties::default(&Block::NETHER_BRICK_FENCE);
            props.west = west;
            props.east = east;
            props.north = north;
            props.south = south;
            papokin_data::BlockState::from_id(props.to_state_id(&Block::NETHER_BRICK_FENCE))
        };
        let f_ew = fence(true, true, false, false);
        let f_ns = fence(false, false, true, true);

        p.fill_with_outline(chunk, &bb, false, 0, 0, 0, 4, 1, 4, nb, nb);
        p.fill_with_outline(chunk, &bb, false, 0, 2, 0, 4, 5, 4, air, air);

        // 右墙（关闭）
        p.fill_with_outline(chunk, &bb, false, 4, 2, 0, 4, 5, 4, nb, nb);
        p.fill_with_outline(chunk, &bb, false, 4, 3, 1, 4, 4, 1, f_ns, f_ns);
        p.fill_with_outline(chunk, &bb, false, 4, 3, 3, 4, 4, 3, f_ns, f_ns);

        // 前墙（关闭）
        p.fill_with_outline(chunk, &bb, false, 0, 2, 0, 0, 5, 0, nb, nb);

        // 后墙
        p.fill_with_outline(chunk, &bb, false, 0, 2, 4, 3, 5, 4, nb, nb);
        p.fill_with_outline(chunk, &bb, false, 1, 3, 4, 1, 4, 4, f_ew, f_ew);
        p.fill_with_outline(chunk, &bb, false, 3, 3, 4, 3, 4, 4, f_ew, f_ew);

        // 可选的箱子
        if self.contains_chest {
            let chest_pos = p.offset_pos(3, 2, 3);
            if bb.contains_pos(&chest_pos) {
                self.contains_chest = false;
                p.add_chest(
                    chunk,
                    &bb,
                    random,
                    3,
                    2,
                    3,
                    "minecraft:chests/nether_bridge",
                );
            }
        }

        // 天花板
        p.fill_with_outline(chunk, &bb, false, 0, 6, 0, 4, 6, 4, nb, nb);

        for i in 0..=4i32 {
            for j in 0..=4i32 {
                p.fill_downwards(chunk, nb, i, -1, j, &bb);
            }
        }
    }
}
