use std::sync::Arc;

use crate::plugin::{
    loader::wasm::wasm_host::{
        state::PluginHostState,
        wit::v0_1::{
            events::{
                ToFromWasmEvent, cleanup_event, consume_world, from_wasm_block_position,
                from_wasm_position, to_wasm_block_position, to_wasm_position,
            },
            papokin::plugin::event::{
                ChunkLoadEventData, ChunkSaveEventData, ChunkSendEventData, Event,
                SpawnChangeEventData, StructuresLocateEventData, ThunderChangeEventData,
                WeatherChangeEventData, WorldBorderBoundsChangeEventData,
                WorldBorderCenterChangeEventData, WorldDifficultyChangeEventData,
                WorldGameRuleChangeEventData, WorldLoadEventData, WorldUnloadEventData,
            },
        },
    },
    world::{
        chunk_load::ChunkLoad,
        chunk_save::ChunkSave,
        chunk_send::ChunkSend,
        spawn_change::SpawnChangeEvent,
        structures_locate::StructuresLocateEvent,
        world_border_change::{WorldBorderBoundsChangeEvent, WorldBorderCenterChangeEvent},
        world_difficulty_change::WorldDifficultyChangeEvent,
        world_game_rule_change::WorldGameRuleChangeEvent,
    },
};

