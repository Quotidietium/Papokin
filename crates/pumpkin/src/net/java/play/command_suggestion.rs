#[allow(clippy::wildcard_imports)]
use super::*;
use pumpkin_protocol::java::client::play::CommandSuggestion;

impl JavaClient {
    pub fn handle_command_suggestion(
        &self,
        player: &Arc<Player>,
        packet: &SCommandSuggestion<'_>,
        server: &Arc<Server>,
    ) {
        let Some(cmd) = &packet.command.get(1..) else {
            return;
        };

        let Some((last_word_start, _)) = cmd.char_indices().rfind(|(_, c)| c.is_whitespace())
        else {
            return;
        };

        let suggestions = server
            .command_dispatcher
            .load()
            .suggest(cmd, &player.get_command_source(server));

        // Suggestion hooks: tab-complete carries the raw buffer and the
        // computed completions (modifiable); send-suggestions carries the
        // final list. Both are cancellable. Tooltips are dropped: the wire
        // data is plain strings.
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

        let response = CCommandSuggestions::new(
            packet.id,
            ((last_word_start + 2) as i32).into(),
            ((cmd.len() - last_word_start - 1) as i32).into(),
            send_event
                .suggestions
                .into_iter()
                .map(|suggestion| CommandSuggestion::new(suggestion, None))
                .collect(),
        );

        player.try_send_client_packet(&response);
    }
}
