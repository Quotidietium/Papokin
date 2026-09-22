use crate::enchantments::AttributeModifierSlot;
use heck::{ToPascalCase, ToShoutySnakeCase};
use papokin_util::registry::TagType;
use papokin_util::text::TextContent;
use papokin_util::{registry::RegistryEntryList, text::TextComponent};
use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, format_ident, quote};
use serde::Deserialize;
use std::{collections::BTreeMap, fs};
use syn::{Ident, LitBool, LitFloat, LitInt, LitStr};

/// 从 `items.json` 反序列化得到的物品条目。
#[derive(Deserialize)]
pub struct Item {
    /// 此物品的数字协议 ID。
    pub id: u16,
    /// 附加到此物品的所有数据组件。
    pub components: ItemComponents,
}

/// 单个物品在 `items.json` 中存储的全部反序列化数据组件。
#[derive(Deserialize)]
pub struct ItemComponents {
    /// 显示名称翻译组件。
    #[serde(rename = "minecraft:item_name")]
    pub item_name: TextComponent,
    /// 每个物品堆的最大物品数量。
    #[serde(rename = "minecraft:max_stack_size")]
    pub max_stack_size: u8,
    /// 使用冷却组件。
    #[serde(rename = "minecraft:use_cooldown")]
    pub use_cooldown: Option<UseCooldownComponent>,
    /// 如果此物品是音乐唱片，则为唱片机乐曲键，否则为 `None`。
    #[serde(rename = "minecraft:jukebox_playable")]
    pub jukebox_playable: Option<String>,
    /// 可损耗物品的当前损伤值（如果有）。
    #[serde(rename = "minecraft:damage")]
    pub damage: Option<u16>,
    /// 可受损物品的最大耐久度（如有）。
    #[serde(rename = "minecraft:max_damage")]
    pub max_damage: Option<u16>,
    /// 物品被持有或穿戴时应用的属性修饰符（如果有）。
    #[serde(rename = "minecraft:attribute_modifiers")]
    pub attribute_modifiers: Option<Vec<Modifier>>,
    /// 若该物品是工具，则为包含挖掘规则的工具组件。
    #[serde(rename = "minecraft:tool")]
    pub tool: Option<ToolComponent>,
    /// 食物组件，若此物品可食用则存在。
    #[serde(rename = "minecraft:food")]
    pub food: Option<FoodComponent>,
    /// 可装备组件，如果此物品可以穿在盔甲槽位中则存在。
    #[serde(rename = "minecraft:equippable")]
    pub equippable: Option<EquippableComponent>,
    /// 可消耗组件，当此物品具有自定义使用动画或使用时长时存在。
    #[serde(rename = "minecraft:consumable")]
    pub consumable: Option<Consumable>,
    /// 当此物品可格挡攻击（例如盾牌）时存在。
    #[serde(rename = "minecraft:blocks_attacks")]
    pub blocks_attacks: Option<BlocksAttacks>,
    /// 当此物品提供死亡保护（例如不死图腾）时存在。
    #[serde(rename = "minecraft:death_protection")]
    pub death_protection: Option<DeathProtection>,
    /// 伤害类型抗性，用于该物品免疫特定伤害类型的情况。
    #[serde(rename = "minecraft:damage_resistant")]
    pub damage_resistant: Option<DamageResistantComponent>,
    #[serde(rename = "minecraft:weapon")]
    pub weapon: Option<WeaponComponent>,
    #[serde(rename = "minecraft:enchantable")]
    pub enchantable: Option<EnchantableComponent>,
    #[serde(rename = "minecraft:attack_range")]
    pub attack_range: Option<AttackRangeComponent>,
    #[serde(rename = "minecraft:banner_patterns")]
    pub banner_patterns: Option<serde_json::Value>,
    #[serde(rename = "minecraft:bees")]
    pub bees: Option<serde_json::Value>,
    #[serde(rename = "minecraft:block_state")]
    pub block_state: Option<serde_json::Value>,
    #[serde(rename = "minecraft:break_sound")]
    pub break_sound: Option<serde_json::Value>,
    #[serde(rename = "minecraft:brewing_fuel")]
    pub brewing_fuel: Option<serde_json::Value>,
    #[serde(rename = "minecraft:bucket_entity_data")]
    pub bucket_entity_data: Option<serde_json::Value>,
    #[serde(rename = "minecraft:bundle_contents")]
    pub bundle_contents: Option<serde_json::Value>,
    #[serde(rename = "minecraft:charged_projectiles")]
    pub charged_projectiles: Option<serde_json::Value>,
    #[serde(rename = "minecraft:chicken/variant")]
    pub chicken_variant: Option<serde_json::Value>,
    #[serde(rename = "minecraft:container")]
    pub container: Option<serde_json::Value>,
    #[serde(rename = "minecraft:damage_type")]
    pub damage_type: Option<String>,
    #[serde(rename = "minecraft:debug_stick_state")]
    pub debug_stick_state: Option<serde_json::Value>,
    #[serde(rename = "minecraft:dye")]
    pub dye: Option<serde_json::Value>,
    #[serde(rename = "minecraft:enchantment_glint_override")]
    pub enchantment_glint_override: Option<serde_json::Value>,
    #[serde(rename = "minecraft:enchantments")]
    pub enchantments: Option<serde_json::Value>,
    #[serde(rename = "minecraft:entity_data")]
    pub entity_data: Option<serde_json::Value>,
    #[serde(rename = "minecraft:fireworks")]
    pub fireworks: Option<serde_json::Value>,
    #[serde(rename = "minecraft:glider")]
    pub glider: Option<serde_json::Value>,
    #[serde(rename = "minecraft:instrument")]
    pub instrument: Option<serde_json::Value>,
    #[serde(rename = "minecraft:item_model")]
    pub item_model: Option<String>,
    #[serde(rename = "minecraft:kinetic_weapon")]
    pub kinetic_weapon: Option<KineticWeaponComponent>,
    #[serde(rename = "minecraft:lore")]
    pub lore: Option<serde_json::Value>,
    #[serde(rename = "minecraft:map_color")]
    pub map_color: Option<serde_json::Value>,
    #[serde(rename = "minecraft:map_decorations")]
    pub map_decorations: Option<serde_json::Value>,
    #[serde(rename = "minecraft:minimum_attack_charge")]
    pub minimum_attack_charge: Option<f32>,
    #[serde(rename = "minecraft:ominous_bottle_amplifier")]
    pub ominous_bottle_amplifier: Option<i32>,
    #[serde(rename = "minecraft:piercing_weapon")]
    pub piercing_weapon: Option<PiercingWeaponComponent>,
    #[serde(rename = "minecraft:pot_decorations")]
    pub pot_decorations: Option<serde_json::Value>,
    #[serde(rename = "minecraft:potion_contents")]
    pub potion_contents: Option<serde_json::Value>,
    #[serde(rename = "minecraft:potion_duration_scale")]
    pub potion_duration_scale: Option<f32>,
    #[serde(rename = "minecraft:provides_banner_patterns")]
    pub provides_banner_patterns: Option<serde_json::Value>,
    #[serde(rename = "minecraft:provides_trim_material")]
    pub provides_trim_material: Option<serde_json::Value>,
    #[serde(rename = "minecraft:rarity")]
    pub rarity: Option<String>,
    #[serde(rename = "minecraft:recipes")]
    pub recipes: Option<serde_json::Value>,
    #[serde(rename = "minecraft:repair_cost")]
    pub repair_cost: Option<i32>,
    #[serde(rename = "minecraft:repairable")]
    pub repairable: Option<RepairableComponent>,
    #[serde(rename = "minecraft:stored_enchantments")]
    pub stored_enchantments: Option<serde_json::Value>,
    #[serde(rename = "minecraft:suspicious_stew_effects")]
    pub suspicious_stew_effects: Option<serde_json::Value>,
    #[serde(
        rename = "minecraft:attack_animation",
        alias = "minecraft:swing_animation"
    )]
    pub swing_animation: Option<SwingAnimationComponent>,
    #[serde(rename = "minecraft:tooltip_display")]
    pub tooltip_display: Option<serde_json::Value>,
    #[serde(rename = "minecraft:use_effects")]
    pub use_effects: Option<serde_json::Value>,
    #[serde(rename = "minecraft:use_remainder")]
    pub use_remainder: Option<serde_json::Value>,
    #[serde(rename = "minecraft:writable_book_content")]
    pub writable_book_content: Option<serde_json::Value>,
}

