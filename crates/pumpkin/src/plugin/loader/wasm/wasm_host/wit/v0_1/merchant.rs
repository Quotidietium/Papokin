use std::borrow::Cow;
use std::sync::Arc;

use tokio::sync::Mutex;
use wasmtime::component::Resource;

use pumpkin_protocol::codec::item_stack_seralizer::ItemStackSerializer;
use pumpkin_protocol::java::client::play::MerchantOffer;

use crate::entity::EntityBase;
use crate::entity::passive::villager::VillagerEntity;
use crate::entity::passive::wandering_trader::WanderingTraderEntity;
use crate::plugin::loader::wasm::wasm_host::state::{
    EntityResource, MerchantResource, PluginHostState,
};
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::pumpkin::plugin::merchant::{
    Host, HostMerchant, Merchant as MerchantHandle, TradeOffer as WitTradeOffer,
};
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::pumpkin::plugin::world::Entity;

impl Host for PluginHostState {}

/// A borrowed view of an entity that can trade, used to reach the `offers`
/// list and the re-send pipeline shared by villagers and wandering traders.
enum MerchantRef<'a> {
    Villager(&'a VillagerEntity),
    WanderingTrader(&'a WanderingTraderEntity),
}

impl MerchantRef<'_> {
    fn trade_offers(&self) -> Vec<MerchantOffer> {
        match self {
            Self::Villager(villager) => villager.trade_offers(),
            Self::WanderingTrader(trader) => trader.trade_offers(),
        }
    }

    fn set_trade_offers(&self, offers: Vec<MerchantOffer>) {
        match self {
            Self::Villager(villager) => villager.set_trade_offers(offers),
            Self::WanderingTrader(trader) => trader.set_trade_offers(offers),
        }
    }

    fn add_trade_offer(&self, offer: MerchantOffer) {
        match self {
            Self::Villager(villager) => villager.add_trade_offer(offer),
            Self::WanderingTrader(trader) => trader.add_trade_offer(offer),
        }
    }

    fn remove_trade_offer(&self, index: usize) -> bool {
        match self {
            Self::Villager(villager) => villager.remove_trade_offer(index),
            Self::WanderingTrader(trader) => trader.remove_trade_offer(index),
        }
    }
}

/// Downcasts an entity to the merchant it backs, if it can trade.
fn merchant_ref(base: &dyn EntityBase) -> Option<MerchantRef<'_>> {
    if let Some(villager) = base.cast_any().downcast_ref::<VillagerEntity>() {
        return Some(MerchantRef::Villager(villager));
    }
    if let Some(trader) = base.cast_any().downcast_ref::<WanderingTraderEntity>() {
        return Some(MerchantRef::WanderingTrader(trader));
    }
    None
}

fn merchant_from_resource(
    state: &PluginHostState,
    merchant: &Resource<MerchantHandle>,
) -> wasmtime::Result<Arc<dyn EntityBase>> {
    state
        .get_merchant_res(merchant)
        .map(|resource| resource.provider.clone())
        .map_err(|_| wasmtime::Error::msg("invalid merchant resource handle"))
}

fn require_merchant(base: &dyn EntityBase) -> wasmtime::Result<MerchantRef<'_>> {
    merchant_ref(base)
        .ok_or_else(|| wasmtime::Error::msg("entity is not a merchant (villager/wandering trader)"))
}

/// The owned parts of a [`WitTradeOffer`]: the item stack handles resolved to
/// their shared stacks, plus the copied scalar fields.
///
/// Kept fully owned so nothing borrowed from the host state is held across an
/// `.await` (the state is not `Sync`).
struct OfferParts {
    base_cost_a: Arc<Mutex<pumpkin_data::item_stack::ItemStack>>,
    output: Arc<Mutex<pumpkin_data::item_stack::ItemStack>>,
    cost_b: Option<Arc<Mutex<pumpkin_data::item_stack::ItemStack>>>,
    reward_exp: bool,
    uses: i32,
    max_uses: i32,
    xp: i32,
    special_price: i32,
    price_multiplier: f32,
    demand: i32,
}

impl OfferParts {
    fn resolve(state: &PluginHostState, offer: &WitTradeOffer) -> wasmtime::Result<Self> {
        Ok(Self {
            base_cost_a: state.get_item_stack(&offer.base_cost_a)?,
            output: state.get_item_stack(&offer.output)?,
            cost_b: offer
                .cost_b
                .as_ref()
                .map(|cost| state.get_item_stack(cost))
                .transpose()?,
            reward_exp: offer.reward_exp,
            uses: offer.uses,
            max_uses: offer.max_uses,
            xp: offer.xp,
            special_price: offer.special_price,
            price_multiplier: offer.price_multiplier,
            demand: offer.demand,
        })
    }

