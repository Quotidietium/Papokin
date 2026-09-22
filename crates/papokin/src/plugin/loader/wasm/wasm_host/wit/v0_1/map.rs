use std::sync::{Arc, Mutex as StdMutex, Weak};

use wasmtime::component::Resource;

use crate::plugin::loader::wasm::wasm_host::{
    state::{MapViewResource, PluginHostState},
    wit::v0_1::papokin::plugin::{
        map::{self, MapCursor, MapView},
        world::World,
    },
};
use crate::server::Server;
use crate::world::World as InternalWorld;
use crate::world::map::MapData;

/// 客户机 `map-view` 资源背后的宿主端状态。
pub struct PluginMapView {
    pub map_id: i32,
    pub data: Arc<StdMutex<MapData>>,
    pub world: Weak<InternalWorld>,
    pub server: Weak<Server>,
}

impl PluginMapView {
    fn lock_data(&self) -> std::sync::MutexGuard<'_, MapData> {
        self.data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// 将画布标记为脏（以便每刻的地图同步将其重新发送，
    /// 实时玩家光标）并立即推送新画布以及
    /// 将插件添加的光标同步给持有此地图的所有玩家。
    fn mark_dirty_and_flush(&self, map: &mut MapData) {
        map.dirty = true;
        if let Some(server) = self.server.upgrade() {
            map.send_to_holders(&server, self.map_id);
        }
    }
}

fn to_wit_cursor(decoration: &crate::world::map::MapDecoration) -> MapCursor {
    MapCursor {
        icon_type: decoration.icon_type,
        x: decoration.x,
        z: decoration.z,
        direction: decoration.direction,
        display_name: decoration.display_name.clone(),
    }
}

fn from_wit_cursor(cursor: &MapCursor) -> crate::world::map::MapDecoration {
    crate::world::map::MapDecoration {
        icon_type: cursor.icon_type,
        x: cursor.x,
        z: cursor.z,
        direction: cursor.direction,
        display_name: cursor.display_name.clone(),
    }
}

impl map::Host for PluginHostState {
    async fn create_map(
        &mut self,
        world: Resource<World>,
        center_x: i32,
        center_z: i32,
        scale: u8,
    ) -> wasmtime::Result<Resource<MapView>> {
        let world = self.get_world_res(&world)?.provider.clone();
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?
            .clone();

        let map_id = server.next_map_id();
        let data = server.map_manager.create_map(
            map_id,
            world.dimension.clone(),
            center_x,
            center_z,
            scale.min(4) as i8,
        );

        self.add_map_view(PluginMapView {
            map_id,
            data,
            world: Arc::downgrade(&world),
            server: Arc::downgrade(&server),
        })
    }

    async fn get_map(&mut self, map_id: i32) -> wasmtime::Result<Option<Resource<MapView>>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("服务器不可用"))?
            .clone();

        let Some(data) = server.map_manager.get_map(map_id) else {
            return Ok(None);
        };

        let dimension = {
            let map = data
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            map.dimension.clone()
        };
        let world = server.get_world_from_dimension(&dimension);

        let view = self.add_map_view(PluginMapView {
            map_id,
            data,
            world: Arc::downgrade(&world),
            server: Arc::downgrade(&server),
        })?;
        Ok(Some(view))
    }
}

impl map::HostMapView for PluginHostState {
    async fn get_id(&mut self, res: Resource<MapView>) -> wasmtime::Result<i32> {
        Ok(self.get_map_view_res(&res)?.provider.map_id)
    }

    async fn get_scale(&mut self, res: Resource<MapView>) -> wasmtime::Result<u8> {
        let view = &self.get_map_view_res(&res)?.provider;
        let map = view.lock_data();
        Ok(map.scale.clamp(0, 4) as u8)
    }

    async fn set_scale(&mut self, res: Resource<MapView>, scale: u8) -> wasmtime::Result<()> {
        let view = &self.get_map_view_res(&res)?.provider;
        let mut map = view.lock_data();
        map.scale = scale.min(4) as i8;
        view.mark_dirty_and_flush(&mut map);
        Ok(())
    }

    async fn is_locked(&mut self, res: Resource<MapView>) -> wasmtime::Result<bool> {
        let view = &self.get_map_view_res(&res)?.provider;
        Ok(view.lock_data().locked)
    }

    async fn lock(&mut self, res: Resource<MapView>) -> wasmtime::Result<()> {
        let view = &self.get_map_view_res(&res)?.provider;
        let mut map = view.lock_data();
        map.locked = true;
        view.mark_dirty_and_flush(&mut map);
        Ok(())
    }

    async fn unlock(&mut self, res: Resource<MapView>) -> wasmtime::Result<()> {
        let view = &self.get_map_view_res(&res)?.provider;
        let mut map = view.lock_data();
        map.locked = false;
        view.mark_dirty_and_flush(&mut map);
        Ok(())
    }

