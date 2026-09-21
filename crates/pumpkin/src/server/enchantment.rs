use crate::server::Server;
use pumpkin_data::Enchantment;
use pumpkin_data::data_component_impl::{
    CustomEnchantmentDefinition, custom_enchantment_by_name, custom_enchantment_names,
    intern_custom_enchantment,
};
use pumpkin_data::enchantment::{AttributeModifierSlot, Cost};
use pumpkin_data::item::Item;
use pumpkin_data::tag::{RegistryKey, Tag, get_tag_values};
use pumpkin_nbt::Nbt;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;
use pumpkin_util::text::TextComponent;
use pumpkin_util::version::JavaMinecraftVersion;
use rustc_hash::FxHashMap;
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
pub struct CustomEnchantmentEntry {
    pub id: String,
    pub description: TextComponent,
    pub max_level: u32,
    pub anvil_cost: u32,
    pub supported_items: String,
    pub weight: u32,
    pub slots: Vec<AttributeModifierSlot>,
    pub exclusive_set: Vec<String>,
}

/// Consistent snapshot of the registered custom enchantments: lookup by name
/// plus the registration order, which fixes the network ids (vanilla count +
/// registration index) assigned by the intern table, the registry sync and
/// the protocol-side id bridge alike.
struct CustomEnchantmentState {
    by_name: FxHashMap<String, CustomEnchantmentEntry>,
    order: Vec<String>,
}

pub struct EnchantmentManager {
    custom_enchantments: RwLock<CustomEnchantmentState>,
}

/// Placeholder cost curve for custom enchantments; only relevant for the
/// enchanting table, which never offers custom entries.
const MIN_COST: Cost = Cost {
    base: 1,
    per_level_above_first: 10,
};
const MAX_COST: Cost = Cost {
    base: 41,
    per_level_above_first: 10,
};

impl Default for EnchantmentManager {
    fn default() -> Self {
        Self::new()
    }
}

impl EnchantmentManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            custom_enchantments: RwLock::new(CustomEnchantmentState {
                by_name: FxHashMap::default(),
                order: Vec::new(),
            }),
        }
    }

    /// Registers a custom enchantment end to end: syncs it to clients through
    /// `server.registry_manager` (appended after the vanilla entries of the
    /// `enchantment` registry), interns it as a `&'static Enchantment` so item
    /// components can carry it, and refreshes the protocol-side id bridge.
    ///
    /// The manager's write lock is held across all three inserts so
    /// concurrent registrations cannot interleave and skew the id spaces.
    ///
    /// # Errors
    /// Duplicate names, vanilla name collisions, a frozen registry (plugin
    /// loading finished), an exhausted id space, or unresolvable
    /// `supported_items`/`exclusive_set` references are reported as `Err`.
    pub async fn register(
        &self,
        server: &Server,
        enchantment: CustomEnchantmentEntry,
    ) -> Result<(), String> {
        let mut state = self.custom_enchantments.write().await;
        if state.by_name.contains_key(&enchantment.id) {
            return Err(format!(
                "Enchantment '{}' is already registered",
                enchantment.id
            ));
        }
        if Enchantment::from_name(&enchantment.id).is_some() {
            return Err(format!(
                "Enchantment '{}' collides with a vanilla enchantment",
                enchantment.id
            ));
        }
        if server.registry_manager.is_frozen() {
            return Err(format!(
                "Cannot register enchantment '{}': the registry is frozen",
                enchantment.id
            ));
        }
        if Enchantment::ALL.len() + state.order.len() >= 256 {
            return Err("Custom enchantment id space (u8) is exhausted".to_string());
        }

        // Fallible resolutions happen before any mutation, so a failure here
        // leaves all three id spaces untouched.
        let supported_items = resolve_supported_items(&enchantment.supported_items)?;
        let exclusive_set = resolve_exclusive_set(&enchantment.exclusive_set)?;
        let nbt = custom_enchantment_nbt(&enchantment);

        server
            .registry_manager
            .register("enchantment", enchantment.id.clone(), nbt)?;

        let (namespace, path) = enchantment
            .id
            .split_once(':')
            .unwrap_or(("minecraft", enchantment.id.as_str()));
        intern_custom_enchantment(CustomEnchantmentDefinition {
            name: enchantment.id.clone(),
            description: format!("enchantment.{namespace}.{path}"),
            max_level: i32::try_from(enchantment.max_level).unwrap_or(i32::MAX),
            anvil_cost: enchantment.anvil_cost,
            supported_items,
            weight: i32::try_from(enchantment.weight).unwrap_or(i32::MAX),
            slots: enchantment.slots.clone(),
            exclusive_set,
            min_cost: MIN_COST,
            max_cost: MAX_COST,
        })?;

        state.order.push(enchantment.id.clone());
        pumpkin_protocol::codec::data_component::set_custom_ids(
            "enchantment",
            custom_enchantment_names(),
        );
        state.by_name.insert(enchantment.id.clone(), enchantment);
        Ok(())
    }

    pub async fn get(&self, id: &str) -> Option<CustomEnchantmentEntry> {
        let state = self.custom_enchantments.read().await;
        state.by_name.get(id).cloned()
    }

    pub async fn has(&self, id: &str) -> bool {
        let state = self.custom_enchantments.read().await;
        state.by_name.contains_key(id)
    }

    /// All registered custom enchantment ids, in registration (network id)
    /// order.
    pub async fn get_all_ids(&self) -> Vec<String> {
        let state = self.custom_enchantments.read().await;
        state.order.clone()
    }
}

