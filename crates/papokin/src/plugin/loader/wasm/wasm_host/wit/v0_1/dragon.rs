use std::sync::{Arc, Weak};

use wasmtime::component::{Access, HasSelf, Resource};

use crate::plugin::loader::wasm::wasm_host::wit::v0_1::uuid::UuidExt;
use crate::plugin::loader::wasm::wasm_host::{
    WasmPlugin,
    state::{DragonFightResource, PluginHostState},
    wit::v0_1::papokin::plugin::{
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

/// 客户机 `dragon-fight` 资源背后的宿主端状态。
///
/// 持有拥有这场战斗的末地世界的弱引用句柄。这场战斗本身
/// 存放于 `World::dragon_fight`，因此每次调用都会升级世界并
/// 仅在该调用期间锁定战斗互斥锁。
pub struct PluginDragonFight {
    pub world: Weak<InternalWorld>,
}

impl PluginDragonFight {
    fn world(&self) -> wasmtime::Result<Arc<InternalWorld>> {
        self.world
            .upgrade()
            .ok_or_else(|| wasmtime::Error::msg("末影龙战斗所在的世界不再可用"))
    }
}

/// 锁定世界的末影龙战斗。当世界不可用时返回错误而非 panic
/// 没有战斗（资源创建时已将这些世界过滤掉，因此这
/// 仅是防御性保护）。
fn lock_fight(
    world: &InternalWorld,
) -> wasmtime::Result<std::sync::MutexGuard<'_, InternalDragonFight>> {
    let Some(fight_mutex) = world.dragon_fight.as_ref() else {
        return Err(wasmtime::Error::msg("世界没有末影龙战斗"));
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
        .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))?;
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
        // 只有末地才有末影龙战斗；其他维度均为 `none`。
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
