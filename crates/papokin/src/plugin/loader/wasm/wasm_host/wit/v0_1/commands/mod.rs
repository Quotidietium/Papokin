use wasmtime::component::{Access, HasSelf, Resource};

use crate::{
    command::{
        argument_builder::{argument, literal},
        argument_types::{
            block::BlockArgumentType,
            block_predicate::BlockPredicateArgumentType,
            component::ComponentArgumentType,
            coordinates::{
                block_pos::BlockPosArgumentType, rotation::RotationArgumentType,
                vec2::Vec2ArgumentType, vec3::Vec3ArgumentType,
            },
            core::{
                bool::BoolArgumentType, double::DoubleArgumentType, float::FloatArgumentType,
                integer::IntegerArgumentType, long::LongArgumentType, string::StringArgumentType,
            },
            entity::EntityArgumentType,
            entity_anchor::EntityAnchorArgumentType,
            game_profile::GameProfileArgumentType,
            gamemode::GameModeArgumentType,
            identifier::IdentifierArgumentType,
            item::ItemStackArgumentType,
            item_predicate::ItemPredicateArgumentType,
            time::TimeArgumentType,
        },
    },
    plugin::loader::wasm::wasm_host::{
        state::{
            CommandNodeResource, CommandResource, CommandSenderResource, ConsumedArgsResource,
            PluginHostState, ServerResource, TextComponentResource, WasmCommand, WasmCommandNode,
        },
        wit::v0_1::{
            commands::executor::{WasmCommandExecutor, WasmCommandSuggestionProvider},
            papokin::{
                self,
                plugin::{
                    command::{
                        Arg, ArgumentType, Command, CommandNode, CommandSender, CommandSenderType,
                        ConsumedArgs, PermissionLevel, StringType,
                    },
                    common::{BlockPos as WitBlockPos, Locale, Position},
                    player::Player,
                    server::Server,
                    text::TextComponent,
                    world::World,
                },
            },
        },
    },
};

pub mod executor;

impl PluginHostState {
    fn get_command_mut(
        &mut self,
        res: &Resource<Command>,
    ) -> wasmtime::Result<&mut CommandResource> {
        self.resource_table
            .get_mut::<CommandResource>(&Resource::new_own(res.rep()))
            .map_err(wasmtime::Error::from)
    }
    fn get_node_mut(
        &mut self,
        res: &Resource<CommandNode>,
    ) -> wasmtime::Result<&mut CommandNodeResource> {
        self.resource_table
            .get_mut::<CommandNodeResource>(&Resource::new_own(res.rep()))
            .map_err(wasmtime::Error::from)
    }
    fn take_node(&mut self, res: &Resource<CommandNode>) -> wasmtime::Result<CommandNodeResource> {
        self.resource_table
            .delete::<CommandNodeResource>(Resource::new_own(res.rep()))
            .map_err(wasmtime::Error::from)
    }
    fn get_sender_res(
        &self,
        res: &Resource<CommandSender>,
    ) -> wasmtime::Result<&CommandSenderResource> {
        self.resource_table
            .get::<CommandSenderResource>(&Resource::new_own(res.rep()))
            .map_err(wasmtime::Error::from)
    }
    fn get_sender_mut(
        &mut self,
        res: &Resource<CommandSender>,
    ) -> wasmtime::Result<&mut CommandSenderResource> {
        self.resource_table
            .get_mut::<CommandSenderResource>(&Resource::new_own(res.rep()))
            .map_err(wasmtime::Error::from)
    }
}

impl papokin::plugin::command::Host for PluginHostState {}

