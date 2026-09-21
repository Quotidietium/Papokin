use std::sync::Arc;
use tokio::sync::Mutex;
use wasmtime::component::Resource;

use crate::plugin::{
    entity::{
        area_effect_cloud_apply::AreaEffectCloudApplyEvent,
        arrow_body_count_change::ArrowBodyCountChangeEvent,
        bat_toggle_sleep::BatToggleSleepEvent,
        creature_spawn::CreatureSpawnEvent,
        creeper_ignite::CreeperIgniteEvent,
        creeper_power::CreeperPowerEvent,
        ender_dragon_change_phase::EnderDragonChangePhaseEvent,
        ender_dragon_flame::EnderDragonFlameEvent,
        enderman_attack_player::EndermanAttackPlayerEvent,
        enderman_escape::EndermanEscapeEvent,
        entity_attempt_smash_attack::{EntityAttemptSmashAttackEvent, SmashAttackResult},
        entity_break_door::EntityBreakDoorEvent,
        entity_change_block::EntityChangeBlockEvent,
        entity_combust::EntityCombustEvent,
        entity_combust_by_block::EntityCombustByBlockEvent,
        entity_combust_by_entity::EntityCombustByEntityEvent,
        entity_damage::EntityDamageEvent,
        entity_damage_by_block::EntityDamageByBlockEvent,
        entity_damage_by_entity::EntityDamageByEntityEvent,
        entity_damage_item::EntityDamageItemEvent,
        entity_death::{EntityDeathEvent, PlayerDeathEvent},
        entity_drop_item::EntityDropItemEvent,
        entity_effect_tick::EntityEffectTickEvent,
        entity_enter_block::EntityEnterBlockEvent,
        entity_equipment_changed::EntityEquipmentChangedEvent,
        entity_exhaustion::EntityExhaustionEvent,
        entity_fertilize_egg::EntityFertilizeEggEvent,
        entity_inside_block::EntityInsideBlockEvent,
        entity_interact::EntityInteractEvent,
        entity_jump::EntityJumpEvent,
        entity_knockback::EntityKnockbackEvent,
        entity_knockback_by_entity::EntityKnockbackByEntityEvent,
        entity_load_crossbow::EntityLoadCrossbowEvent,
        entity_move::EntityMoveEvent,
        entity_pathfind::EntityPathfindEvent,
        entity_place::EntityPlaceEvent,
        entity_portal_enter::EntityPortalEnterEvent,
        entity_portal_exit::EntityPortalExitEvent,
        entity_pose_change::EntityPoseChangeEvent,
        entity_potion_effect::EntityPotionEffectEvent,
        entity_regain_health::EntityRegainHealthEvent,
        entity_remove::EntityRemoveEvent,
        entity_spawn::EntitySpawnEvent,
        entity_spell_cast::EntitySpellCastEvent,
        entity_target_block::EntityTargetBlockEvent,
        entity_target_living_entity::EntityTargetLivingEntityEvent,
        entity_teleport_end_gateway::EntityTeleportEndGatewayEvent,
        entity_toggle_sit::EntityToggleSitEvent,
        entity_toggle_swim::EntityToggleSwimEvent,
        entity_unleash::EntityUnleashEvent,
        entity_zap::EntityZapEvent,
        exp_bottle::ExpBottleEvent,
        explosion_prime::ExplosionPrimeEvent,
        firework_explode::FireworkExplodeEvent,
        food_level_change::FoodLevelChangeEvent,
        horse_jump::HorseJumpEvent,
        item_despawn::ItemDespawnEvent,
        item_merge::ItemMergeEvent,
        item_spawn::ItemSpawnEvent,
        item_transporting_entity_validate_target::ItemTransportingEntityValidateTargetEvent,
        lingering_potion_splash::LingeringPotionSplashEvent,
        pig_zap::PigZapEvent,
        pig_zombie_anger::PigZombieAngerEvent,
        piglin_barter::PiglinBarterEvent,
        potion_splash::PotionSplashEvent,
        pre_creature_spawn::PreCreatureSpawnEvent,
        pre_spawner_spawn::PreSpawnerSpawnEvent,
        projectile_hit::ProjectileHitEvent,
        projectile_launch::ProjectileLaunchEvent,
        sheep_dye_wool::SheepDyeWoolEvent,
        sheep_regrow_wool::SheepRegrowWoolEvent,
        slime_split::SlimeSplitEvent,
        spawner_spawn::SpawnerSpawnEvent,
        strider_temperature_change::StriderTemperatureChangeEvent,
        tameable_death_message::TameableDeathMessageEvent,
        thrown_egg_hatch::ThrownEggHatchEvent,
        trial_spawner_spawn::TrialSpawnerSpawnEvent,
        villager_acquire_trade::VillagerAcquireTradeEvent,
        villager_career_change::VillagerCareerChangeEvent,
        villager_replenish_trade::VillagerReplenishTradeEvent,
        villager_reputation_change::VillagerReputationChangeEvent,
        warden_anger_change::WardenAngerChangeEvent,
        water_bottle_splash::WaterBottleSplashEvent,
        witch_consume_potion::WitchConsumePotionEvent,
        witch_ready_potion::WitchReadyPotionEvent,
        witch_throw_potion::WitchThrowPotionEvent,
    },
    loader::wasm::wasm_host::{
        state::{ItemStackResource, PluginHostState},
        wit::v0_1::{
            events::{
                ToFromWasmEvent, cleanup_event, consume_player, consume_text_component,
                consume_world, from_wasm_block_position, from_wasm_entity_type, from_wasm_position,
                to_wasm_block_position, to_wasm_entity_type, to_wasm_position,
            },
            living_entity::{from_wit_damage_type, to_wit_damage_type},
            pumpkin::plugin::event::{
                AreaEffectCloudApplyEventData, ArrowBodyCountChangeEventData,
                BatToggleSleepEventData, CreatureSpawnEventData, CreeperIgniteEventData,
                CreeperPowerEventData, EnderDragonChangePhaseEventData, EnderDragonFlameEventData,
                EndermanAttackPlayerEventData, EndermanEscapeEventData, EntityAirChangeEventData,
                EntityAttemptSmashAttackEventData, EntityBreakDoorEventData, EntityBreedEventData,
                EntityChangeBlockEventData, EntityCombustByBlockEventData,
                EntityCombustByEntityEventData, EntityCombustEventData,
                EntityDamageByBlockEventData, EntityDamageByEntityEventData, EntityDamageEventData,
                EntityDamageItemEventData, EntityDeathEventData, EntityDismountEventData,
                EntityDropItemEventData, EntityDyeEventData, EntityEffectTickEventData,
                EntityEnterBlockEventData, EntityEnterLoveModeEventData,
                EntityEquipmentChangedEventData, EntityExhaustionEventData, EntityExplodeEventData,
                EntityFertilizeEggEventData, EntityInsideBlockEventData, EntityInteractEventData,
                EntityJumpEventData, EntityKnockbackByEntityEventData, EntityKnockbackEventData,
                EntityLoadCrossbowEventData, EntityMountEventData, EntityMoveEventData,
                EntityPathfindEventData, EntityPickupItemEventData, EntityPlaceEventData,
                EntityPortalEnterEventData, EntityPortalEventData, EntityPortalExitEventData,
                EntityPoseChangeEventData, EntityPotionEffectEventData,
                EntityRegainHealthEventData, EntityRemoveEventData, EntityResurrectEventData,
                EntityShootBowEventData, EntitySpawnEventData, EntitySpellCastEventData,
                EntityTameEventData, EntityTargetBlockEventData, EntityTargetEventData,
                EntityTargetLivingEntityEventData, EntityTeleportEndGatewayEventData,
                EntityTeleportEventData, EntityToggleGlideEventData, EntityToggleSitEventData,
                EntityToggleSwimEventData, EntityTransformEventData, EntityUnleashEventData,
                EntityZapEventData, Event, ExpBottleEventData, ExplosionPrimeEventData,
                FireworkExplodeEventData, FoodLevelChangeEventData, HorseJumpEventData,
                ItemDespawnEventData, ItemMergeEventData, ItemSpawnEventData,
                ItemTransportingEntityValidateTargetEventData, LingeringPotionSplashEventData,
                PigZapEventData, PigZombieAngerEventData, PiglinBarterEventData,
                PlayerDeathEventData, PotionSplashEventData, PreCreatureSpawnEventData,
                PreSpawnerSpawnEventData, ProjectileHitEventData, ProjectileLaunchEventData,
                SheepDyeWoolEventData, SheepRegrowWoolEventData, SlimeSplitEventData,
                SmashAttackResult as WitSmashAttackResult, SpawnerSpawnEventData,
                StriderTemperatureChangeEventData, TameableDeathMessageEventData,
                ThrownEggHatchEventData, TrialSpawnerSpawnEventData, VillagerAcquireTradeEventData,
                VillagerCareerChangeEventData, VillagerReplenishTradeEventData,
                VillagerReputationChangeEventData, WardenAngerChangeEventData,
                WaterBottleSplashEventData, WitchConsumePotionEventData, WitchReadyPotionEventData,
                WitchThrowPotionEventData,
            },
        },
    },
};

