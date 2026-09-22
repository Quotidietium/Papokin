#![allow(clippy::print_stderr)]
#![allow(clippy::print_stdout)]

use flate2::write::GzEncoder;
use rustyline::completion::Completer;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::history::FileHistory;
use rustyline::validate::Validator;
use rustyline::{Editor, Helper};
use std::borrow::Cow;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;
use std::sync::Arc;
use time::{Duration, OffsetDateTime};
use tracing::Subscriber;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::LevelFilter;

use crate::command::CommandSender;
use crate::command::string_reader::StringReader;
use crate::server::Server;

#[macro_export]
macro_rules! log_at_level {
    ($level:expr, $($arg:tt)*) => {
        match $level {
            tracing::Level::TRACE => tracing::trace!($($arg)*),
            tracing::Level::DEBUG => tracing::debug!($($arg)*),
            tracing::Level::INFO => tracing::info!($($arg)*),
            tracing::Level::WARN => tracing::warn!($($arg)*),
            tracing::Level::ERROR => tracing::error!($($arg)*),
        }
    };
}

#[macro_export]
macro_rules! plugin_log {
    ($level:expr, $plugin_name:expr, $($arg:tt)*) => {{
        let plugin_name = $plugin_name;
        match $level {
            tracing::Level::TRACE => {
                tracing::trace!(
                    target: "pumpkin_plugin",
                    plugin = plugin_name,
                    $($arg)*
                )
            },
            tracing::Level::DEBUG => {
                tracing::debug!(
                    target: "pumpkin_plugin",
                    plugin = plugin_name,
                    $($arg)*
                )
            },
            tracing::Level::INFO => {
                tracing::info!(
                    target: "pumpkin_plugin",
                    plugin = plugin_name,
                    $($arg)*
                )
            },
            tracing::Level::WARN => {
                tracing::warn!(
                    target: "pumpkin_plugin",
                    plugin = plugin_name,
                    $($arg)*
                )
            },
            tracing::Level::ERROR => {
                tracing::error!(
                    target: "pumpkin_plugin",
                    plugin = plugin_name,
                    $($arg)*
                )
            },
        }
    }};
}

const LOG_DIR: &str = "logs";
const MAX_ATTEMPTS: u32 = 1000;

/// 我们日志记录器的一个包装器，在没有预期输入时保存终端输入，以便
/// 在日志产生时正确刷新到输出，而不是批量
pub struct ReadlineLogWrapper {
    readline: std::sync::Mutex<Option<Editor<PapokinCommandCompleter, FileHistory>>>,
}

pub struct ConsoleWriter {
    printer: Option<Box<dyn rustyline::ExternalPrinter + Send>>,
    buffer: Vec<u8>,
}

impl ConsoleWriter {
    #[must_use]
    pub fn new(printer: Option<Box<dyn rustyline::ExternalPrinter + Send>>) -> Self {
        Self {
            printer,
            buffer: Vec::new(),
        }
    }
}

impl Write for ConsoleWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Some(ref mut printer) = self.printer {
            self.buffer.extend_from_slice(buf);
            while let Some(pos) = self.buffer.iter().position(|&b| b == b'\n') {
                let line_bytes: Vec<u8> = self.buffer.drain(..=pos).collect();
                let msg = String::from_utf8_lossy(&line_bytes).into_owned();
                if printer.print(msg).is_err() {
                    let mut stdout = io::stdout().lock();
                    let _ = stdout.write_all(line_bytes.as_slice());
                    let _ = stdout.flush();
                }
            }
            Ok(buf.len())
        } else {
            io::stdout().write(buf)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(ref mut printer) = self.printer {
            if !self.buffer.is_empty() {
                let buffer = std::mem::take(&mut self.buffer);
                let msg = String::from_utf8_lossy(&buffer).into_owned();
                if printer.print(msg).is_err() {
                    let mut stdout = io::stdout().lock();
                    let _ = stdout.write_all(buffer.as_slice());
                    let _ = stdout.flush();
                }
            }
            Ok(())
        } else {
            io::stdout().flush()
        }
    }
}

struct GzipRollingLoggerData {
    pub current_day_of_month: u8,
    pub last_rotate_time: time::OffsetDateTime,
    pub file: BufWriter<File>,
    latest_filename: String,
}

pub struct GzipRollingLogger {
    log_level: LevelFilter,
    data: std::sync::Mutex<GzipRollingLoggerData>,
}

