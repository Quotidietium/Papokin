use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Weak};
use uuid::Uuid;

use papokin_data::data_component_impl::EquipmentSlot;
use papokin_data::effect::StatusEffect;
use papokin_data::entity::EntityType;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_data::potion::Effect;
use papokin_data::sound::{Sound, SoundCategory};
use papokin_data::statistic::{CustomStatistic, StatisticCategory};
use papokin_data::villager::{
    TRADES_WANDERING_TRADER_BUYING, TRADES_WANDERING_TRADER_COMMON,
    TRADES_WANDERING_TRADER_UNCOMMON, VillagerTrade, VillagerTradeModifier,
};
use papokin_inventory::SimpleInventory;
use papokin_inventory::merchant::merchant_screen_handler::MerchantScreenHandler;
use papokin_inventory::screen_handler::{
    InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler,
};
use papokin_nbt::compound::NbtCompound;
use papokin_protocol::codec::item_stack_seralizer::ItemStackSerializer;
use papokin_protocol::codec::var_int::VarInt;
use papokin_protocol::java::client::play::CMerchantOffers;
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector3::Vector3;
use papokin_util::text::TextComponent;
use rand::RngExt;
use rand::seq::IndexedRandom;

use super::villager::{
    apply_potion, apply_random_dye, apply_random_stew_effect, trigger_trade_advancement,
};
use crate::entity::ageable::{AgeableData, AgeableMob};
use crate::entity::ai::goal::avoid_entity::AvoidEntityGoal;
use crate::entity::ai::goal::escape_danger::EscapeDangerGoal;
use crate::entity::ai::goal::look_at_entity::LookAtEntityGoal;
use crate::entity::ai::goal::swim::SwimGoal;
use crate::entity::ai::goal::trade_with_player::TradeWithPlayerGoal;
use crate::entity::ai::goal::wander_around::WanderAroundGoal;
use crate::entity::ai::goal::{Controls, Goal};
use crate::entity::ai::pathfinder::NavigatorGoal;
use crate::entity::experience_orb::ExperienceOrbEntity;
use crate::entity::mob::{Mob, MobEntity};
use crate::entity::player::Player;
use crate::entity::{Entity, EntityBase};
use crate::world::World;

const DEFAULT_DESPAWN_DELAY: i32 = 0;

fn create_invisibility_potion() -> ItemStack {
    let mut stack = ItemStack::new(1, &Item::POTION);
    apply_potion(&mut stack, "invisibility");
    stack
}

fn add_offers_from_trade_set(
    offers: &mut Vec<papokin_protocol::java::client::play::MerchantOffer>,
    trade_pool: &'static [VillagerTrade],
    amount: usize,
    rng: &mut impl rand::Rng,
) {
    let mut remaining = trade_pool.iter().collect::<Vec<_>>();
    let mut added = 0;
    while added < amount && !remaining.is_empty() {
        let index = rng.random_range(0..remaining.len());
        let trade = remaining.remove(index);

        let base_cost_a = ItemStack::new(trade.wants.count as u8, trade.wants.item);
        let mut output = ItemStack::new(trade.gives.count as u8, trade.gives.item);
        let cost_b = trade
            .wants_b
            .as_ref()
            .map(|b| ItemStack::new(b.count as u8, b.item));

        match trade.modifier {
            VillagerTradeModifier::RandomDyes => apply_random_dye(rng, &mut output),
            VillagerTradeModifier::RandomPotion => {
                if let Some(potion_name) =
                    papokin_data::tag::Potion::MINECRAFT_TRADEABLE.0.choose(rng)
                {
                    apply_potion(&mut output, potion_name);
                }
            }
            VillagerTradeModifier::SuspiciousStew => {
                apply_random_stew_effect(rng, &mut output);
            }
            VillagerTradeModifier::Potion(potion) => apply_potion(&mut output, potion),
            _ => {}
        }

        offers.push(papokin_protocol::java::client::play::MerchantOffer {
            base_cost_a: ItemStackSerializer(Cow::Owned(base_cost_a)),
            output: ItemStackSerializer(Cow::Owned(output)),
            cost_b: cost_b.map(|stack| ItemStackSerializer(Cow::Owned(stack))),
            reward_exp: true,
            uses: 0,
            max_uses: trade.max_uses,
            xp: trade.xp,
            special_price: 0,
            price_multiplier: trade.price_multiplier,
            demand: 0,
        });
        added += 1;
    }
}

