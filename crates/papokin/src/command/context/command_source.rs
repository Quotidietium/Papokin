use crate::command::CommandSender;
use crate::command::argument_types::entity_anchor::{EntityAnchor, EntityAnchorExt};
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::server::Server;
use crate::world::World;
use papokin_command::errors::command_syntax_error::CommandSyntaxError;
use papokin_command::errors::error_types::CommandErrorType;
pub use papokin_command::source::{
    ResultValueTaker, ReturnValue, ReturnValueCallable, ReturnValueCallback,
};
use papokin_data::translation;
use papokin_util::math::position::BlockPos;
use papokin_util::math::vector2::Vector2;
use papokin_util::math::vector3::Vector3;
use papokin_util::math::wrap_degrees;
use papokin_util::text::TextComponent;
use papokin_util::text::color::{Color, NamedColor};
use std::sync::Arc;

pub const REQUIRES_PLAYER: CommandErrorType<0> =
    CommandErrorType::new(translation::java::PERMISSIONS_REQUIRES_PLAYER);
pub const REQUIRES_ENTITY: CommandErrorType<0> =
    CommandErrorType::new(translation::java::PERMISSIONS_REQUIRES_ENTITY);

/// 表示一条命令的来源，其中包含自身的状态，可记录：
/// - 位置
/// - 朝向
/// - 世界
/// - 权限
/// - 名称
/// - 显示名称
/// - 内部服务器
/// - 是否静默
/// - 可能代表的实体
///
/// 不要与 [`CommandSender`] 混淆：[`CommandSource`] 可以被命令修改，
/// 从而改变命令链中后续命令的行为。
///
/// 来源带有玩家和某个位置，并不意味着该玩家真的位于该位置；
/// 它只是为更复杂的功能提供的状态。
#[derive(Clone)]
pub struct CommandSource {
    pub output: CommandSender,
    pub world: Option<Arc<World>>,
    pub entity: Option<Arc<dyn EntityBase>>,
    pub position: Vector3<f64>,
    pub rotation: Vector2<f32>,
    pub name: String,
    pub display_name: TextComponent,
    pub server: Option<Arc<Server>>,
    pub silent: bool,
    pub command_result_taker: ResultValueTaker,
    pub entity_anchor: EntityAnchor,
}

impl CommandSource {
    /// 创建一个空的 [`CommandSource`]，适用于单元测试。
    ///
    /// # Note
    /// **只应用于单元测试！！！**
    ///
    /// 返回的 [`CommandSource`] 不包含服务器和世界。
    /// 若尝试从该来源获取服务器或世界，会发生 panic！
    #[must_use]
    // 测试中用到了它
    #[allow(dead_code)]
    pub(crate) fn dummy() -> Self {
        Self {
            output: CommandSender::Dummy,
            world: None,
            entity: None,
            position: Vector3::default(),
            rotation: Vector2::default(),
            name: String::new(),
            display_name: TextComponent::empty(),
            server: None,
            silent: false,
            command_result_taker: ResultValueTaker::new(),
            entity_anchor: EntityAnchor::Feet,
        }
    }