impl papokin::plugin::command::HostConsumedArgs for PluginHostState {
    #[expect(clippy::too_many_lines)]
    async fn get_value(
        &mut self,
        consumed_args: Resource<ConsumedArgs>,
        key: String,
    ) -> wasmtime::Result<Arg> {
        use crate::plugin::loader::wasm::wasm_host::args::OwnedArg;

        let resource = self
            .resource_table
            .get::<ConsumedArgsResource>(&Resource::new_own(consumed_args.rep()))
            .map_err(wasmtime::Error::from)?;

        let Some(owned_arg) = resource.provider.get(&key).cloned() else {
            return Ok(Arg::Simple(String::new()));
        };

        Ok(match owned_arg {
            OwnedArg::Simple(s) => Arg::Simple(s),
            OwnedArg::Msg(s) => Arg::Msg(s),
            OwnedArg::Bool(b) => Arg::Bool(b),
            OwnedArg::Item(s) => Arg::Item(s),
            OwnedArg::ItemPredicate(s) => Arg::ItemPredicate(s),
            OwnedArg::ResourceLocation(s) => Arg::ResourceLocation(s),
            OwnedArg::Block(s) => Arg::Block(s),
            OwnedArg::BlockPredicate(s) => Arg::BlockPredicate(s),
            OwnedArg::Time(t) => Arg::Time(t),
            OwnedArg::Num(n) => {
                use crate::plugin::loader::wasm::wasm_host::args::{NotInBounds, Number};
                let convert_num = |n: Number| match n {
                    Number::F64(v) => papokin::plugin::command::Number::Float64(v),
                    Number::F32(v) => papokin::plugin::command::Number::Float32(v),
                    Number::I32(v) => papokin::plugin::command::Number::Int32(v),
                    Number::I64(v) => papokin::plugin::command::Number::Int64(v),
                };
                Arg::Num(n.map(convert_num).map_err(|e| match e {
                    NotInBounds::LowerBound(a, b) => {
                        papokin::plugin::command::NotInBounds::LowerBound((
                            convert_num(a),
                            convert_num(b),
                        ))
                    }
                    NotInBounds::UpperBound(a, b) => {
                        papokin::plugin::command::NotInBounds::UpperBound((
                            convert_num(a),
                            convert_num(b),
                        ))
                    }
                }))
            }
            OwnedArg::BlockPos(p) => Arg::BlockPos(WitBlockPos {
                x: p.0.x,
                y: p.0.y,
                z: p.0.z,
            }),
            OwnedArg::Pos3D(v) => Arg::Pos3d((v.x, v.y, v.z)),
            OwnedArg::Pos2D(v) => Arg::Pos2d((v.x, v.y)),
            OwnedArg::Rotation(a, b, c, d) => Arg::Rotation((a, b, c, d)),
            OwnedArg::GameMode(g) => Arg::Gamemode(match g {
                papokin_util::GameMode::Survival => papokin::plugin::common::GameMode::Survival,
                papokin_util::GameMode::Creative => papokin::plugin::common::GameMode::Creative,
                papokin_util::GameMode::Adventure => papokin::plugin::common::GameMode::Adventure,
                papokin_util::GameMode::Spectator => papokin::plugin::common::GameMode::Spectator,
            }),
            OwnedArg::Difficulty(d) => Arg::Difficulty(match d {
                papokin_util::Difficulty::Peaceful => papokin::plugin::server::Difficulty::Peaceful,
                papokin_util::Difficulty::Easy => papokin::plugin::server::Difficulty::Easy,
                papokin_util::Difficulty::Normal => papokin::plugin::server::Difficulty::Normal,
                papokin_util::Difficulty::Hard => papokin::plugin::server::Difficulty::Hard,
            }),
            OwnedArg::Players(players) => {
                let mut resources = Vec::new();
                for p in players {
                    if let Ok(r) = self.add_player(p) {
                        resources.push(r);
                    }
                }
                Arg::Players(resources)
            }
            OwnedArg::Particle(p) => Arg::Particle(format!("{p:?}")),
            OwnedArg::TextComponent(t) => {
                let r = self
                    .resource_table
                    .push(TextComponentResource { provider: t })
                    .map_err(wasmtime::Error::from)?;
                Arg::TextComponent(wasmtime::component::Resource::new_own(r.rep()))
            }
            OwnedArg::BossbarColor(c) => Arg::BossbarColor(match c {
                crate::world::bossbar::BossbarColor::Pink => {
                    papokin::plugin::command::BossbarColor::Pink
                }
                crate::world::bossbar::BossbarColor::Blue => {
                    papokin::plugin::command::BossbarColor::Blue
                }
                crate::world::bossbar::BossbarColor::Red => {
                    papokin::plugin::command::BossbarColor::Red
                }
                crate::world::bossbar::BossbarColor::Green => {
                    papokin::plugin::command::BossbarColor::Green
                }
                crate::world::bossbar::BossbarColor::Yellow => {
                    papokin::plugin::command::BossbarColor::Yellow
                }
                crate::world::bossbar::BossbarColor::Purple => {
                    papokin::plugin::command::BossbarColor::Purple
                }
                crate::world::bossbar::BossbarColor::White => {
                    papokin::plugin::command::BossbarColor::White
                }
            }),
            OwnedArg::BossbarStyle(s) => Arg::BossbarStyle(match s {
                crate::world::bossbar::BossbarDivisions::NoDivision => {
                    papokin::plugin::command::BossbarStyle::NoDivision
                }
                crate::world::bossbar::BossbarDivisions::Notches6 => {
                    papokin::plugin::command::BossbarStyle::Notches6
                }
                crate::world::bossbar::BossbarDivisions::Notches10 => {
                    papokin::plugin::command::BossbarStyle::Notches10
                }
                crate::world::bossbar::BossbarDivisions::Notches12 => {
                    papokin::plugin::command::BossbarStyle::Notches12
                }
                crate::world::bossbar::BossbarDivisions::Notches20 => {
                    papokin::plugin::command::BossbarStyle::Notches20
                }
            }),
            OwnedArg::SoundCategory(s) => Arg::SoundCategory(match s {
                papokin_data::sound::SoundCategory::Master
                | papokin_data::sound::SoundCategory::Ui => {
                    papokin::plugin::command::SoundCategory::Master
                }
                papokin_data::sound::SoundCategory::Music => {
                    papokin::plugin::command::SoundCategory::Music
                }
                papokin_data::sound::SoundCategory::Records => {
                    papokin::plugin::command::SoundCategory::Records
                }
                papokin_data::sound::SoundCategory::Weather => {
                    papokin::plugin::command::SoundCategory::Weather
                }
                papokin_data::sound::SoundCategory::Blocks => {
                    papokin::plugin::command::SoundCategory::Blocks
                }
                papokin_data::sound::SoundCategory::Hostile => {
                    papokin::plugin::command::SoundCategory::Hostile
                }
                papokin_data::sound::SoundCategory::Neutral => {
                    papokin::plugin::command::SoundCategory::Neutral
                }
                papokin_data::sound::SoundCategory::Players => {
                    papokin::plugin::command::SoundCategory::Players
                }
                papokin_data::sound::SoundCategory::Ambient => {
                    papokin::plugin::command::SoundCategory::Ambient
                }
                papokin_data::sound::SoundCategory::Voice => {
                    papokin::plugin::command::SoundCategory::Voice
                }
            }),
            OwnedArg::DamageType(d) => Arg::DamageType(d.message_id.to_string()),
            OwnedArg::Effect(e) => Arg::Effect(e.minecraft_name.to_string()),
            OwnedArg::Enchantment(e) => Arg::Enchantment(e.name.to_string()),
            OwnedArg::Advancement(a) => Arg::Advancement(a.to_string()),
            OwnedArg::EntityAnchor(a) => Arg::EntityAnchor(match a {
                crate::command::argument_types::entity_anchor::EntityAnchor::Eyes => {
                    papokin::plugin::command::EntityAnchor::Eyes
                }
                crate::command::argument_types::entity_anchor::EntityAnchor::Feet => {
                    papokin::plugin::command::EntityAnchor::Feet
                }
            }),
            // 这些类型目前还没有直接的 WIT 资源映射
            OwnedArg::Entities(_) | OwnedArg::Entity(_) | OwnedArg::GameProfiles(_) => {
                Arg::Simple(String::new())
            }
        })
    }

