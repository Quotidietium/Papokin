use crate::command::node::dispatcher::CommandDispatcher;
use papokin_config::CommandsConfig;
use papokin_util::{
    PermissionLvl,
    permission::{Permission, PermissionDefault, PermissionManager, PermissionRegistry},
};
use tracing::{info, warn};

mod advancement;
mod attribute;
mod ban;
mod banip;
mod banlist;
mod bossbar;
mod clear;
mod clone;
mod damage;
mod data;
mod datapack;
mod debug;
pub mod defaultgamemode;
mod deop;
mod dialog;
mod difficulty;
mod effect;
mod enchant;
mod execute;
mod experience;
mod fetchprofile;
mod fill;
mod fillbiome;
mod forceload;
mod function;
mod gamemode;
mod gamerule;
mod give;
mod help;
mod item;
mod kick;
mod kill;
mod list;
mod locate;
mod loot;
mod me;
mod msg;
mod op;
mod papokin;
mod pardon;
mod pardonip;
mod particle;
mod place;
mod playsound;
mod plugin;
mod plugins;
mod raid;
mod random;
mod recipe;
mod reload;
mod r#return;
mod ride;
mod rotate;
mod saveall;
mod saveoff;
mod saveon;
mod say;
mod schedule;
mod scoreboard;
mod seed;
mod setblock;
mod setidletimeout;
mod setworldspawn;
mod spawnpoint;
mod spectate;
mod spreadplayers;
mod stop;
mod stopsound;
mod stopwatch;
mod summon;
mod tag;
mod team;
mod teammsg;
mod teleport;
mod tellraw;
mod test;
mod tick;
mod time;
mod title;
mod tps;
mod transfer;
mod trigger;
mod waypoint;
mod weather;
mod whitelist;
mod worldborder;

#[allow(clippy::too_many_lines)]
#[must_use]
pub fn default_dispatcher(
    permission_manager: &PermissionManager,
    commands_config: &CommandsConfig,
) -> CommandDispatcher {
    let registry = &permission_manager.registry;

    register_permissions(registry);

    let mut dispatcher = CommandDispatcher::new();

    say::register(&mut dispatcher, registry);
    banlist::register(&mut dispatcher, registry);
    difficulty::register(&mut dispatcher, registry);
    debug::register(&mut dispatcher, registry);
    dialog::register(&mut dispatcher, registry);
    execute::register(&mut dispatcher, registry);
    fillbiome::register(&mut dispatcher, registry);
    forceload::register(&mut dispatcher, registry);
    ride::register(&mut dispatcher, registry);
    recipe::register(&mut dispatcher, registry);
    reload::register(&mut dispatcher, registry);
    help::register(&mut dispatcher, registry);
    kill::register(&mut dispatcher, registry);
    op::register(&mut dispatcher, registry);
    place::register(&mut dispatcher, registry);
    random::register(&mut dispatcher, registry);
    list::register(&mut dispatcher, registry);
    locate::register(&mut dispatcher, registry);
    loot::register(&mut dispatcher, registry);
    seed::register(&mut dispatcher, registry);
    saveall::register(&mut dispatcher, registry);
    saveoff::register(&mut dispatcher, registry);
    saveon::register(&mut dispatcher, registry);
    setidletimeout::register(&mut dispatcher, registry);
    spreadplayers::register(&mut dispatcher, registry);
    stop::register(&mut dispatcher, registry);
    tag::register(&mut dispatcher, registry);
    tick::register(&mut dispatcher, registry);
    advancement::register(&mut dispatcher, registry);
    data::register(&mut dispatcher, registry);
    stopwatch::register(&mut dispatcher, registry);
    trigger::register(&mut dispatcher, registry);
    scoreboard::register(&mut dispatcher, registry);
    test::register(&mut dispatcher, registry);
    team::register(&mut dispatcher, registry);
    teammsg::register(&mut dispatcher, registry);
    clone::register(&mut dispatcher, registry);
    attribute::register(&mut dispatcher, registry);
    datapack::register(&mut dispatcher, registry);
    function::register(&mut dispatcher, registry);
    r#return::register(&mut dispatcher, registry);
    schedule::register(&mut dispatcher, registry);
    fetchprofile::register(&mut dispatcher, registry);
    worldborder::register(&mut dispatcher, registry);
    particle::register(&mut dispatcher, registry);
    playsound::register(&mut dispatcher, registry);
    fill::register(&mut dispatcher, registry);
    clear::register(&mut dispatcher, registry);
    papokin::register(&mut dispatcher, registry);
    me::register(&mut dispatcher, registry);
    msg::register(&mut dispatcher, registry);
    tps::register(&mut dispatcher, registry);
    transfer::register(&mut dispatcher, registry);
    gamemode::register(&mut dispatcher, registry);
    defaultgamemode::register(&mut dispatcher, registry);
    weather::register(&mut dispatcher, registry);
    time::register(&mut dispatcher, registry);
    teleport::register(&mut dispatcher, registry);
    setworldspawn::register(&mut dispatcher, registry);
    spawnpoint::register(&mut dispatcher, registry);
    spectate::register(&mut dispatcher, registry);
    setblock::register(&mut dispatcher, registry);
    give::register(&mut dispatcher, registry);
    item::register(&mut dispatcher, registry);
    enchant::register(&mut dispatcher, registry);
    effect::register(&mut dispatcher, registry);
    summon::register(&mut dispatcher, registry);
    damage::register(&mut dispatcher, registry);
    rotate::register(&mut dispatcher, registry);
    tellraw::register(&mut dispatcher, registry);
    title::register(&mut dispatcher, registry);
    experience::register(&mut dispatcher, registry);
    bossbar::register(&mut dispatcher, registry);
    gamerule::register(&mut dispatcher, registry);
    stopsound::register(&mut dispatcher, registry);
    waypoint::register(&mut dispatcher, registry);
    raid::register(&mut dispatcher, registry);
    deop::register(&mut dispatcher, registry);
    kick::register(&mut dispatcher, registry);
    plugin::register(&mut dispatcher, registry);
    plugins::register(&mut dispatcher, registry);
    ban::register(&mut dispatcher, registry);
    banip::register(&mut dispatcher, registry);
    pardon::register(&mut dispatcher, registry);
    pardonip::register(&mut dispatcher, registry);
    whitelist::register(&mut dispatcher, registry);

    apply_command_overrides(&mut dispatcher, registry, commands_config);

    dispatcher
}

