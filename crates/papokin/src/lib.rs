#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
#![allow(clippy::significant_drop_in_scrutinee)]
// 非警告事件发送宏
#![allow(unused_labels, deprecated)]

#[macro_use]
extern crate papokin_macros;

use crate::crash::CrashReport;
use crate::data::VanillaData;
use crate::logging::{
    ConsoleWriter, GzipRollingLogger, PapokinCommandCompleter, ReadlineLogWrapper,
};
use crate::net::java::JavaClient;
use crate::net::java::pending::PendingConnection;
use crate::net::{PacketHandlerResult, PacketRateLimiter};
use crate::net::{lan_broadcast::LANBroadcast, query, rcon::RCONServer};
use crate::plugin::server::server_command::ServerCommandEvent;
use crate::server::{Server, ticker::Ticker};
use papokin_config::{AdvancedConfiguration, BasicConfiguration, TelemetryConfig};
use papokin_util::text::TextComponent;
use papokin_util::text::color::{Color, NamedColor};
use plugin::server::server_load::{LoadType, ServerLoadEvent};
use rustyline::Editor;
use rustyline::history::FileHistory;
use rustyline::{Config, error::ReadlineError};
use std::io::{ErrorKind, IsTerminal, stdin};
use std::process::exit;
use std::str::FromStr;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::select;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{debug, error, info, warn};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub mod block;
pub mod command;
pub mod crash;
pub mod data;
pub mod enchantment;
pub mod entity;
pub mod error;
pub mod http_client;
pub mod item;
pub mod logging;
pub mod net;
pub mod plugin;
pub mod server;
pub mod telemetry;
pub mod world;

pub struct LoggingConfig {
    pub color: bool,
    pub threads: bool,
    pub thread_ids: bool,
    pub target: bool,
    pub timestamp: bool,
}

pub type LoggerOption = Option<(ReadlineLogWrapper, LevelFilter, LoggingConfig)>;
pub static LOGGER_IMPL: LazyLock<Arc<OnceLock<LoggerOption>>> =
    LazyLock::new(|| Arc::new(OnceLock::new()));

#[expect(clippy::print_stderr, clippy::too_many_lines)]
pub fn init_logger(advanced_config: &AdvancedConfiguration) {
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::fmt;

    let logger = advanced_config.logging.enabled.then(|| {
        let level = std::env::var("RUST_LOG")
            .ok()
            .as_deref()
            .or(Some(advanced_config.logging.level.as_str()))
            .map(LevelFilter::from_str)
            .and_then(Result::ok)
            .unwrap_or(LevelFilter::INFO);

        let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            let level_str = match level {
                LevelFilter::OFF => "off",
                LevelFilter::ERROR => "error",
                LevelFilter::WARN => "warn",
                LevelFilter::INFO => "info",
                LevelFilter::DEBUG => "debug",
                LevelFilter::TRACE => "trace",
            };
            EnvFilter::new(level_str)
        });

        let file_logger: Option<GzipRollingLogger> = if advanced_config.logging.file.is_empty() {
            None
        } else {
            match GzipRollingLogger::new(level, advanced_config.logging.file.clone()) {
                Ok(logger) => Some(logger),
                Err(err) => {
                    error!("初始化文件日志记录器失败：{err}");
                    None
                }
            }
        };

        let (logger, rl): (
            ConsoleWriter,
            Option<Editor<PapokinCommandCompleter, FileHistory>>,
        ) = if advanced_config.commands.use_tty && stdin().is_terminal() {
            let rl_config = Config::builder()
                .auto_add_history(true)
                .completion_type(rustyline::CompletionType::List)
                .edit_mode(rustyline::EditMode::Emacs)
                .build();
            let helper = PapokinCommandCompleter::new();

            match Editor::with_config(rl_config) {
                Ok(mut rl) => {
                    rl.set_helper(Some(helper));
                    let printer = rl.create_external_printer().ok().map(|p| {
                        let boxed: Box<dyn rustyline::ExternalPrinter + Send> = Box::new(p);
                        boxed
                    });
                    (ConsoleWriter::new(printer), Some(rl))
                }
                Err(e) => {
                    eprintln!("初始化控制台输入失败（{e}）；回退到简单日志记录器");
                    (ConsoleWriter::new(None), None)
                }
            }
        } else {
            (ConsoleWriter::new(None), None)
        };

        let fmt_layer = fmt::layer()
            .with_writer(std::sync::Mutex::new(logger))
            .with_ansi(advanced_config.logging.color)
            .with_ansi_sanitization(false)
            .with_target(advanced_config.logging.target)
            .with_thread_names(advanced_config.logging.threads)
            .with_thread_ids(advanced_config.logging.thread_ids);

        if advanced_config.logging.timestamp {
            let local_offset =
                time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
            let format_str: &'static str = Box::leak(
                advanced_config
                    .logging
                    .timestamp_format
                    .clone()
                    .into_boxed_str(),
            );
            let timer_format = time::format_description::parse(format_str).unwrap_or_else(|_| {
                time::macros::format_description!("[hour]:[minute]:[second]").to_vec()
            });
            let fmt_layer =
                fmt_layer.with_timer(fmt::time::OffsetTime::new(local_offset, timer_format));
            let registry = tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt_layer);
            if let Some(file_logger) = file_logger {
                registry.with(file_logger).init();
            } else {
                registry.init();
            }
        } else {
            let fmt_layer = fmt_layer.without_time();
            let registry = tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt_layer);
            if let Some(file_logger) = file_logger {
                registry.with(file_logger).init();
            } else {
                registry.init();
            }
        }

        let logging_config = LoggingConfig {
            color: advanced_config.logging.color,
            threads: advanced_config.logging.threads,
            thread_ids: advanced_config.logging.thread_ids,
            target: advanced_config.logging.target,
            timestamp: advanced_config.logging.timestamp,
        };

        (ReadlineLogWrapper::new(rl), level, logging_config)
    });

    assert!(
        LOGGER_IMPL.set(logger).is_ok(),
        "设置日志记录器失败：已初始化"
    );
}