    async fn drop(&mut self, rep: Resource<ConsumedArgs>) -> wasmtime::Result<()> {
        self.resource_table
            .delete::<ConsumedArgsResource>(Resource::new_own(rep.rep()))
            .map_err(wasmtime::Error::from)?;
        Ok(())
    }
}

impl papokin::plugin::command::HostCommand for PluginHostState {
    async fn new(
        &mut self,
        names: Vec<String>,
        description: String,
    ) -> wasmtime::Result<Resource<Command>> {
        self.add_command(WasmCommand::new(names, description))
            .map_err(|_| wasmtime::Error::msg("添加命令资源失败"))
    }

    async fn then(
        &mut self,
        command: Resource<Command>,
        node: Resource<CommandNode>,
    ) -> wasmtime::Result<()> {
        let node_data = self.take_node(&node)?;
        let command_res = self.get_command_mut(&command)?;
        let cmd = std::mem::replace(
            &mut command_res.provider,
            WasmCommand::new(Vec::new(), String::new()),
        );
        command_res.provider = cmd.then(node_data.provider);
        Ok(())
    }

    async fn execute_with_handler_id(
        &mut self,
        command: Resource<Command>,
        handler_id: u32,
    ) -> wasmtime::Result<()> {
        let plugin = self
            .plugin
            .as_ref()
            .and_then(std::sync::Weak::upgrade)
            .ok_or_else(|| wasmtime::Error::msg("插件已被丢弃"))?;
        let server = self
            .server
            .clone()
            .ok_or_else(|| wasmtime::Error::msg("服务器未初始化"))?;

        let executor = WasmCommandExecutor {
            handler_id,
            plugin,
            server,
        };
        let command_res = self.get_command_mut(&command)?;
        let cmd = std::mem::replace(
            &mut command_res.provider,
            WasmCommand::new(Vec::new(), String::new()),
        );
        command_res.provider = cmd.executes(executor);
        Ok(())
    }

    async fn drop(&mut self, rep: Resource<Command>) -> wasmtime::Result<()> {
        self.resource_table
            .delete::<CommandResource>(Resource::new_own(rep.rep()))
            .map_err(wasmtime::Error::from)?;
        Ok(())
    }
}

