use crate::command::argument_builder::{ArgumentBuilder, argument, command, literal};
use crate::command::argument_types::core::string::StringArgumentType;
use crate::command::argument_types::entity::EntityArgumentType;
use crate::command::argument_types::resource_key::{ADVANCEMENT_REGISTRY, ResourceKeyArgument};
use crate::command::context::command_context::CommandContext;
use crate::command::context::command_source::CommandSource;
use crate::command::errors::command_syntax_error::CommandSyntaxError;
use crate::command::errors::error_types::CommandErrorType;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::node::{CommandExecutor, CommandExecutorResult};
use crate::command::suggestion::provider::{SuggestionProvider, SuggestionProviderResult};
use crate::command::suggestion::suggestions::SuggestionsBuilder;
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::entity::player::advancement::{AdvancementAward, PlayerAdvancement};
use papokin_data::advancement_data::AdvancementNode;
use papokin_data::{ADVANCEMENT_TREE, Advancement, translation};
use papokin_util::PermissionLvl;
use papokin_util::permission::{Permission, PermissionDefault, PermissionRegistry};
use papokin_util::text::TextComponent;
use std::sync::Arc;

const NAME: &str = "advancement";
const DESCRIPTION: &str = "管理玩家的进度。";
const PERMISSION: &str = "minecraft:command.advancement";

const ARG_TARGETS: &str = "targets";
const ARG_ADVANCEMENT: &str = "advancement";
const ARG_CRITERION: &str = "criterion";

const ERROR_CRITERION_NOT_FOUND: CommandErrorType<2> =
    CommandErrorType::new(translation::java::COMMANDS_ADVANCEMENT_CRITERIONNOTFOUND);
const ERROR_GRANT_ONE_TO_ONE: CommandErrorType<2> =
    CommandErrorType::new(translation::java::COMMANDS_ADVANCEMENT_GRANT_ONE_TO_ONE_FAILURE);
const ERROR_REVOKE_ONE_TO_ONE: CommandErrorType<2> =
    CommandErrorType::new(translation::java::COMMANDS_ADVANCEMENT_REVOKE_ONE_TO_ONE_FAILURE);
const ERROR_GRANT_ONE_TO_MANY: CommandErrorType<2> =
    CommandErrorType::new(translation::java::COMMANDS_ADVANCEMENT_GRANT_ONE_TO_MANY_FAILURE);
const ERROR_REVOKE_ONE_TO_MANY: CommandErrorType<2> =
    CommandErrorType::new(translation::java::COMMANDS_ADVANCEMENT_REVOKE_ONE_TO_MANY_FAILURE);
const ERROR_GRANT_MANY_TO_ONE: CommandErrorType<2> =
    CommandErrorType::new(translation::java::COMMANDS_ADVANCEMENT_GRANT_MANY_TO_ONE_FAILURE);
const ERROR_REVOKE_MANY_TO_ONE: CommandErrorType<2> =
    CommandErrorType::new(translation::java::COMMANDS_ADVANCEMENT_REVOKE_MANY_TO_ONE_FAILURE);
const ERROR_GRANT_MANY_TO_MANY: CommandErrorType<2> =
    CommandErrorType::new(translation::java::COMMANDS_ADVANCEMENT_GRANT_MANY_TO_MANY_FAILURE);
const ERROR_REVOKE_MANY_TO_MANY: CommandErrorType<2> =
    CommandErrorType::new(translation::java::COMMANDS_ADVANCEMENT_REVOKE_MANY_TO_MANY_FAILURE);

const ERROR_GRANT_CRITERION_TO_ONE_FAILURE: CommandErrorType<3> =
    CommandErrorType::new(translation::java::COMMANDS_ADVANCEMENT_GRANT_CRITERION_TO_ONE_FAILURE);

const ERROR_REVOKE_CRITERION_TO_ONE_FAILURE: CommandErrorType<3> =
    CommandErrorType::new(translation::java::COMMANDS_ADVANCEMENT_REVOKE_CRITERION_TO_ONE_FAILURE);

const ERROR_GRANT_CRITERION_TO_MANY_FAILURE: CommandErrorType<3> =
    CommandErrorType::new(translation::java::COMMANDS_ADVANCEMENT_GRANT_CRITERION_TO_MANY_FAILURE);

