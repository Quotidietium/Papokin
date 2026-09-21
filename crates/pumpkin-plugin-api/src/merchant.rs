//! Merchant trade offer management for villagers and wandering traders.
//!
//! This module lets a plugin inspect and rewrite the trade list of any entity
//! that can trade. Obtain a [`Merchant`] view from an [`Entity`] with
//! [`Merchant::from_entity`] (or the [`EntityMerchantExt`] convenience trait),
//! then query or mutate its offers. Every mutation re-sends the offer list to
//! the player currently trading with the entity, on both editions (Java
//! `ClientboundMerchantOffers` and Bedrock `UpdateTrade`).
//!
//! # Examples
//!
//! ## Replacing a villager's trades
//! ```rust,ignore
//! use pumpkin_plugin_api::{
//!     Entity, ItemStack,
//!     merchant::{EntityMerchantExt, TradeOfferBuilder},
//! };
//!
//! fn make_emerald_shop(entity: &Entity) {
//!     let Some(merchant) = entity.as_merchant() else {
//!         return; // not a villager / wandering trader
//!     };
//!
//!     merchant.set_trade_offers(vec![
//!         TradeOfferBuilder::new(
//!             ItemStack::new("minecraft:emerald", 3),
//!             ItemStack::new("minecraft:diamond", 1),
//!         )
//!         .max_uses(16)
//!         .xp(5)
//!         .build(),
//!     ]);
//! }
//! ```
//!
//! ## Inspecting offers
//! ```rust,ignore
//! use pumpkin_plugin_api::{Entity, merchant::EntityMerchantExt};
//!
//! fn log_offers(entity: &Entity) {
//!     if let Some(merchant) = entity.as_merchant() {
//!         for (index, offer) in merchant.get_trade_offers().iter().enumerate() {
//!             tracing::info!(
//!                 "offer #{index}: {}x {} -> {}x {} (uses {}/{})",
//!                 offer.base_cost_a.get_count(),
//!                 offer.base_cost_a.get_registry_key(),
//!                 offer.output.get_count(),
//!                 offer.output.get_registry_key(),
//!                 offer.uses,
//!                 offer.max_uses,
//!             );
//!         }
//!     }
//! }
//! ```
//!
//! Note that the item stacks inside a returned [`TradeOffer`] are snapshots:
//! editing them does not write back. Apply changes with
//! [`Merchant::set_trade_offers`] or [`Merchant::add_trade_offer`].

pub use crate::wit::pumpkin::plugin::merchant::{Merchant, TradeOffer};

use crate::wit::pumpkin::plugin::item_stack::ItemStack;
use crate::wit::pumpkin::plugin::world::Entity;

/// Extension trait on [`Entity`] for obtaining a [`Merchant`] view.
pub trait EntityMerchantExt {
    /// Returns a [`Merchant`] handle when this entity can trade (a villager or
    /// a wandering trader), `None` otherwise.
    fn as_merchant(&self) -> Option<Merchant>;
}

impl EntityMerchantExt for Entity {
    fn as_merchant(&self) -> Option<Merchant> {
        Merchant::from_entity(self)
    }
}

/// Fluent builder for [`TradeOffer`] with vanilla-like defaults.
///
/// Defaults: no secondary cost, `reward_exp = true`, `uses = 0`,
/// `max_uses = 12`, `xp = 0`, `special_price = 0`, `price_multiplier = 0.05`,
/// `demand = 0`.
#[must_use]
pub struct TradeOfferBuilder {
    base_cost_a: ItemStack,
    output: ItemStack,
    cost_b: Option<ItemStack>,
    reward_exp: bool,
    uses: i32,
    max_uses: i32,
    xp: i32,
    special_price: i32,
    price_multiplier: f32,
    demand: i32,
}

impl TradeOfferBuilder {
    /// Creates a builder for an offer trading `base_cost_a` for `output`.
    pub fn new(base_cost_a: ItemStack, output: ItemStack) -> Self {
        Self {
            base_cost_a,
            output,
            cost_b: None,
            reward_exp: true,
            uses: 0,
            max_uses: 12,
            xp: 0,
            special_price: 0,
            price_multiplier: 0.05,
            demand: 0,
        }
    }

    /// Sets the optional secondary cost stack.
    pub fn cost_b(mut self, cost_b: ItemStack) -> Self {
        self.cost_b = Some(cost_b);
        self
    }

    /// Sets whether completing the trade spawns experience orbs.
    pub fn reward_exp(mut self, reward_exp: bool) -> Self {
        self.reward_exp = reward_exp;
        self
    }

    /// Sets how many times this offer has already been used.
    pub fn uses(mut self, uses: i32) -> Self {
        self.uses = uses;
        self
    }

    /// Sets how many times this offer can be used before it is out of stock.
    pub fn max_uses(mut self, max_uses: i32) -> Self {
        self.max_uses = max_uses;
        self
    }

    /// Sets the merchant experience the trade grants.
    pub fn xp(mut self, xp: i32) -> Self {
        self.xp = xp;
        self
    }

    /// Sets the special price modifier applied to the first cost (e.g.
    /// reputation discounts).
    pub fn special_price(mut self, special_price: i32) -> Self {
        self.special_price = special_price;
        self
    }

    /// Sets the demand price multiplier for the first cost.
    pub fn price_multiplier(mut self, price_multiplier: f32) -> Self {
        self.price_multiplier = price_multiplier;
        self
    }

    /// Sets the demand value driving dynamic price adjustments.
    pub fn demand(mut self, demand: i32) -> Self {
        self.demand = demand;
        self
    }

    /// Builds the trade offer.
    pub fn build(self) -> TradeOffer {
        TradeOffer {
            base_cost_a: self.base_cost_a,
            output: self.output,
            cost_b: self.cost_b,
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

impl From<TradeOfferBuilder> for TradeOffer {
    fn from(builder: TradeOfferBuilder) -> Self {
        builder.build()
    }
}
