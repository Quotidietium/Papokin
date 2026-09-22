use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
};

pub use crate::wit::papokin::plugin::command::Command;
use crate::{
    Result, Server,
    command::CommandNode,
    wit::papokin::plugin::command::{CommandError, CommandSender, ConsumedArgs},
};

pub(crate) static NEXT_COMMAND_ID: AtomicU32 = AtomicU32::new(0);
pub(crate) static COMMAND_HANDLERS: Mutex<BTreeMap<u32, Arc<dyn CommandHandler>>> =
    Mutex::new(BTreeMap::new());
pub(crate) static COMMAND_SUGGESTION_HANDLERS: Mutex<
    BTreeMap<u32, Arc<dyn CommandSuggestionHandler>>,
> = Mutex::new(BTreeMap::new());

/// 处理已注册命令的执行。
///
/// 实现此 trait 以定义命令被调用时执行的逻辑。
/// 返回值是传回服务器的退出码；返回 `Ok(0)` 表示
/// 成功，或返回 [`Err`] 变体以向发送者报告失败消息。
pub trait CommandHandler: Send + Sync {
    /// 执行命令。
    ///
    /// # Arguments
    /// - `sender`——命令的调用者（玩家或控制台）。
    /// - `server`——服务器句柄。
    /// - `args`——本次命令调用解析出的参数映射。
    fn handle(
        &self,
        sender: CommandSender,
        server: Server,
        args: ConsumedArgs,
    ) -> Result<i32, CommandError>;
}

/// 处理已注册命令参数的服务器端建议。
///
/// ```rust,ignore
/// use papokin_plugin_api::command::{CommandSuggestion, CommandSuggestions, SuggestionRequest};
/// use papokin_plugin_api::commands::CommandSuggestionHandler;
/// use papokin_plugin_api::Server;
///
/// struct PatternSuggestions;
///
/// impl CommandSuggestionHandler for PatternSuggestions {
///     fn suggest(
///         &self,
///         _sender: papokin_plugin_api::command::CommandSender,
///         _server: Server,
///         request: SuggestionRequest,
///     ) -> CommandSuggestions {
///         let token_start = request
///             .input
///             .rfind([',', ' '])
///             .map_or(request.start as usize, |index| index + 1);
///         let block_start = request.input[token_start..]
///             .rfind('%')
///             .map_or(token_start, |index| token_start + index + 1);
///         let prefix = &request.input[block_start..];
///         let values = ["stone", "stripped_oak_log", "dirt", "diamond_block"]
///             .into_iter()
///             .filter(|block| block.starts_with(prefix))
///             .map(|block| CommandSuggestion {
///                 value: block.to_string(),
///                 tooltip: None,
///             })
///             .collect();
///
///         CommandSuggestions {
///             start: block_start as u32,
///             length: (request.input.len() - block_start) as u32,
///             values,
///         }
///     }
/// }
/// ```
pub trait CommandSuggestionHandler: Send + Sync {
    /// 为当前命令输入计算建议。
    ///
    /// `request.remaining` 包含当前被选区覆盖的替换文本
    /// 建议范围。当只有部分匹配时，处理器可返回更窄的范围
    /// 参数的哪部分应被替换，例如在
    /// 加权方块模式。
    fn suggest(
        &self,
        sender: CommandSender,
        server: Server,
        request: SuggestionRequest,
    ) -> CommandSuggestions;
}

impl Command {
    /// 为该命令附加执行处理器。
    ///
    /// 注册 `handler`，使其在该命令被调用时执行。
    ///返回 `self`，以支持构建器风格的链式调用。
    pub fn execute<H: CommandHandler + Send + Sync + 'static>(self, handler: H) -> Self {
        let id = NEXT_COMMAND_ID.fetch_add(1, Ordering::Relaxed);

        COMMAND_HANDLERS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, Arc::new(handler));

        self.execute_with_handler_id(id)
    }
}

impl CommandNode {
    /// 为该命令节点附加执行处理器。
    ///
    /// 注册 `handler`，使该特定节点（子命令
    /// 或参数分支）是命令分发时最终匹配到的节点。
    ///返回 `self`，以支持构建器风格的链式调用。
    pub fn execute<H: CommandHandler + Send + Sync + 'static>(self, handler: H) -> Self {
        let id = NEXT_COMMAND_ID.fetch_add(1, Ordering::Relaxed);

        COMMAND_HANDLERS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, Arc::new(handler));

        self.execute_with_handler_id(id)
    }

    /// 为该参数节点附加服务器端建议处理器。
    ///
    /// 该节点以 `minecraft:ask_server` 向 Java 客户端通告，并且
    /// 只要客户端为此请求补全，处理函数就会被调用
    /// 参数。
    pub fn suggest<H: CommandSuggestionHandler + Send + Sync + 'static>(self, handler: H) -> Self {
        let id = NEXT_COMMAND_ID.fetch_add(1, Ordering::Relaxed);

        COMMAND_SUGGESTION_HANDLERS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, Arc::new(handler));

        self.suggest_with_handler_id(id)
    }
}

pub use crate::wit::papokin::plugin::command::{
    CommandSuggestion, CommandSuggestions, SuggestionRequest,
};
