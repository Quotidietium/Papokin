use crate::entity::player::Player;
use dashmap::DashMap;
use pumpkin_data::dimension::Dimension;
use pumpkin_util::math::{position::BlockPos, vector2::Vector2};
use std::sync::{Arc, Mutex};

pub struct MapManager {
    pub maps: DashMap<i32, Arc<Mutex<MapData>>>,
}

impl Default for MapManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MapManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            maps: DashMap::new(),
        }
    }

    #[must_use]
    pub fn get_map(&self, id: i32) -> Option<Arc<Mutex<MapData>>> {
        self.maps.get(&id).map(|m| m.clone())
    }

    #[must_use]
    pub fn create_map(
        &self,
        id: i32,
        dimension: Dimension,
        x: i32,
        z: i32,
        scale: i8,
    ) -> Arc<Mutex<MapData>> {
        let map = Arc::new(Mutex::new(MapData::new(dimension, x, z, scale)));
        self.maps.insert(id, map.clone());
        map
    }
}

pub struct MapData {
    pub scale: i8,
    pub locked: bool,
    pub dimension: Dimension,
    pub center_x: i32,
    pub center_z: i32,
    pub colors: Box<[u8; 128 * 128]>,
    pub decorations: Vec<MapDecoration>,
    pub dirty: bool,
    pub fully_updated: bool,
}

impl MapData {
    #[must_use]
    pub fn new(dimension: Dimension, x: i32, z: i32, scale: i8) -> Self {
        Self {
            scale,
            locked: false,
            dimension,
            center_x: x,
            center_z: z,
            colors: Box::new([0; 128 * 128]),
            decorations: Vec::new(),
            dirty: true,
            fully_updated: false,
        }
    }

    pub fn set_color(&mut self, x: usize, z: usize, color: u8) {
        if x < 128 && z < 128 {
            let idx = z * 128 + x;
            if self.colors[idx] != color {
                self.colors[idx] = color;
                self.dirty = true;
            }
        }
    }

    pub fn update(&mut self, player: &Player) {
        // Vanilla semantic: a locked map's canvas is frozen; only cursors
        // (added by the caller) keep updating.
        if self.locked {
            return;
        }
        let world = player.world();
        let scale = 1 << self.scale;
        let center_x = self.center_x;
        let center_z = self.center_z;

        let player_pos = player.position();
        let player_x = player_pos.x as i32;
        let player_z = player_pos.z as i32;

        let start_img_x = ((player_x - center_x) / scale + 64).clamp(0, 127) as usize;
        let start_img_z = ((player_z - center_z) / scale + 64).clamp(0, 127) as usize;

        let radius = 16;
        let (range_x, range_z) = if self.fully_updated {
            (
                (start_img_x.saturating_sub(radius))..(start_img_x + radius).min(128),
                (start_img_z.saturating_sub(radius))..(start_img_z + radius).min(128),
            )
        } else {
            self.fully_updated = true;
            (0..128, 0..128)
        };

        self.render_range(&world, range_x, range_z);
    }

    /// Re-renders the full 128x128 canvas from the terrain of `world`, using
    /// the same height-shading pipeline as the player-driven [`Self::update`].
    ///
    /// This is the entry point for plugin-triggered terrain renders, where no
    /// holding player context is available.
    pub fn render_full(&mut self, world: &crate::world::World) {
        self.fully_updated = true;
        self.render_range(world, 0..128, 0..128);
    }

    fn render_range(
        &mut self,
        world: &crate::world::World,
        range_x: std::ops::Range<usize>,
        range_z: std::ops::Range<usize>,
    ) {
        let scale = 1 << self.scale;
        let center_x = self.center_x;
        let center_z = self.center_z;

        for img_x in range_x {
            let mut prev_y = -1;
            for img_z in range_z.clone() {
                let world_x = (img_x as i32 - 64) * scale + center_x;
                let world_z = (img_z as i32 - 64) * scale + center_z;

                let top_y = world.get_top_block(Vector2::new(world_x, world_z));
                let block = world.get_block(&BlockPos::new(world_x, top_y, world_z));

                let color_base = block.map_color;

                let mut brightness = 2; // Normal
                if prev_y != -1 {
                    if top_y > prev_y {
                        brightness = 3; // High
                    } else if top_y < prev_y {
                        brightness = 1; // Low
                    }
                }
                prev_y = top_y;

                let color = color_base * 4 + brightness;
                self.set_color(img_x, img_z, color);
            }
        }
    }