pub struct WanderingTraderEntity {
    pub mob_entity: MobEntity,
    pub despawn_delay: AtomicI32,
    pub wander_target: std::sync::Mutex<Option<BlockPos>>,
    pub offers: std::sync::Mutex<Vec<papokin_protocol::java::client::play::MerchantOffer>>,
    pub merchant_inventory: Arc<SimpleInventory>,
    pub trading_player: std::sync::Mutex<Option<(Uuid, u8)>>,
    pub is_trading: AtomicBool,
    pub trade_sound_cooldown: AtomicI32,
    pub ambient_sound_timer: AtomicI32,
    pub ageable_data: AgeableData,
    self_weak: std::sync::Mutex<Option<Weak<Self>>>,
}

impl WanderingTraderEntity {
    #[allow(clippy::too_many_lines)]
    #[must_use]
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let trader = Self {
            mob_entity,
            despawn_delay: AtomicI32::new(DEFAULT_DESPAWN_DELAY),
            wander_target: std::sync::Mutex::new(None),
            offers: std::sync::Mutex::new(Vec::new()),
            merchant_inventory: Arc::new(SimpleInventory::new(3)),
            trading_player: std::sync::Mutex::new(None),
            is_trading: AtomicBool::new(false),
            trade_sound_cooldown: AtomicI32::new(0),
            ambient_sound_timer: AtomicI32::new(120),
            ageable_data: AgeableData::default(),
            self_weak: std::sync::Mutex::new(None),
        };
        let mob_arc = Arc::new(trader);
        *mob_arc
            .self_weak
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::downgrade(&mob_arc));

        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };
        let trader_weak = Arc::downgrade(&mob_arc);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            // 优先级 0：FloatGoal
            goal_selector.add_goal(0, Box::new(SwimGoal::default()));

            // 优先级 0：UseItemGoal（夜晚喝隐身药水 / 白天用奶桶）
            goal_selector.add_goal(
                0,
                Box::new(WanderingTraderUseItemGoal::new(trader_weak.clone())),
            );

            // 优先级 1：TradeWithPlayerGoal
            goal_selector.add_goal(1, Box::new(TradeWithPlayerGoal::new()));

            // 优先级 1：AvoidEntityGoals
            goal_selector.add_goal(
                1,
                Box::new(AvoidEntityGoal::new(&EntityType::ZOMBIE, 8.0, 0.5, 0.5)),
            );
            goal_selector.add_goal(
                1,
                Box::new(AvoidEntityGoal::new(&EntityType::DROWNED, 8.0, 0.5, 0.5)),
            );
            goal_selector.add_goal(
                1,
                Box::new(AvoidEntityGoal::new(&EntityType::HUSK, 8.0, 0.5, 0.5)),
            );
            goal_selector.add_goal(
                1,
                Box::new(AvoidEntityGoal::new(
                    &EntityType::ZOMBIE_VILLAGER,
                    8.0,
                    0.5,
                    0.5,
                )),
            );
            goal_selector.add_goal(
                1,
                Box::new(AvoidEntityGoal::new(&EntityType::EVOKER, 12.0, 0.5, 0.5)),
            );
            goal_selector.add_goal(
                1,
                Box::new(AvoidEntityGoal::new(&EntityType::VINDICATOR, 8.0, 0.5, 0.5)),
            );
            goal_selector.add_goal(
                1,
                Box::new(AvoidEntityGoal::new(&EntityType::VEX, 8.0, 0.5, 0.5)),
            );
            goal_selector.add_goal(
                1,
                Box::new(AvoidEntityGoal::new(&EntityType::PILLAGER, 15.0, 0.5, 0.5)),
            );
            goal_selector.add_goal(
                1,
                Box::new(AvoidEntityGoal::new(
                    &EntityType::ILLUSIONER,
                    12.0,
                    0.5,
                    0.5,
                )),
            );
            goal_selector.add_goal(
                1,
                Box::new(AvoidEntityGoal::new(&EntityType::ZOGLIN, 10.0, 0.5, 0.5)),
            );

            // 优先级 1：PanicGoal（EscapeDangerGoal）
            goal_selector.add_goal(1, EscapeDangerGoal::new(0.5));

            // 优先级 1：LookAtTradingPlayerGoal
            goal_selector.add_goal(1, Box::new(LookAtTradingPlayerGoal::new(8.0)));

            // 优先级 2：WanderToPositionGoal
            goal_selector.add_goal(
                2,
                Box::new(WanderToPositionGoal::new(trader_weak, 2.0, 0.35)),
            );

            // 优先级 4：MoveTowardsRestrictionGoal
            goal_selector.add_goal(4, Box::new(MoveTowardsRestrictionGoal::new(0.35)));

            // 优先级 8：WaterAvoidingRandomStrollGoal（WanderAroundGoal）
            goal_selector.add_goal(8, Box::new(WanderAroundGoal::new(0.35)));

            // 优先级 9：InteractGoal (Player.class, 3.0F, 1.0F)
            goal_selector.add_goal(
                9,
                Box::new(LookAtEntityGoal::new(
                    mob_weak.clone(),
                    &EntityType::PLAYER,
                    3.0,
                    1.0,
                    false,
                )),
            );

            // 优先级 10：LookAtPlayerGoal (Mob.class, 8.0F)
            goal_selector.add_goal(
                10,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 8.0),
            );
        };

        mob_arc
    }

    #[must_use]
    pub fn get_despawn_delay(&self) -> i32 {
        self.despawn_delay.load(Ordering::Relaxed)
    }

    pub fn set_despawn_delay(&self, delay: i32) {
        self.despawn_delay.store(delay, Ordering::Relaxed);
    }

    pub fn get_wander_target(&self) -> Option<BlockPos> {
        *self
            .wander_target
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn set_wander_target(&self, target: Option<BlockPos>) {
        *self
            .wander_target
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = target;
    }

    pub fn generate_trades(&self) {
        let mut offers = self
            .offers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        offers.clear();
        let mut rng = rand::rng();
        add_offers_from_trade_set(&mut offers, TRADES_WANDERING_TRADER_BUYING, 2, &mut rng);
        add_offers_from_trade_set(&mut offers, TRADES_WANDERING_TRADER_UNCOMMON, 2, &mut rng);
        add_offers_from_trade_set(&mut offers, TRADES_WANDERING_TRADER_COMMON, 5, &mut rng);
    }

    pub fn open_trading_screen(&self, player: &Arc<Player>) {
        if let Some(sync_id) = player.open_handled_screen(self, None) {
            let offers = self
                .offers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            Self::send_trade_offers(player, sync_id, &offers);
        }
    }

    fn send_trade_offers(
        player: &Player,
        sync_id: u8,
        offers: &[papokin_protocol::java::client::play::MerchantOffer],
    ) {
        let java = CMerchantOffers::new(
            VarInt(i32::from(sync_id)),
            offers.to_owned(),
            VarInt(1),
            VarInt(0),
            false,
            false,
        );
        player.client.try_send_packet(&java);
    }

    fn resend_offers_to_trading_player(&self) {
        let trading_player = *self
            .trading_player
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some((player_uuid, sync_id)) = trading_player else {
            return;
        };
        let world = self.get_entity().world.load();
        let Some(player) = world.get_player_by_uuid(player_uuid) else {
            return;
        };
        let offers = self
            .offers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();

        let ok = {
            let screen = player
                .current_screen_handler
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            let mut screen = screen
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if screen.sync_id() != sync_id {
                false
            } else if let Some(handler) =
                screen.as_any_mut().downcast_mut::<MerchantScreenHandler>()
            {
                handler.offers.clone_from(&offers);
                handler.update_result_slot();
                true
            } else {
                false
            }
        };
        if !ok {
            return;
        }
        Self::send_trade_offers(&player, sync_id, &offers);
    }

    /// 将当前交易报价重新发送给当前正在交易的玩家
    /// 此流浪商人的交易列表（如有）（Java `CMerchantOffers`）。
    pub fn notify_trading_player_offers_updated(&self) {
        if self
            .trading_player
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
        {
            self.resend_offers_to_trading_player();
        }
    }

    ///返回该流浪商人当前交易报价的快照。
    pub fn trade_offers(&self) -> Vec<papokin_protocol::java::client::play::MerchantOffer> {
        self.offers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// 替换该流浪商人的交易提议，并将其重新发送给
    /// 与其交易的玩家（如有）。
    pub fn set_trade_offers(
        &self,
        new_offers: Vec<papokin_protocol::java::client::play::MerchantOffer>,
    ) {
        *self
            .offers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = new_offers;
        self.notify_trading_player_offers_updated();
    }

    /// 追加一个交易报价，并将报价列表重新发送给正在交易的玩家
    /// 玩家（如有）。
    pub fn add_trade_offer(&self, offer: papokin_protocol::java::client::play::MerchantOffer) {
        self.offers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(offer);
        self.notify_trading_player_offers_updated();
    }

    /// 移除 `index` 处的交易提议，并将提议列表重新发送给
    /// 与其交易的玩家。当 `index` 越界时返回 `false`。
    pub fn remove_trade_offer(&self, index: usize) -> bool {
        let mut offers = self
            .offers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if index >= offers.len() {
            return false;
        }
        offers.remove(index);
        drop(offers);
        self.notify_trading_player_offers_updated();
        true
    }

    fn can_continue_trading(
        &self,
        inventory_player: &dyn InventoryPlayer,
        player_uuid: Uuid,
        sync_id: u8,
    ) -> bool {
        let Some(player) = inventory_player.as_any().downcast_ref::<Player>() else {
            return false;
        };
        let entity = self.get_entity();
        let range = player
            .living_entity
            .get_attribute_value(&papokin_data::attributes::Attributes::ENTITY_INTERACTION_RANGE)
            + 4.0;
        entity.is_alive()
            && self.mob_entity.living_entity.health.load() > 0.0
            && self
                .trading_player
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_ref()
                .is_some_and(|(uuid, id)| *uuid == player_uuid && *id == sync_id)
            && entity
                .bounding_box
                .load()
                .squared_magnitude(player.eye_position())
                < range * range
    }

    fn complete_trade(&self, offer_index: usize, world: &Arc<World>, player_uuid: Uuid) {
        let (reward_exp, reward_amount) = {
            let mut offers = self
                .offers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(offer) = offers.get_mut(offer_index) else {
                return;
            };
            offer.uses += 1;
            let reward_xp = rand::rng().random_range(3..=6);
            (offer.reward_exp, reward_xp)
        };

        self.get_entity()
            .play_sound(Sound::EntityWanderingTraderYes);
        self.trade_sound_cooldown.store(20, Ordering::Relaxed);

        if reward_exp {
            let position = self.get_entity().pos.load().add_raw(0.0, 0.5, 0.0);
            ExperienceOrbEntity::spawn(world, position, reward_amount);
        }

        if let Some(player) = world.get_player_by_uuid(player_uuid) {
            trigger_trade_advancement(&player);
        }
    }
}

impl ScreenHandlerFactory for WanderingTraderEntity {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        player_inventory: &Arc<papokin_inventory::player::player_inventory::PlayerInventory>,
        player: &dyn InventoryPlayer,
    ) -> Option<SharedScreenHandler> {
        let self_weak = self
            .self_weak
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()?;
        let server_player = player.as_any().downcast_ref::<Player>();
        let player_uuid =
            server_player.map_or_else(uuid::Uuid::nil, |p| p.get_entity().entity_uuid);

        let offers = self
            .offers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let world = self.get_entity().world.load().clone();

        let mut handler = MerchantScreenHandler::new(
            sync_id,
            player_inventory,
            self.merchant_inventory.clone(),
            offers,
        );

        self.is_trading.store(true, Ordering::Relaxed);
        *self
            .trading_player
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some((player_uuid, sync_id));

        let validity_weak = self_weak.clone();
        handler.validity_check = Some(Box::new(move |inventory_player| {
            validity_weak.upgrade().is_some_and(|trader| {
                trader.can_continue_trading(inventory_player, player_uuid, sync_id)
            })
        }));

        let update_weak = self_weak.clone();
        handler.on_trade_updated = Some(Box::new(move |has_result| {
            let Some(trader) = update_weak.upgrade() else {
                return;
            };
            if trader
                .trade_sound_cooldown
                .compare_exchange(0, 20, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                trader.get_entity().play_sound(if has_result {
                    Sound::EntityWanderingTraderYes
                } else {
                    Sound::EntityWanderingTraderNo
                });
            }
        }));

        let close_weak = self_weak.clone();
        handler.on_close = Some(Box::new(move || {
            if let Some(trader) = close_weak.upgrade() {
                trader.is_trading.store(false, Ordering::Relaxed);
                *trader
                    .trading_player
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
            }
        }));

        let trade_weak = self_weak;
        handler.on_trade = Some(Box::new(move |offer_index| {
            let trade_weak = trade_weak.clone();
            let world = world.clone();
            if let Some(trader) = trade_weak.upgrade() {
                trader.complete_trade(offer_index, &world, player_uuid);
            }
        }));

        Some(Arc::new(std::sync::Mutex::new(handler)) as SharedScreenHandler)
    }

    fn get_display_name(&self) -> TextComponent {
        TextComponent::translate("entity.minecraft.wandering_trader", [])
    }
}

