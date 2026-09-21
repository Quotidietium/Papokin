use crate::plugin::loader::wasm::wasm_host::state::PluginHostState;
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::pumpkin::plugin::damage_types::{
    CustomDamageType as WitCustomDamageType, DamageEffects as WitDamageEffects,
    DamageScaling as WitDamageScaling, DamageTypeManager as WitDamageTypeManager,
    DeathMessageType as WitDeathMessageType, HostDamageTypeManager,
};
use crate::server::damage_type::CustomDamageTypeDefinition;
use pumpkin_data::damage::{DamageEffects, DamageScaling, DeathMessageType};
use pumpkin_data::damage_ext::CustomDamageType;
use wasmtime::component::Resource;

impl HostDamageTypeManager for PluginHostState {
    async fn register_damage_type(
        &mut self,
        _res: Resource<WitDamageTypeManager>,
        damage_type: WitCustomDamageType,
    ) -> wasmtime::Result<Result<(), String>> {
        let definition = CustomDamageTypeDefinition {
            name: damage_type.name,
            message_id: damage_type.message_id,
            scaling: to_data_scaling(damage_type.scaling),
            exhaustion: damage_type.exhaustion,
            effects: damage_type.effects.map(to_data_effects),
            death_message_type: to_data_death_message_type(damage_type.death_message_type),
        };

        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("Server not available"))?;
        // The manager reports duplicate names, vanilla name collisions and a
        // frozen registry (plugin loading finished) as `Err`; registration
        // never panics.
        Ok(server
            .damage_type_manager
            .register(server, definition)
            .map(|_| ()))
    }

    async fn get_damage_type(
        &mut self,
        _res: Resource<WitDamageTypeManager>,
        name: String,
    ) -> wasmtime::Result<Option<WitCustomDamageType>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("Server not available"))?;
        Ok(server
            .damage_type_manager
            .get(&name)
            .map(|custom| to_wit_custom_damage_type(&custom)))
    }

    async fn has_damage_type(
        &mut self,
        _res: Resource<WitDamageTypeManager>,
        name: String,
    ) -> wasmtime::Result<bool> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("Server not available"))?;
        Ok(server.damage_type_manager.get(&name).is_some())
    }

    async fn get_all_custom_damage_type_names(
        &mut self,
        _res: Resource<WitDamageTypeManager>,
    ) -> wasmtime::Result<Vec<String>> {
        let server = self
            .server
            .as_ref()
            .ok_or_else(|| wasmtime::Error::msg("Server not available"))?;
        Ok(server
            .damage_type_manager
            .all()
            .into_iter()
            .map(|custom| custom.name)
            .collect())
    }

    async fn drop(&mut self, _rep: Resource<WitDamageTypeManager>) -> wasmtime::Result<()> {
        Ok(())
    }
}

fn to_wit_custom_damage_type(custom: &CustomDamageType) -> WitCustomDamageType {
    WitCustomDamageType {
        name: custom.name.clone(),
        message_id: custom.message_id.clone(),
        scaling: to_wit_scaling(custom.scaling),
        exhaustion: custom.exhaustion,
        effects: custom.effects.map(to_wit_effects),
        death_message_type: to_wit_death_message_type(custom.death_message_type),
    }
}

#[must_use]
pub const fn to_data_scaling(scaling: WitDamageScaling) -> DamageScaling {
    match scaling {
        WitDamageScaling::Never => DamageScaling::Never,
        WitDamageScaling::WhenCausedByLivingNonPlayer => DamageScaling::WhenCausedByLivingNonPlayer,
        WitDamageScaling::Always => DamageScaling::Always,
    }
}

#[must_use]
pub const fn to_wit_scaling(scaling: DamageScaling) -> WitDamageScaling {
    match scaling {
        DamageScaling::Never => WitDamageScaling::Never,
        DamageScaling::WhenCausedByLivingNonPlayer => WitDamageScaling::WhenCausedByLivingNonPlayer,
        DamageScaling::Always => WitDamageScaling::Always,
    }
}

#[must_use]
pub const fn to_data_effects(effects: WitDamageEffects) -> DamageEffects {
    match effects {
        WitDamageEffects::Hurt => DamageEffects::Hurt,
        WitDamageEffects::Thorns => DamageEffects::Thorns,
        WitDamageEffects::Drowning => DamageEffects::Drowning,
        WitDamageEffects::Burning => DamageEffects::Burning,
        WitDamageEffects::Poking => DamageEffects::Poking,
        WitDamageEffects::Freezing => DamageEffects::Freezing,
    }
}

#[must_use]
pub const fn to_wit_effects(effects: DamageEffects) -> WitDamageEffects {
    match effects {
        DamageEffects::Hurt => WitDamageEffects::Hurt,
        DamageEffects::Thorns => WitDamageEffects::Thorns,
        DamageEffects::Drowning => WitDamageEffects::Drowning,
        DamageEffects::Burning => WitDamageEffects::Burning,
        DamageEffects::Poking => WitDamageEffects::Poking,
        DamageEffects::Freezing => WitDamageEffects::Freezing,
    }
}

#[must_use]
pub const fn to_data_death_message_type(
    death_message_type: WitDeathMessageType,
) -> DeathMessageType {
    match death_message_type {
        WitDeathMessageType::Default => DeathMessageType::Default,
        WitDeathMessageType::FallVariants => DeathMessageType::FallVariants,
        WitDeathMessageType::IntentionalGameDesign => DeathMessageType::IntentionalGameDesign,
    }
}

#[must_use]
pub const fn to_wit_death_message_type(
    death_message_type: DeathMessageType,
) -> WitDeathMessageType {
    match death_message_type {
        DeathMessageType::Default => WitDeathMessageType::Default,
        DeathMessageType::FallVariants => WitDeathMessageType::FallVariants,
        DeathMessageType::IntentionalGameDesign => WitDeathMessageType::IntentionalGameDesign,
    }
}