    async fn into_merchant_offer(self) -> MerchantOffer {
        let cost_b = match self.cost_b {
            Some(stack) => Some(ItemStackSerializer(Cow::Owned(stack.lock().await.clone()))),
            None => None,
        };
        MerchantOffer {
            base_cost_a: ItemStackSerializer(Cow::Owned(self.base_cost_a.lock().await.clone())),
            output: ItemStackSerializer(Cow::Owned(self.output.lock().await.clone())),
            cost_b,
            reward_exp: self.reward_exp,
            uses: self.uses,
            max_uses: self.max_uses,
            xp: self.xp,
            special_price: self.special_price,
            price_multiplier: self.price_multiplier,
            demand: self.demand,
        }
    }
}

fn offer_to_wit(
    state: &mut PluginHostState,
    offer: &MerchantOffer,
) -> wasmtime::Result<WitTradeOffer> {
    let cost_b = match &offer.cost_b {
        Some(cost) => Some(state.add_item_stack(Arc::new(Mutex::new(cost.0.as_ref().clone())))?),
        None => None,
    };
    Ok(WitTradeOffer {
        base_cost_a: state
            .add_item_stack(Arc::new(Mutex::new(offer.base_cost_a.0.as_ref().clone())))?,
        output: state.add_item_stack(Arc::new(Mutex::new(offer.output.0.as_ref().clone())))?,
        cost_b,
        reward_exp: offer.reward_exp,
        uses: offer.uses,
        max_uses: offer.max_uses,
        xp: offer.xp,
        special_price: offer.special_price,
        price_multiplier: offer.price_multiplier,
        demand: offer.demand,
    })
}

impl HostMerchant for PluginHostState {
    async fn from_entity(
        &mut self,
        entity: Resource<Entity>,
    ) -> wasmtime::Result<Option<Resource<MerchantHandle>>> {
        let entity_res = self
            .resource_table
            .get::<EntityResource>(&Resource::new_borrow(entity.rep()))
            .map_err(|_| wasmtime::Error::msg("invalid entity resource handle"))?;
        if merchant_ref(entity_res.provider.as_ref()).is_some() {
            let merchant = self.add_merchant(entity_res.provider.clone())?;
            Ok(Some(merchant))
        } else {
            Ok(None)
        }
    }

    async fn get_trade_offers(
        &mut self,
        merchant: Resource<MerchantHandle>,
    ) -> wasmtime::Result<Vec<WitTradeOffer>> {
        let provider = merchant_from_resource(self, &merchant)?;
        let offers = require_merchant(provider.as_ref())?.trade_offers();
        offers
            .iter()
            .map(|offer| offer_to_wit(self, offer))
            .collect()
    }

    async fn set_trade_offers(
        &mut self,
        merchant: Resource<MerchantHandle>,
        offers: Vec<WitTradeOffer>,
    ) -> wasmtime::Result<()> {
        let mut parts = Vec::with_capacity(offers.len());
        for offer in &offers {
            parts.push(OfferParts::resolve(self, offer)?);
        }
        let provider = merchant_from_resource(self, &merchant)?;
        let merchant = require_merchant(provider.as_ref())?;
        let mut internal = Vec::with_capacity(parts.len());
        for part in parts {
            internal.push(part.into_merchant_offer().await);
        }
        merchant.set_trade_offers(internal);
        Ok(())
    }

    async fn add_trade_offer(
        &mut self,
        merchant: Resource<MerchantHandle>,
        offer: WitTradeOffer,
    ) -> wasmtime::Result<()> {
        let parts = OfferParts::resolve(self, &offer)?;
        let provider = merchant_from_resource(self, &merchant)?;
        let merchant = require_merchant(provider.as_ref())?;
        merchant.add_trade_offer(parts.into_merchant_offer().await);
        Ok(())
    }

    async fn remove_trade_offer(
        &mut self,
        merchant: Resource<MerchantHandle>,
        index: i32,
    ) -> wasmtime::Result<bool> {
        let provider = merchant_from_resource(self, &merchant)?;
        let merchant = require_merchant(provider.as_ref())?;
        let Ok(index) = usize::try_from(index) else {
            return Ok(false);
        };
        Ok(merchant.remove_trade_offer(index))
    }

    async fn drop(&mut self, rep: Resource<MerchantHandle>) -> wasmtime::Result<()> {
        let _ = self
            .resource_table
            .delete::<MerchantResource>(Resource::new_own(rep.rep()));
        Ok(())
    }
}