impl papokin::plugin::command::HostCommandSender for PluginHostState {
    async fn get_command_sender_type(
        &mut self,
        res: Resource<CommandSender>,
    ) -> wasmtime::Result<CommandSenderType> {
        let sender = self.get_sender_res(&res)?.provider.clone();
        match sender {
            crate::command::CommandSender::Rcon(_) => Ok(CommandSenderType::Rcon),
            crate::command::CommandSender::Console => Ok(CommandSenderType::Console),
            crate::command::CommandSender::Player(player) => {
                Ok(CommandSenderType::Player(self.add_player(player)?))
            }
            crate::command::CommandSender::CommandBlock(block_entity, world) => {
                Ok(CommandSenderType::CommandBlock((
                    self.add_block_entity(block_entity)?,
                    self.add_world(world)?,
                )))
            }
            crate::command::CommandSender::Dummy => Ok(CommandSenderType::Dummy),
        }
    }

    async fn get_name(&mut self, sender: Resource<CommandSender>) -> wasmtime::Result<String> {
        Ok(self.get_sender_res(&sender)?.provider.to_string())
    }

    async fn send_message(
        &mut self,
        sender: Resource<CommandSender>,
        text: Resource<TextComponent>,
    ) -> wasmtime::Result<()> {
        let component = self
            .resource_table
            .get::<TextComponentResource>(&Resource::new_own(text.rep()))?
            .provider
            .clone();
        self.get_sender_res(&sender)?
            .provider
            .send_message(component);
        Ok(())
    }

    async fn send_system_message(
        &mut self,
        sender: Resource<CommandSender>,
        text: Resource<TextComponent>,
    ) -> wasmtime::Result<()> {
        let component = self
            .resource_table
            .get::<TextComponentResource>(&Resource::new_own(text.rep()))?
            .provider
            .clone();
        self.get_sender_res(&sender)?
            .provider
            .send_message(component);
        Ok(())
    }

    async fn send_error(
        &mut self,
        sender: Resource<CommandSender>,
        text: Resource<TextComponent>,
    ) -> wasmtime::Result<()> {
        let component = self
            .resource_table
            .get::<TextComponentResource>(&Resource::new_own(text.rep()))?
            .provider
            .clone();
        self.get_sender_res(&sender)?
            .provider
            .send_message(component.color(papokin_util::text::color::Color::Named(
                papokin_util::text::color::NamedColor::Red,
            )));
        Ok(())
    }

    async fn set_success_count(
        &mut self,
        sender: Resource<CommandSender>,
        count: i32,
    ) -> wasmtime::Result<()> {
        self.get_sender_mut(&sender)?
            .provider
            .set_success_count(count as u32);
        Ok(())
    }

    async fn is_player(&mut self, sender: Resource<CommandSender>) -> wasmtime::Result<bool> {
        Ok(matches!(
            self.get_sender_res(&sender)?.provider,
            crate::command::CommandSender::Player(_)
        ))
    }

    async fn is_console(&mut self, sender: Resource<CommandSender>) -> wasmtime::Result<bool> {
        Ok(matches!(
            self.get_sender_res(&sender)?.provider,
            crate::command::CommandSender::Console | crate::command::CommandSender::Rcon(_)
        ))
    }

    async fn as_player(
        &mut self,
        sender: Resource<CommandSender>,
    ) -> wasmtime::Result<Option<Resource<Player>>> {
        if let crate::command::CommandSender::Player(player) =
            &self.get_sender_res(&sender)?.provider
        {
            Ok(Some(
                self.add_player(player.clone())
                    .map_err(|_| wasmtime::Error::msg("添加玩家资源失败"))?,
            ))
        } else {
            Ok(None)
        }
    }

    async fn permission_level(
        &mut self,
        sender: Resource<CommandSender>,
    ) -> wasmtime::Result<PermissionLevel> {
        Ok(
            match self.get_sender_res(&sender)?.provider.permission_lvl() {
                papokin_util::PermissionLvl::Zero => PermissionLevel::Zero,
                papokin_util::PermissionLvl::One => PermissionLevel::One,
                papokin_util::PermissionLvl::Two => PermissionLevel::Two,
                papokin_util::PermissionLvl::Three => PermissionLevel::Three,
                papokin_util::PermissionLvl::Four => PermissionLevel::Four,
            },
        )
    }

    async fn has_permission_level(
        &mut self,
        sender: Resource<CommandSender>,
        level: PermissionLevel,
    ) -> wasmtime::Result<bool> {
        let required = match level {
            PermissionLevel::Zero => papokin_util::PermissionLvl::Zero,
            PermissionLevel::One => papokin_util::PermissionLvl::One,
            PermissionLevel::Two => papokin_util::PermissionLvl::Two,
            PermissionLevel::Three => papokin_util::PermissionLvl::Three,
            PermissionLevel::Four => papokin_util::PermissionLvl::Four,
        };
        Ok(self.get_sender_res(&sender)?.provider.permission_lvl() >= required)
    }

    async fn position(
        &mut self,
        sender: Resource<CommandSender>,
    ) -> wasmtime::Result<Option<Position>> {
        Ok(self
            .get_sender_res(&sender)?
            .provider
            .position()
            .map(|p| (p.x, p.y, p.z)))
    }