    /// 创建一个可在真实环境中运行命令的 [`CommandSource`]。
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        output: CommandSender,
        world: Arc<World>,
        entity: Option<Arc<dyn EntityBase>>,
        position: Vector3<f64>,
        rotation: Vector2<f32>,
        name: String,
        display_name: TextComponent,
        server: Arc<Server>,
    ) -> Self {
        Self {
            output,
            world: Some(world),
            entity,
            position,
            rotation,
            name,
            display_name,
            server: Some(server),
            silent: false,
            command_result_taker: ResultValueTaker(Vec::new()),
            entity_anchor: EntityAnchor::Feet,
        }
    }

    /// 返回一个带有指定输出的新 [`CommandSource`]，其余字段沿用原 `source` 的值。
    #[must_use]
    pub fn with_output(self, output: CommandSender) -> Self {
        Self {
            output,
            world: self.world,
            entity: self.entity,
            position: self.position,
            rotation: self.rotation,
            name: self.name,
            display_name: self.display_name,
            server: self.server,
            silent: self.silent,
            command_result_taker: self.command_result_taker,
            entity_anchor: self.entity_anchor,
        }
    }

    /// 返回一个带有指定世界的新 [`CommandSource`]，其余字段沿用原 `source` 的值。
    #[must_use]
    pub fn with_world(self, world: Arc<World>) -> Self {
        Self {
            output: self.output,
            world: Some(world),
            entity: self.entity,
            position: self.position,
            rotation: self.rotation,
            name: self.name,
            display_name: self.display_name,
            server: self.server,
            silent: true,
            command_result_taker: self.command_result_taker,
            entity_anchor: self.entity_anchor,
        }
    }

    /// 返回一个带有指定实体的新 [`CommandSource`]，其余字段沿用原 `source` 的值。
    #[must_use]
    pub fn with_entity(self, entity: Arc<dyn EntityBase>) -> Self {
        let name = entity.get_name().get_text();
        let display_name = entity.get_display_name();
        Self {
            output: self.output,
            world: self.world,
            entity: Some(entity),
            position: self.position,
            rotation: self.rotation,
            name,
            display_name,
            server: self.server,
            silent: self.silent,
            command_result_taker: self.command_result_taker,
            entity_anchor: self.entity_anchor,
        }
    }

    /// 返回一个带有指定位置的新 [`CommandSource`]，其余字段沿用原 `source` 的值。
    #[must_use]
    pub fn with_position(self, position: Vector3<f64>) -> Self {
        Self {
            output: self.output,
            world: self.world,
            entity: self.entity,
            position,
            rotation: self.rotation,
            name: self.name,
            display_name: self.display_name,
            server: self.server,
            silent: self.silent,
            command_result_taker: self.command_result_taker,
            entity_anchor: self.entity_anchor,
        }
    }

    /// 返回一个带有指定朝向的新 [`CommandSource`]，其余字段沿用原 `source` 的值。
    #[must_use]
    pub fn with_rotation(self, rotation: Vector2<f32>) -> Self {
        Self {
            output: self.output,
            world: self.world,
            entity: self.entity,
            position: self.position,
            rotation,
            name: self.name,
            display_name: self.display_name,
            server: self.server,
            silent: self.silent,
            command_result_taker: self.command_result_taker,
            entity_anchor: self.entity_anchor,
        }
    }

    /// 将指定的 taker 与当前 taker 合并，返回一个带有合并结果的新 [`CommandSource`]。
    #[must_use]
    pub fn merge_command_result_taker(self, command_result_taker: &ResultValueTaker) -> Self {
        let merged = ResultValueTaker::merge(&self.command_result_taker, command_result_taker);
        self.with_command_result_taker(merged)
    }

    /// 返回一个带有指定静默状态的新 [`CommandSource`]，其余字段沿用原 `source` 的值。
    #[must_use]
    pub fn with_silent(self) -> Self {
        Self {
            output: self.output,
            world: self.world,
            entity: self.entity,
            position: self.position,
            rotation: self.rotation,
            name: self.name,
            display_name: self.display_name,
            server: self.server,
            silent: true,
            command_result_taker: self.command_result_taker,
            entity_anchor: self.entity_anchor,
        }
    }

    /// 返回一个带有指定命令结果 taker 的新 [`CommandSource`]，其余字段沿用原 `source` 的值。
    #[must_use]
    pub fn with_command_result_taker(self, command_result_taker: ResultValueTaker) -> Self {
        Self {
            output: self.output,
            world: self.world,
            entity: self.entity,
            position: self.position,
            rotation: self.rotation,
            name: self.name,
            display_name: self.display_name,
            server: self.server,
            silent: self.silent,
            command_result_taker,
            entity_anchor: self.entity_anchor,
        }
    }

    /// 返回一个带有指定实体锚点的新 [`CommandSource`]，其余字段沿用原 `source` 的值。
    #[must_use]
    pub fn with_entity_anchor(self, entity_anchor: EntityAnchor) -> Self {
        Self {
            output: self.output,
            world: self.world,
            entity: self.entity,
            position: self.position,
            rotation: self.rotation,
            name: self.name,
            display_name: self.display_name,
            server: self.server,
            silent: true,
            command_result_taker: self.command_result_taker,
            entity_anchor,
        }
    }

    /// 返回一个新的 [`CommandSource`]，其朝向被调整为面向该实体的锚点，
    /// 其余字段沿用原 `source` 的值。
    #[must_use]
    pub fn with_looking_at_entity(
        self,
        entity: &Arc<dyn EntityBase>,
        anchor: EntityAnchor,
    ) -> Self {
        self.with_looking_at_pos(anchor.position_at_entity(entity.get_entity()))
    }

    /// 返回一个新的 [`CommandSource`]，其朝向被调整为面向给定位置，
    /// 其余字段沿用原 `source` 的值。
    #[must_use]
    pub fn with_looking_at_pos(self, pos: Vector3<f64>) -> Self {
        let source_pos = self.entity_anchor.position_at_source(&self);
        let delta = pos.sub(&source_pos);
        let horizontal_len = delta.horizontal_length();
        let pitch = -delta.y.atan2(horizontal_len).to_degrees();
        let yaw = delta.z.atan2(delta.x).to_degrees() - 90.0;
        self.with_rotation(Vector2::new(
            wrap_degrees(pitch as f32),
            wrap_degrees(yaw as f32),
        ))
    }

    /// 以 `Result` 形式获取实体：
    ///
    /// - 若该来源确实包含实体，返回包在 [`Ok`] 中的实体。
    /// - 否则返回包在 [`Err`] 中的命令错误。
    pub fn entity_or_err(&self) -> Result<Arc<dyn EntityBase>, CommandSyntaxError> {
        self.entity
            .clone()
            .ok_or(REQUIRES_ENTITY.create_without_context())
    }

    /// 以 `Result` 形式获取世界：
    ///
    /// - 若该来源确实包含世界，返回它。
    /// - 否则此函数 **panic**。理想情况下来源应包含世界，但单元测试中可能没有。
    #[must_use]
    pub const fn world(&self) -> &Arc<World> {
        self.world.as_ref().expect("预期世界存在")
    }

    /// 以 `Result` 形式获取服务器：
    ///
    /// - 若该来源确实包含服务器，返回它。
    /// - 否则此函数 **panic**。理想情况下来源应包含服务器，但单元测试中可能没有。
    #[must_use]
    pub const fn server(&self) -> &Arc<Server> {
        self.server.as_ref().expect("预期服务器存在")
    }

    /// 以 `Option` 形式获取玩家：
    ///
    /// - 若该来源确实包含玩家，返回包在 [`Some`] 中的玩家。
    /// - 否则返回 [`None`]。
    #[must_use]
    pub fn player_or_none(&self) -> Option<&Player> {
        self.entity.as_ref().and_then(|entity| entity.get_player())
    }

    /// 从底层输出发送者获取 `Arc<Player>` 形式的玩家。
    #[must_use]
    pub fn as_player(&self) -> Option<Arc<Player>> {
        self.output.as_player()
    }

    /// 以 `Result` 形式获取玩家：
    ///
    /// - 若该来源确实包含玩家，返回包在 [`Ok`] 中的玩家。
    /// - 否则返回包在 [`Err`] 中的命令错误。
    pub fn player_or_err(&self) -> Result<&Player, CommandSyntaxError> {
        self.player_or_none()
            .ok_or(REQUIRES_PLAYER.create_without_context())
    }

    /// 返回该命令是否由玩家执行。
    #[must_use]
    pub fn executed_by_player(&self) -> bool {
        self.player_or_none().is_some()
    }

    /// 向该来源发送一条消息。
    pub fn send_message(&self, message: TextComponent) {
        if !self.silent {
            self.output.send_message(message);
        }
    }

    /// 向所有在线管理员发送一条消息。
    fn send_to_ops(&self, message: TextComponent) {
        let text =
            TextComponent::translate("chat.type.admin", &[self.display_name.clone(), message])
                .color(Color::Named(NamedColor::Gray))
                .italic();
        let Some(server) = &self.server else {
            return;
        };
        if server.level_info.load().game_rules.send_command_feedback {
            let output_player = match &self.output {
                CommandSender::Player(sender) => Some(sender),
                _ => None,
            };
            for player in server.get_all_players() {
                if output_player != Some(&player)
                    && player.permission_lvl.load() >= server.basic_config.op_permission_level
                {
                    player.send_system_message(&text);
                }
            }
        }
    }

    /// 向该来源发送反馈。
    pub fn send_feedback(&self, message: TextComponent, broadcast_to_ops: bool) {
        if !self.silent {
            let should_send_to_output = self.output.should_receive_feedback();
            let should_send_to_ops =
                broadcast_to_ops && self.output.should_broadcast_console_to_ops();

            if should_send_to_output {
                self.output.send_message(message.clone());
            }
            if should_send_to_ops {
                self.send_to_ops(message);
            }
        }
    }

    /// 向控制台发送错误消息。
    ///
    /// # Note
    /// 若要报告 [`CommandSyntaxError`]，不要使用此函数；
    /// 应把错误包在 [`Err`] 中返回（或使用 `?` 运算符）。
    ///
    /// 不过在只想发送错误而不直接报告命令失败的场景下，此函数仍然适用。
    pub fn send_error(&self, error: TextComponent) {
        if !self.silent && self.output.should_track_output() {
            self.output.send_message(
                TextComponent::empty()
                    .add_child(error)
                    .color(Color::Named(NamedColor::Red)),
            );
        }
    }

    /// 返回该来源是否拥有指定权限。
    ///
    /// # Panics
    ///
    /// 若该来源没有服务器引用（即是空 [`CommandSource`]），发生 panic。
    #[must_use]
    pub fn has_permission(&self, permission: &str) -> bool {
        self.server.as_ref().map_or(
            matches!(
                self.output,
                crate::command::CommandSender::Console
                    | crate::command::CommandSender::Rcon(_)
                    | crate::command::CommandSender::Dummy
            ),
            |server| self.output.has_permission(server, permission),
        )
    }

    /// 返回该来源是否拥有指定权限。
    ///
    /// # Panics
    ///
    /// **当且仅当**以下两个条件同时满足时发生 panic：
    ///
    /// - 权限不是 [`None`]。
    /// - 该来源没有服务器引用（即是空 [`CommandSource`]）。
    #[must_use]
    pub fn has_permission_from_option(&self, permission: Option<&str>) -> bool {
        permission.is_none_or(|permission| self.has_permission(permission))
    }
}

