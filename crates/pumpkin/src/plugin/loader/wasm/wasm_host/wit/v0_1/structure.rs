use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::generation::structure::template::{
    self as template_registry, place_template_with_options,
};
use wasmtime::component::{Access, HasSelf, Resource};

use crate::plugin::loader::wasm::wasm_host::state::PluginHostState;
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::pumpkin::plugin::common::BlockPos as WitBlockPos;
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::pumpkin::plugin::structure::{
    Host as StructureHost, HostWithStore as StructureHostWithStore, Mirror as WitMirror,
    Rotation as WitRotation,
};
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::pumpkin::plugin::world::World;
use crate::world::block_placer::WorldBlockPlacer;

const fn to_internal_rotation(rotation: WitRotation) -> pumpkin_data::Rotation {
    match rotation {
        WitRotation::None => pumpkin_data::Rotation::None,
        WitRotation::Clockwise90 => pumpkin_data::Rotation::Clockwise90,
        WitRotation::Clockwise180 => pumpkin_data::Rotation::Rotate180,
        WitRotation::Counterclockwise90 => pumpkin_data::Rotation::CounterClockwise90,
    }
}

const fn to_internal_mirror(mirror: WitMirror) -> pumpkin_data::Mirror {
    match mirror {
        WitMirror::None => pumpkin_data::Mirror::None,
        WitMirror::LeftRight => pumpkin_data::Mirror::LeftRight,
        WitMirror::FrontBack => pumpkin_data::Mirror::FrontBack,
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
                .ok_or_else(|| wasmtime::Error::msg("Plugin instance not available"))?;
            (world, plugin)
        };

        let Some(template) = template_registry::get_template(&name) else {
            return Ok(Err(format!("structure template not found: {name}")));
        };

        let rotation = rotation.map_or(pumpkin_data::Rotation::None, to_internal_rotation);
        let mirror = mirror.map_or(pumpkin_data::Mirror::None, to_internal_mirror);
        let origin = Vector3::new(pos.x, pos.y, pos.z);

        // Same placement mechanism as the `/place template` command
        // (`PlaceTemplateExecutor`): buffer through a `WorldBlockPlacer`, then
        // flush the queued block updates so every client version sees them.
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