impl GzipRollingLogger {
    pub fn new(
        log_level: LevelFilter,
        filename: String,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let now = time::OffsetDateTime::now_utc();
        std::fs::create_dir_all(LOG_DIR)?;

        let latest_path = PathBuf::from(LOG_DIR).join(&filename);

        // 如果 latest.log 存在，我们会将其 gzip 压缩
        if latest_path.exists() {
            eprintln!(
                "发现已有日志文件 '{}'，正在压缩为 gzip...",
                latest_path.display()
            );

            let new_gz_path = Self::new_filename(true)?;

            let mut file = File::open(&latest_path)?;

            let mut encoder = GzEncoder::new(
                BufWriter::new(File::create(&new_gz_path)?),
                flate2::Compression::best(),
            );

            io::copy(&mut file, &mut encoder)?;
            encoder.finish()?;

            std::fs::remove_file(&latest_path)?;
        }

        let file = BufWriter::new(File::create(&latest_path)?);

        Ok(Self {
            log_level,
            data: std::sync::Mutex::new(GzipRollingLoggerData {
                current_day_of_month: now.day(),
                last_rotate_time: now,
                latest_filename: filename,
                file,
            }),
        })
    }

    pub fn new_filename(yesterday: bool) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let mut now = OffsetDateTime::now_utc();

        if yesterday {
            now -= Duration::days(1);
        }

        let date_format = format!("{}-{:02}-{:02}", now.year(), now.month() as u8, now.day());

        let log_path = PathBuf::from(LOG_DIR);

        let mut oldest_log = None;

        for id in 1..=MAX_ATTEMPTS {
            let filename = log_path.join(format!("{date_format}-{id}.log.gz"));

            if !filename.exists() {
                return Ok(filename);
            }

            let Ok(modified_time) = filename.metadata().and_then(|m| m.modified()) else {
                continue;
            };

            if let Some((_, old_time)) = oldest_log {
                if modified_time < old_time {
                    oldest_log = Some((filename, modified_time));
                }

                continue;
            }

            oldest_log = Some((filename, modified_time));
        }

        if let Some((path, _)) = oldest_log {
            eprintln!(
                "{date_format} 的日志编号已达上限（{MAX_ATTEMPTS}）；覆盖最旧的日志文件：{}",
                path.display()
            );
            return Ok(path);
        }

        Err(
            format!("尝试 {MAX_ATTEMPTS} 次后仍无法为日期 {date_format} 找到唯一的日志文件名。")
                .into(),
        )
    }

    fn rotate_log(&self) -> Result<(), Box<dyn std::error::Error>> {
        let now = time::OffsetDateTime::now_utc();
        let mut data = self
            .data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let new_gz_path = Self::new_filename(true)?;
        let latest_path = PathBuf::from(LOG_DIR).join(&data.latest_filename);

        // 刷新并丢弃当前文件
        data.file.flush()?;
        drop(std::mem::replace(
            &mut data.file,
            BufWriter::new(File::create("/dev/null")?),
        ));

        let mut file = File::open(&latest_path)?;
        let mut encoder = GzEncoder::new(
            BufWriter::new(File::create(&new_gz_path)?),
            flate2::Compression::best(),
        );
        io::copy(&mut file, &mut encoder)?;
        encoder.finish()?;

        data.current_day_of_month = now.day();
        data.last_rotate_time = now;
        data.file = BufWriter::new(File::create(&latest_path)?);
        Ok(())
    }
}

fn remove_ansi_color_code(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut it = s.chars();

    while let Some(c) = it.next() {
        if c == '\x1b' {
            for c_seq in it.by_ref() {
                if c_seq.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

impl<S> Layer<S> for GzipRollingLogger
where
    S: Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();
        let level = metadata.level();

        // 检查是否应根据级别记录此事件
        let should_log = match *level {
            tracing::Level::ERROR => self.log_level >= LevelFilter::ERROR,
            tracing::Level::WARN => self.log_level >= LevelFilter::WARN,
            tracing::Level::INFO => self.log_level >= LevelFilter::INFO,
            tracing::Level::DEBUG => self.log_level >= LevelFilter::DEBUG,
            tracing::Level::TRACE => self.log_level >= LevelFilter::TRACE,
        };

        if !should_log {
            return;
        }

        let now = time::OffsetDateTime::now_utc();

        if let Ok(mut data) = self.data.lock() {
            // 格式化事件
            let mut visitor = StringVisitor::default();
            event.record(&mut visitor);
            let message = visitor.0;

            let clean_message = remove_ansi_color_code(&message);

            // 写入文件
            let _ = writeln!(data.file, "[{level}] {clean_message}");
            let _ = data.file.flush();

            // 检查是否需要旋转
            if data.current_day_of_month != now.day() {
                drop(data);
                if let Err(e) = self.rotate_log() {
                    eprintln!("轮转日志失败：{e}");
                }
            }
        }
    }
}

#[derive(Default)]
struct StringVisitor(String);

impl tracing::field::Visit for StringVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = format!("{value:?}");
            // 如存在则移除引号
            if self.0.starts_with('"') && self.0.ends_with('"') {
                self.0 = self.0[1..self.0.len() - 1].to_string();
            }
        }
    }
}

