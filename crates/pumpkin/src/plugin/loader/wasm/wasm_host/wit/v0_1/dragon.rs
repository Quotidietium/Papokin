use std::sync::{Arc, Weak};

use wasmtime::component::{Access, HasSelf, Resource};

use crate::plugin::loader::wasm::wasm_host::wit::v0_1::uuid::UuidExt;
use crate::plugin::loader::wasm::wasm_host::{
    WasmPlugin,
    state::{DragonFightResource, PluginHostState},
    wit::v0_1::pumpkin::plugin::{
        common::BlockPos as WitBlockPos,
        dragon::{
            DragonFight, DragonRespawnStage, Host as DragonHost, HostDragonFight,
            HostDragonFightWithStore,
        },
        uuid::Uuid as WitUuid,
        world::World,
    },
};
use crate::world::World as InternalWorld;
use crate::world::dragon_fight::{
    DragonFight as InternalDragonFight, DragonRespawnStage as InternalDragonRespawnStage,
};

/// Host-side state behind a guest `dragon-fight` resource.
///
/// Holds a weak handle to the End world owning the fight. The fight itself
/// lives in `World::dragon_fight`, so every call upgrades the world and
/// locks the fight mutex for the duration of that call only.
pub struct PluginDragonFight {
    pub world: Weak<InternalWorld>,
}

impl PluginDragonFight {
    fn world(&self) -> wasmtime::Result<Arc<InternalWorld>> {
        self.world
            .upgrade()
            .ok_or_else(|| wasmtime::Error::msg("dragon fight world no longer available"))
    }
}

/// Locks the world's dragon fight. Fails instead of panicking when the world
/// has no fight (resource creation already filtered those worlds out, so this
/// is only a defensive guard).
fn lock_fight(
    world: &InternalWorld,
) -> wasmtime::Result<std::sync::MutexGuard<'_, InternalDragonFight>> {
    let Some(fight_mutex) = world.dragon_fight.as_ref() else {
        return Err(wasmtime::Error::msg("world has no dragon fight"));
    };
    Ok(fight_mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner))
}

fn fight_world_and_plugin(
    state: &PluginHostState,
    res: &Resource<DragonFight>,
) -> wasmtime::Result<(Arc<InternalWorld>, Arc<WasmPlugin>)> {
    let world = state.get_dragon_fight_res(res)?.provider.world()?;
    let plugin = state
        .plugin
        .as_ref()
        .and_then(std::sync::Weak::upgrade)
        .ok_or_else(|| wasmtime::Error::msg("Plugin instance not available"))?;
    Ok((world, plugin))
}

const fn to_wit_stage(stage: InternalDragonRespawnStage) -> DragonRespawnStage {
    match stage {
        InternalDragonRespawnStage::Start => DragonRespawnStage::Start,
        InternalDragonRespawnStage::PreparingToSummonPillars => {
            DragonRespawnStage::PreparingToSummonPillars
        }
        InternalDragonRespawnStage::SummoningPillars => DragonRespawnStage::SummoningPillars,
        InternalDragonRespawnStage::SummoningDragon => DragonRespawnStage::SummoningDragon,
        InternalDragonRespawnStage::End => DragonRespawnStage::End,
    }
}

const fn from_wit_stage(stage: DragonRespawnStage) -> InternalDragonRespawnStage {
    match stage {
        DragonRespawnStage::Start => InternalDragonRespawnStage::Start,
        DragonRespawnStage::PreparingToSummonPillars => {
            InternalDragonRespawnStage::PreparingToSummonPillars
        }
        DragonRespawnStage::SummoningPillars => InternalDragonRespawnStage::SummoningPillars,
        DragonRespawnStage::SummoningDragon => InternalDragonRespawnStage::SummoningDragon,
        DragonRespawnStage::End => InternalDragonRespawnStage::End,
    }
}

impl DragonHost for PluginHostState {
    async fn get_dragon_fight(
        &mut self,
        world: Resource<World>,
    ) -> wasmtime::Result<Option<Resource<DragonFight>>> {
        let world = self.get_world_res(&world)?.provider.clone();
        // Only The End carries a dragon fight; every other dimension gets `none`.
        if world.dragon_fight.is_none() {
            return Ok(None);
        }
        let fight = self.add_dragon_fight(PluginDragonFight {
            world: Arc::downgrade(&world),
        })?;
        Ok(Some(fight))
    }
}

impl HostDragonFight for PluginHostState {
    async fn get_dragon_uuid(
        &mut self,
        res: Resource<DragonFight>,
    ) -> wasmtime::Result<Option<WitUuid>> {
        let world = self.get_dragon_fight_res(&res)?.provider.world()?;
        let fight = lock_fight(&world)?;
        Ok(fight.dragon_uuid().map(|u| WitUuid::to_wit(&u)))
    }

    async fn is_dragon_alive(&mut self, res: Resource<DragonFight>) -> wasmtime::Result<bool> {
        let world = self.get_dragon_fight_res(&res)?.provider.world()?;
        let fight = lock_fight(&world)?;
        Ok(!fight.dragon_killed)
    }

