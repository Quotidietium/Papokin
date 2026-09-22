//! 村民与流浪商人的交易报价管理。
//!
//! 本模块让插件查看并重写任何
//! 可交易实体的交易列表。从 [`Entity`] 获取 [`Merchant`] 视图：
//! [`Merchant::from_entity`]（或 [`EntityMerchantExt`] 便捷 trait），
//! 然后查询或修改其报价。每次修改都会向
//! 正在与该实体交易的玩家重发报价列表
//! （`ClientboundMerchantOffers`）。
//!
//! # Examples
//!
//! ## Replacing a villager's trades
//! ```rust,ignore
//! use papokin_plugin_api::{
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
//! use papokin_plugin_api::{Entity, merchant::EntityMerchantExt};
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
//! 注意：返回的 [`TradeOffer`] 中的物品堆是快照：
//! 编辑它们不会写回。应用更改请使用
//! [`Merchant::set_trade_offers`] 或 [`Merchant::add_trade_offer`]。

pub use crate::wit::papokin::plugin::merchant::{Merchant, TradeOffer};

use crate::wit::papokin::plugin::item_stack::ItemStack;
use crate::wit::papokin::plugin::world::Entity;

/// 在 [`Entity`] 上获取 [`Merchant`] 视图的扩展 trait。
pub trait EntityMerchantExt {
    ///当该实体可以交易（村民或流浪商人）时，返回 [`Merchant`] 句柄
    /// 流浪商人），否则为 `None`。
    fn as_merchant(&self) -> Option<Merchant>;
}

impl EntityMerchantExt for Entity {
    fn as_merchant(&self) -> Option<Merchant> {
        Merchant::from_entity(self)
    }
}

/// 带原版风格默认值的 [`TradeOffer`] 流式构建器。
///
/// 默认值：无第二花费，`reward_exp = true`、`uses = 0`、
/// `max_uses = 12`、`xp = 0`、`special_price = 0`、`price_multiplier = 0.05`、
/// `demand = 0`。
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
    /// 创建一个以 `base_cost_a` 换 `output` 的报价构建器。
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

    /// 设置可选的次要费用物品堆。
    pub fn cost_b(mut self, cost_b: ItemStack) -> Self {
        self.cost_b = Some(cost_b);
        self
    }

    /// 设置完成交易时是否生成经验球。
    pub fn reward_exp(mut self, reward_exp: bool) -> Self {
        self.reward_exp = reward_exp;
        self
    }

    /// 设置此交易已被使用的次数。
    pub fn uses(mut self, uses: i32) -> Self {
        self.uses = uses;
        self
    }

    /// 设置此交易在售罄前可使用的次数。
    pub fn max_uses(mut self, max_uses: i32) -> Self {
        self.max_uses = max_uses;
        self
    }

    /// 设置该交易给予的商人经验。
    pub fn xp(mut self, xp: i32) -> Self {
        self.xp = xp;
        self
    }

    /// 设置应用于第一项费用的特殊价格修饰值（例如
    /// 声望折扣）。
    pub fn special_price(mut self, special_price: i32) -> Self {
        self.special_price = special_price;
        self
    }

    /// 设置第一项费用的需求价格倍率。
    pub fn price_multiplier(mut self, price_multiplier: f32) -> Self {
        self.price_multiplier = price_multiplier;
        self
    }

    /// 设置驱动动态价格调整的需求值。
    pub fn demand(mut self, demand: i32) -> Self {
        self.demand = demand;
        self
    }

    /// 构建交易报价。
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
