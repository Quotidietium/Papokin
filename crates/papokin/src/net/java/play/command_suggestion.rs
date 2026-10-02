#[allow(clippy::wildcard_imports)]
use super::*;
use papokin_protocol::java::client::play::CommandSuggestion;

impl JavaClient {
    /// 补全结果数量上限：命令树可枚举出上千条目（注册表 id 等），
    /// 插件事件亦可注入任意列表；截断防止响应包膨胀成 MB 级。
    const MAX_SUGGESTIONS: usize = 256;

    pub fn handle_command_suggestion(
        &self,
        player: &Arc<Player>,
        packet: &SCommandSuggestion<'_>,
        server: &Arc<Server>,
    ) {
        // 补全请求按每 tick 配额限流：全命令树解析有成本，
        // 被修改的客户端可高频刷此包做 DoS
        if !player.try_consume_suggestion_quota() {
            return;
        }

        let Some(cmd) = &packet.command.get(1..) else {
            return;
        };

        // 补全范围：最后一个空格之后为待补全的词。缓冲区无空格时
        // 正在补全的是首词（命令名本身），范围为去掉斜杠后的整个
        // 缓冲区——不能提前返回，否则 "/plu"+TAB 得不到任何建议
        let (start, length) = match cmd.char_indices().rfind(|(_, c)| c.is_whitespace()) {
            Some((last_word_start, _)) => (last_word_start + 2, cmd.len() - last_word_start - 1),
            None => (1, cmd.len()),
        };

        let suggestions = server
            .command_dispatcher
            .load()
            .suggest(cmd, &player.get_command_source(server));

        // 建议钩子：Tab 补全携带原始缓冲区与
        // 计算出的补全（可修改）；send-suggestions 携带
        // 最终列表。两者均可取消。工具提示被丢弃：网络传输
        // 数据为纯字符串。
        let completions: Vec<String> = suggestions
            .into_iter()
            .map(|suggestion| suggestion.suggestion)
            .collect();
        let mut tab_event =
            crate::plugin::api::events::player::async_tab_complete::AsyncTabCompleteEvent::new(
                Some(player.clone()),
                packet.command.to_string(),
                completions,
            );
        server.plugin_manager.fire_blocking(server, &mut tab_event);
        if tab_event.cancelled {
            return;
        }

        let mut send_event = crate::plugin::api::events::player::async_player_send_suggestions::AsyncPlayerSendSuggestionsEvent::new(
            player.clone(),
            packet.command.to_string(),
            tab_event.completions,
        );
        server.plugin_manager.fire_blocking(server, &mut send_event);
        if send_event.cancelled {
            return;
        }
        send_event.suggestions.truncate(Self::MAX_SUGGESTIONS);

        let response = CCommandSuggestions::new(
            packet.id,
            (start as i32).into(),
            (length as i32).into(),
            send_event
                .suggestions
                .into_iter()
                .map(|suggestion| CommandSuggestion::new(suggestion, None))
                .collect(),
        );

        player.try_send_client_packet(&response);
    }
}
