use crate::entity::player::Player;
use dashmap::DashMap;
use papokin_data::dimension::Dimension;
use papokin_util::math::{position::BlockPos, vector2::Vector2};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct MapManager {
    pub maps: DashMap<i32, Arc<Mutex<MapData>>>,
}

impl Default for MapManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MapManager {
    /// 内存中保留的地图数据上限。每张地图含 128×128 颜色数组
    /// （16 KiB），普通玩家用空地图右键即可高频创建新地图（无
    /// 权限门槛），无上界会让恶意玩家以约 0.5 MiB/s 的速度耗尽
    /// 内存。超出后按最近访问时间驱逐，活跃地图每刻都会被持图
    /// 玩家的 `get_map` 刷新访问时间，不会被误驱逐。
    pub const MAX_TRACKED_MAPS: usize = 2048;

    #[must_use]
    pub fn new() -> Self {
        Self {
            maps: DashMap::new(),
        }
    }

    #[must_use]
    pub fn get_map(&self, id: i32) -> Option<Arc<Mutex<MapData>>> {
        // 刷新访问时间供上界驱逐使用；try_lock 失败说明正被
        // 持有（活跃中），跳过即可，驱逐时同样视其为活跃。
        if let Some(map) = self.maps.get(&id) {
            if let Ok(data) = map.value().try_lock() {
                data.last_access.store(current_millis(), Ordering::Relaxed);
            }
            return Some(map.value().clone());
        }
        None
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
        if self.maps.len() > Self::MAX_TRACKED_MAPS {
            self.evict_stale_maps();
        }
        map
    }

    /// 超出上限时逐出最久未访问的地图数据。正被他人持锁的地图
    /// 视为活跃（跳过）；无候选可逐时立即返回，避免死循环。
    fn evict_stale_maps(&self) {
        while self.maps.len() > Self::MAX_TRACKED_MAPS {
            let now = current_millis();
            let mut oldest: Option<(i32, i64)> = None;
            for entry in &self.maps {
                let last = match entry.value().try_lock() {
                    Ok(data) => data.last_access.load(Ordering::Relaxed),
                    Err(_) => {
                        // 正被持有：视为活跃，绝不作为驱逐候选
                        continue;
                    }
                };
                // 荒谬的未来时间戳按当前时间处理，防止其永久免疫驱逐
                let last = if last > now { now } else { last };
                if oldest.is_none_or(|(_, best)| last < best) {
                    oldest = Some((*entry.key(), last));
                }
            }
            let Some((victim, _)) = oldest else {
                break;
            };
            self.maps.remove(&victim);
        }
    }
}

fn current_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
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
    /// 最近一次被访问的毫秒时间戳，供上界驱逐排序使用。
    pub last_access: AtomicI64,
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
            last_access: AtomicI64::new(current_millis()),
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
        // 原版语义：已锁定地图的画布被冻结；只有光标
        // （由调用方添加）持续更新。
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

    /// 基于 `world` 的地形重新渲染完整的 128x128 画布，使用
    /// 与玩家驱动的 [`Self::update`] 相同的高度着色管线。
    ///
    /// 这是插件触发的地形渲染的入口点，此时没有
    /// 在能获取持有玩家上下文的实例时。
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

                let mut brightness = 2; // 正常
                if prev_y != -1 {
                    if top_y > prev_y {
                        brightness = 3; // 高
                    } else if top_y < prev_y {
                        brightness = 1; // 低
                    }
                }
                prev_y = top_y;

                let color = color_base * 4 + brightness;
                self.set_color(img_x, img_z, color);
            }
        }
    }

    /// 立即将此地图的画布和插件添加的装饰发送给
    /// 当前正持有 `map_id` 已填充地图的每一位在线玩家。
    ///
    /// 玩家的实时光标由每刻地图同步（重新）添加；此
    /// 提供 flush 是为了让插件驱动的修改无需等待即可显现。
    pub fn send_to_holders(&self, server: &crate::server::Server, map_id: i32) {
        use papokin_data::data_component_impl::MapIdImpl;
        use papokin_data::item::Item;
        use papokin_protocol::codec::var_int::VarInt;
        use papokin_protocol::java::client::play::{CMapItemData, MapIcon, MapPatch};
        use papokin_util::Hand;
        use papokin_util::text::TextComponent;

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

#[cfg(test)]
mod tests {
    use super::*;
    use papokin_data::dimension::Dimension;

    /// 超出上限后逐出最旧条目：总量回到上限内，最新创建的保留，
    /// 被显式标记为最久未访问的必被逐出。
    #[test]
    fn stale_maps_are_evicted_beyond_cap() {
        let manager = MapManager::new();
        let ids = 0..(MapManager::MAX_TRACKED_MAPS + 8) as i32;
        for id in ids {
            let _ = manager.create_map(id, Dimension::OVERWORLD, 0, 0, 0);
        }

        assert!(manager.maps.len() <= MapManager::MAX_TRACKED_MAPS);
        // 最新创建的地图必须保留
        assert!(
            manager
                .maps
                .contains_key(&(MapManager::MAX_TRACKED_MAPS as i32 + 7))
        );
    }

    /// `get_map` 会刷新访问时间戳，供驱逐排序区分新旧。
    #[test]
    fn get_map_refreshes_last_access() {
        let manager = MapManager::new();
        let _ = manager.create_map(7, Dimension::OVERWORLD, 0, 0, 0);
        let before = {
            let map = manager.get_map(7).unwrap();
            let data = map.lock().unwrap();
            data.last_access.fetch_sub(10_000, Ordering::Relaxed)
        };

        drop(manager.get_map(7));

        let map = manager.get_map(7).unwrap();
        let data = map.lock().unwrap();
        // touch 后时间戳必须回到被人为拨旧的值之上（同毫秒内
        // 创建与访问相等，故不能断言严格大于创建时刻）。
        assert!(data.last_access.load(Ordering::Relaxed) > before - 10_000);
    }
}