    /// Immediately sends this map's canvas and plugin-added decorations to
    /// every online player currently holding a filled map with `map_id`.
    ///
    /// The live player cursor is (re)added by the per-tick map sync; this
    /// flush exists so plugin-driven edits show up without waiting for it.
    pub fn send_to_holders(&self, server: &crate::server::Server, map_id: i32) {
        use pumpkin_data::data_component_impl::MapIdImpl;
        use pumpkin_data::item::Item;
        use pumpkin_protocol::codec::var_int::VarInt;
        use pumpkin_protocol::java::client::play::{CMapItemData, MapIcon, MapPatch};
        use pumpkin_util::Hand;
        use pumpkin_util::text::TextComponent;

        let icons: Vec<MapIcon> = self
            .decorations
            .iter()
            .map(|decoration| MapIcon {
                icon_type: VarInt(decoration.icon_type),
                x: decoration.x,
                z: decoration.z,
                direction: decoration.direction,
                display_name: decoration
                    .display_name
                    .as_ref()
                    .map(|name| TextComponent::text(name.clone())),
            })
            .collect();

        for player in server.get_all_players() {
            let holds_map = Hand::all().into_iter().any(|hand| {
                let stack = player.inventory().get_stack_in_hand(hand);
                stack.item.id == Item::FILLED_MAP.id
                    && stack
                        .get_data_component::<MapIdImpl>()
                        .is_some_and(|component| component.id == map_id)
            });
            if holds_map {
                player.try_enqueue_packet_editioned(
                    &CMapItemData {
                        map_id: VarInt(map_id),
                        scale: self.scale,
                        tracking_position: true,
                        locked: self.locked,
                        icons: Some(&icons),
                        data: Some(MapPatch {
                            columns: 128,
                            rows: 128,
                            x: 0,
                            z: 0,
                            data: &*self.colors,
                        }),
                    },
                    &self.bedrock_map_packet(
                        map_id,
                        self.decorations
                            .iter()
                            .map(|decoration| {
                                (
                                    decoration.icon_type,
                                    decoration.x,
                                    decoration.z,
                                    decoration.direction,
                                    decoration.display_name.clone(),
                                )
                            })
                            .collect(),
                        true,
                    ),
                );
            }
        }
    }

    /// Builds the Bedrock `ClientboundMapItemData` (0x43) mirror of this map.
    ///
    /// `icons` are Java-style icon tuples `(icon_type, x, z, direction,
    /// label)`; they are translated to Bedrock decorations plus the
    /// per-decoration tracked objects Bedrock clients require to render any
    /// icon at all. When `include_canvas` is set, the full 128x128 texture is
    /// attached as ABGR pixels; otherwise only the decoration sections update.
    #[must_use]
    pub fn bedrock_map_packet(
        &self,
        map_id: i32,
        icons: Vec<(i32, i8, i8, i8, Option<String>)>,
        include_canvas: bool,
    ) -> pumpkin_protocol::bedrock::client::map_item_data::CMapItemData {
        use pumpkin_protocol::bedrock::client::map_item_data::{
            MapDecoration as BedrockDecoration, MapTrackedObject,
        };
        use pumpkin_protocol::codec::var_long::VarLong;

        let mut tracked_objects = Vec::with_capacity(icons.len());
        let mut decorations = Vec::with_capacity(icons.len());
        for (index, (icon_type, x, z, direction, label)) in icons.into_iter().enumerate() {
            let (image, color) = bedrock_icon(icon_type);
            tracked_objects.push(MapTrackedObject::entity(index as i64));
            decorations.push(BedrockDecoration {
                image,
                rotation: direction as u8,
                x: x as u8,
                y: z as u8,
                label: label.unwrap_or_default(),
                color,
            });
        }

        let (width, height, x_offset, y_offset, colors) = if include_canvas {
            let pixels = self
                .colors
                .iter()
                .map(|color_id| map_color_to_abgr(*color_id))
                .collect();
            (
                Some(pumpkin_protocol::codec::var_int::VarInt(128)),
                Some(pumpkin_protocol::codec::var_int::VarInt(128)),
                Some(pumpkin_protocol::codec::var_int::VarInt(0)),
                Some(pumpkin_protocol::codec::var_int::VarInt(0)),
                Some(pixels),
            )
        } else {
            (None, None, None, None, None)
        };

        pumpkin_protocol::bedrock::client::map_item_data::CMapItemData {
            map_id: VarLong(i64::from(map_id)),
            dimension: bedrock_dimension_id(&self.dimension),
            locked: self.locked,
            origin: BlockPos::new(0, 0, 0),
            tracked_entity_ids: Some(vec![VarLong(i64::from(map_id))]),
            scale: Some(self.scale as u8),
            tracked_objects: Some(tracked_objects),
            decorations: Some(decorations),
            width,
            height,
            x_offset,
            y_offset,
            colors,
        }
    }
}

pub struct MapDecoration {
    pub icon_type: i32,
    pub x: i8,
    pub z: i8,
    pub direction: i8,
    pub display_name: Option<String>,
}

/// Bedrock dimension ids as sent in `ClientboundMapItemData` and
/// `ChangeDimension`: 0 = overworld, 1 = nether, 2 = end.
fn bedrock_dimension_id(dimension: &Dimension) -> u8 {
    match dimension.minecraft_name {
        "minecraft:the_nether" => 1,
        "minecraft:the_end" => 2,
        _ => 0,
    }
}

