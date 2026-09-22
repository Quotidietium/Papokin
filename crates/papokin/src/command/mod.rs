#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::block::entities::BlockEntity;
use crate::block::entities::command_block::CommandBlockEntity;
pub use crate::command::context::command_source::CommandSource;
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::server::Server;
use crate::world::World;
use papokin_data::{
    Block,
    block_properties::{BlockProperties, CommandBlockLikeProperties, Facing},
    dimension::Dimension,
};
use papokin_util::math::vector2::Vector2;
use papokin_util::math::vector3::Vector3;
use papokin_util::permission::{PermissionDefault, PermissionLvl};
use papokin_util::text::TextComponent;
use papokin_util::translation::Locale;

pub use papokin_command::*;

pub mod argument_builder;
pub mod argument_types;
pub mod client_suggestions;
pub mod commands;
pub mod context;
pub mod node;

pub mod dispatcher {
    pub use papokin_command::dispatcher::*;
    pub type CommandDispatcher =
        papokin_command::dispatcher::CommandDispatcher<crate::command::CommandSource>;
}
pub mod errors {
    pub use papokin_command::errors::*;
}
pub mod parser {
    pub use papokin_command::parser::*;
}
pub mod snbt {
    pub use papokin_command::snbt::*;
}
pub mod string_reader {
    pub use papokin_command::string_reader::*;
}
pub mod suggestion {
    pub use papokin_command::suggestion::*;

    pub mod provider {
        use crate::command::context::command_context::CommandContext;
        use crate::command::context::command_source::CommandSource;
        use papokin_command::suggestion::suggestions::{Suggestions, SuggestionsBuilder};

        pub type SuggestionProviderResult = Suggestions;

        pub trait SuggestionProvider: Send + Sync {
            fn suggest(
                &self,
                context: &CommandContext,
                builder: SuggestionsBuilder,
            ) -> SuggestionProviderResult;
        }

        pub struct SuggestionProviderAdapter<T>(pub T);

        impl<T: SuggestionProvider>
            papokin_command::suggestion::provider::SuggestionProvider<CommandSource>
            for SuggestionProviderAdapter<T>
        {
            fn suggest(
                &self,
                context: &papokin_command::context::command_context::CommandContext<
                    '_,
                    CommandSource,
                >,
                builder: SuggestionsBuilder,
            ) -> SuggestionProviderResult {
                self.0.suggest(context, builder)
            }
        }
    }
}

/// 控制台和 RCON 命令输出是否广播给在线管理员。
///
/// 在服务器启动期间根据 [`CommandsConfig::broadcast_console_to_ops`] 设置。
/// 为兼容原版，默认为 `true`。
static BROADCAST_CONSOLE_TO_OPS: AtomicBool = AtomicBool::new(true);

/// 从服务器配置初始化控制台广播设置。
///
/// 在服务器启动期间调用一次。后续调用会被忽略。
pub fn set_broadcast_console_to_ops(value: bool) {
    BROADCAST_CONSOLE_TO_OPS.store(value, std::sync::atomic::Ordering::Relaxed);
}

/// 表示命令执行的来源。
///
/// 不同的发送者拥有不同的权限、输出目标以及
/// 世界中的位置。此枚举对这些差异进行了抽象，供
/// 命令调度器。
#[derive(Clone)]
pub enum CommandSender {
    /// 通过 RCON 协议进行的远程控制台连接。
    ///
    /// 存储用于捕获命令输出的缓冲区
    /// 以便通过网络回传给 RCON 客户端。
    Rcon(Arc<std::sync::Mutex<Vec<String>>>),
    /// 本地服务器终端/控制台。
    ///
    /// 此命令发送者通常拥有绝对权限（绕过检查），并且
    /// 直接输出到服务器日志。
    Console,
    /// 当前已连接到服务器的玩家。
    ///
    /// 包含对 [Player] 结构体的引用，用于访问玩家的
    /// 位置、权限和会话。
    Player(Arc<Player>),
    /// 命令方块或命令方块矿车。
    ///
    /// 包含负责该命令的方块实体以及
    /// 它所处的世界上下文，用于坐标相对执行（例如 `~ ~ ~`）。
    CommandBlock(Arc<CommandBlockEntity>, Arc<World>),
    /// 虚无。发送到此发送者的任何内容都会被丢弃。
    /// 拥有与 `CommandBlock` 相同的权限。
    Dummy,
}

impl fmt::Display for CommandSender {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Console => "Server",
                Self::Rcon(_) => "Rcon",
                Self::Player(p) => &p.gameprofile.name,
                Self::CommandBlock(..) => "@",
                Self::Dummy => "",
            }
        )
    }
}