fn default_swing_animation_type() -> String {
    "whack".to_string()
}

const fn default_swing_animation_duration() -> i32 {
    6
}

#[derive(Deserialize, Clone)]
pub struct SwingAnimationComponent {
    #[serde(default = "default_swing_animation_type")]
    pub r#type: String,
    #[serde(default = "default_swing_animation_duration")]
    pub duration: i32,
}

#[derive(Deserialize)]
pub struct EnchantableComponent {
    pub value: i32,
}

impl ToTokens for ItemComponents {
    /// 生成一系列 `(DataComponent, &impl DataComponentImpl)` 元组表达式，用于代码生成。
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let max_stack_size = LitInt::new(&self.max_stack_size.to_string(), Span::call_site());
        tokens.extend(quote! {
            (MaxStackSize, &MaxStackSizeImpl {
                size: #max_stack_size,
            }),
        });

        if let Some(use_cooldown) = &self.use_cooldown {
            let seconds = LitFloat::new(&format!("{:.1}", use_cooldown.seconds), Span::call_site());
            let cooldown_group = if let Some(cd_group) = &use_cooldown.cooldown_group {
                quote! { Some(#cd_group) }
            } else {
                quote! { None }
            };

            tokens.extend(quote! {
                (UseCooldown, &UseCooldownImpl {
                    seconds: #seconds,
                    cooldown_group: #cooldown_group,
                }),
            });
        }

        if let Some(playable) = &self.jukebox_playable {
            let song = LitStr::new(playable, Span::call_site());
            tokens.extend(quote! {
                (JukeboxPlayable, &JukeboxPlayableImpl{
                    song: #song,
                }),
            });
        }

        let TextContent::Translate {
            translate: text,
            with: _,
        } = *self.item_name.clone().0.content
        else {
            unreachable!()
        };
        let item_name = LitStr::new(&text, Span::call_site());
        tokens.extend(quote! {
            (ItemName, &ItemNameImpl {
                name: Cow::Borrowed(#item_name),
            }),
        });

        if let Some(d) = self.damage {
            let damage_lit = LitInt::new(&d.to_string(), Span::call_site());
            tokens.extend(quote! {
                (Damage, &DamageImpl {
                    damage: #damage_lit,
                }),
            });
        }

        if let Some(md) = self.max_damage {
            let max_damage_lit = LitInt::new(&md.to_string(), Span::call_site());
            tokens.extend(quote! {
                (MaxDamage, &MaxDamageImpl {
                    max_damage: #max_damage_lit,
                }),
            });
        }

        if let Some(modifiers) = &self.attribute_modifiers {
            let modifier_code = modifiers.iter().map(|modifier| {
                let r#type = format_ident!(
                    "{}",
                    modifier
                        .r#type
                        .strip_prefix("minecraft:")
                        .unwrap()
                        .to_uppercase()
                );
                let id = LitStr::new(&modifier.id, Span::call_site());
                let amount = modifier.amount;
                let operation = Ident::new(&format!("{:?}", modifier.operation), Span::call_site());
                let slot = modifier.slot.to_tokens();

                quote! {
                    Modifier {
                        r#type: &Attributes::#r#type,
                        id: #id,
                        amount: #amount,
                        operation: Operation::#operation,
                        slot: #slot,
                    }
                }
            });
            tokens.extend(quote! {
                (AttributeModifiers, &AttributeModifiersImpl {
                    attribute_modifiers: Cow::Borrowed(&[#(#modifier_code),*])
                }),
            });
        }

        if let Some(tool) = &self.tool {
            let rules_code = tool.rules.iter().map(|rule| {
                let block_array;

                if let RegistryEntryList::Single(t) = &rule.blocks {
                    if let TagType::Item(str) = t {
                        let ident = format_ident!(
                            "{}",
                            str.strip_prefix("minecraft:").unwrap().to_uppercase()
                        );
                        block_array = quote! {
                            IDs(Cow::Borrowed(&[&Block::#ident]))
                        }
                    } else if let TagType::Tag(str) = t {
                        block_array = quote! {
                            Tag(Cow::Borrowed(#str))
                        }
                    } else {
                        unreachable!();
                    }
                } else if let RegistryEntryList::Many(t) = &rule.blocks {
                    let mut array = vec![];
                    for i in t {
                        let TagType::Item(str) = i else {
                            unreachable!();
                        };
                        let ident = format_ident!(
                            "{}",
                            str.strip_prefix("minecraft:").unwrap().to_uppercase()
                        );
                        array.push(quote! {
                            &Block::#ident
                        });
                    }
                    block_array = quote! {
                        IDs(Cow::Borrowed(&[#(#array),*]))
                    }
                } else {
                    unreachable!();
                }
                let speed = if let Some(speed) = rule.speed {
                    quote! { Some(#speed) }
                } else {
                    quote! { None }
                };
                let correct_for_drops = if let Some(correct_for_drops) = rule.correct_for_drops {
                    let correct_for_drops = LitBool::new(correct_for_drops, Span::call_site());
                    quote! { Some(#correct_for_drops) }
                } else {
                    quote! { None }
                };
                quote! {
                    ToolRule {
                        blocks: #block_array,
                        speed: #speed,
                        correct_for_drops: #correct_for_drops
                    }
                }
            });
            let damage_per_block = {
                let speed = LitInt::new(&tool.damage_per_block.to_string(), Span::call_site());
                quote! { #speed }
            };
            let default_mining_speed = {
                let speed = LitFloat::new(
                    &format!("{:.1}", tool.default_mining_speed),
                    Span::call_site(),
                );
                quote! { #speed }
            };
            let can_destroy_blocks_in_creative =
                LitBool::new(tool.can_destroy_blocks_in_creative, Span::call_site());
            tokens.extend(quote! { (Tool, &ToolImpl {
                rules: Cow::Borrowed(&[#(#rules_code),*]),
                default_mining_speed: #default_mining_speed,
                damage_per_block: #damage_per_block,
                can_destroy_blocks_in_creative: #can_destroy_blocks_in_creative
            }), });
        }

        if let Some(food) = &self.food {
            let nutrition = LitInt::new(&food.nutrition.to_string(), Span::call_site());
            let saturation = LitFloat::new(&format!("{:.1}", food.saturation), Span::call_site());
            let can_always_eat = {
                let can = LitBool::new(food.can_always_eat, Span::call_site());
                quote! { #can }
            };
            tokens.extend(quote! { (Food, &FoodImpl {
                nutrition: #nutrition,
                saturation: #saturation,
                can_always_eat: #can_always_eat,
            }), });
        }

        if let Some(consumable) = &self.consumable {
            let consume_seconds = LitFloat::new(
                &format!("{:.1}", consumable.consume_seconds.unwrap_or(1.6)),
                Span::call_site(),
            );
            let consume_particles = LitBool::new(
                consumable.has_consume_particles.unwrap_or(true),
                Span::call_site(),
            );

            let anim_str = consumable.animation.clone().unwrap_or("eat".to_string());
            let animation = format_ident!("{}", anim_str.to_pascal_case());

            let sound_id = consumable
                .sound
                .clone()
                .unwrap_or("minecraft:entity.generic.eat".to_string());
            let variant_name = format_ident!(
                "{}",
                sound_id
                    .strip_prefix("minecraft:")
                    .unwrap()
                    .to_pascal_case()
            );
            let effects: Vec<ConsumeEffect> =
                consumable.on_consume_effects.clone().unwrap_or(vec![]);
            let mut effect_tokens = TokenStream::new();

            for effect in effects {
                match effect.r#type.as_str() {
                    "minecraft:clear_all_effects" => {
                        effect_tokens.extend(quote! { ConsumeEffect::ClearAllEffects, });
                    }
                    "minecraft:teleport_randomly" => {
                        let diameter = effect.diameter.unwrap_or(16.);
                        effect_tokens
                            .extend(quote! { ConsumeEffect::TeleportRandomly(#diameter), });
                    }
                    "minecraft:play_sound" => {
                        let sound = format_ident!(
                            "{}",
                            effect
                                .sound
                                .unwrap()
                                .strip_prefix("minecraft:")
                                .unwrap()
                                .to_pascal_case()
                        );
                        effect_tokens
                            .extend(quote! { ConsumeEffect::PlaySound(IdOr::Id(Sound::#sound)), });
                    }
                    "minecraft:apply_effects" => {
                        let probability = effect.probability.unwrap_or(1.);
                        if let StringOrStatusEffects::Effects(status_effect_instances) =
                            effect.effects.unwrap()
                        {
                            let mut status_tokens = TokenStream::new();

                            for status in status_effect_instances {
                                let effect_id = status.id;
                                let amplifier = status.amplifier.unwrap_or(0);
                                let duration = status.duration.unwrap_or(1);
                                let ambient = status.ambient.unwrap_or(false);
                                let show_particles = status.show_particles.unwrap_or(true);
                                let show_icon = status.show_icon.unwrap_or(true);
                                status_tokens.extend(quote! {
                                    StatusEffectInstance {
                                        effect_id: Cow::Borrowed(#effect_id),
                                        amplifier: #amplifier,
                                        duration: #duration,
                                        ambient: #ambient,
                                        show_particles: #show_particles,
                                        show_icon: #show_icon
                                    },
                                });
                            }
                            effect_tokens.extend(quote! {
                                ConsumeEffect::ApplyEffects((Cow::Borrowed(&[#status_tokens]), #probability)),
                            });
                        }
                    }
                    "minecraft:remove_effects" => {
                        if let StringOrStatusEffects::String(id) = effect.effects.unwrap() {
                            let effect_id = format_ident!(
                                "{}",
                                id.strip_prefix("minecraft:")
                                    .unwrap()
                                    .to_pascal_case()
                                    .to_uppercase()
                            );

                            effect_tokens.extend(quote! {
                                ConsumeEffect::RemoveEffects(IDSet::IDs(Cow::Borrowed(&[&StatusEffect::#effect_id]))),
                            });
                        }
                    }
                    _ => println!("未知的 CustomEffect 类型：{}", effect.r#type),
                }
            }

            tokens.extend(quote! { (Consumable, &ConsumableImpl {
                consume_seconds: #consume_seconds,
                animation: ConsumeAnimation::#animation,
                sound_event: IdOr::Id(Sound::#variant_name),
                consume_particles: #consume_particles,
                effects: Cow::Borrowed(&[#effect_tokens])
            }), });
        }

        if self.blocks_attacks.is_some() {
            tokens.extend(quote! { (BlocksAttacks, &BlocksAttacksImpl), });
        }

        if self.death_protection.is_some() {
            tokens.extend(quote! { (DeathProtection, &DeathProtectionImpl), });
        }

        if let Some(weapon) = &self.weapon {
            let damage = LitInt::new(
                &weapon.item_damage_per_attack.to_string(),
                Span::call_site(),
            );
            tokens.extend(quote! { (Weapon, &WeaponImpl { item_damage_per_attack: #damage }), });
        }

        if let Some(damage_resistant) = &self.damage_resistant {
            let res_type_variant = match damage_resistant.types.as_str() {
                // 常见规范形式与缩写形式到枚举变体名的映射
                "#minecraft:always_hurts_ender_dragons"
                | "minecraft:always_hurts_ender_dragons"
                | "always_hurts_ender_dragons" => "AlwaysHurtsEnderDragons",
                "#minecraft:always_kills_armor_stands"
                | "minecraft:always_kills_armor_stands"
                | "always_kills_armor_stands" => "AlwaysKillsArmorStands",
                "#minecraft:always_most_significant_fall"
                | "minecraft:always_most_significant_fall"
                | "always_most_significant_fall" => "AlwaysMostSignificantFall",
                "#minecraft:always_triggers_silverfish"
                | "minecraft:always_triggers_silverfish"
                | "always_triggers_silverfish" => "AlwaysTriggersSilverfish",
                "#minecraft:avoids_guardian_thorns"
                | "minecraft:avoids_guardian_thorns"
                | "avoids_guardian_thorns" => "AvoidsGuardianThorns",
                "#minecraft:burns_armor_stands"
                | "minecraft:burns_armor_stands"
                | "burns_armor_stands" => "BurnsArmorStands",
                "#minecraft:burn_from_stepping"
                | "minecraft:burn_from_stepping"
                | "burn_from_stepping" => "BurnFromStepping",
                "#minecraft:bypasses_armor" | "minecraft:bypasses_armor" | "bypasses_armor" => {
                    "BypassesArmor"
                }
                "#minecraft:bypasses_cooldown"
                | "minecraft:bypasses_cooldown"
                | "bypasses_cooldown" => "BypassesCooldown",
                "#minecraft:bypasses_effects"
                | "minecraft:bypasses_effects"
                | "bypasses_effects" => "BypassesEffects",
                "#minecraft:bypasses_enchantments"
                | "minecraft:bypasses_enchantments"
                | "bypasses_enchantments" => "BypassesEnchantments",
                "#minecraft:bypasses_invulnerability"
                | "minecraft:bypasses_invulnerability"
                | "bypasses_invulnerability" => "BypassesInvulnerability",
                "#minecraft:bypasses_resistance"
                | "minecraft:bypasses_resistance"
                | "bypasses_resistance" => "BypassesResistance",
                "#minecraft:bypasses_shield" | "minecraft:bypasses_shield" | "bypasses_shield" => {
                    "BypassesShield"
                }
                "#minecraft:bypasses_wolf_armor"
                | "minecraft:bypasses_wolf_armor"
                | "bypasses_wolf_armor" => "BypassesWolfArmor",
                "#minecraft:can_break_armor_stand"
                | "minecraft:can_break_armor_stand"
                | "can_break_armor_stand" => "CanBreakArmorStands",
                "#minecraft:damages_helmet" | "minecraft:damages_helmet" | "damages_helmet" => {
                    "DamagesHelmet"
                }
                "#minecraft:ignites_armor_stands"
                | "minecraft:ignites_armor_stands"
                | "ignites_armor_stands" => "IgnitesArmorStands",
                "#minecraft:is_drowning" | "minecraft:is_drowning" | "is_drowning" => "Drowning",
                "#minecraft:is_explosion"
                | "minecraft:is_explosion"
                | "is_explosion"
                | "explosion" => "Explosion",
                "#minecraft:is_fall" | "minecraft:is_fall" | "is_fall" | "fall" => "Fall",
                "#minecraft:is_fire" | "minecraft:is_fire" | "is_fire" | "fire" | "in_fire"
                | "minecraft:in_fire" => "Fire",
                "#minecraft:is_freezing" | "minecraft:is_freezing" | "is_freezing" => "Freezing",
                "#minecraft:is_lightning" | "minecraft:is_lightning" | "is_lightning" => {
                    "Lightning"
                }
                "#minecraft:is_player_attack"
                | "minecraft:is_player_attack"
                | "is_player_attack" => "PlayerAttack",
                "#minecraft:is_projectile" | "minecraft:is_projectile" | "is_projectile" => {
                    "Projectile"
                }
                "#minecraft:mace_smash" | "minecraft:mace_smash" | "mace_smash" => "MaceSmash",
                "#minecraft:no_anger" | "minecraft:no_anger" | "no_anger" => "NoAnger",
                "#minecraft:no_impact" | "minecraft:no_impact" | "no_impact" => "NoImpact",
                "#minecraft:no_knockback" | "minecraft:no_knockback" | "no_knockback" => {
                    "NoKnockback"
                }
                "#minecraft:panic_causes" | "minecraft:panic_causes" | "panic_causes" => {
                    "PanicCauses"
                }
                "#minecraft:panic_environmental_causes"
                | "minecraft:panic_environmental_causes"
                | "panic_environmental_causes" => "PanicEnvironmentalCauses",
                "#minecraft:witch_resistant_to"
                | "minecraft:witch_resistant_to"
                | "witch_resistant_to" => "WitchResistantTo",
                "#minecraft:wither_immune_to"
                | "minecraft:wither_immune_to"
                | "wither_immune_to" => "WitherImmuneTo",
                _ => "Generic",
            };
            let res_type_ident = format_ident!("{}", res_type_variant);
            tokens.extend(quote! { (DamageResistant, &DamageResistantImpl {
                res_type: DamageResistantType::#res_type_ident,
            }), });
        }

        if let Some(equippable) = &self.equippable {
            let slot = match equippable.slot.as_str() {
                "mainhand" => quote! { &EquipmentSlot::MAIN_HAND },
                "offhand" => quote! { &EquipmentSlot::OFF_HAND },
                "head" => quote! { &EquipmentSlot::HEAD },
                "chest" => quote! { &EquipmentSlot::CHEST },
                "legs" => quote! { &EquipmentSlot::LEGS },
                "feet" => quote! { &EquipmentSlot::FEET },
                "body" => quote! { &EquipmentSlot::BODY },
                "saddle" => quote! { &EquipmentSlot::SADDLE },
                _ => panic!("未知的可装备槽位：{}", equippable.slot),
            };
            let equip_sound = equippable
                .equip_sound
                .as_ref()
                .map(|s| {
                    let variant_name =
                        format_ident!("{}", s.strip_prefix("minecraft:").unwrap().to_pascal_case());
                    quote! { IdOr::Id(Sound::#variant_name) }
                })
                .unwrap_or(quote! { IdOr::Id(Sound::ItemArmorEquipGeneric) });
            let asset_id = equippable
                .asset_id
                .as_ref()
                .map(|s| {
                    let asset_id = LitStr::new(s, Span::call_site());
                    quote! { Some(Cow::Borrowed(#asset_id)) }
                })
                .unwrap_or(quote! { None });
            let camera_overlay = equippable
                .camera_overlay
                .as_ref()
                .map(|s| {
                    let camera_overlay = LitStr::new(s, Span::call_site());
                    quote! { Some(Cow::Borrowed(#camera_overlay)) }
                })
                .unwrap_or(quote! { None });
            let mut entities_option = TokenStream::new();
            if let Some(entities) = equippable.allowed_entities.clone() {
                let mut allowed_entities = TokenStream::new();
                match entities {
                    StringOrList::String(str) => {
                        if str.starts_with("#") {
                            let formatted = str.strip_prefix("#minecraft:").unwrap();
                            allowed_entities.extend(quote! {
                                IDSet::Tag(Cow::Borrowed(#formatted))
                            });
                        } else {
                            let ident = format_ident!(
                                "{}",
                                str.strip_prefix("minecraft:").unwrap().to_uppercase()
                            );
                            allowed_entities.extend(quote! {
                                IDSet::IDs(Cow::Borrowed(&[&crate::entity_type::EntityType::#ident]))
                            });
                        }
                    }
                    StringOrList::List(items) => {
                        let mut ids = TokenStream::new();
                        for x in items {
                            let entity = format_ident!(
                                "{}",
                                x.strip_prefix("minecraft:").unwrap().to_uppercase()
                            );
                            ids.extend(quote! { &crate::entity_type::EntityType::#entity, });
                        }

                        allowed_entities.extend(quote! {
                            IDSet::IDs(Cow::Borrowed(&[#ids]))
                        });
                    }
                }

                entities_option.extend(quote! { Some(#allowed_entities) });
            } else {
                entities_option.extend(quote! { None });
            }
            let dispensable = LitBool::new(equippable.dispensable, Span::call_site());
            let swappable = LitBool::new(equippable.swappable, Span::call_site());
            let damage_on_hurt = LitBool::new(equippable.damage_on_hurt, Span::call_site());
            let equip_on_interact = LitBool::new(equippable.equip_on_interact, Span::call_site());
            let can_be_sheared = LitBool::new(equippable.can_be_sheared, Span::call_site());
            let shearing_sound = equippable
                .shearing_sound
                .as_ref()
                .map(|s| {
                    let variant_name =
                        format_ident!("{}", s.strip_prefix("minecraft:").unwrap().to_pascal_case());
                    quote! { IdOr::Id(Sound::#variant_name) }
                })
                .unwrap_or(quote! { IdOr::Id(Sound::ItemShearsSnip) });

            tokens.extend(quote! { (Equippable, &EquippableImpl {
                slot: #slot,
                equip_sound: #equip_sound,
                asset_id: #asset_id,
                camera_overlay: #camera_overlay,
                allowed_entities: #entities_option,
                dispensable: #dispensable,
                swappable: #swappable,
                damage_on_hurt: #damage_on_hurt,
                equip_on_interact: #equip_on_interact,
                can_be_sheared: #can_be_sheared,
                shearing_sound: #shearing_sound
            }), });
        }

        if let Some(enchantable) = &self.enchantable {
            let value = LitInt::new(&enchantable.value.to_string(), Span::call_site());
            tokens.extend(quote! { (Enchantable, &EnchantableImpl { value: #value }), });
        }

        if let Some(attack_range) = &self.attack_range {
            let min_reach = float_literal(attack_range.min_reach);
            let max_reach = float_literal(attack_range.max_reach);
            let min_creative_reach = float_literal(attack_range.min_creative_reach);
            let max_creative_reach = float_literal(attack_range.max_creative_reach);
            let hitbox_margin = float_literal(attack_range.hitbox_margin);
            let mob_factor = float_literal(attack_range.mob_factor);
            tokens.extend(quote! {
                (AttackRange, &AttackRangeImpl {
                    min_reach: #min_reach,
                    max_reach: #max_reach,
                    min_creative_reach: #min_creative_reach,
                    max_creative_reach: #max_creative_reach,
                    hitbox_margin: #hitbox_margin,
                    mob_factor: #mob_factor,
                }),
            });
        }
        if self.banner_patterns.is_some() {
            tokens.extend(quote! { (BannerPatterns, &BannerPatternsImpl::EMPTY), });
        }
        if self.bees.is_some() {
            tokens.extend(quote! { (Bees, &BeesImpl), });
        }
        if let Some(block_state) = &self.block_state {
            let mut entries = TokenStream::new();
            if let serde_json::Value::Object(map) = block_state {
                for (k, v) in map {
                    let v_str = match v {
                        serde_json::Value::String(s) => s.clone(),
                        _ => v.to_string(),
                    };
                    let k_lit = LitStr::new(k, Span::call_site());
                    let v_lit = LitStr::new(&v_str, Span::call_site());
                    entries.extend(quote! {
                        (Cow::Borrowed(#k_lit), Cow::Borrowed(#v_lit)),
                    });
                }
            }
            tokens.extend(quote! {
                (BlockState, &BlockStateImpl {
                    properties: Cow::Borrowed(&[#entries])
                }),
            });
        }
        if self.break_sound.is_some() {
            tokens.extend(quote! { (BreakSound, &BreakSoundImpl), });
        }
        if self.bucket_entity_data.is_some() {
            tokens.extend(quote! { (BucketEntityData, &BucketEntityDataImpl), });
        }
        if self.bundle_contents.is_some() {
            tokens.extend(quote! { (BundleContents, &BundleContentsImpl { items: Vec::new() }), });
        }
        if self.charged_projectiles.is_some() {
            tokens.extend(quote! { (ChargedProjectiles, &ChargedProjectilesImpl { projectiles: Vec::new() }), });
        }
        if let Some(val) = &self.chicken_variant {
            let val_str = match val {
                serde_json::Value::String(s) => s.clone(),
                _ => val.to_string(),
            };
            let val_lit = LitStr::new(&val_str, Span::call_site());
            tokens.extend(quote! { (ChickenVariant, &ChickenVariantImpl { value: Cow::Borrowed(#val_lit) }), });
        }
        if self.container.is_some() {
            tokens.extend(quote! { (Container, &ContainerImpl { items: Vec::new() }), });
        }
        if let Some(damage_type) = &self.damage_type {
            let damage_type = format_ident!(
                "{}",
                damage_type
                    .strip_prefix("minecraft:")
                    .unwrap_or(damage_type)
                    .to_shouty_snake_case()
            );
            tokens.extend(quote! {
                (DamageType, &DamageTypeImpl {
                    damage_type: crate::damage::DamageType::#damage_type,
                }),
            });
        }
        if self.debug_stick_state.is_some() {
            tokens.extend(quote! { (DebugStickState, &DebugStickStateImpl), });
        }
        if self.dye.is_some() {
            tokens.extend(quote! { (Dye, &DyeImpl), });
        }
        if self.brewing_fuel.is_some() {
            tokens.extend(quote! { (BrewingFuel, &BrewingFuelImpl), });
        }
        if self.enchantment_glint_override.is_some() {
            tokens.extend(quote! { (EnchantmentGlintOverride, &EnchantmentGlintOverrideImpl), });
        }
        if self.enchantments.is_some() {
            tokens.extend(
                quote! { (Enchantments, &EnchantmentsImpl { enchantment: Cow::Borrowed(&[]) }), },
            );
        }
        if self.entity_data.is_some() {
            tokens.extend(quote! { (EntityData, &EntityDataImpl), });
        }
        if let Some(fireworks) = &self.fireworks {
            let flight_duration = if let serde_json::Value::Object(map) = fireworks
                && let Some(serde_json::Value::Number(num)) = map.get("flight_duration")
            {
                num.as_i64().unwrap_or(1) as i32
            } else {
                1
            };
            tokens.extend(quote! {
                (Fireworks, &FireworksImpl {
                    flight_duration: #flight_duration,
                    explosions: Vec::new(),
                }),
            });
        }
        if self.glider.is_some() {
            tokens.extend(quote! { (Glider, &GliderImpl), });
        }
        if self.instrument.is_some() {
            tokens.extend(quote! { (Instrument, &InstrumentImpl), });
        }
        if let Some(model) = &self.item_model {
            let model_lit = LitStr::new(model, Span::call_site());
            tokens
                .extend(quote! { (ItemModel, &ItemModelImpl { id: Cow::Borrowed(#model_lit) }), });
        }
        if let Some(kinetic_weapon) = &self.kinetic_weapon {
            let contact_cooldown_ticks = LitInt::new(
                &kinetic_weapon.contact_cooldown_ticks.to_string(),
                Span::call_site(),
            );
            let delay_ticks =
                LitInt::new(&kinetic_weapon.delay_ticks.to_string(), Span::call_site());
            let dismount_conditions =
                kinetic_condition_tokens(kinetic_weapon.dismount_conditions.as_ref());
            let knockback_conditions =
                kinetic_condition_tokens(kinetic_weapon.knockback_conditions.as_ref());
            let damage_conditions =
                kinetic_condition_tokens(kinetic_weapon.damage_conditions.as_ref());
            let forward_movement = float_literal(kinetic_weapon.forward_movement);
            let damage_multiplier = float_literal(kinetic_weapon.damage_multiplier);
            let sound = optional_sound_tokens(kinetic_weapon.sound.as_deref());
            let hit_sound = optional_sound_tokens(kinetic_weapon.hit_sound.as_deref());
            tokens.extend(quote! {
                (KineticWeapon, &KineticWeaponImpl {
                    contact_cooldown_ticks: #contact_cooldown_ticks,
                    delay_ticks: #delay_ticks,
                    dismount_conditions: #dismount_conditions,
                    knockback_conditions: #knockback_conditions,
                    damage_conditions: #damage_conditions,
                    forward_movement: #forward_movement,
                    damage_multiplier: #damage_multiplier,
                    sound: #sound,
                    hit_sound: #hit_sound,
                }),
            });
        }
        if self.lore.is_some() {
            tokens.extend(quote! { (Lore, &LoreImpl { lines: Vec::new() }), });
        }
        if self.map_decorations.is_some() {
            tokens.extend(quote! { (MapDecorations, &MapDecorationsImpl), });
        }
        if let Some(charge) = self.minimum_attack_charge {
            let charge = float_literal(charge);
            tokens.extend(
                quote! { (MinimumAttackCharge, &MinimumAttackChargeImpl { charge: #charge }), },
            );
        }
        if let Some(amp) = self.ominous_bottle_amplifier {
            let amp_lit = LitInt::new(&amp.to_string(), Span::call_site());
            tokens.extend(quote! { (OminousBottleAmplifier, &OminousBottleAmplifierImpl { amplifier: #amp_lit }), });
        }
        if let Some(piercing_weapon) = &self.piercing_weapon {
            let deals_knockback = LitBool::new(piercing_weapon.deals_knockback, Span::call_site());
            let dismounts = LitBool::new(piercing_weapon.dismounts, Span::call_site());
            let sound = optional_sound_tokens(piercing_weapon.sound.as_deref());
            let hit_sound = optional_sound_tokens(piercing_weapon.hit_sound.as_deref());
            tokens.extend(quote! {
                (PiercingWeapon, &PiercingWeaponImpl {
                    deals_knockback: #deals_knockback,
                    dismounts: #dismounts,
                    sound: #sound,
                    hit_sound: #hit_sound,
                }),
            });
        }
        if self.pot_decorations.is_some() {
            tokens.extend(quote! { (PotDecorations, &PotDecorationsImpl), });
        }
        if self.potion_contents.is_some() {
            tokens.extend(quote! {
                (PotionContents, &PotionContentsImpl {
                    potion_id: None,
                    custom_color: None,
                    custom_effects: Vec::new(),
                    custom_name: None,
                }),
            });
        }
        if let Some(scale) = self.potion_duration_scale {
            let scale_lit = LitFloat::new(&format!("{scale:?}f32"), Span::call_site());
            tokens.extend(
                quote! { (PotionDurationScale, &PotionDurationScaleImpl { scale: #scale_lit }), },
            );
        }
        if self.provides_banner_patterns.is_some() {
            tokens.extend(quote! { (ProvidesBannerPatterns, &ProvidesBannerPatternsImpl), });
        }
        if self.provides_trim_material.is_some() {
            tokens.extend(quote! { (ProvidesTrimMaterial, &ProvidesTrimMaterialImpl), });
        }
        if let Some(rarity_str) = &self.rarity {
            let rarity_variant = match rarity_str.as_str() {
                "uncommon" => quote! { crate::data_component_impl::Rarity::Uncommon },
                "rare" => quote! { crate::data_component_impl::Rarity::Rare },
                "epic" => quote! { crate::data_component_impl::Rarity::Epic },
                _ => quote! { crate::data_component_impl::Rarity::Common },
            };
            tokens.extend(quote! { (Rarity, &RarityImpl { rarity: #rarity_variant }), });
        }
        if self.recipes.is_some() {
            tokens.extend(quote! { (Recipes, &RecipesImpl), });
        }
        if let Some(cost) = self.repair_cost {
            tokens.extend(quote! { (RepairCost, &RepairCostImpl { cost: #cost }), });
        }
        if let Some(repairable) = &self.repairable {
            let mut items_tokens = TokenStream::new();
            match &repairable.items {
                StringOrList::String(str) => {
                    if let Some(formatted) = str.strip_prefix('#') {
                        items_tokens.extend(quote! {
                            IDSet::Tag(Cow::Borrowed(#formatted))
                        });
                    } else {
                        let item_name = str.strip_prefix("minecraft:").unwrap_or(str);
                        let ident = format_ident!("{}", item_name.to_shouty_snake_case());
                        items_tokens.extend(quote! {
                            IDSet::IDs(Cow::Borrowed(&[&Self::#ident]))
                        });
                    }
                }
                StringOrList::List(items) => {
                    let mut ids = TokenStream::new();
                    for x in items {
                        let item_name = x.strip_prefix("minecraft:").unwrap_or(x);
                        let ident = format_ident!("{}", item_name.to_shouty_snake_case());
                        ids.extend(quote! { &Self::#ident, });
                    }
                    items_tokens.extend(quote! {
                        IDSet::IDs(Cow::Borrowed(&[#ids]))
                    });
                }
            }
            tokens.extend(quote! { (Repairable, &RepairableImpl { items: #items_tokens }), });
        }
        if self.stored_enchantments.is_some() {
            tokens.extend(quote! { (StoredEnchantments, &StoredEnchantmentsImpl { enchantment: Cow::Borrowed(&[]) }), });
        }
        if self.suspicious_stew_effects.is_some() {
            tokens.extend(quote! { (SuspiciousStewEffects, &SuspiciousStewEffectsImpl::EMPTY), });
        }
        if let Some(swing) = &self.swing_animation {
            let anim_type = match swing.r#type.as_str() {
                "whack" => quote! { SwingAnimationType::Whack },
                "stab" => quote! { SwingAnimationType::Stab },
                "none" => quote! { SwingAnimationType::None },
                _ => quote! { SwingAnimationType::Whack },
            };
            let duration = swing.duration;
            tokens.extend(quote! {
                (
                    AttackAnimation,
                    &SwingAnimationImpl {
                        animation_type: #anim_type,
                        duration: #duration,
                    },
                ),
            });
        }
        if self.tooltip_display.is_some() {
            tokens.extend(quote! { (TooltipDisplay, &TooltipDisplayImpl), });
        }
        if self.use_effects.is_some() {
            tokens.extend(quote! { (UseEffects, &UseEffectsImpl), });
        }
        if self.use_remainder.is_some() {
            tokens.extend(quote! { (UseRemainder, &UseRemainderImpl), });
        }
        if self.writable_book_content.is_some() {
            tokens.extend(
                quote! { (WritableBookContent, &WritableBookContentImpl { pages: Vec::new() }), },
            );
        }
    }
}

/// 返回 `1f32` 的 Serde 默认值辅助函数。
const fn return_1f32() -> f32 {
    1.
}

/// 返回 `true` 的 Serde 默认值辅助函数。
const fn return_true() -> bool {
    true
}

/// 反序列化得到的工具组件，包含挖掘规则和默认速度。
const fn default_item_damage() -> u32 {
    1
}

#[derive(Deserialize)]
pub struct ToolComponent {
    /// 按单个方块或方块标签应用的挖掘规则的有序列表。
    rules: Vec<ToolRule>,
    /// 没有规则匹配时的默认挖掘速度，默认为 `1.0`。
    #[serde(default = "return_1f32")]
    default_mining_speed: f32,
    /// 每破坏一个方块消耗的耐久度，默认为 `1`。
    #[serde(default = "default_item_damage")]
    damage_per_block: u32,
    /// 该工具在创造模式下能否破坏方块，默认为 `true`。
    #[serde(default = "return_true")]
    can_destroy_blocks_in_creative: bool,
}

#[derive(Deserialize)]
pub struct UseCooldownComponent {
    seconds: f32,
    cooldown_group: Option<String>,
}

/// 返回 `false` 的 Serde 默认值辅助函数。
const fn return_false() -> bool {
    false
}

/// 反序列化得到的食物组件，描述营养值和饱和度。
#[derive(Deserialize, Copy, Clone)]
pub struct FoodComponent {
    /// 食用后恢复的饥饿值。
    nutrition: u8,
    /// 进食时应用的饱和度修改器。
    saturation: f32,
    /// 该物品是否可在饱食度已满时食用，默认为 `false`。
    #[serde(default = "return_false")]
    can_always_eat: bool,
}

/// 单个工具挖掘规则，将一组方块映射到可选的速度覆盖值。
#[derive(Deserialize, Clone)]
pub struct ToolRule {
    /// 此规则适用的方块或方块标签集合。
    blocks: RegistryEntryList,
    /// 挖掘匹配方块时的可选速度覆盖。
    speed: Option<f32>,
    /// 若已指定，该工具是否为匹配的方块产生掉落物。
    correct_for_drops: Option<bool>,
}

/// 持有或穿戴该物品时施加的单个属性修饰符。
#[derive(Deserialize, Clone)]
pub struct Modifier {
    /// 带命名空间的属性键（例如 `"minecraft:attack_damage"`）。
    pub r#type: String,
    /// 此修改器实例的唯一标识符。
    pub id: String,
    /// 根据 `operation` 相加或相乘的数值。
    pub amount: f64,
    /// `amount` 如何与基础属性值合并。
    pub operation: Operation,
    // TODO: 将此改为枚举
    /// 此修饰符生效所在的装备槽位。
    pub slot: AttributeModifierSlot,
}

/// 返回 `true` 的 Serde 默认值辅助函数。
const fn _true() -> bool {
    true
}

/// 反序列化得到的可消耗组件，描述使用时长。
#[derive(Deserialize, Clone)]
pub struct Consumable {
    consume_seconds: Option<f32>,
    has_consume_particles: Option<bool>,
    animation: Option<String>,
    sound: Option<String>,
    on_consume_effects: Option<Vec<ConsumeEffect>>,
}
#[derive(Deserialize, Clone)]
pub struct ConsumeEffect {
    r#type: String,
    probability: Option<f32>,
    diameter: Option<f32>,
    sound: Option<String>,
    effects: Option<StringOrStatusEffects>,
}
#[derive(Deserialize, Clone)]
pub struct StatusEffectInstance {
    pub id: String,
    pub amplifier: Option<i32>,
    pub duration: Option<i32>,
    pub ambient: Option<bool>,
    pub show_particles: Option<bool>,
    pub show_icon: Option<bool>,
}
#[derive(Deserialize, Clone)]
#[serde(untagged)]
pub enum StringOrStatusEffects {
    String(String),
    Effects(Vec<StatusEffectInstance>),
}

/// 反序列化得到的死亡保护组件（例如不死图腾）；字段尚未实现。
#[derive(Deserialize, Clone)]
pub struct DeathProtection {
    // TODO
}

/// 反序列化得到的格挡攻击组件（例如盾牌）；字段尚未实现。
#[derive(Deserialize, Clone)]
pub struct WeaponComponent {
    #[serde(default = "default_item_damage")]
    pub item_damage_per_attack: u32,
    // TODO: 待盾牌禁用机制实现后，添加 disable_blocking_for_seconds 的解析。
    // 这为原版物品和数据包保留了往返转换的保真度。
}

#[derive(Deserialize, Clone)]
pub struct AttackRangeComponent {
    #[serde(default)]
    pub min_reach: f32,
    #[serde(default = "default_max_reach")]
    pub max_reach: f32,
    #[serde(default)]
    pub min_creative_reach: f32,
    #[serde(default = "default_max_creative_reach")]
    pub max_creative_reach: f32,
    #[serde(default = "default_hitbox_margin")]
    pub hitbox_margin: f32,
    #[serde(default = "return_1f32")]
    pub mob_factor: f32,
}

const fn default_max_reach() -> f32 {
    3.0
}

const fn default_max_creative_reach() -> f32 {
    5.0
}

const fn default_hitbox_margin() -> f32 {
    0.3
}

#[derive(Deserialize, Clone)]
pub struct KineticConditionComponent {
    pub max_duration_ticks: i32,
    #[serde(default)]
    pub min_speed: f32,
    #[serde(default)]
    pub min_relative_speed: f32,
}

#[derive(Deserialize, Clone)]
pub struct KineticWeaponComponent {
    #[serde(default = "default_contact_cooldown_ticks")]
    pub contact_cooldown_ticks: i32,
    #[serde(default)]
    pub delay_ticks: i32,
    pub dismount_conditions: Option<KineticConditionComponent>,
    pub knockback_conditions: Option<KineticConditionComponent>,
    pub damage_conditions: Option<KineticConditionComponent>,
    #[serde(default)]
    pub forward_movement: f32,
    #[serde(default = "return_1f32")]
    pub damage_multiplier: f32,
    pub sound: Option<String>,
    pub hit_sound: Option<String>,
}

const fn default_contact_cooldown_ticks() -> i32 {
    10
}

#[derive(Deserialize, Clone)]
pub struct PiercingWeaponComponent {
    #[serde(default = "_true")]
    pub deals_knockback: bool,
    #[serde(default)]
    pub dismounts: bool,
    pub sound: Option<String>,
    pub hit_sound: Option<String>,
}

fn float_literal(value: f32) -> LitFloat {
    LitFloat::new(&format!("{value:?}f32"), Span::call_site())
}

fn optional_sound_tokens(sound: Option<&str>) -> TokenStream {
    sound.map_or_else(
        || quote! { None },
        |sound| {
            let variant = format_ident!(
                "{}",
                sound
                    .strip_prefix("minecraft:")
                    .unwrap_or(sound)
                    .to_pascal_case()
            );
            quote! { Some(IdOr::Id(Sound::#variant)) }
        },
    )
}

fn kinetic_condition_tokens(condition: Option<&KineticConditionComponent>) -> TokenStream {
    condition.map_or_else(
        || quote! { None },
        |condition| {
            let max_duration_ticks =
                LitInt::new(&condition.max_duration_ticks.to_string(), Span::call_site());
            let min_speed = float_literal(condition.min_speed);
            let min_relative_speed = float_literal(condition.min_relative_speed);
            quote! {
                Some(KineticConditionImpl {
                    max_duration_ticks: #max_duration_ticks,
                    min_speed: #min_speed,
                    min_relative_speed: #min_relative_speed,
                })
            }
        },
    )
}

#[derive(Deserialize, Clone)]
pub struct BlocksAttacks {
    // TODO
}

/// 反序列化得到的伤害抗性组件，指示物品抵抗哪些伤害类型。
#[derive(Deserialize, Clone)]
pub struct DamageResistantComponent {
    /// 该物品免疫的、带命名空间的伤害类型标签。
    pub types: String,
}
#[derive(Deserialize, Clone)]
#[serde(untagged)]
pub enum StringOrList {
    String(String),
    List(Vec<String>),
}

#[derive(Deserialize, Clone)]
pub struct RepairableComponent {
    pub items: StringOrList,
}

/// 反序列化得到的可装备组件，描述物品如何被穿戴或装备。
#[derive(Deserialize, Clone)]
pub struct EquippableComponent {
    /// 物品所占用的装备槽位（例如 `"head"`、`"chest"`）。
    pub slot: String,
    /// 物品被装备时播放的音效事件；若缺失则使用通用回退音效。
    pub equip_sound: Option<String>,
    /// 已装备模型的纹理资源标识符（如有）。
    pub asset_id: Option<String>,
    /// 装备时显示的屏幕叠加层纹理（如有）。
    pub camera_overlay: Option<String>,
    pub allowed_entities: Option<StringOrList>,
    #[serde(default = "_true")]
    pub dispensable: bool,
    /// 按住 Shift 点击是否将物品换入装备槽位，默认为 `true`。
    #[serde(default = "_true")]
    pub swappable: bool,
    /// 穿戴者受伤时该物品是否受损，默认为 `true`。
    #[serde(default = "_true")]
    pub damage_on_hurt: bool,
    /// 右键点击实体时是否装备该物品，默认为 `false`。
    #[serde(default)]
    pub equip_on_interact: bool,
    /// 剪刀能否从实体身上移除该物品，默认为 `false`。
    #[serde(default)]
    pub can_be_sheared: bool,
    /// 被剪下时播放的音效事件（若有）。
    pub shearing_sound: Option<String>,
}

/// 将属性修饰符的数值与基础值合并时应用的算术运算。
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[expect(clippy::enum_variant_names)]
pub enum Operation {
    /// 将修饰符数值直接加到基础值上。
    AddValue,
    /// 将 `amount * base` 加到累计总值上。
    AddMultipliedBase,
    /// 将当前累计值乘以 `1 + amount`。
    AddMultipliedTotal,
}

/// 读取 `items.json` 并生成完整的物品注册表 `TokenStream`。
pub fn build() -> TokenStream {
    let items: BTreeMap<String, Item> =
        serde_json::from_str(&fs::read_to_string("../../assets/items.json").unwrap())
            .expect("解析 items.json 失败");

    let mut type_from_raw_id_arms = TokenStream::new();
    let mut type_from_name = TokenStream::new();

    let mut constants = TokenStream::new();

    for (name, item) in &items {
        let const_ident = format_ident!("{}", name.to_shouty_snake_case());

        let components = &item.components;
        let components_tokens = components.to_token_stream();

        let id_lit = LitInt::new(&item.id.to_string(), Span::call_site());

        constants.extend(quote! {
            pub const #const_ident: Self = Self {
                id: #id_lit,
                registry_key: #name,
                components: &[#components_tokens],
            };
        });

        type_from_raw_id_arms.extend(quote! {
            #id_lit => Some(&Self::#const_ident),
        });

        type_from_name.extend(quote! {
            #name => Some(&Self::#const_ident),
        });
    }

    quote! {
        #[allow(clippy::wildcard_imports, clippy::enum_glob_use, clippy::too_many_lines)]
        use crate::data_component::DataComponent::*;
        use crate::data_component_impl::*;
        use crate::tag::{RegistryKey, Taggable};
        use papokin_util::text::TextComponent;
        use std::borrow::Cow;
        use std::hash::{Hash, Hasher};
        use crate::{tag, AttributeModifierSlot};
        use crate::attributes::Attributes;
        use crate::data_component_impl::IDSet::{IDs, Tag};
        use crate::data_component::DataComponent;
        use crate::effect::StatusEffect;
        use crate::Block;
        use crate::sound::Sound;

        #[derive(Clone)]
        pub struct Item {
            pub id: u16,
            pub registry_key: &'static str,
            pub components: &'static [(DataComponent, &'static dyn DataComponentImpl)],
        }

        impl Eq for Item {}

        impl Hash for Item {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.id.hash(state);
            }
        }

        impl PartialEq for Item {
            fn eq(&self, other: &Self) -> bool {
                self.id == other.id
            }
        }

        impl std::fmt::Debug for Item {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("Item")
                    .field("id", &self.id)
                    .field("registry_key", &self.registry_key)
                    .finish()
            }
        }

        impl Item {
            #constants

            #[must_use]
            #[allow(deprecated)]
            pub fn translated_name(&self) -> TextComponent {
                let name = self
                    .components
                    .iter()
                    .find_map(|(id, data)| {
                        if id == &ItemName {
                            data.as_any()
                                .downcast_ref::<ItemNameImpl>()
                                .map(|name| name.name.as_ref())
                        } else {
                            None
                        }
                    })
                    .unwrap_or(self.registry_key);
                TextComponent::translate(name, &[])
            }

            #[doc = "Try to parse an item from a resource location string."]
            #[must_use]
            pub fn from_registry_key(name: &str) -> Option<&'static Self> {
                let name = name.strip_prefix("minecraft:").unwrap_or(name);
                match name {
                    #type_from_name
                    _ => None
                }
            }

            #[doc = "Try to parse an item from a raw id."]
            #[must_use]
            pub const fn from_id(id: u16) -> Option<&'static Self> {
                match id {
                    #type_from_raw_id_arms
                    _ => None
                }
            }
        }

        impl Taggable for Item {
            #[inline]
            fn tag_key() -> RegistryKey {
                RegistryKey::Item
            }

            #[inline]
            fn registry_key(&self) -> &str {
                self.registry_key
            }

            #[inline]
            fn registry_id(&self) -> u16 {
                self.id
            }
        }
    }
}