impl ReadlineLogWrapper {
    #[must_use]
    pub const fn new(rl: Option<Editor<PapokinCommandCompleter, FileHistory>>) -> Self {
        Self {
            readline: std::sync::Mutex::new(rl),
        }
    }

    pub fn take_readline(&self) -> Option<Editor<PapokinCommandCompleter, FileHistory>> {
        self.readline
            .lock()
            .map_or_else(|_| None, |mut result| result.take())
    }

    // 这并不是真正的死代码。它只是只被库（lib）使用而不被可执行文件（bin）使用，针对的是
    // crate，因此会产生编译器警告。
    #[allow(dead_code)]
    pub fn return_readline(&self, rl: Editor<PapokinCommandCompleter, FileHistory>) {
        if let Ok(mut result) = self.readline.lock() {
            let _ = result.insert(rl);
        }
    }
}

#[derive(Clone, Default)]
pub struct PapokinCommandCompleter {
    pub server: Arc<std::sync::RwLock<Option<Arc<Server>>>>,
    pub rt: Arc<std::sync::OnceLock<tokio::runtime::Handle>>,
}

impl PapokinCommandCompleter {
    #[must_use]
    pub fn new() -> Self {
        Self {
            server: Arc::new(std::sync::RwLock::new(None)),
            rt: Arc::new(std::sync::OnceLock::new()),
        }
    }
}

impl Helper for PapokinCommandCompleter {}
impl Highlighter for PapokinCommandCompleter {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        line.find(' ').map_or_else(
            || Cow::Owned(format!("\x1b[1;36m{line}\x1b[0m")),
            |first_space| {
                let (cmd, args) = line.split_at(first_space);
                Cow::Owned(format!("\x1b[1;36m{cmd}\x1b[0m{args}"))
            },
        )
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Cow::Owned(format!("\x1b[90m{hint}\x1b[0m"))
    }
}
impl Hinter for PapokinCommandCompleter {
    type Hint = String;
    fn hint(&self, line: &str, pos: usize, ctx: &rustyline::Context<'_>) -> Option<Self::Hint> {
        if line.is_empty() || pos < line.len() {
            return None;
        }

        if let Ok((_, candidates)) = self.complete(line, pos, ctx)
            && let Some(first) = candidates.first()
        {
            let last_word = line.split_whitespace().last().unwrap_or("");
            if first.starts_with('<') {
                return line.ends_with(' ').then(|| first.clone());
            }

            if let Some(stripped) = first.strip_prefix(last_word) {
                return Some(stripped.to_string());
            }
        }
        None
    }
}

impl Validator for PapokinCommandCompleter {}

impl Completer for PapokinCommandCompleter {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        let cmd_to_cursor = &line[..pos];
        let has_slash = cmd_to_cursor.starts_with('/');
        let cmd = if has_slash {
            &cmd_to_cursor[1..]
        } else {
            cmd_to_cursor
        };

        let Ok(server_guard) = self.server.try_read() else {
            return Ok((0, Vec::new()));
        };
        let Some(server) = server_guard.as_ref() else {
            return Ok((0, Vec::new()));
        };

        let dispatcher = server.command_dispatcher.load();
        let source = CommandSender::Console.into_source(server);

        // 目前临时统一两个调度器的设置：

        {
            if cmd.trim().is_empty() {
                // 将所有命令作为建议提供。

                let suggestions: Vec<String> = dispatcher
                    .get_all_commands()
                    .keys()
                    .map(ToString::to_string)
                    .collect();
                return Ok((pos, suggestions));
            }
        }

        // 不确定这是否必要，但宁可稳妥起见。
        if let Some(cursor) = pos.checked_sub(usize::from(has_slash)) {
            let mut reader = StringReader::new(cmd);
            if reader.peek() == Some('/') {
                reader.skip();
            }
            let parsed = dispatcher.parse(&mut reader, &source);
            let suggestions = dispatcher.get_completion_suggestions(parsed, cursor);

            if !suggestions.is_empty() {
                let start = suggestions.range.start;
                let suggestions = suggestions
                    .suggestions
                    .into_iter()
                    .map(|s| s.text.cached_text().clone())
                    .collect();
                return Ok((start + usize::from(has_slash), suggestions));
            }
        }

        Ok((0, Vec::new()))
    }
}