    async fn set_pixel(
        &mut self,
        res: Resource<MapView>,
        x: u32,
        y: u32,
        color: u32,
    ) -> wasmtime::Result<()> {
        if x >= 128 || y >= 128 {
            return Ok(());
        }
        let view = &self.get_map_view_res(&res)?.provider;
        let palette = if color >> 24 == 0 {
            // 完全透明：原版“未探索”调色板颜色。
            0
        } else {
            crate::block::entities::map::rgb_to_map_color(
                ((color >> 16) & 0xFF) as u8,
                ((color >> 8) & 0xFF) as u8,
                (color & 0xFF) as u8,
            )
        };
        let mut map = view.lock_data();
        map.set_color(x as usize, y as usize, palette);
        view.mark_dirty_and_flush(&mut map);
        Ok(())
    }

    async fn get_pixel(&mut self, res: Resource<MapView>, x: u32, y: u32) -> wasmtime::Result<u32> {
        let view = &self.get_map_view_res(&res)?.provider;
        let map = view.lock_data();
        if x >= 128 || y >= 128 {
            return Ok(0);
        }
        Ok(u32::from(map.colors[y as usize * 128 + x as usize]))
    }

    async fn set_colors_data(
        &mut self,
        res: Resource<MapView>,
        colors: Vec<u8>,
    ) -> wasmtime::Result<()> {
        let view = &self.get_map_view_res(&res)?.provider;
        let mut map = view.lock_data();
        match colors.len() {
            16384 => map.colors.copy_from_slice(&colors),
            49152 | 65536 => {
                let stride = colors.len() / 16384;
                for i in 0..16384 {
                    map.colors[i] = crate::block::entities::map::rgb_to_map_color(
                        colors[i * stride],
                        colors[i * stride + 1],
                        colors[i * stride + 2],
                    );
                }
            }
            len => {
                return Err(wasmtime::Error::msg(format!(
                    "set-colors-data 需要 16384 字节调色板、49152 字节 RGB 或 65536 字节 RGBA 数据，实际为 {len}"
                )));
            }
        }
        view.mark_dirty_and_flush(&mut map);
        Ok(())
    }

    async fn get_colors_data(&mut self, res: Resource<MapView>) -> wasmtime::Result<Vec<u8>> {
        let view = &self.get_map_view_res(&res)?.provider;
        let map = view.lock_data();
        Ok(map.colors.to_vec())
    }

    async fn render_terrain(&mut self, res: Resource<MapView>) -> wasmtime::Result<()> {
        let view = &self.get_map_view_res(&res)?.provider;
        let world = view
            .world
            .upgrade()
            .ok_or_else(|| wasmtime::Error::msg("地图所在的世界不再可用"))?;
        let mut map = view.lock_data();
        map.render_full(&world);
        view.mark_dirty_and_flush(&mut map);
        Ok(())
    }

    async fn add_cursor(
        &mut self,
        res: Resource<MapView>,
        cursor: MapCursor,
    ) -> wasmtime::Result<u32> {
        let view = &self.get_map_view_res(&res)?.provider;
        let mut map = view.lock_data();
        let index = map.decorations.len() as u32;
        map.decorations.push(from_wit_cursor(&cursor));
        view.mark_dirty_and_flush(&mut map);
        Ok(index)
    }

    async fn remove_cursor(
        &mut self,
        res: Resource<MapView>,
        index: u32,
    ) -> wasmtime::Result<bool> {
        let view = &self.get_map_view_res(&res)?.provider;
        let mut map = view.lock_data();
        let index = index as usize;
        if index >= map.decorations.len() {
            return Ok(false);
        }
        map.decorations.remove(index);
        view.mark_dirty_and_flush(&mut map);
        Ok(true)
    }

    async fn get_cursors(&mut self, res: Resource<MapView>) -> wasmtime::Result<Vec<MapCursor>> {
        let view = &self.get_map_view_res(&res)?.provider;
        let map = view.lock_data();
        Ok(map.decorations.iter().map(to_wit_cursor).collect())
    }

    async fn clear_cursors(&mut self, res: Resource<MapView>) -> wasmtime::Result<()> {
        let view = &self.get_map_view_res(&res)?.provider;
        let mut map = view.lock_data();
        map.decorations.clear();
        view.mark_dirty_and_flush(&mut map);
        Ok(())
    }

    async fn drop(&mut self, res: Resource<MapView>) -> wasmtime::Result<()> {
        self.resource_table
            .delete::<MapViewResource>(Resource::new_own(res.rep()))
            .map_err(wasmtime::Error::from)?;
        Ok(())
    }
}