impl ToFromWasmEvent for SpawnChangeEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let world = state
            .add_world(self.world.clone())
            .expect("添加世界资源失败");

        Event::SpawnChangeEvent(SpawnChangeEventData {
            target_world: world,
            previous_position: to_wasm_block_position(self.previous_position),
            previous_yaw: self.previous_yaw,
            previous_pitch: self.previous_pitch,
            new_position: to_wasm_block_position(self.new_position),
            new_yaw: self.new_yaw,
            new_pitch: self.new_pitch,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::SpawnChangeEvent(data) => Self {
                world: consume_world(state, &data.target_world),
                previous_position: from_wasm_block_position(data.previous_position),
                previous_yaw: data.previous_yaw,
                previous_pitch: data.previous_pitch,
                new_position: from_wasm_block_position(data.new_position),
                new_yaw: data.new_yaw,
                new_pitch: data.new_pitch,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for ChunkLoad {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let target_world = state
            .add_world(self.world.clone())
            .expect("添加世界资源失败");

        let (chunk_x, chunk_z) = self
            .chunk
            .try_read()
            .map_or((0, 0), |guard| (guard.x, guard.z));
        Event::ChunkLoadEvent(ChunkLoadEventData {
            target_world,
            chunk_x,
            chunk_z,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::ChunkLoadEvent(data) => {
                let world = consume_world(state, &data.target_world);
                let chunk_data = papokin_world::chunk::ChunkData {
                    section: papokin_world::chunk::ChunkSections::new(24, -64),
                    heightmap: std::sync::Mutex::new(
                        papokin_world::chunk::ChunkHeightmaps::default(),
                    ),
                    x: data.chunk_x,
                    z: data.chunk_z,
                    block_ticks: papokin_world::tick::scheduler::ChunkTickScheduler::default(),
                    fluid_ticks: papokin_world::tick::scheduler::ChunkTickScheduler::default(),
                    pending_block_entities: std::sync::Mutex::new(
                        std::collections::HashMap::default(),
                    ),
                    light_engine: std::sync::Mutex::new(papokin_world::chunk::ChunkLight::default()),
                    light_populated: std::sync::atomic::AtomicBool::new(false),
                    status: papokin_data::chunk::ChunkStatus::Empty,
                    blending_data: None,
                    dirty: std::sync::atomic::AtomicBool::new(false),
                    inhabited_time: std::sync::atomic::AtomicU64::new(0),
                    custom_data: std::sync::Mutex::new(papokin_nbt::compound::NbtCompound::new()),
                    preserved_data: std::sync::Mutex::new(None),
                };
                Self {
                    world,
                    chunk: Arc::new(tokio::sync::RwLock::new(chunk_data)),
                    cancelled: data.cancelled,
                }
            }
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for ChunkSave {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let target_world = state
            .add_world(self.world.clone())
            .expect("添加世界资源失败");

        let (chunk_x, chunk_z) = self
            .chunk
            .try_read()
            .map_or((0, 0), |guard| (guard.x, guard.z));
        Event::ChunkSaveEvent(ChunkSaveEventData {
            target_world,
            chunk_x,
            chunk_z,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::ChunkSaveEvent(data) => {
                let world = consume_world(state, &data.target_world);
                let chunk_data = papokin_world::chunk::ChunkData {
                    section: papokin_world::chunk::ChunkSections::new(24, -64),
                    heightmap: std::sync::Mutex::new(
                        papokin_world::chunk::ChunkHeightmaps::default(),
                    ),
                    x: data.chunk_x,
                    z: data.chunk_z,
                    block_ticks: papokin_world::tick::scheduler::ChunkTickScheduler::default(),
                    fluid_ticks: papokin_world::tick::scheduler::ChunkTickScheduler::default(),
                    pending_block_entities: std::sync::Mutex::new(
                        std::collections::HashMap::default(),
                    ),
                    light_engine: std::sync::Mutex::new(papokin_world::chunk::ChunkLight::default()),
                    light_populated: std::sync::atomic::AtomicBool::new(false),
                    status: papokin_data::chunk::ChunkStatus::Empty,
                    blending_data: None,
                    dirty: std::sync::atomic::AtomicBool::new(false),
                    inhabited_time: std::sync::atomic::AtomicU64::new(0),
                    custom_data: std::sync::Mutex::new(papokin_nbt::compound::NbtCompound::new()),
                    preserved_data: std::sync::Mutex::new(None),
                };
                Self {
                    world,
                    chunk: Arc::new(tokio::sync::RwLock::new(chunk_data)),
                    cancelled: data.cancelled,
                }
            }
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for ChunkSend {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let target_world = state
            .add_world(self.world.clone())
            .expect("添加世界资源失败");

        Event::ChunkSendEvent(ChunkSendEventData {
            target_world,
            chunk_x: self.chunk.x,
            chunk_z: self.chunk.z,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::ChunkSendEvent(data) => {
                let world = consume_world(state, &data.target_world);
                let chunk_data = papokin_world::chunk::ChunkData {
                    section: papokin_world::chunk::ChunkSections::new(24, -64),
                    heightmap: std::sync::Mutex::new(
                        papokin_world::chunk::ChunkHeightmaps::default(),
                    ),
                    x: data.chunk_x,
                    z: data.chunk_z,
                    block_ticks: papokin_world::tick::scheduler::ChunkTickScheduler::default(),
                    fluid_ticks: papokin_world::tick::scheduler::ChunkTickScheduler::default(),
                    pending_block_entities: std::sync::Mutex::new(
                        std::collections::HashMap::default(),
                    ),
                    light_engine: std::sync::Mutex::new(papokin_world::chunk::ChunkLight::default()),
                    light_populated: std::sync::atomic::AtomicBool::new(false),
                    status: papokin_data::chunk::ChunkStatus::Empty,
                    blending_data: None,
                    dirty: std::sync::atomic::AtomicBool::new(false),
                    inhabited_time: std::sync::atomic::AtomicU64::new(0),
                    custom_data: std::sync::Mutex::new(papokin_nbt::compound::NbtCompound::new()),
                    preserved_data: std::sync::Mutex::new(None),
                };
                Self {
                    world,
                    chunk: Arc::new(chunk_data),
                    cancelled: data.cancelled,
                }
            }
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::world::weather_change::WeatherChangeEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let target_world = state
            .add_world(self.world.clone())
            .expect("添加世界资源失败");

        Event::WeatherChangeEvent(WeatherChangeEventData {
            target_world,
            to_weather_state: self.to_weather_state,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::WeatherChangeEvent(data) => Self {
                world: consume_world(state, &data.target_world),
                to_weather_state: data.to_weather_state,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::world::weather_change::ThunderChangeEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let target_world = state
            .add_world(self.world.clone())
            .expect("添加世界资源失败");

        Event::ThunderChangeEvent(ThunderChangeEventData {
            target_world,
            to_thunder_state: self.to_thunder_state,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::ThunderChangeEvent(data) => Self {
                world: consume_world(state, &data.target_world),
                to_thunder_state: data.to_thunder_state,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::world::world_load::WorldLoadEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let target_world = state
            .add_world(self.world.clone())
            .expect("添加世界资源失败");

        Event::WorldLoadEvent(WorldLoadEventData { target_world })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::WorldLoadEvent(data) => Self {
                world: consume_world(state, &data.target_world),
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::world::world_load::WorldUnloadEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let target_world = state
            .add_world(self.world.clone())
            .expect("添加世界资源失败");

        Event::WorldUnloadEvent(WorldUnloadEventData {
            target_world,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::WorldUnloadEvent(data) => Self {
                world: consume_world(state, &data.target_world),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent
    for crate::plugin::api::events::world::async_structure_generate::AsyncStructureGenerateEvent
{
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::AsyncStructureGenerateEvent(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::event::AsyncStructureGenerateEventData {
            world_name: self.world_name.clone(),
            structure_name: self.structure_name.clone(),
            pos: to_wasm_block_position(self.pos),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::AsyncStructureGenerateEvent(data) => Self {
                world_name: data.world_name,
                structure_name: data.structure_name,
                pos: from_wasm_block_position(data.pos),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::AsyncStructureGenerateEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }
}

impl ToFromWasmEvent
    for crate::plugin::api::events::world::async_structure_spawn::AsyncStructureSpawnEvent
{
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::AsyncStructureSpawnEvent(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::event::AsyncStructureSpawnEventData {
            world_name: self.world_name.clone(),
            structure_name: self.structure_name.clone(),
            pos: to_wasm_block_position(self.pos),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::AsyncStructureSpawnEvent(data) => Self {
                world_name: data.world_name,
                structure_name: data.structure_name,
                pos: from_wasm_block_position(data.pos),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::AsyncStructureSpawnEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::world::chunk_populate::ChunkPopulateEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::ChunkPopulateEvent(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::event::ChunkPopulateEventData {
            chunk_x: self.chunk_pos.x,
            chunk_z: self.chunk_pos.y,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::ChunkPopulateEvent(data) => Self {
                chunk_pos: papokin_util::math::vector2::Vector2::new(data.chunk_x, data.chunk_z),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::ChunkPopulateEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::world::chunk_unload::ChunkUnloadEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::ChunkUnloadEvent(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::event::ChunkUnloadEventData {
            chunk_x: self.chunk_pos.x,
            chunk_z: self.chunk_pos.y,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::ChunkUnloadEvent(data) => Self {
                chunk_pos: papokin_util::math::vector2::Vector2::new(data.chunk_x, data.chunk_z),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::ChunkUnloadEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::world::entities_load::EntitiesLoadEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntitiesLoadEvent(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::event::EntitiesLoadEventData {
            chunk_x: self.chunk_pos.x,
            chunk_z: self.chunk_pos.y,
            entity_count: self.entity_count as u32,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntitiesLoadEvent(data) => Self {
                chunk_pos: papokin_util::math::vector2::Vector2::new(data.chunk_x, data.chunk_z),
                entity_count: data.entity_count as usize,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntitiesLoadEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::world::entities_unload::EntitiesUnloadEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntitiesUnloadEvent(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::event::EntitiesUnloadEventData {
            chunk_x: self.chunk_pos.x,
            chunk_z: self.chunk_pos.y,
            entity_count: self.entity_count as u32,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntitiesUnloadEvent(data) => Self {
                chunk_pos: papokin_util::math::vector2::Vector2::new(data.chunk_x, data.chunk_z),
                entity_count: data.entity_count as usize,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntitiesUnloadEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::world::generic_game::GenericGameEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::GenericGameEvent(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::event::GenericGameEventData {
            event_id: self.event_key.clone(),
            pos: (self.position.x, self.position.y, self.position.z),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::GenericGameEvent(data) => Self {
                event_key: data.event_id,
                position: papokin_util::math::vector3::Vector3::new(
                    data.pos.0, data.pos.1, data.pos.2,
                ),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::GenericGameEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::world::loot_generate::LootGenerateEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::LootGenerateEvent(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::event::LootGenerateEventData {
            loot_table: self.loot_table.clone(),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::LootGenerateEvent(data) => Self {
                loot_table: data.loot_table,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::LootGenerateEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::world::portal_create::PortalCreateEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::PortalCreateEvent(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::event::PortalCreateEventData {
            pos: to_wasm_block_position(self.block_pos),
            portal_type: format!("{:?}", self.portal_type),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PortalCreateEvent(data) => Self {
                block_pos: from_wasm_block_position(data.pos),
                portal_type: match data.portal_type.as_str() {
                    "Nether" => {
                        crate::plugin::api::events::world::portal_create::PortalType::Nether
                    }
                    "End" => crate::plugin::api::events::world::portal_create::PortalType::End,
                    _ => crate::plugin::api::events::world::portal_create::PortalType::Custom,
                },
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PortalCreateEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::world::structure_grow::StructureGrowEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::StructureGrowEvent(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::event::StructureGrowEventData {
            pos: to_wasm_block_position(self.block_pos),
            species: format!("{:?}", self.species),
            bone_meal: self.bone_meal,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::StructureGrowEvent(data) => Self {
                block_pos: from_wasm_block_position(data.pos),
                species: match data.species.as_str() {
                    "Oak" => crate::plugin::api::events::world::structure_grow::TreeType::Oak,
                    "Spruce" => crate::plugin::api::events::world::structure_grow::TreeType::Spruce,
                    "Birch" => crate::plugin::api::events::world::structure_grow::TreeType::Birch,
                    "Jungle" => crate::plugin::api::events::world::structure_grow::TreeType::Jungle,
                    "Acacia" => crate::plugin::api::events::world::structure_grow::TreeType::Acacia,
                    "DarkOak" => {
                        crate::plugin::api::events::world::structure_grow::TreeType::DarkOak
                    }
                    "Mangrove" => {
                        crate::plugin::api::events::world::structure_grow::TreeType::Mangrove
                    }
                    "Cherry" => crate::plugin::api::events::world::structure_grow::TreeType::Cherry,
                    "Poplar" => crate::plugin::api::events::world::structure_grow::TreeType::Poplar,
                    "Azalea" => crate::plugin::api::events::world::structure_grow::TreeType::Azalea,
                    "BrownMushroom" => {
                        crate::plugin::api::events::world::structure_grow::TreeType::BrownMushroom
                    }
                    "RedMushroom" => {
                        crate::plugin::api::events::world::structure_grow::TreeType::RedMushroom
                    }
                    _ => crate::plugin::api::events::world::structure_grow::TreeType::Custom,
                },
                bone_meal: data.bone_meal,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::StructureGrowEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::world::time_skip::TimeSkipEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::TimeSkipEvent(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::event::TimeSkipEventData {
            skip_amount: self.skip_amount,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::TimeSkipEvent(data) => Self {
                skip_amount: data.skip_amount,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::TimeSkipEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::world::world_init::WorldInitEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let target_world = state
            .add_world(self.world.clone())
            .expect("添加世界资源失败");

        Event::WorldInitEvent(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::event::WorldInitEventData {
            target_world,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::WorldInitEvent(data) => Self {
                world: consume_world(state, &data.target_world),
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::world::world_save::WorldSaveEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::WorldSaveEvent(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::event::WorldSaveEventData {
            world_name: self.world_name.clone(),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::WorldSaveEvent(data) => Self {
                world_name: data.world_name,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::WorldSaveEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::world::lightning_strike::LightningStrikeEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::LightningStrikeEvent(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::event::LightningStrikeEventData {
            position: crate::plugin::loader::wasm::wasm_host::wit::v0_1::events::to_wasm_position(self.position),
            is_effect: self.is_effect,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::LightningStrikeEvent(data) => Self {
                position:
                    crate::plugin::loader::wasm::wasm_host::wit::v0_1::events::from_wasm_position(
                        data.position,
                    ),
                is_effect: data.is_effect,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::LightningStrikeEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }
}

impl ToFromWasmEvent for StructuresLocateEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let target_world = state
            .add_world(self.world.clone())
            .expect("添加世界资源失败");
        Event::StructuresLocateEvent(StructuresLocateEventData {
            target_world,
            origin: to_wasm_block_position(self.origin),
            structure: self.structure.clone(),
            radius: self.radius,
            results: self
                .results
                .iter()
                .map(|pos| to_wasm_block_position(*pos))
                .collect(),
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::StructuresLocateEvent(data) => Self {
                world: consume_world(state, &data.target_world),
                origin: from_wasm_block_position(data.origin),
                structure: data.structure,
                radius: data.radius,
                results: data
                    .results
                    .into_iter()
                    .map(from_wasm_block_position)
                    .collect(),
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for WorldDifficultyChangeEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let target_world = state
            .add_world(self.world.clone())
            .expect("添加世界资源失败");
        Event::WorldDifficultyChangeEvent(WorldDifficultyChangeEventData {
            target_world,
            old_difficulty: self.old_difficulty.clone(),
            new_difficulty: self.new_difficulty.clone(),
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::WorldDifficultyChangeEvent(data) => Self {
                world: consume_world(state, &data.target_world),
                old_difficulty: data.old_difficulty,
                new_difficulty: data.new_difficulty,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for WorldGameRuleChangeEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let target_world = state
            .add_world(self.world.clone())
            .expect("添加世界资源失败");
        Event::WorldGameRuleChangeEvent(WorldGameRuleChangeEventData {
            target_world,
            rule: self.rule.clone(),
            value: self.value.clone(),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::WorldGameRuleChangeEvent(data) => Self {
                world: consume_world(state, &data.target_world),
                rule: data.rule,
                value: data.value,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for WorldBorderBoundsChangeEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let target_world = state
            .add_world(self.world.clone())
            .expect("添加世界资源失败");
        Event::WorldBorderBoundsChangeEvent(WorldBorderBoundsChangeEventData {
            target_world,
            old_diameter: self.old_diameter,
            new_diameter: self.new_diameter,
            duration_ms: self.duration_ms,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::WorldBorderBoundsChangeEvent(data) => Self {
                world: consume_world(state, &data.target_world),
                old_diameter: data.old_diameter,
                new_diameter: data.new_diameter,
                duration_ms: data.duration_ms,
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for WorldBorderCenterChangeEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let target_world = state
            .add_world(self.world.clone())
            .expect("添加世界资源失败");
        Event::WorldBorderCenterChangeEvent(WorldBorderCenterChangeEventData {
            target_world,
            old_center: to_wasm_position(self.old_center),
            new_center: to_wasm_position(self.new_center),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::WorldBorderCenterChangeEvent(data) => Self {
                world: consume_world(state, &data.target_world),
                old_center: from_wasm_position(data.old_center),
                new_center: from_wasm_position(data.new_center),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}