pub static SHOULD_STOP: AtomicBool = AtomicBool::new(false);
pub static STOP_INTERRUPT: LazyLock<CancellationToken> = LazyLock::new(CancellationToken::new);
pub static SERVER_IS_STOPPING: AtomicBool = AtomicBool::new(false);
pub static CRASH_REPORT: OnceLock<CrashReport> = OnceLock::new();
pub static SERVER_EXIT_CODE: AtomicI32 = AtomicI32::new(0);

pub fn stop_server() {
    SHOULD_STOP.store(true, Ordering::Relaxed);
    STOP_INTERRUPT.cancel();
}

pub fn stop_or_exit_server() {
    if SERVER_IS_STOPPING.load(Ordering::Acquire) {
        // 服务器已在关停中，因此我们强制退出。
        exit(SERVER_EXIT_CODE.load(Ordering::Acquire));
    }
    stop_server();
}

fn resolve_some<T: Future, D, F: FnOnce(D) -> T>(
    opt: Option<D>,
    func: F,
) -> futures::future::Either<T, std::future::Pending<T::Output>> {
    use futures::future::Either;
    opt.map_or_else(
        || Either::Right(std::future::pending()),
        |val| Either::Left(func(val)),
    )
}

pub struct PapokinServer {
    pub server: Arc<Server>,
    pub tcp_listener: Option<TcpListener>,
}

impl PapokinServer {
    pub fn log_info(&self, message: &str) {
        tracing::info!(target: "plugin", "{}", message);
    }
    pub async fn new(
        basic_config: BasicConfiguration,
        advanced_config: AdvancedConfiguration,
        telemetry_config: TelemetryConfig,
        vanilla_data: VanillaData,
    ) -> Self {
        let server = Server::new(
            basic_config,
            advanced_config,
            telemetry_config,
            vanilla_data,
        )
        .await;

        #[cfg(target_family = "unix")]
        adjust_file_descriptor_limit();

        let rcon = server.advanced_config.networking.rcon.clone();

        if rcon.enabled {
            warn!(
                "RCON 已启用，但它极不安全：密码和命令均以明文传输，容易被网络上的任何人拦截和利用"
            );
            let rcon_server = server.clone();
            server.spawn_task(async move {
                RCONServer::run(&rcon, rcon_server).await;
            });
        }

        let tcp_listener = if server.advanced_config.networking.java.enabled {
            let address = server.advanced_config.networking.java.address;
            // 设置 TCP 服务器套接字。
            let listener = match TcpListener::bind(address).await {
                Ok(l) => l,
                Err(e) => match e.kind() {
                    ErrorKind::AddrInUse => {
                        error!("错误：地址 {address} 已被占用。");
                        error!("请确认没有另一个服务器实例正在运行");
                        std::process::exit(1);
                    }
                    ErrorKind::PermissionDenied => {
                        error!("错误：绑定到 {address} 时权限被拒绝。");
                        error!("使用 1024 以下的端口可能需要 sudo/管理员权限");
                        std::process::exit(1);
                    }
                    ErrorKind::AddrNotAvailable => {
                        error!("错误：地址 {address} 在本机上不可用");
                        std::process::exit(1);
                    }
                    _ => {
                        error!("在 {address} 上启动 TcpListener 失败：{e}");
                        std::process::exit(1);
                    }
                },
            };
            // 如果用户将端口设为 0，这能让我们知道它实际运行在哪个端口上
            let addr = listener.local_addr().unwrap_or_else(|_| {
                std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), 0)
            });