/// 将服务器配置中针对各条命令的设置叠加应用到默认设置之上，
/// 新构建的分发器。
///
/// 支持两种覆盖方式：
/// - 禁用命令，这会将其从旧版调度器中移除并标记
///   其名称，以便包装分发器在所有其他地方将其隐藏。
/// - 更改命令所需的权限等级，这是通过重写来完成的
///   注册表中权限节点的默认值。由于命令
///   要求会在执行时从注册表查找其权限，
///   这对旧版分发器与基于节点的分发器的影响是一致的。
fn apply_command_overrides(
    dispatcher: &mut CommandDispatcher,
    registry: &PermissionRegistry,
    commands_config: &CommandsConfig,
) {
    for (raw_name, settings) in &commands_config.overrides {
        // 命令名始终为小写，因此在此统一归一化以保持兼容
        // 所有者在配置文件中书写它们的方式。
        let name = raw_name.to_ascii_lowercase();

        // 捕捉笔误：覆盖一个不存在的命令几乎
        // 总是意味着所有者拼写有误，所以应告知他们，而不是默默
        // 什么都不做。
        if !dispatcher.has_command(&name) {
            warn!("忽略 \"{raw_name}\" 的命令设置，因为不存在同名命令（请检查配置中的拼写）");
            continue;
        }

        if !settings.enabled {
            // 如果所有者命名了别名（例如 `teleport` 的 `tp`），关闭
            // 整个命令，而不仅仅是那一个别名。
            let primary = dispatcher.primary_command_name(&name);

            dispatcher.disable_command(name.clone());
            dispatcher.disable_command(primary.clone());
            // 基于节点的命令将其别名保留为重定向的根节点，
            // 所以也要对这些做标记。
            for alias in dispatcher.tree_alias_names(&primary) {
                dispatcher.disable_command(alias);
            }
            info!("命令 /{primary} 已在配置中关闭");
            // 被禁用的命令永远无法运行，因此其权限等级
            // 无关；跳过其余部分。
            continue;
        }

        if let Some(level) = settings.permission_level {
            let default = if level == PermissionLvl::Zero {
                PermissionDefault::Allow
            } else {
                PermissionDefault::Op(level)
            };

            if let Some(node) = resolve_permission_node(dispatcher, registry, &name) {
                if registry.set_default(&node, default) {
                    info!("命令 /{name} 现在需要权限等级 {} 才能使用", level as u8);
                } else {
                    warn!(
                        "命令 /{name} 的覆盖设置了权限等级，但无法更新对应的权限节点；保持原样不变"
                    );
                }
            } else {
                warn!("命令 /{name} 的覆盖设置了权限等级，但找不到对应的权限节点；保持原样不变");
            }
        }
    }
}

/// 查找与命令名关联的权限节点。
///
/// 通过探测注册表来遵循 `<namespace>:command.<name>` 的约定。
fn resolve_permission_node(
    _dispatcher: &CommandDispatcher,
    registry: &PermissionRegistry,
    name: &str,
) -> Option<String> {
    for namespace in ["minecraft", "papokin"] {
        let candidate = format!("{namespace}:command.{name}");
        if registry.get_permission(&candidate).is_some() {
            return Some(candidate);
        }
    }

    None
}

