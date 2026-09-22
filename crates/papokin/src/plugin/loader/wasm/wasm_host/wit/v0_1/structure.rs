use papokin_util::math::vector3::Vector3;
use papokin_world::generation::structure::template::{
    self as template_registry, place_template_with_options,
};
use wasmtime::component::{Access, HasSelf, Resource};

use crate::plugin::loader::wasm::wasm_host::state::PluginHostState;
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::common::BlockPos as WitBlockPos;
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::structure::{
    Host as StructureHost, HostWithStore as StructureHostWithStore, Mirror as WitMirror,
    Rotation as WitRotation,
};
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::world::World;
use crate::world::block_placer::WorldBlockPlacer;

const fn to_internal_rotation(rotation: WitRotation) -> papokin_data::Rotation {
    match rotation {
        WitRotation::None => papokin_data::Rotation::None,
        WitRotation::Clockwise90 => papokin_data::Rotation::Clockwise90,
        WitRotation::Clockwise180 => papokin_data::Rotation::Rotate180,
        WitRotation::Counterclockwise90 => papokin_data::Rotation::CounterClockwise90,
    }
}

const fn to_internal_mirror(mirror: WitMirror) -> papokin_data::Mirror {
    match mirror {
        WitMirror::None => papokin_data::Mirror::None,
        WitMirror::LeftRight => papokin_data::Mirror::LeftRight,
        WitMirror::FrontBack => papokin_data::Mirror::FrontBack,
    }
}

impl StructureHost for PluginHostState {
    async fn register_structure(
        &mut self,
        name: String,
        nbt: Vec<u8>,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(template_registry::register_template(&name, &nbt)
            .map(|_| ())
            .map_err(|err| err.to_string()))
    }

    async fn has_structure(&mut self, name: String) -> wasmtime::Result<bool> {
        Ok(template_registry::has_template(&name))
    }

    async fn list_structures(&mut self) -> wasmtime::Result<Vec<String>> {
        Ok(template_registry::list_template_names())
    }
}

impl StructureHostWithStore<PluginHostState> for HasSelf<PluginHostState> {
    async fn place_structure(
        mut host: Access<'_, PluginHostState, Self>,
        world: Resource<World>,
        name: String,
        pos: WitBlockPos,
        rotation: Option<WitRotation>,
        mirror: Option<WitMirror>,
    ) -> wasmtime::Result<Result<bool, String>> {
        let (world, plugin) = {
            let state = host.get();
            let world = state.get_world_res(&world)?.provider.clone();
            let plugin = state
                .plugin
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))?;
            (world, plugin)
        };

        let Some(template) = template_registry::get_template(&name) else {
            return Ok(Err(format!("未找到结构模板：{name}")));
        };

        let rotation = rotation.map_or(papokin_data::Rotation::None, to_internal_rotation);
        let mirror = mirror.map_or(papokin_data::Mirror::None, to_internal_mirror);
        let origin = Vector3::new(pos.x, pos.y, pos.z);

        // 与 `/place template` 命令相同的放置机制
        // （`PlaceTemplateExecutor`：通过 `WorldBlockPlacer` 缓冲，然后
        // 刷新排队的方块更新，让每个客户端版本都能看到。
        plugin
            .store
            .pump_blocking(&mut host, move || {
                let mut placer = WorldBlockPlacer::new(&world);
                place_template_with_options(
                    &mut placer,
                    &template,
                    origin,
                    (0, 0),
                    rotation,
                    mirror,
                    false,
                    false,
                    &[],
                    None,
                    false,
                );

                placer.finalize();
                world.queue_block_updates(&placer.changed_positions);
                world.flush_block_updates();
            })
            .await?;

        Ok(Ok(true))
    }
}