    async fn world(
        &mut self,
        sender: Resource<CommandSender>,
    ) -> wasmtime::Result<Option<Resource<World>>> {
        if let Some(world) = self.get_sender_res(&sender)?.provider.world() {
            Ok(Some(
                self.add_world(world)
                    .map_err(|_| wasmtime::Error::msg("添加世界资源失败"))?,
            ))
        } else {
            Ok(None)
        }
    }

    async fn get_locale(&mut self, sender: Resource<CommandSender>) -> wasmtime::Result<Locale> {
        Ok(map_util_locale_to_wit(
            self.get_sender_res(&sender)?.provider.get_locale(),
        ))
    }

    async fn should_receive_feedback(
        &mut self,
        sender: Resource<CommandSender>,
    ) -> wasmtime::Result<bool> {
        Ok(self
            .get_sender_res(&sender)?
            .provider
            .should_receive_feedback())
    }

    async fn should_broadcast_console_to_ops(
        &mut self,
        sender: Resource<CommandSender>,
    ) -> wasmtime::Result<bool> {
        Ok(self
            .get_sender_res(&sender)?
            .provider
            .should_broadcast_console_to_ops())
    }

    async fn should_track_output(
        &mut self,
        sender: Resource<CommandSender>,
    ) -> wasmtime::Result<bool> {
        Ok(self.get_sender_res(&sender)?.provider.should_track_output())
    }

    async fn drop(&mut self, rep: Resource<CommandSender>) -> wasmtime::Result<()> {
        self.resource_table
            .delete::<CommandSenderResource>(Resource::new_own(rep.rep()))
            .map_err(wasmtime::Error::from)?;
        Ok(())
    }
}

impl papokin::plugin::command::HostCommandSenderWithStore<PluginHostState>
    for HasSelf<PluginHostState>
{
    async fn has_permission(
        mut host: Access<'_, PluginHostState, Self>,
        sender: Resource<CommandSender>,
        server: Resource<Server>,
        node: String,
    ) -> wasmtime::Result<bool> {
        let (sender, server, plugin) = {
            let state = host.get();
            let sender = state.get_sender_res(&sender)?.provider.clone();
            let server = state
                .resource_table
                .get::<ServerResource>(&Resource::new_own(server.rep()))?
                .provider
                .clone();
            let plugin = state
                .plugin
                .as_ref()
                .and_then(std::sync::Weak::upgrade)
                .ok_or_else(|| wasmtime::Error::msg("插件实例不可用"))?;
            (sender, server, plugin)
        };

        plugin
            .store
            .pump_blocking(&mut host, move || sender.has_permission(&server, &node))
            .await
    }
}

impl papokin::plugin::command::HostCommandNode for PluginHostState {
    async fn literal(&mut self, name: String) -> wasmtime::Result<Resource<CommandNode>> {
        self.add_command_node(WasmCommandNode::Literal(literal(name)))
            .map_err(|_| wasmtime::Error::msg("添加字面量节点失败"))
    }