fn consume_item_stack(
    state: &mut PluginHostState,
    item: &Resource<
        crate::plugin::loader::wasm::wasm_host::wit::v0_1::pumpkin::plugin::item_stack::ItemStack,
    >,
) -> pumpkin_data::item_stack::ItemStack {
    let mutex = state
        .resource_table
        .delete::<ItemStackResource>(Resource::new_own(item.rep()))
        .expect("invalid item stack resource handle")
        .provider;
    mutex.try_lock().expect("lock item stack").clone()
}

impl ToFromWasmEvent for EntityDamageEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityDamageEvent(EntityDamageEventData {
            entity_id: self.entity_id,
            damage: self.damage,
            damage_type: to_wit_damage_type(&self.damage_type),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityDamageEvent(data) => Self {
                entity_id: data.entity_id,
                damage: data.damage,
                damage_type: from_wit_damage_type(data.damage_type),
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityDeathEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityDeathEvent(EntityDeathEventData {
            entity_id: self.entity_id,
            dropped_exp: self.dropped_exp,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityDeathEvent(data) => Self {
                entity_id: data.entity_id,
                dropped_exp: data.dropped_exp,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for PlayerDeathEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("failed to add player resource");
        let death_message = state
            .add_text_component(self.death_message.clone())
            .expect("failed to add text component resource");

        Event::PlayerDeathEvent(PlayerDeathEventData {
            player,
            death_message,
            dropped_exp: self.dropped_exp,
            keep_inventory: self.keep_inventory,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PlayerDeathEvent(data) => Self {
                player: consume_player(state, &data.player),
                death_message: consume_text_component(state, &data.death_message),
                dropped_exp: data.dropped_exp,
                keep_inventory: data.keep_inventory,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntitySpawnEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let target_world = state
            .add_world(self.world.clone())
            .expect("failed to add world resource");

        Event::EntitySpawnEvent(EntitySpawnEventData {
            entity_id: self.entity_id,
            entity_type: self.entity_type.clone(),
            position: to_wasm_position(self.position),
            target_world,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::EntitySpawnEvent(data) => {
                let world = consume_world(state, &data.target_world);
                Self {
                    entity_id: data.entity_id,
                    entity_type: data.entity_type,
                    position: pumpkin_util::math::vector3::Vector3::new(
                        data.position.0,
                        data.position.1,
                        data.position.2,
                    ),
                    world,
                    cancelled: data.cancelled,
                }
            }
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityCombustEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityCombustEvent(EntityCombustEventData {
            entity_id: self.entity_id,
            duration_secs: self.duration_secs,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityCombustEvent(data) => Self {
                entity_id: data.entity_id,
                duration_secs: data.duration_secs,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityRegainHealthEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityRegainHealthEvent(EntityRegainHealthEventData {
            entity_id: self.entity_id,
            amount: self.amount,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityRegainHealthEvent(data) => Self {
                entity_id: data.entity_id,
                amount: data.amount,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent
    for crate::plugin::api::events::entity::entity_air_change::EntityAirChangeEvent
{
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityAirChangeEvent(EntityAirChangeEventData {
            entity_id: self.entity_id,
            amount: self.amount,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityAirChangeEvent(data) => Self {
                entity_id: data.entity_id,
                amount: data.amount,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::entity::entity_breed::EntityBreedEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityBreedEvent(EntityBreedEventData {
            father_id: self.father_id,
            mother_id: self.mother_id,
            child_id: self.child_id,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityBreedEvent(data) => Self {
                father_id: data.father_id,
                mother_id: data.mother_id,
                child_id: data.child_id,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::entity::entity_dismount::EntityDismountEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityDismountEvent(EntityDismountEventData {
            entity_id: self.entity_id,
            dismounted_id: self.dismounted_id,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityDismountEvent(data) => Self {
                entity_id: data.entity_id,
                dismounted_id: data.dismounted_id,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::entity::entity_dye::EntityDyeEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = self
            .player
            .as_ref()
            .and_then(|p| state.add_player(p.clone()).ok());
        Event::EntityDyeEvent(EntityDyeEventData {
            entity_id: self.entity_id,
            color: format!("{:?}", self.color),
            player,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityDyeEvent(data) => Self {
                entity_id: data.entity_id,
                color: crate::plugin::api::events::entity::entity_dye::DyeColor::White,
                player: data.player.map(|p| consume_player(state, &p)),
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent
    for crate::plugin::api::events::entity::entity_enter_love_mode::EntityEnterLoveModeEvent
{
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityEnterLoveModeEvent(EntityEnterLoveModeEventData {
            entity_id: self.entity_id,
            human_entity_id: self.human_entity_id,
            ticks_in_love: self.ticks_in_love,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityEnterLoveModeEvent(data) => Self {
                entity_id: data.entity_id,
                human_entity_id: data.human_entity_id,
                ticks_in_love: data.ticks_in_love,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::entity::entity_explode::EntityExplodeEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityExplodeEvent(EntityExplodeEventData {
            entity_id: self.entity_id,
            position: to_wasm_position(self.position),
            yield_rate: self.yield_rate,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityExplodeEvent(data) => Self {
                entity_id: data.entity_id,
                position: pumpkin_util::math::vector3::Vector3::new(
                    data.position.0,
                    data.position.1,
                    data.position.2,
                ),
                yield_rate: data.yield_rate,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::entity::entity_mount::EntityMountEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityMountEvent(EntityMountEventData {
            entity_id: self.entity_id,
            mounted_id: self.mount_id,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityMountEvent(data) => Self {
                entity_id: data.entity_id,
                mount_id: data.mounted_id,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent
    for crate::plugin::api::events::entity::entity_pickup_item::EntityPickupItemEvent
{
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityPickupItemEvent(EntityPickupItemEventData {
            entity_id: self.entity_id,
            item_name: self.item_name.clone(),
            count: self.count,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityPickupItemEvent(data) => Self {
                entity_id: data.entity_id,
                item_name: data.item_name,
                count: data.count,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::entity::entity_portal::EntityPortalEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityPortalEvent(EntityPortalEventData {
            entity_id: self.entity_id,
            portal_pos: to_wasm_block_position(self.portal_pos),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityPortalEvent(data) => Self {
                entity_id: data.entity_id,
                portal_pos: from_wasm_block_position(data.portal_pos),
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent
    for crate::plugin::api::events::entity::entity_resurrect::EntityResurrectEvent
{
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityResurrectEvent(EntityResurrectEventData {
            entity_id: self.entity_id,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityResurrectEvent(data) => Self {
                entity_id: data.entity_id,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::entity::entity_shoot_bow::EntityShootBowEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityShootBowEvent(EntityShootBowEventData {
            entity_id: self.entity_id,
            weapon_name: self.weapon_name.clone(),
            force: self.force,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityShootBowEvent(data) => Self {
                entity_id: data.entity_id,
                weapon_name: data.weapon_name,
                force: data.force,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::entity::entity_tame::EntityTameEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let owner = state
            .add_player(self.owner.clone())
            .expect("failed to add player resource");
        Event::EntityTameEvent(EntityTameEventData {
            entity_id: self.entity_id,
            owner,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityTameEvent(data) => Self {
                entity_id: data.entity_id,
                owner: consume_player(state, &data.owner),
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::entity::entity_target::EntityTargetEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityTargetEvent(EntityTargetEventData {
            entity_id: self.entity_id,
            target_id: self.target_id,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityTargetEvent(data) => Self {
                entity_id: data.entity_id,
                target_id: data.target_id,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for crate::plugin::api::events::entity::entity_teleport::EntityTeleportEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityTeleportEvent(EntityTeleportEventData {
            entity_id: self.entity_id,
            from_position: to_wasm_position(self.from_position),
            to_position: to_wasm_position(self.to_position),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityTeleportEvent(data) => Self {
                entity_id: data.entity_id,
                from_position: pumpkin_util::math::vector3::Vector3::new(
                    data.from_position.0,
                    data.from_position.1,
                    data.from_position.2,
                ),
                to_position: pumpkin_util::math::vector3::Vector3::new(
                    data.to_position.0,
                    data.to_position.1,
                    data.to_position.2,
                ),
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent
    for crate::plugin::api::events::entity::entity_toggle_glide::EntityToggleGlideEvent
{
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityToggleGlideEvent(EntityToggleGlideEventData {
            entity_id: self.entity_id,
            is_gliding: self.is_gliding,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityToggleGlideEvent(data) => Self {
                entity_id: data.entity_id,
                is_gliding: data.is_gliding,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent
    for crate::plugin::api::events::entity::entity_transform::EntityTransformEvent
{
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityTransformEvent(EntityTransformEventData {
            entity_id: self.entity_id,
            new_entity_id: self.new_entity_id,
            transform_reason: self.transform_reason.clone(),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityTransformEvent(data) => Self {
                entity_id: data.entity_id,
                new_entity_id: data.new_entity_id,
                transform_reason: data.transform_reason,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for CreatureSpawnEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let target_world = state
            .add_world(self.world.clone())
            .expect("failed to add world resource");
        Event::CreatureSpawnEvent(CreatureSpawnEventData {
            entity_id: self.entity_id,
            entity_type: self.entity_type.clone(),
            position: to_wasm_position(self.position),
            target_world,
            spawn_reason: self.spawn_reason.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::CreatureSpawnEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::CreatureSpawnEvent(data) => Self {
                entity_id: data.entity_id,
                entity_type: data.entity_type,
                position: from_wasm_position(data.position),
                world: consume_world(state, &data.target_world),
                spawn_reason: data.spawn_reason,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EnderDragonChangePhaseEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EnderDragonChangePhaseEvent(EnderDragonChangePhaseEventData {
            entity_id: self.entity_id,
            current_phase: self.current_phase.clone(),
            new_phase: self.new_phase.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EnderDragonChangePhaseEvent(data) = event {
            self.cancelled = data.cancelled;
            self.new_phase = data.new_phase;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EnderDragonChangePhaseEvent(data) => Self {
                entity_id: data.entity_id,
                current_phase: data.current_phase,
                new_phase: data.new_phase,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityBreakDoorEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityBreakDoorEvent(EntityBreakDoorEventData {
            entity_id: self.entity_id,
            block_pos: to_wasm_block_position(self.block_pos),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntityBreakDoorEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityBreakDoorEvent(data) => Self {
                entity_id: data.entity_id,
                block_pos: from_wasm_block_position(data.block_pos),
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityChangeBlockEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityChangeBlockEvent(EntityChangeBlockEventData {
            entity_id: self.entity_id,
            block_pos: to_wasm_block_position(self.block_pos),
            new_block: self.new_block.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntityChangeBlockEvent(data) = event {
            self.cancelled = data.cancelled;
            self.new_block = data.new_block;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityChangeBlockEvent(data) => Self {
                entity_id: data.entity_id,
                block_pos: from_wasm_block_position(data.block_pos),
                new_block: data.new_block,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityDamageByBlockEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityDamageByBlockEvent(EntityDamageByBlockEventData {
            entity_id: self.entity_id,
            damager_pos: self.damager_pos.map(to_wasm_block_position),
            damage: self.damage,
            cause: self.cause.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntityDamageByBlockEvent(data) = event {
            self.cancelled = data.cancelled;
            self.damage = data.damage;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityDamageByBlockEvent(data) => Self {
                entity_id: data.entity_id,
                damager_pos: data.damager_pos.map(from_wasm_block_position),
                damage: data.damage,
                cause: data.cause,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityDamageByEntityEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityDamageByEntityEvent(EntityDamageByEntityEventData {
            entity_id: self.entity_id,
            damager_id: self.damager_id,
            damage: self.damage,
            cause: self.cause.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntityDamageByEntityEvent(data) = event {
            self.cancelled = data.cancelled;
            self.damage = data.damage;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityDamageByEntityEvent(data) => Self {
                entity_id: data.entity_id,
                damager_id: data.damager_id,
                damage: data.damage,
                cause: data.cause,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityDropItemEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityDropItemEvent(EntityDropItemEventData {
            entity_id: self.entity_id,
            item_name: self.item_name.clone(),
            count: self.count,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntityDropItemEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityDropItemEvent(data) => Self {
                entity_id: data.entity_id,
                item_name: data.item_name,
                count: data.count,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityEnterBlockEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityEnterBlockEvent(EntityEnterBlockEventData {
            entity_id: self.entity_id,
            block_pos: to_wasm_block_position(self.block_pos),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntityEnterBlockEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityEnterBlockEvent(data) => Self {
                entity_id: data.entity_id,
                block_pos: from_wasm_block_position(data.block_pos),
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityExhaustionEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityExhaustionEvent(EntityExhaustionEventData {
            entity_id: self.entity_id,
            exhaustion: self.exhaustion,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntityExhaustionEvent(data) = event {
            self.cancelled = data.cancelled;
            self.exhaustion = data.exhaustion;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityExhaustionEvent(data) => Self {
                entity_id: data.entity_id,
                exhaustion: data.exhaustion,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityInteractEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityInteractEvent(EntityInteractEventData {
            entity_id: self.entity_id,
            block_pos: to_wasm_block_position(self.block_pos),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntityInteractEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityInteractEvent(data) => Self {
                entity_id: data.entity_id,
                block_pos: from_wasm_block_position(data.block_pos),
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityKnockbackEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityKnockbackEvent(EntityKnockbackEventData {
            entity_id: self.entity_id,
            hit_by_id: self.hit_by_id,
            knockback: to_wasm_position(self.knockback),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntityKnockbackEvent(data) = event {
            self.cancelled = data.cancelled;
            self.knockback = from_wasm_position(data.knockback);
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityKnockbackEvent(data) => Self {
                entity_id: data.entity_id,
                hit_by_id: data.hit_by_id,
                knockback: from_wasm_position(data.knockback),
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityPlaceEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityPlaceEvent(EntityPlaceEventData {
            entity_id: self.entity_id,
            block_pos: to_wasm_block_position(self.block_pos),
            block_name: self.block_name.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntityPlaceEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityPlaceEvent(data) => Self {
                entity_id: data.entity_id,
                block_pos: from_wasm_block_position(data.block_pos),
                block_name: data.block_name,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityPoseChangeEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityPoseChangeEvent(EntityPoseChangeEventData {
            entity_id: self.entity_id,
            pose: self.pose.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntityPoseChangeEvent(data) = event {
            self.cancelled = data.cancelled;
            self.pose = data.pose;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityPoseChangeEvent(data) => Self {
                entity_id: data.entity_id,
                pose: data.pose,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityPotionEffectEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityPotionEffectEvent(EntityPotionEffectEventData {
            entity_id: self.entity_id,
            effect_name: self.effect_name.clone(),
            duration: self.duration,
            amplifier: self.amplifier,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntityPotionEffectEvent(data) = event {
            self.cancelled = data.cancelled;
            self.duration = data.duration;
            self.amplifier = data.amplifier;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityPotionEffectEvent(data) => Self {
                entity_id: data.entity_id,
                effect_name: data.effect_name,
                duration: data.duration,
                amplifier: data.amplifier,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntitySpellCastEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntitySpellCastEvent(EntitySpellCastEventData {
            entity_id: self.entity_id,
            spell: self.spell.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntitySpellCastEvent(data) = event {
            self.cancelled = data.cancelled;
            self.spell = data.spell;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntitySpellCastEvent(data) => Self {
                entity_id: data.entity_id,
                spell: data.spell,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityTargetLivingEntityEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityTargetLivingEntityEvent(EntityTargetLivingEntityEventData {
            entity_id: self.entity_id,
            target_id: self.target_id,
            reason: self.reason.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntityTargetLivingEntityEvent(data) = event {
            self.cancelled = data.cancelled;
            self.target_id = data.target_id;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityTargetLivingEntityEvent(data) => Self {
                entity_id: data.entity_id,
                target_id: data.target_id,
                reason: data.reason,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityToggleSwimEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityToggleSwimEvent(EntityToggleSwimEventData {
            entity_id: self.entity_id,
            is_swimming: self.is_swimming,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntityToggleSwimEvent(data) = event {
            self.cancelled = data.cancelled;
            self.is_swimming = data.is_swimming;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityToggleSwimEvent(data) => Self {
                entity_id: data.entity_id,
                is_swimming: data.is_swimming,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for ExplosionPrimeEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::ExplosionPrimeEvent(ExplosionPrimeEventData {
            entity_id: self.entity_id,
            radius: self.radius,
            fire: self.fire,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::ExplosionPrimeEvent(data) = event {
            self.cancelled = data.cancelled;
            self.radius = data.radius;
            self.fire = data.fire;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::ExplosionPrimeEvent(data) => Self {
                entity_id: data.entity_id,
                radius: data.radius,
                fire: data.fire,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for FireworkExplodeEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::FireworkExplodeEvent(FireworkExplodeEventData {
            entity_id: self.entity_id,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::FireworkExplodeEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::FireworkExplodeEvent(data) => Self {
                entity_id: data.entity_id,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for FoodLevelChangeEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::FoodLevelChangeEvent(FoodLevelChangeEventData {
            entity_id: self.entity_id,
            food_level: self.food_level,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::FoodLevelChangeEvent(data) = event {
            self.cancelled = data.cancelled;
            self.food_level = data.food_level;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::FoodLevelChangeEvent(data) => Self {
                entity_id: data.entity_id,
                food_level: data.food_level,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for ItemDespawnEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::ItemDespawnEvent(ItemDespawnEventData {
            entity_id: self.entity_id,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::ItemDespawnEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::ItemDespawnEvent(data) => Self {
                entity_id: data.entity_id,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for ItemMergeEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::ItemMergeEvent(ItemMergeEventData {
            entity_id: self.entity_id,
            target_id: self.target_id,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::ItemMergeEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::ItemMergeEvent(data) => Self {
                entity_id: data.entity_id,
                target_id: data.target_id,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for ItemSpawnEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::ItemSpawnEvent(ItemSpawnEventData {
            entity_id: self.entity_id,
            position: to_wasm_position(self.position),
            item_name: self.item_name.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::ItemSpawnEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::ItemSpawnEvent(data) => Self {
                entity_id: data.entity_id,
                position: from_wasm_position(data.position),
                item_name: data.item_name,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for PiglinBarterEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let input_item = state
            .add_item_stack(Arc::new(Mutex::new(self.input_item.clone())))
            .expect("failed to add item stack resource");
        let outcome = self
            .outcome
            .iter()
            .map(|i| {
                state
                    .add_item_stack(Arc::new(Mutex::new(i.clone())))
                    .expect("failed to add item stack resource")
            })
            .collect();
        Event::PiglinBarterEvent(PiglinBarterEventData {
            entity_id: self.entity_id,
            input_item,
            outcome,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PiglinBarterEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PiglinBarterEvent(_) => panic!("Cannot construct PiglinBarterEvent from WASM"),
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for ProjectileHitEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::ProjectileHitEvent(ProjectileHitEventData {
            entity_id: self.entity_id,
            hit_position: to_wasm_position(self.hit_position),
            hit_entity_id: self.hit_entity_id,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::ProjectileHitEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::ProjectileHitEvent(data) => Self {
                entity_id: data.entity_id,
                hit_position: from_wasm_position(data.hit_position),
                hit_entity_id: data.hit_entity_id,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for ProjectileLaunchEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::ProjectileLaunchEvent(ProjectileLaunchEventData {
            entity_id: self.entity_id,
            shooter_id: self.shooter_id,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::ProjectileLaunchEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::ProjectileLaunchEvent(data) => Self {
                entity_id: data.entity_id,
                shooter_id: data.shooter_id,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for SheepDyeWoolEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::SheepDyeWoolEvent(SheepDyeWoolEventData {
            entity_id: self.entity_id,
            dye_color: self.dye_color,
            player_id: self.player_id,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::SheepDyeWoolEvent(data) = event {
            self.cancelled = data.cancelled;
            self.dye_color = data.dye_color;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::SheepDyeWoolEvent(data) => Self {
                entity_id: data.entity_id,
                dye_color: data.dye_color,
                player_id: data.player_id,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for SheepRegrowWoolEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::SheepRegrowWoolEvent(SheepRegrowWoolEventData {
            entity_id: self.entity_id,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::SheepRegrowWoolEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::SheepRegrowWoolEvent(data) => Self {
                entity_id: data.entity_id,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for SlimeSplitEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::SlimeSplitEvent(SlimeSplitEventData {
            entity_id: self.entity_id,
            count: self.count,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::SlimeSplitEvent(data) = event {
            self.cancelled = data.cancelled;
            self.count = data.count;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::SlimeSplitEvent(data) => Self {
                entity_id: data.entity_id,
                count: data.count,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for StriderTemperatureChangeEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::StriderTemperatureChangeEvent(StriderTemperatureChangeEventData {
            entity_id: self.entity_id,
            is_shivering: self.is_shivering,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::StriderTemperatureChangeEvent(data) = event {
            self.cancelled = data.cancelled;
            self.is_shivering = data.is_shivering;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::StriderTemperatureChangeEvent(data) => Self {
                entity_id: data.entity_id,
                is_shivering: data.is_shivering,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for VillagerAcquireTradeEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::VillagerAcquireTradeEvent(VillagerAcquireTradeEventData {
            entity_id: self.entity_id,
            recipe_index: self.recipe_index,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::VillagerAcquireTradeEvent(data) = event {
            self.cancelled = data.cancelled;
            self.recipe_index = data.recipe_index;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::VillagerAcquireTradeEvent(data) => Self {
                entity_id: data.entity_id,
                recipe_index: data.recipe_index,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for VillagerCareerChangeEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::VillagerCareerChangeEvent(VillagerCareerChangeEventData {
            entity_id: self.entity_id,
            profession: self.profession.clone(),
            reason: self.reason.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::VillagerCareerChangeEvent(data) = event {
            self.cancelled = data.cancelled;
            self.profession = data.profession;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::VillagerCareerChangeEvent(data) => Self {
                entity_id: data.entity_id,
                profession: data.profession,
                reason: data.reason,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for VillagerReplenishTradeEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::VillagerReplenishTradeEvent(VillagerReplenishTradeEventData {
            entity_id: self.entity_id,
            restock_quantity: self.restock_quantity,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::VillagerReplenishTradeEvent(data) = event {
            self.cancelled = data.cancelled;
            self.restock_quantity = data.restock_quantity;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::VillagerReplenishTradeEvent(data) => Self {
                entity_id: data.entity_id,
                restock_quantity: data.restock_quantity,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for WardenAngerChangeEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::WardenAngerChangeEvent(WardenAngerChangeEventData {
            entity_id: self.entity_id,
            target_id: self.target_id,
            old_anger: self.old_anger,
            new_anger: self.new_anger,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::WardenAngerChangeEvent(data) = event {
            self.cancelled = data.cancelled;
            self.new_anger = data.new_anger;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::WardenAngerChangeEvent(data) => Self {
                entity_id: data.entity_id,
                target_id: data.target_id,
                old_anger: data.old_anger,
                new_anger: data.new_anger,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for AreaEffectCloudApplyEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::AreaEffectCloudApplyEvent(AreaEffectCloudApplyEventData {
            entity_id: self.entity_id,
            affected_entities: self.affected_entities.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::AreaEffectCloudApplyEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::AreaEffectCloudApplyEvent(data) => Self {
                entity_id: data.entity_id,
                affected_entities: data.affected_entities,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for ArrowBodyCountChangeEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::ArrowBodyCountChangeEvent(ArrowBodyCountChangeEventData {
            entity_id: self.entity_id,
            old_amount: self.old_amount,
            new_amount: self.new_amount,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::ArrowBodyCountChangeEvent(data) = event {
            self.cancelled = data.cancelled;
            self.new_amount = data.new_amount;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::ArrowBodyCountChangeEvent(data) => Self {
                entity_id: data.entity_id,
                old_amount: data.old_amount,
                new_amount: data.new_amount,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for BatToggleSleepEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::BatToggleSleepEvent(BatToggleSleepEventData {
            entity_id: self.entity_id,
            is_awake: self.is_awake,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::BatToggleSleepEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::BatToggleSleepEvent(data) => Self {
                entity_id: data.entity_id,
                is_awake: data.is_awake,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for CreeperPowerEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::CreeperPowerEvent(CreeperPowerEventData {
            entity_id: self.entity_id,
            lightning_id: self.lightning_id,
            cause: self.cause.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::CreeperPowerEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::CreeperPowerEvent(data) => Self {
                entity_id: data.entity_id,
                lightning_id: data.lightning_id,
                cause: data.cause,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityCombustByBlockEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityCombustByBlockEvent(EntityCombustByBlockEventData {
            entity_id: self.entity_id,
            combuster: to_wasm_block_position(self.combuster),
            duration: self.duration,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntityCombustByBlockEvent(data) = event {
            self.cancelled = data.cancelled;
            self.duration = data.duration;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityCombustByBlockEvent(data) => Self {
                entity_id: data.entity_id,
                combuster: from_wasm_block_position(data.combuster),
                duration: data.duration,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityCombustByEntityEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityCombustByEntityEvent(EntityCombustByEntityEventData {
            entity_id: self.entity_id,
            combuster_id: self.combuster_id,
            duration: self.duration,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntityCombustByEntityEvent(data) = event {
            self.cancelled = data.cancelled;
            self.duration = data.duration;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityCombustByEntityEvent(data) => Self {
                entity_id: data.entity_id,
                combuster_id: data.combuster_id,
                duration: data.duration,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityKnockbackByEntityEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityKnockbackByEntityEvent(EntityKnockbackByEntityEventData {
            entity_id: self.entity_id,
            hit_by_id: self.hit_by_id,
            force: self.force,
            x: self.x,
            z: self.z,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntityKnockbackByEntityEvent(data) = event {
            self.cancelled = data.cancelled;
            self.force = data.force;
            self.x = data.x;
            self.z = data.z;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityKnockbackByEntityEvent(data) => Self {
                entity_id: data.entity_id,
                hit_by_id: data.hit_by_id,
                force: data.force,
                x: data.x,
                z: data.z,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityPortalEnterEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityPortalEnterEvent(EntityPortalEnterEventData {
            entity_id: self.entity_id,
            location: to_wasm_block_position(self.location),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntityPortalEnterEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityPortalEnterEvent(data) => Self {
                entity_id: data.entity_id,
                location: from_wasm_block_position(data.location),
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityPortalExitEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityPortalExitEvent(EntityPortalExitEventData {
            entity_id: self.entity_id,
            from_pos: to_wasm_block_position(self.from_pos),
            to_pos: self.to_pos.map(to_wasm_block_position),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntityPortalExitEvent(data) = event {
            self.cancelled = data.cancelled;
            self.to_pos = data.to_pos.map(from_wasm_block_position);
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityPortalExitEvent(data) => Self {
                entity_id: data.entity_id,
                from_pos: from_wasm_block_position(data.from_pos),
                to_pos: data.to_pos.map(from_wasm_block_position),
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityRemoveEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityRemoveEvent(EntityRemoveEventData {
            entity_id: self.entity_id,
            cause: self.cause.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntityRemoveEvent(data) = event {
            self.cancelled = data.cancelled;
            self.cause = data.cause;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityRemoveEvent(data) => Self {
                entity_id: data.entity_id,
                cause: data.cause,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityTargetBlockEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityTargetBlockEvent(EntityTargetBlockEventData {
            entity_id: self.entity_id,
            block_pos: to_wasm_block_position(self.block_pos),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntityTargetBlockEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityTargetBlockEvent(data) => Self {
                entity_id: data.entity_id,
                block_pos: from_wasm_block_position(data.block_pos),
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityUnleashEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityUnleashEvent(EntityUnleashEventData {
            entity_id: self.entity_id,
            reason: self.reason.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::EntityUnleashEvent(data) = event {
            self.cancelled = data.cancelled;
            self.reason = data.reason;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityUnleashEvent(data) => Self {
                entity_id: data.entity_id,
                reason: data.reason,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for ExpBottleEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::ExpBottleEvent(ExpBottleEventData {
            entity_id: self.entity_id,
            experience: self.experience,
            location: to_wasm_block_position(self.location),
            show_effect: self.show_effect,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::ExpBottleEvent(data) = event {
            self.cancelled = data.cancelled;
            self.experience = data.experience;
            self.show_effect = data.show_effect;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::ExpBottleEvent(data) => Self {
                entity_id: data.entity_id,
                experience: data.experience,
                location: from_wasm_block_position(data.location),
                show_effect: data.show_effect,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for HorseJumpEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::HorseJumpEvent(HorseJumpEventData {
            entity_id: self.entity_id,
            power: self.power,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::HorseJumpEvent(data) = event {
            self.cancelled = data.cancelled;
            self.power = data.power;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::HorseJumpEvent(data) => Self {
                entity_id: data.entity_id,
                power: data.power,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for LingeringPotionSplashEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::LingeringPotionSplashEvent(LingeringPotionSplashEventData {
            entity_id: self.entity_id,
            location: to_wasm_block_position(self.location),
            potion_item: self.potion_item.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::LingeringPotionSplashEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::LingeringPotionSplashEvent(data) => Self {
                entity_id: data.entity_id,
                location: from_wasm_block_position(data.location),
                potion_item: data.potion_item,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for PigZapEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::PigZapEvent(PigZapEventData {
            entity_id: self.entity_id,
            lightning_id: self.lightning_id,
            pig_zombie_id: self.pig_zombie_id,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PigZapEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PigZapEvent(data) => Self {
                entity_id: data.entity_id,
                lightning_id: data.lightning_id,
                pig_zombie_id: data.pig_zombie_id,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for PigZombieAngerEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::PigZombieAngerEvent(PigZombieAngerEventData {
            entity_id: self.entity_id,
            target_id: self.target_id,
            new_anger: self.new_anger,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PigZombieAngerEvent(data) = event {
            self.cancelled = data.cancelled;
            self.new_anger = data.new_anger;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PigZombieAngerEvent(data) => Self {
                entity_id: data.entity_id,
                target_id: data.target_id,
                new_anger: data.new_anger,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for PotionSplashEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::PotionSplashEvent(PotionSplashEventData {
            entity_id: self.entity_id,
            location: to_wasm_block_position(self.location),
            potion_item: self.potion_item.clone(),
            affected_entities: self.affected_entities.clone(),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::PotionSplashEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PotionSplashEvent(data) => Self {
                entity_id: data.entity_id,
                location: from_wasm_block_position(data.location),
                potion_item: data.potion_item,
                affected_entities: data.affected_entities,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for SpawnerSpawnEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::SpawnerSpawnEvent(SpawnerSpawnEventData {
            entity_id: self.entity_id,
            spawner_pos: to_wasm_block_position(self.spawner_pos),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::SpawnerSpawnEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::SpawnerSpawnEvent(data) => Self {
                entity_id: data.entity_id,
                spawner_pos: from_wasm_block_position(data.spawner_pos),
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for TrialSpawnerSpawnEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::TrialSpawnerSpawnEvent(TrialSpawnerSpawnEventData {
            entity_id: self.entity_id,
            spawner_pos: to_wasm_block_position(self.spawner_pos),
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::TrialSpawnerSpawnEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::TrialSpawnerSpawnEvent(data) => Self {
                entity_id: data.entity_id,
                spawner_pos: from_wasm_block_position(data.spawner_pos),
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for VillagerReputationChangeEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::VillagerReputationChangeEvent(VillagerReputationChangeEventData {
            entity_id: self.entity_id,
            target_id: self.target_id,
            reputation_change: self.reputation_change,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::VillagerReputationChangeEvent(data) = event {
            self.cancelled = data.cancelled;
            self.reputation_change = data.reputation_change;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::VillagerReputationChangeEvent(data) => Self {
                entity_id: data.entity_id,
                target_id: data.target_id,
                reputation_change: data.reputation_change,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for WitchConsumePotionEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let potion = state
            .add_item_stack(Arc::new(Mutex::new(self.potion.clone())))
            .expect("failed to add item stack resource");
        Event::WitchConsumePotionEvent(WitchConsumePotionEventData {
            entity_id: self.entity_id,
            potion,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::WitchConsumePotionEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::WitchConsumePotionEvent(_) => {
                panic!("Cannot construct WitchConsumePotionEvent from WASM")
            }
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for WitchReadyPotionEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let potion = state
            .add_item_stack(Arc::new(Mutex::new(self.potion.clone())))
            .expect("failed to add item stack resource");
        Event::WitchReadyPotionEvent(WitchReadyPotionEventData {
            entity_id: self.entity_id,
            potion,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::WitchReadyPotionEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::WitchReadyPotionEvent(_) => {
                panic!("Cannot construct WitchReadyPotionEvent from WASM")
            }
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for WitchThrowPotionEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let potion = state
            .add_item_stack(Arc::new(Mutex::new(self.potion.clone())))
            .expect("failed to add item stack resource");
        Event::WitchThrowPotionEvent(WitchThrowPotionEventData {
            entity_id: self.entity_id,
            potion,
            target_id: self.target_id,
            cancelled: self.cancelled,
        })
    }

    fn apply_wasm_event(&mut self, event: Event, state: &mut PluginHostState) {
        cleanup_event(&event, state);
        if let Event::WitchThrowPotionEvent(data) = event {
            self.cancelled = data.cancelled;
        }
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::WitchThrowPotionEvent(_) => {
                panic!("Cannot construct WitchThrowPotionEvent from WASM")
            }
            _ => panic!("unexpected event type"),
        }
    }
}

const fn to_wasm_smash_attack_result(result: SmashAttackResult) -> WitSmashAttackResult {
    match result {
        SmashAttackResult::Allowed => WitSmashAttackResult::Allowed,
        SmashAttackResult::Denied => WitSmashAttackResult::Denied,
    }
}

const fn from_wasm_smash_attack_result(result: WitSmashAttackResult) -> SmashAttackResult {
    match result {
        WitSmashAttackResult::Allowed => SmashAttackResult::Allowed,
        WitSmashAttackResult::Denied => SmashAttackResult::Denied,
    }
}

impl ToFromWasmEvent for EntityAttemptSmashAttackEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityAttemptSmashAttackEvent(EntityAttemptSmashAttackEventData {
            entity_id: self.entity_id,
            target_id: self.target_id,
            result: to_wasm_smash_attack_result(self.result),
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityAttemptSmashAttackEvent(data) => Self {
                entity_id: data.entity_id,
                target_id: data.target_id,
                result: from_wasm_smash_attack_result(data.result),
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityDamageItemEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let item = state
            .add_item_stack(Arc::new(Mutex::new(self.item.clone())))
            .expect("failed to add item stack resource");
        Event::EntityDamageItemEvent(EntityDamageItemEventData {
            entity_id: self.entity_id,
            item,
            damage: self.damage,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityDamageItemEvent(data) => Self {
                entity_id: data.entity_id,
                item: consume_item_stack(state, &data.item),
                damage: data.damage,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityEffectTickEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityEffectTickEvent(EntityEffectTickEventData {
            entity_id: self.entity_id,
            effect_type: self.effect_type.clone(),
            amplifier: self.amplifier,
            duration: self.duration,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityEffectTickEvent(data) => Self {
                entity_id: data.entity_id,
                effect_type: data.effect_type,
                amplifier: data.amplifier,
                duration: data.duration,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityEquipmentChangedEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let old_item = self
            .old_item
            .as_ref()
            .and_then(|i| state.add_item_stack(Arc::new(Mutex::new(i.clone()))).ok());
        let new_item = self
            .new_item
            .as_ref()
            .and_then(|i| state.add_item_stack(Arc::new(Mutex::new(i.clone()))).ok());
        Event::EntityEquipmentChangedEvent(EntityEquipmentChangedEventData {
            entity_id: self.entity_id,
            slot: self.slot.clone(),
            old_item,
            new_item,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityEquipmentChangedEvent(data) => Self {
                entity_id: data.entity_id,
                slot: data.slot,
                old_item: data.old_item.map(|i| consume_item_stack(state, &i)),
                new_item: data.new_item.map(|i| consume_item_stack(state, &i)),
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityFertilizeEggEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let breeder = self
            .breeder
            .as_ref()
            .and_then(|p| state.add_player(p.clone()).ok());
        Event::EntityFertilizeEggEvent(EntityFertilizeEggEventData {
            entity_id: self.entity_id,
            breeder,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityFertilizeEggEvent(data) => Self {
                entity_id: data.entity_id,
                breeder: data.breeder.map(|p| consume_player(state, &p)),
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityInsideBlockEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityInsideBlockEvent(EntityInsideBlockEventData {
            entity_id: self.entity_id,
            block_pos: to_wasm_block_position(self.block_pos),
            block_name: self.block_name.clone(),
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityInsideBlockEvent(data) => Self {
                entity_id: data.entity_id,
                block_pos: from_wasm_block_position(data.block_pos),
                block_name: data.block_name,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityLoadCrossbowEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let crossbow = state
            .add_item_stack(Arc::new(Mutex::new(self.crossbow.clone())))
            .expect("failed to add item stack resource");
        let projectiles = self
            .projectiles
            .iter()
            .map(|i| {
                state
                    .add_item_stack(Arc::new(Mutex::new(i.clone())))
                    .expect("failed to add item stack resource")
            })
            .collect();
        Event::EntityLoadCrossbowEvent(EntityLoadCrossbowEventData {
            entity_id: self.entity_id,
            crossbow,
            projectiles,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityLoadCrossbowEvent(data) => Self {
                entity_id: data.entity_id,
                crossbow: consume_item_stack(state, &data.crossbow),
                projectiles: data
                    .projectiles
                    .iter()
                    .map(|i| consume_item_stack(state, i))
                    .collect(),
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityMoveEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityMoveEvent(EntityMoveEventData {
            entity_id: self.entity_id,
            from_position: to_wasm_position(self.from_position),
            to_position: to_wasm_position(self.to_position),
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityMoveEvent(data) => Self {
                entity_id: data.entity_id,
                from_position: from_wasm_position(data.from_position),
                to_position: from_wasm_position(data.to_position),
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityToggleSitEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityToggleSitEvent(EntityToggleSitEventData {
            entity_id: self.entity_id,
            sitting: self.sitting,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityToggleSitEvent(data) => Self {
                entity_id: data.entity_id,
                sitting: data.sitting,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for ItemTransportingEntityValidateTargetEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::ItemTransportingEntityValidateTargetEvent(
            ItemTransportingEntityValidateTargetEventData {
                entity_id: self.entity_id,
                target_pos: to_wasm_block_position(self.target_pos),
                cancelled: self.cancelled,
            },
        )
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::ItemTransportingEntityValidateTargetEvent(data) => Self {
                entity_id: data.entity_id,
                target_pos: from_wasm_block_position(data.target_pos),
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for TameableDeathMessageEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let death_message = state
            .add_text_component(self.death_message.clone())
            .expect("failed to add text component resource");
        Event::TameableDeathMessageEvent(TameableDeathMessageEventData {
            entity_id: self.entity_id,
            death_message,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::TameableDeathMessageEvent(data) => Self {
                entity_id: data.entity_id,
                death_message: consume_text_component(state, &data.death_message),
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for WaterBottleSplashEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::WaterBottleSplashEvent(WaterBottleSplashEventData {
            entity_id: self.entity_id,
            affected_entities: self.affected_entities.clone(),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::WaterBottleSplashEvent(data) => Self {
                entity_id: data.entity_id,
                affected_entities: data.affected_entities,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for CreeperIgniteEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::CreeperIgniteEvent(CreeperIgniteEventData {
            entity_id: self.entity_id,
            igniter_id: self.igniter_id,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::CreeperIgniteEvent(data) => Self {
                entity_id: data.entity_id,
                igniter_id: data.igniter_id,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EnderDragonFlameEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EnderDragonFlameEvent(EnderDragonFlameEventData {
            entity_id: self.entity_id,
            area_effect_cloud_id: self.area_effect_cloud_id,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EnderDragonFlameEvent(data) => Self {
                entity_id: data.entity_id,
                area_effect_cloud_id: data.area_effect_cloud_id,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EndermanAttackPlayerEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let player = state
            .add_player(self.player.clone())
            .expect("failed to add player resource");
        Event::EndermanAttackPlayerEvent(EndermanAttackPlayerEventData {
            entity_id: self.entity_id,
            player,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::EndermanAttackPlayerEvent(data) => Self {
                entity_id: data.entity_id,
                player: consume_player(state, &data.player),
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EndermanEscapeEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EndermanEscapeEvent(EndermanEscapeEventData {
            entity_id: self.entity_id,
            reason: self.reason.clone(),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EndermanEscapeEvent(data) => Self {
                entity_id: data.entity_id,
                reason: data.reason,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityJumpEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityJumpEvent(EntityJumpEventData {
            entity_id: self.entity_id,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityJumpEvent(data) => Self {
                entity_id: data.entity_id,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityPathfindEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityPathfindEvent(EntityPathfindEventData {
            entity_id: self.entity_id,
            target_id: self.target_id,
            path: self.path.iter().map(|p| to_wasm_position(*p)).collect(),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityPathfindEvent(data) => Self {
                entity_id: data.entity_id,
                target_id: data.target_id,
                path: data.path.into_iter().map(from_wasm_position).collect(),
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityTeleportEndGatewayEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityTeleportEndGatewayEvent(EntityTeleportEndGatewayEventData {
            entity_id: self.entity_id,
            gateway: to_wasm_block_position(self.gateway),
            from_position: to_wasm_position(self.from_position),
            to_position: to_wasm_position(self.to_position),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityTeleportEndGatewayEvent(data) => Self {
                entity_id: data.entity_id,
                gateway: from_wasm_block_position(data.gateway),
                from_position: from_wasm_position(data.from_position),
                to_position: from_wasm_position(data.to_position),
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for EntityZapEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::EntityZapEvent(EntityZapEventData {
            entity_id: self.entity_id,
            lightning_id: self.lightning_id,
            cause: self.cause.clone(),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::EntityZapEvent(data) => Self {
                entity_id: data.entity_id,
                lightning_id: data.lightning_id,
                cause: data.cause,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for PreCreatureSpawnEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let target_world = state
            .add_world(self.world.clone())
            .expect("failed to add world resource");
        Event::PreCreatureSpawnEvent(PreCreatureSpawnEventData {
            position: to_wasm_position(self.position),
            target_world,
            entity_type: self.entity_type.clone(),
            reason: self.reason.clone(),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::PreCreatureSpawnEvent(data) => Self {
                position: from_wasm_position(data.position),
                world: consume_world(state, &data.target_world),
                entity_type: data.entity_type,
                reason: data.reason,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for PreSpawnerSpawnEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::PreSpawnerSpawnEvent(PreSpawnerSpawnEventData {
            spawner_pos: to_wasm_block_position(self.spawner_pos),
            entity_type: self.entity_type.clone(),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::PreSpawnerSpawnEvent(data) => Self {
                spawner_pos: from_wasm_block_position(data.spawner_pos),
                entity_type: data.entity_type,
                cancelled: data.cancelled,
            },
            _ => panic!("unexpected event type"),
        }
    }
}

impl ToFromWasmEvent for ThrownEggHatchEvent {
    fn to_wasm_event(&self, _state: &mut PluginHostState) -> Event {
        Event::ThrownEggHatchEvent(ThrownEggHatchEventData {
            egg_id: self.egg_id,
            will_hatch: self.will_hatch,
            num_hatches: self.num_hatches,
            hatching_type: to_wasm_entity_type(self.hatching_type),
        })
    }

    fn from_wasm_event(event: Event, _state: &mut PluginHostState) -> Self {
        match event {
            Event::ThrownEggHatchEvent(data) => Self {
                egg_id: data.egg_id,
                will_hatch: data.will_hatch,
                num_hatches: data.num_hatches,
                hatching_type: from_wasm_entity_type(&data.hatching_type),
            },
            _ => panic!("unexpected event type"),
        }
    }
}
