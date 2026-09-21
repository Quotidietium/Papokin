// Wire layout verified against CloudburstMC Protocol 3.0
// `ClientboundMapItemDataSerializer_v2168` (inherited unchanged by v2169/v2193,
// which ship no newer map serializer), cross-checked with Geyser's
// `JavaMapItemDataTranslator`.
//
// Since 1.26.40 the UpdateFlags bitfield is gone: every section after the
// header is its own optional (bool presence flag + payload).

use std::io::{Error, Write};

use pumpkin_macros::packet;
use pumpkin_util::math::position::BlockPos;

use crate::{
    codec::{var_int::VarInt, var_long::VarLong, var_uint::VarUInt},
    serial::PacketWrite,
};

/// An object 'tracked' on a map, either an entity or a block. On the wire both
/// members are written as independent optionals (1.26.40+); exactly one is
/// expected to be `Some` for the variant `object_type` names.
#[derive(Clone, Debug)]
pub struct MapTrackedObject {
    /// 0 = entity, 1 = block (written as a little-endian i32 ordinal).
    pub object_type: i32,
    pub entity_unique_id: Option<VarLong>,
    pub block_position: Option<BlockPos>,
}

impl MapTrackedObject {
    /// A tracked entity, used by clients as the anchor for a decoration.
    #[must_use]
    pub const fn entity(entity_unique_id: i64) -> Self {
        Self {
            object_type: 0,
            entity_unique_id: Some(VarLong(entity_unique_id)),
            block_position: None,
        }
    }
}

/// A fixed decoration on a map; the client does not move it on its own.
#[derive(Clone, Debug)]
pub struct MapDecoration {
    /// Bedrock image id (0 = white marker … 23 = witch hut, 24 = trial chambers).
    pub image: u8,
    /// Rotation in 16 fixed directions.
    pub rotation: u8,
    /// Offsets on the 128x128 canvas; the numeric space matches Java's signed
    /// icon bytes reinterpreted as unsigned (`java_x as u8`).
    pub x: u8,
    pub y: u8,
    pub label: String,
    /// Packed colour written little-endian. Geyser passes ARGB (`0xFFRRGGBB`)
    /// here (unlike the canvas, which is ABGR); mirrored for compatibility.
    pub color: i32,
}

#[packet(0x43)]
pub struct CMapItemData {
    pub map_id: VarLong,
    /// 0 = overworld, 1 = nether, 2 = end.
    pub dimension: u8,
    pub locked: bool,
    /// Map centre; vanilla clients only require the field to be present
    /// (Geyser sends `Vector3i.ZERO`).
    pub origin: BlockPos,
    /// Map ids this map is included in; Geyser always sends `[map_id]`
    /// (required as of 1.19.50).
    pub tracked_entity_ids: Option<Vec<VarLong>>,
    pub scale: Option<u8>,
    /// Clients require one tracked object per decoration to display it.
    pub tracked_objects: Option<Vec<MapTrackedObject>>,
    pub decorations: Option<Vec<MapDecoration>>,
    pub width: Option<VarInt>,
    pub height: Option<VarInt>,
    pub x_offset: Option<VarInt>,
    pub y_offset: Option<VarInt>,
    /// Row-major `width * y + x` canvas pixels. Each pixel is an ABGR int
    /// (`0xAABBGGRR`) serialised little-endian — i.e. wire bytes R, G, B, A —
    /// matching Cloudburst `writeIntLE` and Geyser `MapColor.getABGR()`.
    pub colors: Option<Vec<i32>>,
}

fn write_opt_list<T: PacketWrite, W: Write>(
    writer: &mut W,
    list: &Option<Vec<T>>,
) -> Result<(), Error> {
    match list {
        Some(items) => {
            true.write(writer)?;
            VarUInt(items.len() as u32).write(writer)?;
            for item in items {
                item.write(writer)?;
            }
            Ok(())
        }
        None => false.write(writer),
    }
}

fn write_opt<T: PacketWrite, W: Write>(writer: &mut W, value: &Option<T>) -> Result<(), Error> {
    value.write(writer)
}

impl PacketWrite for CMapItemData {
    fn write<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        self.map_id.write(writer)?;
        self.dimension.write(writer)?;
        self.locked.write(writer)?;
        self.origin.write(writer)?;