    async fn argument(
        &mut self,
        name: String,
        arg_type: ArgumentType,
    ) -> wasmtime::Result<Resource<CommandNode>> {
        let node = match arg_type {
            ArgumentType::Bool => WasmCommandNode::Argument(argument(name, BoolArgumentType)),
            ArgumentType::Float((min, max)) => WasmCommandNode::Argument(argument(
                name,
                FloatArgumentType::new(min.unwrap_or(f32::MIN), max.unwrap_or(f32::MAX)),
            )),
            ArgumentType::Double((min, max)) => WasmCommandNode::Argument(argument(
                name,
                DoubleArgumentType::new(min.unwrap_or(f64::MIN), max.unwrap_or(f64::MAX)),
            )),
            ArgumentType::Integer((min, max)) => WasmCommandNode::Argument(argument(
                name,
                IntegerArgumentType::new(min.unwrap_or(i32::MIN), max.unwrap_or(i32::MAX)),
            )),
            ArgumentType::Long((min, max)) => WasmCommandNode::Argument(argument(
                name,
                LongArgumentType::new(min.unwrap_or(i64::MIN), max.unwrap_or(i64::MAX)),
            )),
            ArgumentType::String(st) => match st {
                StringType::SingleWord => {
                    WasmCommandNode::Argument(argument(name, StringArgumentType::SingleWord))
                }
                StringType::Quotable => {
                    WasmCommandNode::Argument(argument(name, StringArgumentType::QuotablePhrase))
                }
                StringType::Greedy => {
                    WasmCommandNode::Argument(argument(name, StringArgumentType::GreedyPhrase))
                }
            },
            ArgumentType::Entities => {
                WasmCommandNode::Argument(argument(name, EntityArgumentType::Entities))
            }
            ArgumentType::Entity => {
                WasmCommandNode::Argument(argument(name, EntityArgumentType::Entity))
            }
            ArgumentType::Players => {
                WasmCommandNode::Argument(argument(name, EntityArgumentType::Players))
            }
            ArgumentType::GameProfile => {
                WasmCommandNode::Argument(argument(name, GameProfileArgumentType))
            }
            ArgumentType::BlockPos => {
                WasmCommandNode::Argument(argument(name, BlockPosArgumentType))
            }
            ArgumentType::Position3d => {
                WasmCommandNode::Argument(argument(name, Vec3ArgumentType::Default))
            }
            ArgumentType::Position2d => {
                WasmCommandNode::Argument(argument(name, Vec2ArgumentType::Default))
            }
            ArgumentType::BlockState => {
                WasmCommandNode::Argument(argument(name, BlockArgumentType))
            }
            ArgumentType::BlockPredicate => {
                WasmCommandNode::Argument(argument(name, BlockPredicateArgumentType))
            }
            ArgumentType::Item => WasmCommandNode::Argument(argument(name, ItemStackArgumentType)),
            ArgumentType::ItemPredicate => {
                WasmCommandNode::Argument(argument(name, ItemPredicateArgumentType))
            }
            ArgumentType::Component => {
                WasmCommandNode::Argument(argument(name, ComponentArgumentType))
            }
            ArgumentType::Rotation => {
                WasmCommandNode::Argument(argument(name, RotationArgumentType))
            }
            ArgumentType::ResourceLocation | ArgumentType::Resource(_) => {
                WasmCommandNode::Argument(argument(name, IdentifierArgumentType))
            }
            ArgumentType::EntityAnchor => {
                WasmCommandNode::Argument(argument(name, EntityAnchorArgumentType))
            }
            ArgumentType::Gamemode => {
                WasmCommandNode::Argument(argument(name, GameModeArgumentType))
            }
            ArgumentType::Difficulty => {
                WasmCommandNode::Argument(argument(name, StringArgumentType::SingleWord))
            }
            ArgumentType::Time(min) => {
                WasmCommandNode::Argument(argument(name, TimeArgumentType::new(min.unwrap_or(0))))
            }
            _ => {
                return Err(wasmtime::Error::msg(format!(
                    "未实现的参数类型：{arg_type:?}"
                )));
            }
        };
        self.add_command_node(node)
            .map_err(|_| wasmtime::Error::msg("添加参数节点失败"))
    }

    async fn then(
        &mut self,
        self_node: Resource<CommandNode>,
        node: Resource<CommandNode>,
    ) -> wasmtime::Result<()> {
        let child = self.take_node(&node)?;
        let parent = self.get_node_mut(&self_node)?;
        let builder =
            std::mem::replace(&mut parent.provider, WasmCommandNode::Literal(literal("")));
        parent.provider = builder.then(child.provider);
        Ok(())
    }

    async fn execute_with_handler_id(
        &mut self,
        node: Resource<CommandNode>,
        handler_id: u32,
    ) -> wasmtime::Result<()> {
        let plugin = self
            .plugin
            .as_ref()
            .and_then(std::sync::Weak::upgrade)
            .ok_or_else(|| wasmtime::Error::msg("插件已被丢弃"))?;
        let server = self
            .server
            .clone()
            .ok_or_else(|| wasmtime::Error::msg("服务器未初始化"))?;

        let executor = WasmCommandExecutor {
            handler_id,
            plugin,
            server,
        };
        let resource = self.get_node_mut(&node)?;
        let builder = std::mem::replace(
            &mut resource.provider,
            WasmCommandNode::Literal(literal("")),
        );
        resource.provider = builder.executes(executor);
        Ok(())
    }

    async fn suggest_with_handler_id(
        &mut self,
        node: Resource<CommandNode>,
        handler_id: u32,
    ) -> wasmtime::Result<()> {
        let plugin = self
            .plugin
            .as_ref()
            .and_then(std::sync::Weak::upgrade)
            .ok_or_else(|| wasmtime::Error::msg("插件已被丢弃"))?;
        let server = self
            .server
            .clone()
            .ok_or_else(|| wasmtime::Error::msg("服务器未初始化"))?;

        let provider = WasmCommandSuggestionProvider {
            handler_id,
            plugin,
            server,
        };
        let resource = self.get_node_mut(&node)?;
        let builder = std::mem::replace(
            &mut resource.provider,
            WasmCommandNode::Literal(literal("")),
        );
        resource.provider = builder.suggests(provider);
        Ok(())
    }

    async fn require_with_handler_id(
        &mut self,
        _node: Resource<CommandNode>,
        _handler_id: u32,
    ) -> wasmtime::Result<()> {
        Err(wasmtime::Error::msg("require_with_handler_id 未实现"))
    }

