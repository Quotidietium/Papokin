#![deny(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
// 不要对事件发送宏告警
#![recursion_limit = "512"]

#[cfg(target_os = "wasi")]
compile_error!("不支持编译到 WASI 目标！");

use papokin_data::packet::CURRENT_MC_VERSION;
use std::{
    backtrace::{Backtrace, BacktraceStatus},
    io::{self},
    panic::PanicHookInfo,
    process::exit,
    sync::{OnceLock, atomic::Ordering},
    thread::{self, ThreadId},
};
#[cfg(not(unix))]
use tokio::signal::ctrl_c;
#[cfg(unix)]
use tokio::signal::unix::{SignalKind, signal};

use papokin::{
    CRASH_REPORT, SERVER_EXIT_CODE, SERVER_IS_STOPPING,
    crash::{CrashReport, FullBacktrace},
    data::VanillaData,
    stop_or_exit_server,
};
use papokin::{PapokinServer, stop_server};

use papokin_config::{LoadConfiguration, PapokinConfig};
use papokin_util::text::{
    TextComponent,
    color::{Color, NamedColor},
};
use std::time::Instant;
use tracing::{debug, info, warn};

const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

static MAIN_THREAD: OnceLock<ThreadId> = OnceLock::new();

// WARNING：从 tokio 运行时发出的所有 rayon 调用都必须是非阻塞的！这包括诸如
// 类似 `par_iter`。这些任务应在 rayon 线程池中生成，然后交给 tokio
// 运行时搭配通道！参见 `Level::fetch_chunks` 示例！
#[allow(clippy::too_many_lines)]
#[tokio::main]
async fn main() {
    let _ = MAIN_THREAD.set(thread::current().id());

    // reqwest 以 `rustls-no-provider` 构建，因此选择 ring 提供者（
    // wasmtime-wasi-http/rtc 已强制要求的那些）之后，才能构造任何客户端。
    let _ = rustls::crypto::ring::default_provider().install_default();

    // 使用具名工作线程初始化全局 Rayon 线程池
    let _ = rayon::ThreadPoolBuilder::new()
        .thread_name(|i| format!("Rayon-Worker-{i}"))
        .build_global();

    // 设置 panic 处理程序。
    std::panic::set_hook(Box::new(handle_panic));

    #[cfg(feature = "console-subscriber")]
    console_subscriber::init();
    let time = Instant::now();

    let exec_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    let config = PapokinConfig::load(&exec_dir);

    let vanilla_data = VanillaData::load();

    papokin::init_logger(&config.advanced);

    info!(
        "{}",
        TextComponent::text(format!(
            "正在启动 {} {}（Minecraft {}；协议 {}）",
            TextComponent::text("Papokin")
                .color_named(NamedColor::Gold)
                .to_pretty_console(),
            TextComponent::text(CARGO_PKG_VERSION.to_string())
                .color_named(NamedColor::Green)
                .to_pretty_console(),
            TextComponent::text(CURRENT_MC_VERSION.to_string())
                .color_named(NamedColor::Gold)
                .to_pretty_console(),
            TextComponent::text(CURRENT_MC_VERSION.protocol_version().to_string())
                .color_named(NamedColor::DarkBlue)
                .to_pretty_console()
        ))
        .to_pretty_console(),
    );

    debug!(
        "构建信息：FAMILY: \"{}\", OS: \"{}\", ARCH: \"{}\", BUILD: \"{}\"",
        std::env::consts::FAMILY,
        std::env::consts::OS,
        std::env::consts::ARCH,
        if cfg!(debug_assertions) {
            "Debug"
        } else {
            "Release"
        }
    );
    if cfg!(debug_assertions) {
        warn!(
            "Papokin 正在运行未优化的调试构建。请勿使用此构建进行性能测试；请运行 `cargo run --release` 或使用发布版二进制文件。"
        );
    }
    print_support_links_and_warning();

    tokio::spawn(async {
        if let Err(err) = setup_sighandler().await {
            tracing::error!("无法设置信号处理器：{err}");
        }
    });

    let papokin_server = PapokinServer::new(
        config.basic,
        config.advanced,
        config.telemetry,
        vanilla_data,
    )
    .await;
    let plugin_wait_time = papokin_server.init_plugins().await;

    let time_elapsed = time.elapsed().saturating_sub(plugin_wait_time);

    info!(
        "服务器已启动，耗时 {}",
        TextComponent::text(format!("{}ms", time_elapsed.as_millis()))
            .color_named(NamedColor::Gold)
            .to_pretty_console()
    );
    let advanced_config = &papokin_server.server.advanced_config;
    if advanced_config.networking.java.enabled {
        info!(
            "服务器现在正在运行。连接端口：{}",
            TextComponent::text(format!("{}", advanced_config.networking.java.address))
                .color_named(NamedColor::DarkBlue)
                .to_pretty_console()
        );
    } else {
        info!("服务器现在正在运行。");
    }

    papokin_server.start().await;

    info!(
        "{}",
        TextComponent::text("服务器已停止。")
            .color_named(NamedColor::Red)
            .to_pretty_console()
    );

    exit(SERVER_EXIT_CODE.load(Ordering::Acquire));
}
fn print_support_links_and_warning() {
    warn!(
        "{}",
        TextComponent::text("Papokin 目前正在高强度开发中！")
            .color_named(NamedColor::DarkRed)
            .to_pretty_console(),
    );
    info!(
        "在 {} 上报告问题",
        TextComponent::text("https://github.com/Quotidietium/Papokin/issues")
            .color_named(NamedColor::DarkAqua)
            .to_pretty_console()
    );
    info!(
        "加入我们的 {} 获取社区支持：{}",
        TextComponent::text("Discord")
            .color_named(NamedColor::DarkBlue)
            .to_pretty_console(),
        TextComponent::text("https://discord.gg/wT8XjrjKkf")
            .color_named(NamedColor::Aqua)
            .to_pretty_console()
    );
    info!(
        "考虑{}：{}",
        TextComponent::text("捐赠")
            .color_named(NamedColor::DarkPurple)
            .to_pretty_console(),
        TextComponent::text("https://pumpkinmc.org/donate/")
            .color_named(NamedColor::Gold)
            .to_pretty_console()
    );
}

