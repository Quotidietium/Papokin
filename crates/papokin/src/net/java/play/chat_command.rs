#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    /// 应用签名命令包携带的 lastSeen 确认更新。
    ///
    /// 原版客户端把待确认消息的确认捆绑在签名命令包内（`message_count` +
    /// `acknowledged`），若忽略它们，服务端待确认队列只增不减，合法玩家
    /// 在大量收发签名聊天后运行命令时会被 4096 上限误踢。校验规则与聊天
    /// 消息路径保持一致：负偏移、窗口前移越界、确认未知消息均按校验失败
    /// 踢出。
    ///
    /// 仅在安全聊天启用时有意义：关闭时服务端不发送签名聊天，校验器不
    /// 跟踪任何待确认消息，此时不应校验（否则会误踢合法客户端）。
    pub fn apply_signed_command_last_seen(
        &self,
        server: &Arc<Server>,
        player: &Arc<Player>,
        message_count: VarInt,
        acknowledged: &[u8],
        checksum: u8,
    ) {
        if !server.basic_config.allow_chat_reports {
            return;
        }

        let validation_error = if message_count.0 < 0 {
            Some(ChatError::ChatValidationFailed)
        } else {
            let mut cache = player
                .signature_cache
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let applied = if acknowledged.is_empty() {
                cache
                    .last_seen_validator
                    .apply_offset(message_count.0 as usize)
            } else {
                cache
                    .last_seen_validator
                    .apply_update(message_count.0 as usize, acknowledged)
                    .map(|_| ())
            };
            match applied {
                Err(_) => Some(ChatError::ChatValidationFailed),
                Ok(()) => {
                    if cache.last_seen_validator.tracked_messages_count() > 4096 {
                        Some(ChatError::TooManyPendingChats)
                    } else if checksum != 0
                        && polynomial_rolling_hash(cache.last_seen.as_ref()) != checksum
                    {
                        // 校验和与聊天消息路径一致：客户端可发送 0 绕过（原版语义）
                        Some(ChatError::ChatValidationFailed)
                    } else {
                        None
                    }
                }
            }
        };

        if let Some(err) = validation_error {
            warn!(
                "校验来自 {} 的签名命令 lastSeen 更新失败：{}",
                player.gameprofile.name, err
            );
            self.try_kick(&TextComponent::translate(
                translation::java::MULTIPLAYER_DISCONNECT_CHAT_VALIDATION_FAILED,
                [],
            ));
        }
    }

    pub async fn handle_chat_command(
        &self,
        player: &Arc<Player>,
        server: &Arc<Server>,
        command: &SChatCommand<'_>,
    ) {
        player.update_last_action_time();
        if player.check_chat_spam(server, crate::entity::player::SpamType::Command) {
            return;
        }
        let command_str = command.command.strip_prefix('/').unwrap_or(command.command);

        let mut preprocess_event = PlayerCommandPreprocessEvent {
            player: player.clone(),
            command: command_str.to_string(),
            cancelled: false,
        };
        server
            .plugin_manager
            .fire(server, &mut preprocess_event)
            .await;
        if preprocess_event.cancelled {
            return;
        }

        send_cancellable! {{
            server;
            PlayerCommandSendEvent {
                player: player.clone(),
                command: command_str.to_string(),
                cancelled: false
            };

            'after: {
                let command = event.command;
                let dispatcher = server.command_dispatcher.load();
                dispatcher.handle_command(
                    &player.get_command_source(server),
                    &command,
                );

                if server.advanced_config.commands.log_console {
                    info!(
                        "玩家 ({}): 执行命令 /{}",
                        player.gameprofile.name,
                        command
                    );
                }
            }
        }}
    }
}