    async fn drop(&mut self, rep: Resource<CommandNode>) -> wasmtime::Result<()> {
        self.resource_table
            .delete::<CommandNodeResource>(Resource::new_own(rep.rep()))
            .map_err(wasmtime::Error::from)?;
        Ok(())
    }
}

#[expect(clippy::too_many_lines)]
const fn map_util_locale_to_wit(locale: papokin_util::translation::Locale) -> Locale {
    match locale {
        papokin_util::translation::Locale::AfZa => Locale::AfZa,
        papokin_util::translation::Locale::ArSa => Locale::ArSa,
        papokin_util::translation::Locale::AstEs => Locale::AstEs,
        papokin_util::translation::Locale::AzAz => Locale::AzAz,
        papokin_util::translation::Locale::BaRu => Locale::BaRu,
        papokin_util::translation::Locale::Bar => Locale::Bar,
        papokin_util::translation::Locale::BeBy => Locale::BeBy,
        papokin_util::translation::Locale::BgBg => Locale::BgBg,
        papokin_util::translation::Locale::BrFr => Locale::BrFr,
        papokin_util::translation::Locale::Brb => Locale::Brb,
        papokin_util::translation::Locale::BsBa => Locale::BsBa,
        papokin_util::translation::Locale::CaEs => Locale::CaEs,
        papokin_util::translation::Locale::CsCz => Locale::CsCz,
        papokin_util::translation::Locale::CyGb => Locale::CyGb,
        papokin_util::translation::Locale::DaDk => Locale::DaDk,
        papokin_util::translation::Locale::DeAt => Locale::DeAt,
        papokin_util::translation::Locale::DeCh => Locale::DeCh,
        papokin_util::translation::Locale::DeDe => Locale::DeDe,
        papokin_util::translation::Locale::ElGr => Locale::ElGr,
        papokin_util::translation::Locale::EnAu => Locale::EnAu,
        papokin_util::translation::Locale::EnCa => Locale::EnCa,
        papokin_util::translation::Locale::EnGb => Locale::EnGb,
        papokin_util::translation::Locale::EnNz => Locale::EnNz,
        papokin_util::translation::Locale::EnPt => Locale::EnPt,
        papokin_util::translation::Locale::EnUd => Locale::EnUd,
        papokin_util::translation::Locale::EnUs => Locale::EnUs,
        papokin_util::translation::Locale::Enp => Locale::Enp,
        papokin_util::translation::Locale::Enws => Locale::Enws,
        papokin_util::translation::Locale::EoUy => Locale::EoUy,
        papokin_util::translation::Locale::EsAr => Locale::EsAr,
        papokin_util::translation::Locale::EsCl => Locale::EsCl,
        papokin_util::translation::Locale::EsEc => Locale::EsEc,
        papokin_util::translation::Locale::EsEs => Locale::EsEs,
        papokin_util::translation::Locale::EsMx => Locale::EsMx,
        papokin_util::translation::Locale::EsUy => Locale::EsUy,
        papokin_util::translation::Locale::EsVe => Locale::EsVe,
        papokin_util::translation::Locale::Esan => Locale::Esan,
        papokin_util::translation::Locale::EtEe => Locale::EtEe,
        papokin_util::translation::Locale::EuEs => Locale::EuEs,
        papokin_util::translation::Locale::FaIr => Locale::FaIr,
        papokin_util::translation::Locale::FiFi => Locale::FiFi,
        papokin_util::translation::Locale::FilPh => Locale::FilPh,
        papokin_util::translation::Locale::FoFo => Locale::FoFo,
        papokin_util::translation::Locale::FrCa => Locale::FrCa,
        papokin_util::translation::Locale::FrFr => Locale::FrFr,
        papokin_util::translation::Locale::FraDe => Locale::FraDe,
        papokin_util::translation::Locale::FurIt => Locale::FurIt,
        papokin_util::translation::Locale::FyNl => Locale::FyNl,
        papokin_util::translation::Locale::GaIe => Locale::GaIe,
        papokin_util::translation::Locale::GdGb => Locale::GdGb,
        papokin_util::translation::Locale::GlEs => Locale::GlEs,
        papokin_util::translation::Locale::HawUs => Locale::HawUs,
        papokin_util::translation::Locale::HeIl => Locale::HeIl,
        papokin_util::translation::Locale::HiIn => Locale::HiIn,
        papokin_util::translation::Locale::HrHr => Locale::HrHr,
        papokin_util::translation::Locale::HuHu => Locale::HuHu,
        papokin_util::translation::Locale::HyAm => Locale::HyAm,
        papokin_util::translation::Locale::IdId => Locale::IdId,
        papokin_util::translation::Locale::IgNg => Locale::IgNg,
        papokin_util::translation::Locale::IoEn => Locale::IoEn,
        papokin_util::translation::Locale::IsIs => Locale::IsIs,
        papokin_util::translation::Locale::Isv => Locale::Isv,
        papokin_util::translation::Locale::ItIt => Locale::ItIt,
        papokin_util::translation::Locale::JaJp => Locale::JaJp,
        papokin_util::translation::Locale::JboEn => Locale::JboEn,
        papokin_util::translation::Locale::KaGe => Locale::KaGe,
        papokin_util::translation::Locale::KkKz => Locale::KkKz,
        papokin_util::translation::Locale::KnIn => Locale::KnIn,
        papokin_util::translation::Locale::KoKr => Locale::KoKr,
        papokin_util::translation::Locale::Ksh => Locale::Ksh,
        papokin_util::translation::Locale::KwGb => Locale::KwGb,
        papokin_util::translation::Locale::LaLa => Locale::LaLa,
        papokin_util::translation::Locale::LbLu => Locale::LbLu,
        papokin_util::translation::Locale::LiLi => Locale::LiLi,
        papokin_util::translation::Locale::Lmo => Locale::Lmo,
        papokin_util::translation::Locale::LoLa => Locale::LoLa,
        papokin_util::translation::Locale::LolUs => Locale::LolUs,
        papokin_util::translation::Locale::LtLt => Locale::LtLt,
        papokin_util::translation::Locale::LvLv => Locale::LvLv,
        papokin_util::translation::Locale::Lzh => Locale::Lzh,
        papokin_util::translation::Locale::MkMk => Locale::MkMk,
        papokin_util::translation::Locale::MnMn => Locale::MnMn,
        papokin_util::translation::Locale::MsMy => Locale::MsMy,
        papokin_util::translation::Locale::MtMt => Locale::MtMt,
        papokin_util::translation::Locale::Nah => Locale::Nah,
        papokin_util::translation::Locale::NdsDe => Locale::NdsDe,
        papokin_util::translation::Locale::NlBe => Locale::NlBe,
        papokin_util::translation::Locale::NlNl => Locale::NlNl,
        papokin_util::translation::Locale::NnNo => Locale::NnNo,
        papokin_util::translation::Locale::NoNo => Locale::NoNo,
        papokin_util::translation::Locale::OcFr => Locale::OcFr,
        papokin_util::translation::Locale::Ovd => Locale::Ovd,
        papokin_util::translation::Locale::PlPl => Locale::PlPl,
        papokin_util::translation::Locale::PtBr => Locale::PtBr,
        papokin_util::translation::Locale::PtPt => Locale::PtPt,
        papokin_util::translation::Locale::QyaAa => Locale::QyaAa,
        papokin_util::translation::Locale::RoRo => Locale::RoRo,
        papokin_util::translation::Locale::Rpr => Locale::Rpr,
        papokin_util::translation::Locale::RuRu => Locale::RuRu,
        papokin_util::translation::Locale::RyUa => Locale::RyUa,
        papokin_util::translation::Locale::SahSah => Locale::SahSah,
        papokin_util::translation::Locale::SeNo => Locale::SeNo,
        papokin_util::translation::Locale::SkSk => Locale::SkSk,
        papokin_util::translation::Locale::SlSi => Locale::SlSi,
        papokin_util::translation::Locale::SoSo => Locale::SoSo,
        papokin_util::translation::Locale::SqAl => Locale::SqAl,
        papokin_util::translation::Locale::SrCs => Locale::SrCs,
        papokin_util::translation::Locale::SrSp => Locale::SrSp,
        papokin_util::translation::Locale::SvSe => Locale::SvSe,
        papokin_util::translation::Locale::Sxu => Locale::Sxu,
        papokin_util::translation::Locale::Szl => Locale::Szl,
        papokin_util::translation::Locale::TaIn => Locale::TaIn,
        papokin_util::translation::Locale::ThTh => Locale::ThTh,
        papokin_util::translation::Locale::TlPh => Locale::TlPh,
        papokin_util::translation::Locale::TlhAa => Locale::TlhAa,
        papokin_util::translation::Locale::Tok => Locale::Tok,
        papokin_util::translation::Locale::TrTr => Locale::TrTr,
        papokin_util::translation::Locale::TtRu => Locale::TtRu,
        papokin_util::translation::Locale::UkUa => Locale::UkUa,
        papokin_util::translation::Locale::ValEs => Locale::ValEs,
        papokin_util::translation::Locale::VecIt => Locale::VecIt,
        papokin_util::translation::Locale::ViVn => Locale::ViVn,
        papokin_util::translation::Locale::YiDe => Locale::YiDe,
        papokin_util::translation::Locale::YoNg => Locale::YoNg,
        papokin_util::translation::Locale::ZhCn => Locale::ZhCn,
        papokin_util::translation::Locale::ZhHk => Locale::ZhHk,
        papokin_util::translation::Locale::ZhTw => Locale::ZhTw,
        papokin_util::translation::Locale::ZlmArab => Locale::ZlmArab,
    }
}