impl AgeableMob for WanderingTraderEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }

    fn can_be_a_baby(&self) -> bool {
        false
    }
}

impl Mob for WanderingTraderEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn clear_trading_player(&self) {
        *self
            .trading_player
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }

    fn get_trading_player(&self) -> Option<Arc<Player>> {
        let trading = self
            .trading_player
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(|(uuid, _)| *uuid)?;
        self.mob_entity
            .living_entity
            .entity
            .world
            .load()
            .get_player_by_uuid(trading)
    }

    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_int("DespawnDelay", self.despawn_delay.load(Ordering::Relaxed));
        let wander_target = *self
            .wander_target
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(target) = wander_target {
            nbt.put(
                "wander_target",
                papokin_nbt::tag::NbtTag::IntArray(vec![target.0.x, target.0.y, target.0.z]),
            );
        }

        let offers = self
            .offers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !offers.is_empty() {
            let mut recipes = Vec::with_capacity(offers.len());
            for offer in offers.iter() {
                let mut recipe = NbtCompound::new();
                let mut buy = NbtCompound::new();
                offer.base_cost_a.0.write_item_stack(&mut buy);
                recipe.put_compound("buy", buy);

                if let Some(cost_b) = &offer.cost_b {
                    let mut buy_b = NbtCompound::new();
                    cost_b.0.write_item_stack(&mut buy_b);
                    recipe.put_compound("buyB", buy_b);
                }

                let mut sell_item = NbtCompound::new();
                offer.output.0.write_item_stack(&mut sell_item);
                recipe.put_compound("sell", sell_item);

                recipe.put_int("uses", offer.uses);
                recipe.put_int("maxUses", offer.max_uses);
                recipe.put_bool("rewardExp", offer.reward_exp);
                recipe.put_int("xp", offer.xp);
                recipe.put_float("priceMultiplier", offer.price_multiplier);
                recipe.put_int("specialPrice", offer.special_price);
                recipe.put_int("demand", offer.demand);

                recipes.push(papokin_nbt::tag::NbtTag::Compound(recipe));
            }
            let mut offers_compound = NbtCompound::new();
            offers_compound.put("Recipes", papokin_nbt::tag::NbtTag::List(recipes));
            nbt.put_compound("Offers", offers_compound);
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(delay) = nbt.get_int("DespawnDelay") {
            self.despawn_delay.store(delay, Ordering::Relaxed);
        }
        if let Some(target_arr) = nbt.get_int_array("wander_target")
            && target_arr.len() >= 3
        {
            *self
                .wander_target
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some(BlockPos::new(target_arr[0], target_arr[1], target_arr[2]));
        } else if let (Some(x), Some(y), Some(z)) = (
            nbt.get_int("wander_target_x"),
            nbt.get_int("wander_target_y"),
            nbt.get_int("wander_target_z"),
        ) {
            *self
                .wander_target
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(BlockPos::new(x, y, z));
        }

        if let Some(offers_compound) = nbt.get_compound("Offers")
            && let Some(recipes) = offers_compound.get_list("Recipes")
        {
            let mut offers = self
                .offers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            offers.clear();
            for tag in recipes {
                if let Some(recipe) = tag.extract_compound() {
                    let buy = recipe
                        .get_compound("buy")
                        .and_then(ItemStack::read_item_stack);
                    let buy_b = recipe
                        .get_compound("buyB")
                        .and_then(ItemStack::read_item_stack);
                    let sell_item = recipe
                        .get_compound("sell")
                        .and_then(ItemStack::read_item_stack);

                    if let (Some(buy), Some(sell_item)) = (buy, sell_item)
                        && !buy.is_empty()
                        && !sell_item.is_empty()
                        && buy_b.as_ref().is_none_or(|stack| !stack.is_empty())
                    {
                        let uses = recipe.get_int("uses").unwrap_or(0);
                        let max_uses = recipe.get_int("maxUses").unwrap_or(12);
                        let reward_exp = recipe.get_bool("rewardExp").unwrap_or(true);
                        let xp = recipe.get_int("xp").unwrap_or(2);
                        let price_multiplier = recipe.get_float("priceMultiplier").unwrap_or(0.05);
                        let special_price = recipe.get_int("specialPrice").unwrap_or(0);
                        let demand = recipe.get_int("demand").unwrap_or(0);

                        offers.push(papokin_protocol::java::client::play::MerchantOffer {
                            base_cost_a: buy.into(),
                            output: sell_item.into(),
                            cost_b: buy_b.map(Into::into),
                            reward_exp,
                            uses,
                            max_uses,
                            xp,
                            special_price,
                            price_multiplier,
                            demand,
                        });
                    }
                }
            }
        }

        let current_age = self.get_age();
        if current_age < 0 {
            self.set_age(0);
        }
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        if item_stack.item == &Item::VILLAGER_SPAWN_EGG
            || self.mob_entity.living_entity.health.load() <= 0.0
            || self.is_trading.load(Ordering::Relaxed)
            || self.is_baby()
        {
            return false;
        }

        player.increment_stat(
            StatisticCategory::Custom,
            CustomStatistic::TalkedToVillager as i32,
            1,
        );

        let has_offers = {
            let mut offers = self
                .offers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if offers.is_empty() {
                drop(offers);
                self.generate_trades();
                offers = self
                    .offers
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
            !offers.is_empty()
        };

        if !has_offers {
            return true;
        }

        let trader = self
            .self_weak
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .and_then(Weak::upgrade);
        if let Some(trader) = trader {
            trader.open_trading_screen(player);
        }
        true
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        // 消失延迟处理（原版 aiStep / maybeDespawn）
        if !self.is_trading.load(Ordering::Relaxed) {
            let delay = self.despawn_delay.load(Ordering::Relaxed);
            if delay > 0 {
                let new_delay = delay - 1;
                self.despawn_delay.store(new_delay, Ordering::Relaxed);
                if new_delay == 0 {
                    self.mob_entity.living_entity.entity.remove();
                    return;
                }
            }
        }

        // 交易音效冷却
        let cooldown = self.trade_sound_cooldown.load(Ordering::Relaxed);
        if cooldown > 0 {
            self.trade_sound_cooldown
                .store(cooldown - 1, Ordering::Relaxed);
        }

        // 环境音效处理
        if self.ambient_sound_timer.fetch_sub(1, Ordering::Relaxed) <= 0 {
            let mut rng = rand::rng();
            self.ambient_sound_timer
                .store(rng.random_range(80..=160), Ordering::Relaxed);
            let sound = if self.is_trading.load(Ordering::Relaxed) {
                Sound::EntityWanderingTraderTrade
            } else {
                Sound::EntityWanderingTraderAmbient
            };
            self.mob_entity
                .living_entity
                .entity
                .world
                .load()
                .play_sound(
                    sound,
                    SoundCategory::Neutral,
                    &self.mob_entity.living_entity.entity.pos.load(),
                );
        }
    }
}