fn handle_interrupt() {
    warn!(
        "{}",
        TextComponent::text("收到中断信号；正在停止服务器...")
            .color_named(NamedColor::Red)
            .to_pretty_console()
    );
    stop_or_exit_server();
}

fn handle_panic(panic_info: &PanicHookInfo<'_>) {
    // 生成崩溃报告。
    let crash_report = {
        // 我们在这里捕获回溯信息，而不是在
        // 崩溃报告，使回溯不显示
        // CrashReport 的 `new` 函数。
        let captured_backtrace = Backtrace::capture();
        let full_backtrace = if captured_backtrace.status() == BacktraceStatus::Captured {
            FullBacktrace::Captured
        } else {
            FullBacktrace::ForceCaptured(Backtrace::force_capture())
        };

        CrashReport::new(panic_info, captured_backtrace, full_backtrace)
    };

    let payload = panic_info.payload();

    if is_main_thread() {
        // 这是第一次 panic；
        // 我们无法优雅关停，因为主线程
        // 已发生 panic。不过我们仍然可以生成崩溃报告。

        if let Some(crash_report) = try_set_crash_report(crash_report) {
            crash_report.print_to_console();
            crash_report.save_and_log();

            tracing::error!(
                "{}",
                TextComponent::text("主线程发生 panic，正在中止。")
                    .color(Color::Named(NamedColor::Red))
                    .to_pretty_console()
            );
        } else {
            // 这是后续的 panic。
            tracing::error!(
                "{}: {}",
                TextComponent::text("停止服务器时主线程发生 panic；正在中止。")
                    .color(Color::Named(NamedColor::Red))
                    .bold()
                    .to_pretty_console(),
                payload
                    .downcast_ref::<&str>()
                    .copied()
                    .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
                    .unwrap_or("<unknown>")
            );
        }

        exit(1);
    }

    if try_set_crash_report(crash_report).is_some() {
        // 这是第一次 panic；让我们关停服务器。
        stop_server();
    } else {
        // 这是后续的 panic；只需对其发出警告。
        tracing::error!(
            "{}: {}",
            TextComponent::text("关停过程中遇到 panic")
                .color(Color::Named(NamedColor::Red))
                .bold()
                .to_pretty_console(),
            payload
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
                .unwrap_or("<unknown>")
        );
    }
}

fn is_main_thread() -> bool {
    Some(&thread::current().id()) == MAIN_THREAD.get()
}

///若崩溃报告已成功设置，则返回 `Some`。
/// 意味着这是第一次 panic，必须记录并在稍后保存。
///
///否则返回 `None`，因为随后会发生 panic。
fn try_set_crash_report(crash_report: CrashReport) -> Option<&'static CrashReport> {
    if !SERVER_IS_STOPPING.load(Ordering::Acquire) && CRASH_REPORT.set(crash_report).is_ok() {
        CRASH_REPORT.get()
    } else {
        None
    }
}

// 非 UNIX 的 Ctrl-C 处理
#[cfg(not(unix))]
async fn setup_sighandler() -> io::Result<()> {
    if ctrl_c().await.is_ok() {
        handle_interrupt();
    }

    Ok(())
}

// Unix 信号处理
#[cfg(unix)]
async fn setup_sighandler() -> io::Result<()> {
    let mut interrupt = signal(SignalKind::interrupt())?;
    let mut hangup = signal(SignalKind::hangup())?;
    let mut terminate = signal(SignalKind::terminate())?;

    let received = tokio::select! {
        received = interrupt.recv() => received,
        received = hangup.recv() => received,
        received = terminate.recv() => received,
    };

    if received.is_some() {
        handle_interrupt();
    }

    Ok(())
}