/// Vanilla name of an attribute modifier slot, as written in the `slots` list
/// of an enchantment registry entry.
const fn slot_name(slot: &AttributeModifierSlot) -> &'static str {
    match slot {
        AttributeModifierSlot::Any => "any",
        AttributeModifierSlot::MainHand => "mainhand",
        AttributeModifierSlot::OffHand => "offhand",
        AttributeModifierSlot::Hand => "hand",
        AttributeModifierSlot::Feet => "feet",
        AttributeModifierSlot::Legs => "legs",
        AttributeModifierSlot::Chest => "chest",
        AttributeModifierSlot::Head => "head",
        AttributeModifierSlot::Armor => "armor",
        AttributeModifierSlot::Body => "body",
        AttributeModifierSlot::Saddle => "saddle",
    }
}

/// Resolves the WIT `supported_items` string to a static item [`Tag`]:
/// `"#minecraft:enchantable/sword"` resolves the tag's members, a plain item
/// id (e.g. `"minecraft:sword"`) becomes a single-item tag.
fn resolve_supported_items(spec: &str) -> Result<&'static Tag, String> {
    if let Some(tag_name) = spec.strip_prefix('#') {
        let names = get_tag_values(RegistryKey::Item, tag_name)
            .ok_or_else(|| format!("Unknown item tag '{spec}'"))?;
        let mut ids = Vec::with_capacity(names.len());
        for name in names {
            let item = Item::from_registry_key(name)
                .ok_or_else(|| format!("Unknown item '{name}' in tag '{spec}'"))?;
            ids.push(item.id);
        }
        let ids: &'static [u16] = Box::leak(ids.into_boxed_slice());
        Ok(Box::leak(Box::new((names, ids))))
    } else {
        let item = Item::from_registry_key(spec)
            .ok_or_else(|| format!("Unknown supported_items item '{spec}'"))?;
        let names: &'static [&'static str] =
            Box::leak(vec![&*Box::leak(spec.to_string().into_boxed_str())].into_boxed_slice());
        let ids: &'static [u16] = Box::leak(vec![item.id].into_boxed_slice());
        Ok(Box::leak(Box::new((names, ids))))
    }
}

/// Resolves `exclusive_set` enchantment names (vanilla or already-registered
/// custom) to a static enchantment-id tag. An empty list becomes `None`.
fn resolve_exclusive_set(names: &[String]) -> Result<Option<&'static Tag>, String> {
    if names.is_empty() {
        return Ok(None);
    }
    let mut tag_names = Vec::with_capacity(names.len());
    let mut ids = Vec::with_capacity(names.len());
    for name in names {
        let enchantment = Enchantment::from_name(name)
            .or_else(|| custom_enchantment_by_name(name))
            .ok_or_else(|| format!("Unknown enchantment '{name}' in exclusive_set"))?;
        ids.push(u16::from(enchantment.id));
        tag_names.push(&*Box::leak(name.clone().into_boxed_str()));
    }
    let tag_names: &'static [&'static str] = Box::leak(tag_names.into_boxed_slice());
    let ids: &'static [u16] = Box::leak(ids.into_boxed_slice());
    Ok(Some(Box::leak(Box::new((tag_names, ids)))))
}

/// Builds the network NBT payload synced to clients for a custom enchantment,
/// mirroring the field layout of vanilla entries in the `enchantment`
/// registry: `description` (text component), `supported_items`, `weight`,
/// `max_level`, `min_cost`/`max_cost` (`{base, per_level_above_first}`),
/// `anvil_cost`, `slots` (string list), optional `exclusive_set` and an empty
/// `effects` compound (custom effects are not modelled yet; the client codec
/// defaults every effect list to empty).
fn custom_enchantment_nbt(entry: &CustomEnchantmentEntry) -> Vec<u8> {
    let mut nbt = NbtCompound::new();
    nbt.put(
        "description",
        entry
            .description
            .to_nbt_tag_for_version(&JavaMinecraftVersion::V_26_3),
    );
    nbt.put_string("supported_items", entry.supported_items.clone());
    let mut min_cost = NbtCompound::new();
    min_cost.put_int("base", MIN_COST.base);
    min_cost.put_int("per_level_above_first", MIN_COST.per_level_above_first);
    nbt.put("min_cost", NbtTag::Compound(min_cost));
    let mut max_cost = NbtCompound::new();
    max_cost.put_int("base", MAX_COST.base);
    max_cost.put_int("per_level_above_first", MAX_COST.per_level_above_first);
    nbt.put("max_cost", NbtTag::Compound(max_cost));
    nbt.put_int("weight", i32::try_from(entry.weight).unwrap_or(i32::MAX));
    nbt.put_int(
        "max_level",
        i32::try_from(entry.max_level).unwrap_or(i32::MAX),
    );
    nbt.put_int(
        "anvil_cost",
        i32::try_from(entry.anvil_cost).unwrap_or(i32::MAX),
    );
    let slots = entry
        .slots
        .iter()
        .map(|slot| NbtTag::String(slot_name(slot).into()))
        .collect();
    nbt.put_list("slots", slots);
    if !entry.exclusive_set.is_empty() {
        let exclusive = entry
            .exclusive_set
            .iter()
            .map(|name| NbtTag::String(name.clone().into()))
            .collect();
        nbt.put_list("exclusive_set", exclusive);
    }
    nbt.put("effects", NbtTag::Compound(NbtCompound::new()));
    Nbt::from(nbt).write().to_vec()
}