const ERROR_REVOKE_CRITERION_TO_MANY_FAILURE: CommandErrorType<3> =
    CommandErrorType::new(translation::java::COMMANDS_ADVANCEMENT_REVOKE_CRITERION_TO_MANY_FAILURE);

#[derive(Clone, Copy)]
pub enum Action {
    Grant,
    Revoke,
}

impl Action {
    /// 直接接收已加锁 [`PlayerAdvancement`] 的内部函数
    fn perform_single_inner(
        self,
        guard: &mut PlayerAdvancement,
        advancement: &'static Advancement,
    ) -> (bool, AdvancementAward) {
        let progress = guard.progress.get_mut_or_start_progress(advancement);
        match self {
            Self::Grant => {
                if progress.is_done() {
                    return (false, AdvancementAward::default());
                }
                let criteria: Vec<Arc<str>> = progress.get_remaining_criteria().collect();
                let mut result = AdvancementAward::default();
                for criterion in criteria {
                    result = result.combine(guard.award(advancement, &criterion));
                }
                (true, result)
            }
            Self::Revoke => {
                if !progress.has_progress() {
                    return (false, AdvancementAward::default());
                }
                let criteria: Vec<Arc<str>> = progress.get_completed_criteria().collect();
                for criterion in criteria {
                    guard.revoke(advancement, &criterion);
                }
                (true, AdvancementAward::default())
            }
        }
    }

    fn perform_single(self, player: &Arc<Player>, advancement: &'static Advancement) -> bool {
        let (performed, result) = {
            let mut guard = player
                .advancements
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            self.perform_single_inner(&mut guard, advancement)
        };

        if matches!(self, Self::Grant) {
            PlayerAdvancement::finish_award(player, advancement, result);
        }
        performed
    }

    /// 对单个玩家的多个进度执行操作（授予或撤销）。
    ///
    /// 此方法将当前操作应用于所提供切片中的每个进度，
    /// 指定玩家。它会锁定该玩家的进度，并逐条处理进度
    /// 依次执行，并收集成功操作的次数。
    ///
    /// # Arguments
    ///
    /// * `player` - 要修改其进度的玩家
    /// * `advancements` - 要对其应用该操作的进度切片
    /// * `show_advancement` - 控制进度通知显示的标志（当前未使用）
    ///
    /// # Returns
    ///
    /// 返回成功修改的进度数量。一个进度被计为
    /// [`perform_single_inner`] 返回 `true` 即为成功
    fn perform(
        self,
        player: &Arc<Player>,
        advancements: &[&'static Advancement],
        show_advancement: bool,
    ) -> i32 {
        if !show_advancement {
            let mut guard = player
                .advancements
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            guard.flush_dirty(player, true);
        }
        let count = advancements
            .iter()
            .filter(|advancement| self.perform_single(player, advancement))
            .count() as i32;
        if !show_advancement {
            let mut guard = player
                .advancements
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            guard.flush_dirty(player, false);
        }
        count
    }

    fn perform_criterion(
        self,
        player: &Arc<Player>,
        advancement: &'static Advancement,
        criterion: &str,
    ) -> bool {
        let (performed, result) = {
            let mut guard = player
                .advancements
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            match self {
                Self::Grant => {
                    let result = guard.award(advancement, criterion);
                    (result.awarded(), result)
                }
                Self::Revoke => (
                    guard.revoke(advancement, criterion),
                    AdvancementAward::default(),
                ),
            }
        };

        if matches!(self, Self::Grant) {
            PlayerAdvancement::finish_award(player, advancement, result);
        }
        performed
    }

    /// 返回该操作对应的按键
    const fn get_key(&self) -> &str {
        match self {
            Self::Grant => "commands.advancement.grant",
            Self::Revoke => "commands.advancement.revoke",
        }
    }
}
#[derive(Clone, Copy)]
#[allow(unused)]
enum Mode {
    Only,
    Through,
    From,
    Until,
    Everything,
}

impl Mode {
    const fn parents(self) -> bool {
        match self {
            Self::Only | Self::From => false,
            Self::Through | Self::Until | Self::Everything => true,
        }
    }