fn register_permissions(registry: &PermissionRegistry) {
    // 同时注册我们的实体选择器权限。
    registry
        .register_permission(Permission::new(
            "minecraft:command.selector",
            "允许玩家使用选择器变量",
            PermissionDefault::Allow,
        ))
        .unwrap_or_else(|e| tracing::warn!("{e}"));
}

#[cfg(test)]
mod override_tests {
    use papokin_config::{CommandOverride, CommandsConfig};
    use papokin_util::PermissionLvl;
    use papokin_util::permission::{PermissionDefault, PermissionManager};

    use super::default_dispatcher;

    fn disabled(config: &mut CommandsConfig, name: &str) {
        config.overrides.insert(
            name.to_string(),
            CommandOverride {
                enabled: false,
                permission_level: None,
            },
        );
    }

    fn permission(config: &mut CommandsConfig, name: &str, level: PermissionLvl) {
        config.overrides.insert(
            name.to_string(),
            CommandOverride {
                enabled: true,
                permission_level: Some(level),
            },
        );
    }

    #[test]
    fn disabling_a_command_removes_and_hides_it() {
        let mut commands = CommandsConfig::default();
        disabled(&mut commands, "gamemode");

        let manager = PermissionManager::new();
        let dispatcher = default_dispatcher(&manager, &commands);

        assert!(dispatcher.is_disabled("gamemode"));
        assert!(
            !dispatcher.is_disabled("give"),
            "untouched commands stay on"
        );
    }

    #[test]
    fn disabling_an_alias_turns_off_the_whole_command() {
        let mut commands = CommandsConfig::default();
        // `tp` 是 `teleport` 的别名；禁用它应使整个
        // 向下逐级注销命令，包括主名称。
        disabled(&mut commands, "tp");

        let manager = PermissionManager::new();
        let dispatcher = default_dispatcher(&manager, &commands);

        assert!(dispatcher.is_disabled("tp"));
        assert!(dispatcher.is_disabled("teleport"));
    }

    #[test]
    fn disabling_a_node_command_also_disables_its_aliases() {
        let mut commands = CommandsConfig::default();
        // `help` 是基于节点的命令，别名有 `h` 和 `?`。
        disabled(&mut commands, "help");

        let manager = PermissionManager::new();
        let dispatcher = default_dispatcher(&manager, &commands);

        assert!(dispatcher.is_disabled("help"));
        assert!(dispatcher.is_disabled("h"));
        assert!(dispatcher.is_disabled("?"));
    }

    #[test]
    fn override_for_unknown_command_is_ignored() {
        let mut commands = CommandsConfig::default();
        // 不存在的命令名（通常是配置中的笔误）
        // 应当被忽略，而不是静默吞掉真正的命令或引发 panic。
        disabled(&mut commands, "notacommand");

        let manager = PermissionManager::new();
        let dispatcher = default_dispatcher(&manager, &commands);

        assert!(!dispatcher.is_disabled("notacommand"));
        assert!(dispatcher.has_command("gamemode"));
    }

    #[test]
    fn override_is_case_insensitive() {
        let mut commands = CommandsConfig::default();
        disabled(&mut commands, "GameMode");

        let manager = PermissionManager::new();
        let dispatcher = default_dispatcher(&manager, &commands);

        assert!(dispatcher.is_disabled("gamemode"));
    }

    #[test]
    fn permission_level_override_rewrites_the_registry_default() {
        let mut commands = CommandsConfig::default();
        // `gamemode` 通常是 2 级；将其提升为仅限所有者。
        permission(&mut commands, "gamemode", PermissionLvl::Four);

        let manager = PermissionManager::new();
        let _dispatcher = default_dispatcher(&manager, &commands);

        let permission = manager
            .get_permission("minecraft:command.gamemode")
            .expect("游戏模式权限应已注册");
        assert_eq!(
            permission.default,
            PermissionDefault::Op(PermissionLvl::Four)
        );
    }

    #[test]
    fn permission_override_resolves_node_command_by_convention() {
        let mut commands = CommandsConfig::default();
        // `kill` 是基于节点的命令，其权限节点未记录在
        // 旧式调度器，因此覆盖实现必须回退到
        // `minecraft:command.kill` 命名约定。
        permission(&mut commands, "kill", PermissionLvl::Four);

        let manager = PermissionManager::new();
        let _dispatcher = default_dispatcher(&manager, &commands);

        let permission = manager
            .get_permission("minecraft:command.kill")
            .expect("kill 权限应已注册");
        assert_eq!(
            permission.default,
            PermissionDefault::Op(PermissionLvl::Four)
        );
    }

    #[test]
    fn permission_level_zero_allows_everyone() {
        let mut commands = CommandsConfig::default();
        permission(&mut commands, "gamemode", PermissionLvl::Zero);

        let manager = PermissionManager::new();
        let _dispatcher = default_dispatcher(&manager, &commands);

        let permission = manager
            .get_permission("minecraft:command.gamemode")
            .expect("游戏模式权限应已注册");
        assert_eq!(permission.default, PermissionDefault::Allow);
    }
}