// ---------------- AI 目标 ----------------

pub struct LookAtTradingPlayerGoal {
    range: f64,
}

impl LookAtTradingPlayerGoal {
    #[must_use]
    pub const fn new(range: f64) -> Self {
        Self { range }
    }
}

impl Goal for LookAtTradingPlayerGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(player) = mob.get_trading_player() else {
            return false;
        };
        let mob_pos = mob.get_mob_entity().living_entity.entity.pos.load();
        let player_pos = player.get_entity().pos.load();
        mob_pos.squared_distance_to_vec(&player_pos) <= self.range * self.range
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        let Some(player) = mob.get_trading_player() else {
            return false;
        };
        let mob_pos = mob.get_mob_entity().living_entity.entity.pos.load();
        let player_pos = player.get_entity().pos.load();
        mob_pos.squared_distance_to_vec(&player_pos) <= self.range * self.range
    }

    fn tick(&mut self, mob: &dyn Mob) {
        if let Some(player) = mob.get_trading_player() {
            let player_pos = player.get_entity().pos.load();
            mob.get_mob_entity()
                .look_control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .look_at(
                    mob,
                    player_pos.x,
                    player.get_entity().get_eye_y(),
                    player_pos.z,
                );
        }
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::LOOK
    }
}

pub struct WanderToPositionGoal {
    trader: Weak<WanderingTraderEntity>,
    stop_distance: f64,
    speed_modifier: f64,
}