    const fn children(self) -> bool {
        match self {
            Self::Only | Self::Until => false,
            Self::Through | Self::From | Self::Everything => true,
        }
    }
}

/// 根据目标进度和遍历模式检索一组进度。
///
/// 此函数通过遍历进度树来构建进度列表，所依据的是
/// 指定模式。遍历可以包含父进度、子进度或两者兼有，
/// 取决于所选择的模式。
///
/// # Arguments
///
/// * `target` - 作为遍历起点的进度
/// * `mode` - 决定包含哪些进度的遍历模式：
///   - `Mode::Only` - 仅返回目标进度
///   - `Mode::From` - 返回目标及其所有后代
///   - `Mode::Until` - 返回目标及其所有祖先
///   - `Mode::Through` - 返回目标及其祖先与后代
///   - `Mode::Everything` - 返回目标及其所有祖先和后代（从未使用）
///
/// # Returns
///
/// 一个由匹配指定模式的进度引用组成的向量。如果目标进度
/// 在树中未找到，则返回仅包含目标的向量。
fn get_advancements(target: &Advancement, mode: Mode) -> Vec<&Advancement> {
    let tree = &ADVANCEMENT_TREE;
    let target_node = tree.get_node_from_id(&target.id);
    target_node.map_or_else(
        || vec![target],
        |target_node| {
            let mut advancements = Vec::new();
            if mode.parents() {
                let mut parent = target_node.parent;
                while let Some(parent_id) = parent {
                    let current_node = &tree.nodes_vector[parent_id];
                    advancements.push(current_node.value);
                    parent = current_node.parent;
                }
            }
            advancements.push(target);
            if mode.children() {
                add_children(target_node, &mut advancements);
            }
            advancements
        },
    )
}

fn add_children(parent: &AdvancementNode, output: &mut Vec<&Advancement>) {
    for child in &parent.children {
        let node = &ADVANCEMENT_TREE.nodes_vector[*child];
        output.push(node.value);
        add_children(node, output);
    }
}

#[inline]
fn perform_and_show(
    context: &CommandSource,
    players: &[Arc<Player>],
    action: Action,
    advancements: &[&'static Advancement],
) -> Result<i32, CommandSyntaxError> {
    perform(context, players, action, advancements, true)
}

/// 对多个玩家的多个进度执行批量操作（授予或撤销）。
///
/// 此函数遍历每个玩家，并将指定的动作应用到所有提供的
/// 进度。它会根据玩家数量自动处理错误消息，并
/// 所涉及的进度。
///
/// # Arguments
///
/// * `context` - 用于发送反馈的命令源上下文
/// * `targets` - 要对其应用该操作的玩家
/// * `action` - 对每个进度执行的操作（授予或撤销）
/// * `advancements` - 要对其应用该操作的进度切片
/// * `show_advancement` - 是否向玩家显示进度通知
///
/// # Returns
///
///返回 `Ok(count)`，其中为成功授予/撤销的进度总数
/// 所有玩家。
///
///若没有任何操作成功，则返回 `Err` 及相应的本地化错误消息。
/// 错误消息依据以下因素而变化：
/// - 无论目标是一个还是多个玩家
/// - 无论涉及一个还是多个进度
/// - 操作类型（授予或撤销）
fn perform(
    context: &CommandSource,
    targets: &[Arc<Player>],
    action: Action,
    advancements: &[&'static Advancement],
    show_advancement: bool,
) -> Result<i32, CommandSyntaxError> {
    let mut advancement_count = 0;
    let mut player_count = 0;
    for player in targets {
        let changed = action.perform(player, advancements, show_advancement);
        if changed > 0 {
            player_count += 1;
        }
        advancement_count += changed;
    }
    if advancement_count == 0 {
        return if let [first_advancement] = advancements[..] {
            if let [first_player] = targets {
                Err(match action {
                    Action::Grant => &ERROR_GRANT_ONE_TO_ONE,
                    Action::Revoke => &ERROR_REVOKE_ONE_TO_ONE,
                }
                .create_without_context_args_slice(&[
                    first_advancement.name(),
                    first_player.get_display_name(),
                ]))
            } else {
                Err(match action {
                    Action::Grant => &ERROR_GRANT_ONE_TO_MANY,
                    Action::Revoke => &ERROR_REVOKE_ONE_TO_MANY,
                }
                .create_without_context_args_slice(&[
                    first_advancement.name(),
                    TextComponent::text(targets.len().to_string()),
                ]))
            }
        } else if let [first_player] = targets {
            Err(match action {
                Action::Grant => &ERROR_GRANT_MANY_TO_ONE,
                Action::Revoke => &ERROR_REVOKE_MANY_TO_ONE,
            }
            .create_without_context_args_slice(&[
                TextComponent::text(advancements.len().to_string()),
                first_player.get_display_name(),
            ]))
        } else {
            Err(match action {
                Action::Grant => &ERROR_GRANT_MANY_TO_MANY,
                Action::Revoke => &ERROR_REVOKE_MANY_TO_MANY,
            }
            .create_without_context_args_slice(&[
                TextComponent::text(advancements.len().to_string()),
                TextComponent::text(targets.len().to_string()),
            ]))
        };
    }
    let translate = if let [first_advancement] = advancements[..] {
        if let [first_player] = targets {
            TextComponent::translate(
                format!("{}.one.to.one.success", action.get_key()),
                [first_advancement.name(), first_player.get_display_name()],
            )
        } else {
            TextComponent::translate(
                format!("{}.one.to.many.success", action.get_key()),
                [
                    first_advancement.name(),
                    TextComponent::text(player_count.to_string()),
                ],
            )
        }
    } else if let [first] = targets {
        TextComponent::translate(
            format!("{}.many.to.one.success", action.get_key()),
            [
                TextComponent::text(advancement_count.to_string()),
                first.get_display_name(),
            ],
        )
    } else {
        TextComponent::translate(
            format!("{}.many.to.many.success", action.get_key()),
            [
                TextComponent::text(advancement_count.to_string()),
                TextComponent::text(player_count.to_string()),
            ],
        )
    };
    context.send_feedback(translate, true);
    Ok(advancement_count)
}

/// 对多个玩家的特定进度条件执行操作（授予或撤销）。
///
/// 此函数尝试为每个对象将指定的动作应用到进度的某个条件上
/// 给定玩家的操作。它会根据成功操作的数量处理错误报告
/// 并向命令来源提供反馈。
///
/// # Arguments
///
/// * `context` - 用于发送反馈的命令源上下文
/// * `targets` - 要对其应用该操作的玩家
/// * `action` - 要执行的操作（授予或撤销）
/// * `advancement` - 包含该判据的进度
/// * `criterion` - 要操作的具体判据名称
///
/// # Returns
///
///若至少有一个操作成功，则返回 `Ok(count)`，其中为成功的操作数量。
///在以下情况下返回 `Err` 及相应的错误消息：
/// - 该进度中不存在此条件
/// - 没有任何操作成功
pub fn perform_criterion(
    context: &CommandSource,
    targets: &[Arc<Player>],
    action: Action,
    advancement: &'static Advancement,
    criterion: &str,
) -> Result<i32, CommandSyntaxError> {
    if !advancement.criteria.contains(&criterion) {
        return Err(
            ERROR_CRITERION_NOT_FOUND.create_without_context_args_slice(&[
                advancement.name(),
                TextComponent::text(criterion.to_owned()),
            ]),
        );
    }

    let count = targets
        .iter()
        .map(|player| action.perform_criterion(player, advancement, criterion))
        .filter(|&success| success)
        .count() as i32;

    if count == 0 {
        if let [first_player] = targets {
            Err(match action {
                Action::Grant => &ERROR_GRANT_CRITERION_TO_ONE_FAILURE,
                Action::Revoke => &ERROR_REVOKE_CRITERION_TO_ONE_FAILURE,
            }
            .create_without_context_args_slice(&[
                TextComponent::text(criterion.to_owned()),
                advancement.name(),
                first_player.get_display_name(),
            ]))
        } else {
            Err(match action {
                Action::Grant => &ERROR_GRANT_CRITERION_TO_MANY_FAILURE,
                Action::Revoke => &ERROR_REVOKE_CRITERION_TO_MANY_FAILURE,
            }
            .create_without_context_args_slice(&[
                TextComponent::text(criterion.to_owned()),
                advancement.name(),
                TextComponent::text(targets.len().to_string()),
            ]))
        }
    } else {
        let translate = if let [first_player] = targets {
            TextComponent::translate(
                format!("{}.criterion.to.one.success", action.get_key()),
                [
                    TextComponent::text(criterion.to_owned()),
                    advancement.name(),
                    first_player.get_display_name(),
                ],
            )
        } else {
            TextComponent::translate(
                format!("{}.criterion.to.many.success", action.get_key()),
                [
                    TextComponent::text(criterion.to_owned()),
                    advancement.name(),
                    TextComponent::text(count.to_string()),
                ],
            )
        };
        context.send_feedback(translate, true);
        Ok(count)
    }
}

/// 用于指定了某个条件时，在授予/撤销该条件之际
struct OnlyAdvancementCriterionExecutor {
    action: Action,
}

impl CommandExecutor for OnlyAdvancementCriterionExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let action = self.action;
        perform_criterion(
            &context.source,
            &EntityArgumentType::get_players(context, ARG_TARGETS)?,
            action,
            ResourceKeyArgument::get_advancement(context, ARG_ADVANCEMENT)?,
            StringArgumentType::get(context, ARG_CRITERION)?,
        )
    }
}

/// 用于根据所选的 `mode` 授予/撤销进度
struct AdvancementExecutor {
    action: Action,
    mode: Mode,
}

impl CommandExecutor for AdvancementExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let action = self.action;
        let mode = self.mode;
        perform_and_show(
            &context.source,
            &EntityArgumentType::get_players(context, ARG_TARGETS)?,
            action,
            &get_advancements(
                ResourceKeyArgument::get_advancement(context, ARG_ADVANCEMENT)?,
                mode,
            ),
        )
    }
}

