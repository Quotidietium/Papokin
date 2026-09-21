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
                player.try_send_client_packet(&CMapItemData {
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
                });
            }
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