impl WanderToPositionGoal {
    #[must_use]
    pub const fn new(
        trader: Weak<WanderingTraderEntity>,
        stop_distance: f64,
        speed_modifier: f64,
    ) -> Self {
        Self {
            trader,
            stop_distance,
            speed_modifier,
        }
    }

    fn is_too_far_away(pos: &BlockPos, trader_pos: &Vector3<f64>, distance: f64) -> bool {
        let center = Vector3::new(
            pos.0.x as f64 + 0.5,
            pos.0.y as f64 + 0.5,
            pos.0.z as f64 + 0.5,
        );
        center.squared_distance_to_vec(trader_pos) >= distance * distance
    }
}

impl Goal for WanderToPositionGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        let Some(trader) = self.trader.upgrade() else {
            return false;
        };
        let wander_target = *trader
            .wander_target
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(wander_pos) = wander_target else {
            return false;
        };
        let entity_pos = trader.mob_entity.living_entity.entity.pos.load();
        Self::is_too_far_away(&wander_pos, &entity_pos, self.stop_distance)
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        let Some(trader) = self.trader.upgrade() else {
            return false;
        };
        let wander_target = *trader
            .wander_target
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(wander_pos) = wander_target else {
            return false;
        };
        let entity_pos = trader.mob_entity.living_entity.entity.pos.load();
        Self::is_too_far_away(&wander_pos, &entity_pos, self.stop_distance)
    }

    fn stop(&mut self, _mob: &dyn Mob) {
        if let Some(trader) = self.trader.upgrade() {
            *trader
                .wander_target
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
            trader
                .mob_entity
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .stop();
        }
    }

    fn tick(&mut self, _mob: &dyn Mob) {
        let Some(trader) = self.trader.upgrade() else {
            return;
        };
        let wander_target = *trader
            .wander_target
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(wander_pos) = wander_target else {
            return;
        };
        let is_idle = trader
            .mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_idle();
        if is_idle {
            let entity_pos = trader.mob_entity.living_entity.entity.pos.load();
            let center = Vector3::new(
                wander_pos.0.x as f64 + 0.5,
                wander_pos.0.y as f64 + 0.5,
                wander_pos.0.z as f64 + 0.5,
            );
            let target_pos = if Self::is_too_far_away(&wander_pos, &entity_pos, 10.0) {
                let dx = center.x - entity_pos.x;
                let dy = center.y - entity_pos.y;
                let dz = center.z - entity_pos.z;
                let len = (dx * dx + dy * dy + dz * dz).sqrt();
                if len > 0.0 {
                    Vector3::new(
                        entity_pos.x + (dx / len) * 10.0,
                        entity_pos.y + (dy / len) * 10.0,
                        entity_pos.z + (dz / len) * 10.0,
                    )
                } else {
                    center
                }
            } else {
                center
            };
            trader
                .mob_entity
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .set_progress(NavigatorGoal::new(
                    entity_pos,
                    target_pos,
                    self.speed_modifier,
                ));
        }
    }

    fn controls(&self) -> Controls {
        Controls::MOVE
    }
}