    async fn has_been_killed_previously(
        &mut self,
        res: Resource<DragonFight>,
    ) -> wasmtime::Result<bool> {
        let world = self.get_dragon_fight_res(&res)?.provider.world()?;
        let fight = lock_fight(&world)?;
        Ok(fight.has_previously_killed_dragon())
    }

    async fn get_respawn_stage(
        &mut self,
        res: Resource<DragonFight>,
    ) -> wasmtime::Result<Option<DragonRespawnStage>> {
        let world = self.get_dragon_fight_res(&res)?.provider.world()?;
        let fight = lock_fight(&world)?;
        Ok(fight.respawn_stage.map(to_wit_stage))
    }

    async fn get_alive_crystals(&mut self, res: Resource<DragonFight>) -> wasmtime::Result<i32> {
        let world = self.get_dragon_fight_res(&res)?.provider.world()?;
        let fight = lock_fight(&world)?;
        Ok(fight.alive_crystals())
    }

    async fn get_exit_portal_location(
        &mut self,
        res: Resource<DragonFight>,
    ) -> wasmtime::Result<Option<WitBlockPos>> {
        let world = self.get_dragon_fight_res(&res)?.provider.world()?;
        let fight = lock_fight(&world)?;
        Ok(fight.exit_portal_location.map(|pos| WitBlockPos {
            x: pos.0.x,
            y: pos.0.y,
            z: pos.0.z,
        }))
    }

    async fn drop(&mut self, res: Resource<DragonFight>) -> wasmtime::Result<()> {
        self.resource_table
            .delete::<DragonFightResource>(Resource::new_own(res.rep()))
            .map_err(wasmtime::Error::from)?;
        Ok(())
    }
}

impl HostDragonFightWithStore<PluginHostState> for HasSelf<PluginHostState> {
    async fn set_respawn_stage(
        mut host: Access<'_, PluginHostState, Self>,
        res: Resource<DragonFight>,
        stage: DragonRespawnStage,
    ) -> wasmtime::Result<bool> {
        let (world, plugin) = fight_world_and_plugin(host.get(), &res)?;
        let stage = from_wit_stage(stage);
        plugin
            .store
            .pump_blocking(&mut host, move || {
                let mut fight = lock_fight(&world)?;
                if !fight.is_respawning() {
                    return Ok(false);
                }
                fight.set_respawn_stage(&world, stage);
                Ok(true)
            })
            .await?
    }

    async fn initiate_respawn(
        mut host: Access<'_, PluginHostState, Self>,
        res: Resource<DragonFight>,
    ) -> wasmtime::Result<bool> {
        let (world, plugin) = fight_world_and_plugin(host.get(), &res)?;
        plugin
            .store
            .pump_blocking(&mut host, move || {
                let mut fight = lock_fight(&world)?;
                fight.try_respawn(&world);
                Ok(fight.is_respawning())
            })
            .await?
    }

    async fn abort_respawn(
        mut host: Access<'_, PluginHostState, Self>,
        res: Resource<DragonFight>,
    ) -> wasmtime::Result<bool> {
        let (world, plugin) = fight_world_and_plugin(host.get(), &res)?;
        plugin
            .store
            .pump_blocking(&mut host, move || {
                let mut fight = lock_fight(&world)?;
                if !fight.is_respawning() {
                    return Ok(false);
                }
                fight.abort_respawn_sequence(&world);
                Ok(true)
            })
            .await?
    }

    async fn reset_crystals(
        mut host: Access<'_, PluginHostState, Self>,
        res: Resource<DragonFight>,
    ) -> wasmtime::Result<()> {
        let (world, plugin) = fight_world_and_plugin(host.get(), &res)?;
        plugin
            .store
            .pump_blocking(&mut host, move || {
                let fight = lock_fight(&world)?;
                fight.reset_spike_crystals(&world);
                Ok(())
            })
            .await?
    }

    async fn spawn_gateway(
        mut host: Access<'_, PluginHostState, Self>,
        res: Resource<DragonFight>,
    ) -> wasmtime::Result<()> {
        let (world, plugin) = fight_world_and_plugin(host.get(), &res)?;
        plugin
            .store
            .pump_blocking(&mut host, move || {
                let mut fight = lock_fight(&world)?;
                fight.spawn_new_gateway(&world);
                Ok(())
            })
            .await?
    }

    async fn spawn_exit_portal(
        mut host: Access<'_, PluginHostState, Self>,
        res: Resource<DragonFight>,
        active: bool,
    ) -> wasmtime::Result<()> {
        let (world, plugin) = fight_world_and_plugin(host.get(), &res)?;
        plugin
            .store
            .pump_blocking(&mut host, move || {
                let mut fight = lock_fight(&world)?;
                fight.spawn_exit_portal(&world, active);
                Ok(())
            })
            .await?
    }

    async fn spawn_crystals(
        mut host: Access<'_, PluginHostState, Self>,
        res: Resource<DragonFight>,
    ) -> wasmtime::Result<()> {
        let (world, plugin) = fight_world_and_plugin(host.get(), &res)?;
        plugin
            .store
            .pump_blocking(&mut host, move || {
                let mut fight = lock_fight(&world)?;
                fight.spawn_crystals(&world);
                Ok(())
            })
            .await?
    }
}