/// Converts a Java map colour byte (`base_id * 4 + brightness`) into the
/// packed ABGR int Bedrock expects on the canvas, serialised little-endian.
///
/// Brightness multipliers are the vanilla `MapColor.Brightness` table
/// [180, 220, 255, 135] applied with floor division; verified against
/// Geyser's precomputed `MapColor` entries (e.g. shaded id 4 = grass low =
/// RGB(89, 125, 39)). Base id 0 is fully transparent.
fn map_color_to_abgr(color_id: u8) -> i32 {
    use pumpkin_data::map_color::MapColor;

    const BRIGHTNESS: [u32; 4] = [180, 220, 255, 135];

    let base = color_id >> 2;
    if base == 0 {
        return 0;
    }
    let Some(map_color) = MapColor::from_id(base) else {
        return 0;
    };
    let multiplier = BRIGHTNESS[(color_id & 3) as usize];
    let (r, g, b) = map_color.rgb;
    let r = u32::from(r) * multiplier / 255;
    let g = u32::from(g) * multiplier / 255;
    let b = u32::from(b) * multiplier / 255;
    (0xFF00_0000u32 | (b << 16) | (g << 8) | r) as i32
}

/// Maps a Java `MapDecorationType` id (pumpkin-data ordering) to the Bedrock
/// decoration image id and its packed ARGB colour (`0xFFRRGGBB`, written
/// little-endian), mirroring Geyser's `BedrockMapIcon` table. Java types with
/// no Bedrock image (1.21.11 additions past trial chambers) fall back to the
/// plain white marker.
const fn bedrock_icon(java_icon_type: i32) -> (u8, i32) {
    const fn argb(r: u32, g: u32, b: u32) -> i32 {
        (0xFF00_0000u32 | (r << 16) | (g << 8) | b) as i32
    }
    const WHITE: i32 = argb(255, 255, 255);

    let (image, color): (u8, i32) = match java_icon_type {
        1 => (7, WHITE),                 // frame -> marker_sign (green arrow)
        2 => (2, WHITE),                 // red_marker
        3 => (3, WHITE),                 // blue_marker
        4 => (4, argb(0, 0, 0)),         // target_x -> black cross (Geyser)
        5 => (5, WHITE),                 // target_point -> triangle_red
        6 => (6, WHITE),                 // player_off_map -> square_white
        7 => (13, WHITE),                // player_off_limits -> small_square_white
        8 => (14, WHITE),                // mansion
        9 => (15, WHITE),                // monument
        10 => (13, argb(255, 255, 255)), // banner_white
        11 => (13, argb(249, 128, 29)),  // banner_orange
        12 => (13, argb(199, 78, 189)),  // banner_magenta
        13 => (13, argb(58, 179, 218)),  // banner_light_blue
        14 => (13, argb(254, 216, 61)),  // banner_yellow
        15 => (13, argb(128, 199, 31)),  // banner_lime
        16 => (13, argb(243, 139, 170)), // banner_pink
        17 => (13, argb(71, 79, 82)),    // banner_gray
        18 => (13, argb(157, 157, 151)), // banner_light_gray
        19 => (13, argb(22, 156, 156)),  // banner_cyan
        20 => (13, argb(137, 50, 184)),  // banner_purple
        21 => (13, argb(60, 68, 170)),   // banner_blue
        22 => (13, argb(131, 84, 50)),   // banner_brown
        23 => (13, argb(94, 124, 22)),   // banner_green
        24 => (13, argb(176, 46, 38)),   // banner_red
        25 => (13, argb(29, 29, 33)),    // banner_black
        26 => (4, WHITE),                // red_x -> cross_white
        27 => (17, WHITE),               // village_desert
        28 => (18, WHITE),               // village_plains
        29 => (19, WHITE),               // village_savanna
        30 => (20, WHITE),               // village_snowy
        31 => (21, WHITE),               // village_taiga
        32 => (22, WHITE),               // jungle_temple
        33 => (23, WHITE),               // swamp_hut -> witch_hut
        34 => (24, WHITE),               // trial_chambers
        // 0 = player -> marker_white; newer types have no Bedrock image
        _ => (0, WHITE),
    };
    (image, color)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_color_abgr_matches_geyser_golden_entries() {
        // Transparent band.
        assert_eq!(map_color_to_abgr(0), 0);
        assert_eq!(map_color_to_abgr(3), 0);
        // Grass shades (Geyser COLOR_4..=7 = RGB (89,125,39) (109,153,48)
        // (127,178,56) (67,94,29)); ABGR int = A<<24|B<<16|G<<8|R.
        let abgr = |r: u32, g: u32, b: u32| (0xFF00_0000u32 | (b << 16) | (g << 8) | r) as i32;
        assert_eq!(map_color_to_abgr(4), abgr(89, 125, 39));
        assert_eq!(map_color_to_abgr(5), abgr(109, 153, 48));
        assert_eq!(map_color_to_abgr(6), abgr(127, 178, 56));
        assert_eq!(map_color_to_abgr(7), abgr(67, 94, 29));
    }

    #[test]
    fn bedrock_icon_table_covers_java_ids() {
        assert_eq!(bedrock_icon(0).0, 0); // player
        assert_eq!(bedrock_icon(1).0, 7); // frame
        assert_eq!(bedrock_icon(10).0, 13); // banner_white
        assert_eq!(bedrock_icon(34).0, 24); // trial_chambers
        assert_eq!(bedrock_icon(41).0, 0); // unmapped falls back
    }
}