            if server.advanced_config.networking.query.enabled {
                info!("Query 协议已启用。正在启动...");
                server.spawn_task(query::start_query_handler(
                    server.clone(),
                    server.advanced_config.networking.query.address,
                ));
            }

            if server.advanced_config.networking.lan_broadcast.enabled {
                info!("LAN 广播已启用。正在启动...");

                let lan_broadcast = LANBroadcast::new(
                    &server.advanced_config.networking.lan_broadcast,
                    &server.advanced_config.networking.java.motd,
                );
                server.spawn_task(lan_broadcast.start(addr));
            }

            Some(listener)
        } else {
            None
        };

        // 计时器
        {
            let ticker_server = server.clone();
            if let Err(err) = std::thread::Builder::new()
                .name("Server-Ticker".into())
                .spawn(move || {
                    Ticker::run(&ticker_server);
                })
            {
                error!("启动 Server-Ticker 线程失败：{err}");
                std::process::exit(1);
            }
        };

        Self {
            server,
            tcp_listener,
        }
    }

    pub async fn init_plugins(&self) -> std::time::Duration {
        if !self.server.advanced_config.plugins.enabled {
            info!("插件系统已在配置中禁用。");
            return std::time::Duration::ZERO;
        }

        let duration = match self
            .server
            .plugin_manager
            .load_plugins(&self.server, crate::plugin::api::LoadOrder::PostWorld)
            .await
        {
            Ok(duration) => duration,
            Err(err) => {
                error!("{err}");
                std::time::Duration::ZERO
            }
        };

        if self.server.advanced_config.plugins.hot_reload {
            if let Err(err) = self.server.plugin_manager.start_watcher(&self.server).await {
                error!("启动插件热重载监视器失败：{err}");
            } else {
                info!("已根据配置启动插件热重载监视器。");
            }
        }

        duration
    }

    pub async fn unload_plugins(&self) {
        if let Err(err) = self.server.plugin_manager.unload_all_plugins().await {
            error!("卸载插件时出错：{err}");
        } else {
            info!("所有插件已成功卸载");
        }
    }

    pub async fn start(&self) {
        if self.server.advanced_config.commands.use_console
            && let Some((wrapper, _, _)) = LOGGER_IMPL.wait()
        {
            if let Some(rl) = wrapper.take_readline() {
                setup_console(rl, self.server.clone());
            } else {
                if self.server.advanced_config.commands.use_tty {
                    warn!("输入不是 TTY；回退到简单日志记录器并忽略 `use_tty` 设置");
                }
                setup_stdin_console(&self.server);
            }
        }

        let tasks = Arc::new(TaskTracker::new());
        let mut master_client_id: u64 = 0;

        self.server
            .plugin_manager
            .fire(&self.server, &mut ServerLoadEvent::new(LoadType::Startup))
            .await;

        self.server.start_telemetry();

        while !SHOULD_STOP.load(Ordering::Relaxed) {
            if !self
                .unified_listener_task(&mut master_client_id, &tasks)
                .await
            {
                break;
            }
        }

        SERVER_IS_STOPPING.store(true, Ordering::Release);

        if let Some(crash_report) = CRASH_REPORT.get() {
            crash_report.print_to_console();
            crash_report.save_and_log();

            info!(
                "{}",
                TextComponent::text("正在优雅关停...")
                    .color(Color::Named(NamedColor::Green))
                    .to_pretty_console()
            );

            SERVER_EXIT_CODE.store(1, Ordering::Release);
        }

        info!("已停止接受新连接");

        if let Err(e) = self
            .server
            .player_data_storage
            .save_all_players(&self.server)
        {
            error!("关停期间保存所有玩家数据时出错：{e}");
        }

        if let Err(e) = self
            .server
            .advancement_manager
            .save_all_players(&self.server.get_all_players())
            .await
        {
            error!("关停期间保存所有玩家进度时出错：{e}");
        }

        let kick_message = TextComponent::text("服务器已停止");
        for player in self.server.get_all_players() {
            player.kick(&kick_message);
        }

        info!("正在结束玩家任务");

        tasks.close();
        tasks.wait().await;

        self.unload_plugins().await;

        info!("开始保存。");

        self.server.shutdown().await;

        info!("保存完成！");

        if let Some((wrapper, _, _)) = LOGGER_IMPL.wait()
            && let Some(rl) = wrapper.take_readline()
        {
            let _ = rl;
        }
    }

    #[allow(clippy::too_many_lines)]
    pub async fn unified_listener_task(
        &self,
        master_client_id_counter: &mut u64,
        tasks: &Arc<TaskTracker>,
    ) -> bool {
        select! {
            // 用于 TCP 连接的分支
            tcp_result = resolve_some(self.tcp_listener.as_ref(), tokio::net::TcpListener::accept) => {
                match tcp_result {
                    Ok((connection, client_addr)) => {
                        if let Err(e) = connection.set_nodelay(true) {
                            warn!("设置 TCP_NODELAY 失败：{e}");
                        }

                        let client_id = *master_client_id_counter;
                        *master_client_id_counter += 1;

                        let formatted_address = if self.server.basic_config.scrub_ips {
                            scrub_address(&format!("{client_addr}"))
                        } else {
                            format!("{client_addr}")
                        };
                        debug!("已接受来自 {formatted_address} 的连接（id {client_id}）");
                        let server_clone = self.server.clone();

                        tasks.spawn(async move {
                            let packet_limiter = PacketRateLimiter::from_config(
                                &server_clone.advanced_config.networking.java.packet_limiter,
                            );
                            let mut pending = PendingConnection::new(
                                connection,
                                client_addr,
                                client_id,
                                packet_limiter,
                            );
                            let login_result = pending.handle_login_sequence(&server_clone).await;

                            match login_result {
                                PacketHandlerResult::Stop => {
                                     pending.close();
                                },
                                PacketHandlerResult::ReadyToPlay(profile, config) => {
                                     let mut java_client = JavaClient::from_pending(pending, profile.clone(), config.clone());
                                     java_client.start_outgoing_packet_task();

                                     if let Some((player, world)) = server_clone
                                         .add_player(Arc::new(java_client), profile, Some(config))
                                 {
                                     let client = player.client.clone();
                                     client.set_player(player.clone());
                                     world
                                         .spawn_java_player(&server_clone.basic_config, &player, &server_clone)
                                         .await;

                                     client.progress_player_packets(&player, &server_clone).await;

                                     // 完成后关闭
                                     client.close();
                                     client.await_tasks().await;
                                     player.remove().await;
                                     server_clone.remove_player(&player);
                                    if let Err(e) = server_clone
                                        .player_data_storage
                                        .handle_player_leave(&player)
                                    {
                                        error!("断开连接时保存玩家数据失败：{e}");
                                    }
                                    if let Err(e) = server_clone.advancement_manager
                                        .save_player(&player)
                                        .await {
                                            error!("断开连接时保存玩家进度失败：{e}");
                                        }
                                    }
                                },
                            }
                        });
                    }
                    Err(e) => {
                        #[cfg(target_family = "unix")]
                        if e.raw_os_error() == Some(libc::EMFILE) {
                            error!(
                                "打开的文件过多！服务器已达到文件描述符上限。\
                                在现有连接关闭或提高 `ulimit -n` 之前，无法接受新连接。"
                            );
                            sleep(Duration::from_millis(500)).await;
                            return true;
                        }
                        error!("接受客户端连接失败：{e}");
                        sleep(Duration::from_millis(50)).await;
                    }
                }
            },

            // 用于全局停止信号的分支
            () = STOP_INTERRUPT.cancelled() => {
                return false;
            }
        }
        true
    }
}