impl papokin_command::source::CommandSource for CommandSource {
    fn send_message(&self, message: TextComponent) {
        self.send_message(message);
    }

    fn send_error(&self, error: TextComponent) {
        self.send_error(error);
    }

    fn call_result(&self, result: ReturnValue) {
        self.command_result_taker.call(result);
    }

    fn has_permission(&self, permission: &str) -> bool {
        self.has_permission(permission)
    }

    fn position(&self) -> Vector3<f64> {
        self.position
    }

    fn rotation(&self) -> Vector2<f32> {
        self.rotation
    }

    fn check_block_loaded(&self, pos: &BlockPos) -> Result<(), CommandSyntaxError> {
        let world = self.world();
        if world
            .level
            .read_chunk_sync(&pos.chunk_position(), |_| ())
            .is_none()
        {
            Err(
                papokin_command::argument_types::coordinates::block_pos::NOT_LOADED_ERROR_TYPE
                    .create_without_context(),
            )
        } else if !world.is_in_build_limit(*pos) {
            Err(
                papokin_command::argument_types::coordinates::block_pos::OUT_OF_WORLD_ERROR_TYPE
                    .create_without_context(),
            )
        } else {
            Ok(())
        }
    }

    fn entity_anchor(&self) -> EntityAnchor {
        self.entity_anchor
    }

    fn anchor_position(&self, anchor: EntityAnchor) -> Vector3<f64> {
        let pos = self.position;
        self.entity
            .as_ref()
            .map_or_else(|| pos, |e| anchor.position_at_entity(e.get_entity()))
    }
}