impl CommandSender {
    pub fn send_message(&self, text: TextComponent) {
        match self {
            #[allow(clippy::print_stdout)]
            Self::Console => println!("{}", text.to_pretty_console()),
            Self::Player(c) => c.send_system_message(&text),
            Self::Rcon(s) => s
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(text.to_pretty_console()),
            Self::CommandBlock(block_entity, _) => {
                let mut last_output = block_entity
                    .last_output
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);

                let now = time::OffsetDateTime::now_utc();
                let format = time::macros::format_description!("[hour]:[minute]:[second]");
                let timestamp = now
                    .format(&format)
                    .unwrap_or_else(|_| "00:00:00".to_string());

                *last_output = format!("[{}] {}", timestamp, text.get_text());
            }
            Self::Dummy => {}
        }
    }

    pub fn set_success_count(&self, count: u32) {
        if let Self::CommandBlock(c, _) = self {
            c.success_count
                .store(count, std::sync::atomic::Ordering::SeqCst);
        }
    }

    #[must_use]
    pub const fn is_player(&self) -> bool {
        matches!(self, Self::Player(_))
    }

    #[must_use]
    pub const fn is_console(&self) -> bool {
        matches!(self, Self::Console)
    }
    #[must_use]
    pub fn as_player(&self) -> Option<Arc<Player>> {
        match self {
            Self::Player(player) => Some(player.clone()),
            _ => None,
        }
    }

    /// 建议使用 `has_permission_lvl(lvl)`
    #[must_use]
    pub fn permission_lvl(&self) -> PermissionLvl {
        match self {
            Self::Console | Self::Rcon(_) => PermissionLvl::Four,
            Self::Player(p) => p.permission_lvl.load(),
            Self::CommandBlock(..) | Self::Dummy => PermissionLvl::Two,
        }
    }

    #[must_use]
    pub fn has_permission_lvl(&self, lvl: PermissionLvl) -> bool {
        match self {
            Self::Console | Self::Rcon(_) => true,
            Self::Player(p) => p.permission_lvl.load().ge(&lvl),
            Self::CommandBlock(..) | Self::Dummy => PermissionLvl::Two >= lvl,
        }
    }

    /// 检查发送者是否拥有特定权限
    pub fn has_permission(&self, server: &Server, node: &str) -> bool {
        match self {
            Self::Console | Self::Rcon(_) => true, // 控制台和 RCON 始终拥有全部权限
            Self::Player(p) => p.has_permission(server, node),
            Self::CommandBlock(..) | Self::Dummy => {
                let Some(p) = server.permission_manager.get_permission(node) else {
                    return false;
                };
                match p.default {
                    PermissionDefault::Allow => true,
                    PermissionDefault::Deny => false,
                    PermissionDefault::Op(o) => o <= PermissionLvl::Two,
                }
            }
        }
    }

    #[must_use]
    pub fn position(&self) -> Option<Vector3<f64>> {
        match self {
            Self::Console | Self::Rcon(..) | Self::Dummy => None,
            Self::Player(p) => Some(p.living_entity.entity.pos.load()),
            Self::CommandBlock(c, _) => Some(c.get_position().to_centered_f64()),
        }
    }

    #[must_use]
    pub fn rotation(&self) -> Option<(f32, f32)> {
        match self {
            Self::Console | Self::Rcon(..) | Self::Dummy => None,
            Self::Player(player) => Some(player.rotation()),
            Self::CommandBlock(command_block, world) => {
                let pos = command_block.get_position();
                let (chunk_coordinate, relative) = pos.chunk_and_chunk_relative_position();
                let state_id = world.level.read_chunk_sync(&chunk_coordinate, |chunk| {
                    chunk.section.get_block_absolute_y(
                        relative.x as usize,
                        relative.y,
                        relative.z as usize,
                    )
                })??;
                let block = Block::from_state_id(state_id);
                if !CommandBlockLikeProperties::handles_block_id(block.id) {
                    return None;
                }

                let props = CommandBlockLikeProperties::from_state_id(state_id);
                Some((0.0, command_block_y_rot(props.facing)))
            }
        }
    }

    #[must_use]
    pub fn world(&self) -> Option<Arc<World>> {
        match self {
            // 这些发送器未绑定到某个世界。使用 `world_or_first` 来
            // 改为回退到第一个世界。
            Self::Console | Self::Rcon(..) | Self::Dummy => None,
            Self::Player(p) => Some(p.living_entity.entity.world.load_full()),
            Self::CommandBlock(_, w) => Some(w.clone()),
        }
    }

    /// 返回此发送者所在的世界，若无则回退到服务器的
    /// 对未绑定世界的发送方使用第一个世界。
    ///
    /// 控制台、RCON 和虚拟发送者没有自己的世界，因此与
    /// 原版中它们操作第一个（主世界）世界。返回 [`None`]
    /// 仅当服务器完全没有加载任何世界时。
    #[must_use]
    pub fn world_or_first(&self, server: &Server) -> Option<Arc<World>> {
        self.world()
            .or_else(|| server.worlds.load().first().cloned())
    }

    #[must_use]
    pub fn get_locale(&self) -> Locale {
        match self {
            Self::CommandBlock(..) | Self::Console | Self::Rcon(..) | Self::Dummy => Locale::EnUs, // 控制台和 RCON 的默认区域设置
            Self::Player(player) => {
                Locale::from_str(&player.config.load().locale).unwrap_or(Locale::EnUs)
            }
        }
    }

    #[must_use]
    pub fn should_receive_feedback(&self) -> bool {
        match self {
            Self::CommandBlock(_, world) => {
                world.level_info.load().game_rules.send_command_feedback
            }
            Self::Player(player) => {
                player
                    .world()
                    .level_info
                    .load()
                    .game_rules
                    .send_command_feedback
            }
            Self::Console | Self::Rcon(_) => true,
            Self::Dummy => false,
        }
    }

    #[must_use]
    pub fn should_broadcast_console_to_ops(&self) -> bool {
        match self {
            Self::CommandBlock(_, world) => world.level_info.load().game_rules.command_block_output,
            Self::Player(..) => true,
            Self::Console | Self::Rcon(_) => {
                BROADCAST_CONSOLE_TO_OPS.load(std::sync::atomic::Ordering::Relaxed)
            }
            Self::Dummy => false,
        }
    }

    #[must_use]
    pub const fn should_track_output(&self) -> bool {
        match self {
            Self::Dummy => false,
            Self::Player(..) | Self::Console | Self::Rcon(_) | Self::CommandBlock(..) => true,
        }
    }

    #[must_use]
    pub fn into_source(self, server: &Arc<Server>) -> CommandSource {
        match self {
            Self::Rcon(rcon) => {
                let (world, spawn_point) = Self::get_world_and_spawn_point(server);
                CommandSource::new(
                    Self::Rcon(rcon),
                    world,
                    None,
                    spawn_point,
                    Vector2::new(0.0, 0.0),
                    "Rcon".to_owned(),
                    TextComponent::text("Rcon"),
                    server.clone(),
                )
            }
            Self::Console => {
                let (world, spawn_point) = Self::get_world_and_spawn_point(server);
                CommandSource::new(
                    Self::Console,
                    world,
                    None,
                    spawn_point,
                    Vector2::new(0.0, 0.0),
                    "服务器".to_owned(),
                    TextComponent::text("服务器"),
                    server.clone(),
                )
            }
            Self::Player(player) => CommandSource::new(
                Self::Player(player.clone()),
                player.world(),
                Some(player.clone()),
                player.position(),
                player.rotation().into(),
                player.get_display_name().get_text(),
                player.get_display_name(),
                server.clone(),
            ),
            Self::CommandBlock(command_entity, world) => {
                let pos = command_entity.position;

                let (_block, state_id) = world.get_block_and_state_id(&pos);
                let command_block_props = CommandBlockLikeProperties::from_state_id(state_id);
                let facing = command_block_props.facing;

                let horizontal_direction = match facing {
                    Facing::South => 0.0,
                    Facing::West => 90.0,
                    Facing::North => 180.0,
                    Facing::Up | Facing::Down | Facing::East => 270.0,
                };

                // TODO: 命令方块支持自定义名称后，为其添加检查
                let name = TextComponent::text("@");

                CommandSource::new(
                    Self::CommandBlock(command_entity, world.clone()),
                    world,
                    None,
                    pos.to_centered_f64(),
                    Vector2::new(0.0, horizontal_direction),
                    name.clone().get_text(),
                    name,
                    server.clone(),
                )
            }
            Self::Dummy => {
                let (world, spawn_point) = Self::get_world_and_spawn_point(server);
                CommandSource::new(
                    Self::Dummy,
                    world,
                    None,
                    spawn_point,
                    Vector2::new(0.0, 0.0),
                    String::new(),
                    TextComponent::empty(),
                    server.clone(),
                )
            }
        }
    }

    fn get_world_and_spawn_point(server: &Arc<Server>) -> (Arc<World>, Vector3<f64>) {
        let world = server.get_world_from_dimension(&Dimension::OVERWORLD);
        let spawn_point = {
            let level_data = world.level_info.load();

            Vector3::new(level_data.spawn_x, level_data.spawn_y, level_data.spawn_z)
        };

        (world, spawn_point.to_f64())
    }
}

const fn command_block_y_rot(facing: Facing) -> f32 {
    match facing {
        Facing::North => 180.0,
        Facing::South => 0.0,
        Facing::West => 90.0,
        Facing::East | Facing::Up | Facing::Down => 270.0,
    }
}

pub use context::command_context::CommandContext;
pub use node::dispatcher::CommandDispatcher;
pub use node::{Command, CommandExecutor, CommandExecutorResult};