fn setup_stdin_console(server: &Arc<Server>) {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let rt = tokio::runtime::Handle::current();
    std::thread::spawn(move || {
        while !SHOULD_STOP.load(Ordering::Relaxed) {
            let mut line = String::new();
            if let Ok(size) = stdin().read_line(&mut line) {
                // 若一个字节都没读到，则可能已到达 EOF
                if size == 0 {
                    break;
                }
            } else {
                break;
            }
            if line.is_empty() || line.as_bytes()[line.len() - 1] != b'\n' {
                warn!("控制台命令未以换行符结尾");
            }
            let _ = rt.block_on(tx.send(line.trim().to_string()));
        }
    });
    let server_clone = server.clone();
    server.spawn_task(async move {
        while !SHOULD_STOP.load(Ordering::Relaxed)
            && let Some(command) = rx.recv().await
        {
            let mut event = ServerCommandEvent::new(command.clone());
            server_clone
                .plugin_manager
                .fire(&server_clone, &mut event)
                .await;
            if !event.cancelled {
                server_clone.command_dispatcher.load().handle_command(
                    &command::CommandSender::Console.into_source(&server_clone),
                    command.as_str(),
                );
            }
        }
    });
}

fn setup_console(mut rl: Editor<PapokinCommandCompleter, FileHistory>, server: Arc<Server>) {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let (tx_reply, mut rx_reply) = tokio::sync::mpsc::channel(1);

    if let Some(helper) = rl.helper_mut() {
        if let Ok(mut server_lock) = helper.server.write() {
            *server_lock = Some(server.clone());
        }
        let _ = helper.rt.set(tokio::runtime::Handle::current());
    }

    std::thread::spawn(move || {
        while !SHOULD_STOP.load(Ordering::Relaxed) {
            let readline = rl.readline("$ ");
            match readline {
                Ok(line) => {
                    let _ = rl.add_history_entry(line.clone());
                    if tx.blocking_send(line).is_err() {
                        break;
                    }

                    // 等待命令被完全处理后再继续
                    let _ = rx_reply.blocking_recv();
                }
                Err(ReadlineError::Interrupted) => {
                    info!("CTRL-C");
                    stop_or_exit_server();
                    break;
                }
                Err(ReadlineError::Eof) => {
                    info!("CTRL-D");
                    stop_server();
                    break;
                }
                Err(err) => {
                    error!("读取控制台输入时出错：{err}");
                    break;
                }
            }
        }
        if let Some((wrapper, _, _)) = LOGGER_IMPL.wait() {
            wrapper.return_readline(rl);
        }
    });

    server.clone().spawn_task(async move {
        while !SHOULD_STOP.load(Ordering::Relaxed) {
            let t1 = rx.recv();
            let t2 = STOP_INTERRUPT.cancelled();

            let result = select! {
                line = t1 => line,
                () = t2 => None,
            };

            if let Some(line) = result {
                let mut event = ServerCommandEvent::new(line.clone());
                server.plugin_manager.fire(&server, &mut event).await;
                if !event.cancelled {
                    server.command_dispatcher.load().handle_command(
                        &command::CommandSender::Console.into_source(&server),
                        &line,
                    );
                }
                let _ = tx_reply.send(1).await;
            } else {
                break;
            }
        }
        drop(rx);
        debug!("控制台命令任务已停止");
    });
}