/// 建议指定进度对应的判据
struct CriterionSuggestionProvider;

impl SuggestionProvider for CriterionSuggestionProvider {
    fn suggest(
        &self,
        context: &CommandContext,
        builder: SuggestionsBuilder,
    ) -> SuggestionProviderResult {
        let suggestion = ResourceKeyArgument::get_advancement(context, ARG_ADVANCEMENT)
            .ok()
            .map(|adv| adv.criteria)
            .into_iter()
            .flatten()
            .map(ToString::to_string);
        builder.filter_and_suggest_iter(suggestion).build()
    }
}

/// 执行器向指定玩家授予/撤销所有进度
struct EveryAdvancementExecutor {
    action: Action,
    show_advancement: bool,
}

impl CommandExecutor for EveryAdvancementExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let action = self.action;
        let show_advancement = self.show_advancement;
        perform(
            &context.source,
            &EntityArgumentType::get_players(context, ARG_TARGETS)?,
            action,
            &Advancement::get_advancements_list(),
            show_advancement,
        )
    }
}

/// 注册进度命令
pub fn register(dispatcher: &mut CommandDispatcher, registry: &PermissionRegistry) {
    registry.register_permission_or_panic(Permission::new(
        PERMISSION,
        DESCRIPTION,
        PermissionDefault::Op(PermissionLvl::Two),
    ));

    macro_rules! build_action {
        ($name:expr, $action:expr) => {
            literal($name).then(
                argument(ARG_TARGETS, EntityArgumentType::Players)
                    .then(
                        literal("only").then(
                            argument(ARG_ADVANCEMENT, ResourceKeyArgument(&ADVANCEMENT_REGISTRY))
                                .executes(AdvancementExecutor {
                                    action: $action,
                                    mode: Mode::Only,
                                })
                                .then(
                                    argument(ARG_CRITERION, StringArgumentType::GreedyPhrase)
                                        .suggests(CriterionSuggestionProvider)
                                        .executes(OnlyAdvancementCriterionExecutor {
                                            action: $action,
                                        }),
                                ),
                        ),
                    )
                    .then(
                        literal("from").then(
                            argument(ARG_ADVANCEMENT, ResourceKeyArgument(&ADVANCEMENT_REGISTRY))
                                .executes(AdvancementExecutor {
                                    action: $action,
                                    mode: Mode::From,
                                }),
                        ),
                    )
                    .then(
                        literal("until").then(
                            argument(ARG_ADVANCEMENT, ResourceKeyArgument(&ADVANCEMENT_REGISTRY))
                                .executes(AdvancementExecutor {
                                    action: $action,
                                    mode: Mode::Until,
                                }),
                        ),
                    )
                    .then(
                        literal("through").then(
                            argument(ARG_ADVANCEMENT, ResourceKeyArgument(&ADVANCEMENT_REGISTRY))
                                .executes(AdvancementExecutor {
                                    action: $action,
                                    mode: Mode::Through,
                                }),
                        ),
                    )
                    .then(literal("everything").executes(EveryAdvancementExecutor {
                        action: $action,
                        show_advancement: matches!($action, Action::Revoke),
                    })),
            )
        };
    }

    dispatcher.register(
        command(NAME, DESCRIPTION)
            .requires(PERMISSION)
            .then(build_action!("grant", Action::Grant))
            .then(build_action!("revoke", Action::Revoke)),
    );
}