pub struct MoveTowardsRestrictionGoal {
    speed: f64,
}

impl MoveTowardsRestrictionGoal {
    #[must_use]
    pub const fn new(speed: f64) -> Self {
        Self { speed }
    }
}

impl Goal for MoveTowardsRestrictionGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let mob_entity = mob.get_mob_entity();
        mob_entity.has_position_target() && !mob_entity.is_in_position_target_range()
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        let mob_entity = mob.get_mob_entity();
        !mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_idle()
            && mob_entity.has_position_target()
            && !mob_entity.is_in_position_target_range()
    }

    fn start(&mut self, mob: &dyn Mob) {
        let mob_entity = mob.get_mob_entity();
        let target = mob_entity.position_target.load();
        let entity_pos = mob_entity.living_entity.entity.pos.load();
        let dest = Vector3::new(
            target.0.x as f64 + 0.5,
            target.0.y as f64,
            target.0.z as f64 + 0.5,
        );
        mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_progress(NavigatorGoal::new(entity_pos, dest, self.speed));
    }

    fn controls(&self) -> Controls {
        Controls::MOVE
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PotionGoalType {
    Invisibility,
    Milk,
}

struct WanderingTraderUseItemGoal {
    trader: Weak<WanderingTraderEntity>,
    goal_type: Option<PotionGoalType>,
    timer: i32,
}

impl WanderingTraderUseItemGoal {
    pub const fn new(trader: Weak<WanderingTraderEntity>) -> Self {
        Self {
            trader,
            goal_type: None,
            timer: 0,
        }
    }
}

impl Goal for WanderingTraderUseItemGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        let Some(trader) = self.trader.upgrade() else {
            return false;
        };
        if !trader.mob_entity.living_entity.entity.is_alive()
            || trader.is_trading.load(Ordering::Relaxed)
        {
            return false;
        }
        let world = trader.mob_entity.living_entity.entity.world.load();
        let is_dark = world.is_dark_outside();
        let is_bright = world.is_bright_outside();
        let is_invisible = trader
            .mob_entity
            .living_entity
            .has_effect(&StatusEffect::INVISIBILITY);

        if is_dark && !is_invisible {
            self.goal_type = Some(PotionGoalType::Invisibility);
            return true;
        }
        if is_bright && is_invisible {
            self.goal_type = Some(PotionGoalType::Milk);
            return true;
        }
        false
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        let Some(trader) = self.trader.upgrade() else {
            return false;
        };
        self.timer > 0
            && trader.mob_entity.living_entity.entity.is_alive()
            && !trader.is_trading.load(Ordering::Relaxed)
    }

    fn start(&mut self, _mob: &dyn Mob) {
        let Some(trader) = self.trader.upgrade() else {
            return;
        };
        self.timer = 32;
        let stack = match self.goal_type {
            Some(PotionGoalType::Invisibility) => create_invisibility_potion(),
            Some(PotionGoalType::Milk) => ItemStack::new(1, &Item::MILK_BUCKET),
            None => return,
        };
        if let Ok(mut equip) = trader.mob_entity.living_entity.entity_equipment.try_lock() {
            equip.put(&EquipmentSlot::MAIN_HAND, stack.clone());
            drop(equip);
            trader
                .mob_entity
                .living_entity
                .send_equipment_changes(&[(EquipmentSlot::MAIN_HAND, stack)]);
        }
    }

    fn tick(&mut self, _mob: &dyn Mob) {
        let Some(trader) = self.trader.upgrade() else {
            return;
        };
        self.timer -= 1;
        if self.timer > 0 && self.timer % 4 == 0 {
            let sound = match self.goal_type {
                Some(PotionGoalType::Invisibility) => Sound::EntityWanderingTraderDrinkPotion,
                Some(PotionGoalType::Milk) => Sound::EntityWanderingTraderDrinkMilk,
                None => return,
            };
            trader
                .mob_entity
                .living_entity
                .entity
                .world
                .load()
                .play_sound(
                    sound,
                    SoundCategory::Neutral,
                    &trader.mob_entity.living_entity.entity.pos.load(),
                );
        }
        if self.timer == 0 {
            match self.goal_type {
                Some(PotionGoalType::Invisibility) => {
                    trader.mob_entity.living_entity.add_effect(Effect {
                        effect_type: &StatusEffect::INVISIBILITY,
                        duration: 6000,
                        amplifier: 0,
                        ambient: false,
                        show_particles: true,
                        show_icon: true,
                        blend: false,
                    });
                    trader
                        .mob_entity
                        .living_entity
                        .entity
                        .world
                        .load()
                        .play_sound(
                            Sound::EntityWanderingTraderDisappeared,
                            SoundCategory::Neutral,
                            &trader.mob_entity.living_entity.entity.pos.load(),
                        );
                }
                Some(PotionGoalType::Milk) => {
                    trader
                        .mob_entity
                        .living_entity
                        .remove_effect(&StatusEffect::INVISIBILITY);
                    trader
                        .mob_entity
                        .living_entity
                        .entity
                        .world
                        .load()
                        .play_sound(
                            Sound::EntityWanderingTraderReappeared,
                            SoundCategory::Neutral,
                            &trader.mob_entity.living_entity.entity.pos.load(),
                        );
                }
                None => {}
            }
        }
    }

    fn stop(&mut self, _mob: &dyn Mob) {
        if let Some(trader) = self.trader.upgrade() {
            let empty = ItemStack::EMPTY;
            if let Ok(mut equip) = trader.mob_entity.living_entity.entity_equipment.try_lock() {
                equip.put(&EquipmentSlot::MAIN_HAND, empty.clone());
                drop(equip);
                trader
                    .mob_entity
                    .living_entity
                    .send_equipment_changes(&[(EquipmentSlot::MAIN_HAND, empty.clone())]);
            }
        }
        self.goal_type = None;
        self.timer = 0;
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn default_wandering_trader_despawn_delay_is_zero() {
        assert_eq!(DEFAULT_DESPAWN_DELAY, 0);
    }

    #[tokio::test]
    async fn generate_trades_creates_expected_trade_count() {
        let mut offers = Vec::new();
        let mut rng = rand::rng();
        add_offers_from_trade_set(&mut offers, TRADES_WANDERING_TRADER_BUYING, 2, &mut rng);
        add_offers_from_trade_set(&mut offers, TRADES_WANDERING_TRADER_UNCOMMON, 2, &mut rng);
        add_offers_from_trade_set(&mut offers, TRADES_WANDERING_TRADER_COMMON, 5, &mut rng);

        assert_eq!(offers.len(), 9);
    }

    #[test]
    fn wander_target_distance_check() {
        let target = BlockPos::new(10, 64, 10);
        let trader_pos = Vector3::new(10.0, 64.0, 10.0);
        // 中心为 (10.5, 64.5, 10.5)，距离约 0.866
        assert!(!WanderToPositionGoal::is_too_far_away(
            &target,
            &trader_pos,
            2.0
        ));
        assert!(WanderToPositionGoal::is_too_far_away(
            &target,
            &trader_pos,
            0.5
        ));
    }

    #[test]
    fn invisibility_potion_item_stack_creation() {
        let potion = create_invisibility_potion();
        assert_eq!(potion.item, &Item::POTION);
        assert_eq!(potion.item_count, 1);
        assert!(!potion.patch.is_empty());
    }
}