fn scrub_address(ip: &str) -> String {
    ip.chars()
        .map(|ch| if ch == '.' || ch == ':' { ch } else { 'x' })
        .collect()
}

#[cfg(target_family = "unix")]
fn adjust_file_descriptor_limit() {
    let mut rlim = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };

    // SAFETY: 传递指向栈上分配的 `rlimit` 结构体的有效可变指针。
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &raw mut rlim) } != 0 {
        return;
    }

    let max_target = if rlim.rlim_max == libc::RLIM_INFINITY {
        1_048_576
    } else {
        rlim.rlim_max
    };

    if rlim.rlim_cur < max_target {
        let old_limit = rlim.rlim_cur;
        rlim.rlim_cur = max_target;

        // SAFETY: 以有效的资源和指向已初始化 `rlimit` 的有效指针调用 `setrlimit`。
        let res = unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &raw const rlim) };
        if res == 0 {
            debug!("已将文件描述符上限从 {old_limit} 提高到 {max_target}");
        } else {
            // 回退：若 max_target 被操作系统拒绝，尝试设为合理的高值（65,536）
            let fallback = 65_536.min(max_target);
            if fallback > old_limit {
                rlim.rlim_cur = fallback;
                // SAFETY: 以有效的资源和指向已初始化 `rlimit` 的有效指针调用 `setrlimit`。
                if unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &raw const rlim) } == 0 {
                    debug!("已将文件描述符上限从 {old_limit} 提高到 {fallback}");
                }
            }
        }
    }

    let mut current_rlim = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: 传递指向栈上分配的 `rlimit` 结构体的有效可变指针。
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &raw mut current_rlim) } == 0
        && current_rlim.rlim_cur < 4096
    {
        warn!(
            "文件描述符上限较低（{}）。支持超过 1000 名并发玩家可能会失败并提示 'Too many open files'。\
            建议使用 `ulimit -n 65535` 提高上限，或在 systemd 中设置 `LimitNOFILE=65535`。",
            current_rlim.rlim_cur
        );
    }
}