        write_opt_list(writer, &self.tracked_entity_ids)?;
        write_opt(writer, &self.scale)?;

        match &self.tracked_objects {
            Some(objects) => {
                true.write(writer)?;
                VarUInt(objects.len() as u32).write(writer)?;
                for object in objects {
                    object.object_type.write(writer)?;
                    object.entity_unique_id.write(writer)?;
                    object.block_position.write(writer)?;
                }
            }
            None => false.write(writer)?,
        }

        match &self.decorations {
            Some(decorations) => {
                true.write(writer)?;
                VarUInt(decorations.len() as u32).write(writer)?;
                for decoration in decorations {
                    decoration.image.write(writer)?;
                    decoration.rotation.write(writer)?;
                    decoration.x.write(writer)?;
                    decoration.y.write(writer)?;
                    decoration.label.write(writer)?;
                    decoration.color.write(writer)?;
                }
            }
            None => false.write(writer)?,
        }

        write_opt(writer, &self.width)?;
        write_opt(writer, &self.height)?;
        write_opt(writer, &self.x_offset)?;
        write_opt(writer, &self.y_offset)?;
        write_opt_list(writer, &self.colors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_layout_matches_v2168_ground_truth() {
        let packet = CMapItemData {
            map_id: VarLong(1),
            dimension: 0,
            locked: true,
            origin: BlockPos::new(0, 0, 0),
            tracked_entity_ids: Some(vec![VarLong(1)]),
            scale: Some(1),
            tracked_objects: Some(vec![MapTrackedObject::entity(5)]),
            decorations: Some(vec![MapDecoration {
                image: 4,
                rotation: 3,
                x: 200,
                y: 7,
                label: "hi".to_string(),
                color: 0xFF00_0000u32 as i32,
            }]),
            width: Some(VarInt(128)),
            height: Some(VarInt(128)),
            x_offset: Some(VarInt(0)),
            y_offset: Some(VarInt(0)),
            colors: Some(vec![0xFF11_2233u32 as i32]),
        };

        let mut buf = Vec::new();
        packet.write(&mut buf).unwrap();

        let expected: &[u8] = &[
            0x02, // map_id VarLong zigzag(1)
            0x00, // dimension
            0x01, // locked
            0x00, 0x00, 0x00, // origin VarInt x3
            0x01, 0x01, 0x02, // tracked_entity_ids: present, count 1, zigzag(1)
            0x01, 0x01, // scale: present, 1
            0x01, 0x01, // tracked_objects: present, count 1
            0x00, 0x00, 0x00, 0x00, // object_type i32 LE = entity
            0x01, 0x0A, // entity id present, zigzag(5) = 10
            0x00, // block position absent
            0x01, 0x01, // decorations: present, count 1
            0x04, 0x03, 0xC8, 0x07, // image, rotation, x, y
            0x02, 0x68, 0x69, // label "hi"
            0x00, 0x00, 0x00, 0xFF, // color i32 LE
            0x01, 0x80, 0x02, // width: present, zigzag(128) = 256
            0x01, 0x80, 0x02, // height
            0x01, 0x00, // x_offset
            0x01, 0x00, // y_offset
            0x01, 0x01, // colors: present, count 1
            0x33, 0x22, 0x11, 0xFF, // pixel LE
        ];
        assert_eq!(buf, expected);
    }

    #[test]
    fn empty_sections_write_presence_flags_only() {
        let packet = CMapItemData {
            map_id: VarLong(3),
            dimension: 2,
            locked: false,
            origin: BlockPos::new(0, 0, 0),
            tracked_entity_ids: None,
            scale: None,
            tracked_objects: None,
            decorations: None,
            width: None,
            height: None,
            x_offset: None,
            y_offset: None,
            colors: None,
        };
        let mut buf = Vec::new();
        packet.write(&mut buf).unwrap();
        assert_eq!(
            buf,
            &[
                0x06, 0x02, 0x00, // header
                0x00, 0x00, 0x00, // origin
                0x00, // tracked_entity_ids absent
                0x00, // scale absent
                0x00, // tracked_objects absent
                0x00, // decorations absent
                0x00, // width absent
                0x00, // height absent
                0x00, // x_offset absent
                0x00, // y_offset absent
                0x00, // colors absent
            ]
        );
    }
}
