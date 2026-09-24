#[allow(clippy::wildcard_imports)]
use super::*;
use papokin_data::world::RAW;

impl JavaClient {
    pub async fn handle_chat_message(
        &self,
        server: &Arc<Server>,
        player: &Arc<Player>,
        chat_message: SChatMessage<'_>,
    ) {
        player.update_last_action_time();

        if let Some(command) = chat_message.message.strip_prefix('/') {
            let command_packet = SChatCommand { command };
            self.handle_chat_command(player, server, &command_packet)
                .await;
            return;
        }

        let gameprofile = &player.gameprofile;

        if let Err(err) = self.validate_chat_message(server, player, &chat_message) {
            log_at_level!(
                err.severity(),
                "{} (uuid {}) {}",
                gameprofile.name,
                gameprofile.id,
                err
            );
            if err.is_kick()
                && let Some(reason) = err.client_kick_reason()
            {
                self.kick(TextComponent::text(reason)).await;
            }
            return;
        }

        if player.check_chat_spam(server, crate::entity::player::SpamType::Chat) {
            return;
        }

        send_cancellable! {{
            server;
            PlayerChatEvent::new(
                player.clone(),
                chat_message.message.to_string(),
                vec![],
                chat_message.signature.map(<[u8]>::to_vec),
            );

            'after: {
                info!("<chat> {}: {}", gameprofile.name, event.message);

                let config = &server.advanced_config;

                let message = match seasonal_events::modify_chat_message(&event.message, config) {
                    Some(m) => m,
                    None => event.message.clone(),
                };

                let decorated_message = TextComponent::chat_decorated(
                    &config.chat.format,
                    &gameprofile.name,
                    &message,
                );

                let mut async_chat_event = AsyncPlayerChatEvent {
                    player: player.clone(),
                    message: event.message.clone(),
                    format: decorated_message,
                    cancelled: false,
                };
                server
                    .plugin_manager
                    .fire(server, &mut async_chat_event)
                    .await;
                if async_chat_event.cancelled {
                    return;
                }

                let entity = &player.get_entity();
                let world = entity.world.load_full();
                if server.basic_config.allow_chat_reports {
                    world.broadcast_secure_player_chat(
                        player,
                        &chat_message,
                        &async_chat_event.format,
                    );
                } else {
                    let outgoing = crate::net::chat::PlayerChatMessage::system(message)
                        .with_unsigned_content(async_chat_event.format);
                    world.broadcast_chat_message(
                        &outgoing,
                        Player::is_text_filtering_enabled,
                        Some(player),
                        (RAW + 1).into(),
                        &TextComponent::empty(),
                        None,
                    );
                }
            }
        }}
    }

    /// 执行全部原版检查以确保聊天消息有效
    pub fn validate_chat_message(
        &self,
        server: &Server,
        player: &Arc<Player>,
        chat_message: &SChatMessage<'_>,
    ) -> Result<(), ChatError> {
        // 检查超大消息
        // 如果能找到第 257 个 UTF-16 字符，则消息过大。
        if chat_message.message.encode_utf16().nth(256).is_some() {
            return Err(ChatError::OversizedMessage);
        }
        // 检查非法字符
        if chat_message
            .message
            .chars()
            .any(|c| c == '§' || c < ' ' || c == '\x7F')
        {
            return Err(ChatError::IllegalCharacters);
        }
        // 这些检查只在安全聊天模式下运行
        if server.basic_config.allow_chat_reports {
            // 检查未签名聊天
            if let Some(signature) = &chat_message.signature {
                if signature.len() != 256 {
                    return Err(ChatError::UnsignedChat); // 签名长度错误
                }
            } else {
                return Err(ChatError::UnsignedChat); // 没有签名
            }

            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;

            // 验证消息时间戳
            if chat_message.timestamp > now || chat_message.timestamp < (now - CHAT_MESSAGE_MAX_AGE)
            {
                return Err(ChatError::OutOfOrderChat);
            }

            // 验证会话是否过期
            if player
                .chat_session
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .expires_at
                < now
            {
                return Err(ChatError::ExpiredPublicKey);
            }

            let offset = chat_message.message_count.0;
            if offset < 0 {
                return Err(ChatError::ChatValidationFailed);
            }

            {
                let mut cache = player
                    .signature_cache
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if !chat_message.acknowledged.is_empty() {
                    if cache
                        .last_seen_validator
                        .apply_update(offset as usize, chat_message.acknowledged)
                        .is_err()
                    {
                        return Err(ChatError::ChatValidationFailed);
                    }
                } else if cache
                    .last_seen_validator
                    .apply_offset(offset as usize)
                    .is_err()
                {
                    return Err(ChatError::ChatValidationFailed);
                }

                if cache.last_seen_validator.tracked_messages_count() > 4096 {
                    return Err(ChatError::TooManyPendingChats);
                }
            }

            // 校验先前的签名校验和（1.21.5 新增）
            // 客户端可以通过发送 0 绕过此检查
            if chat_message.checksum != 0 {
                let checksum = polynomial_rolling_hash(
                    player
                        .signature_cache
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .last_seen
                        .as_ref(),
                );
                if checksum != chat_message.checksum {
                    return Err(ChatError::ChatValidationFailed);
                }
            }
        }
        Ok(())
    }

    pub async fn handle_chat_session_update(
        &self,
        player: &Arc<Player>,
        server: &Server,
        session: SPlayerSession,
    ) {
        // 如果我们不需要举报，保留聊天会话默认值
        if !server.basic_config.allow_chat_reports {
            return;
        }

        // 与当前已生效会话完全一致的重复更新直接早退：该会话在首次应用时
        // 已通过公钥验证，重复执行只会给恶意客户端留一个以包速率刷 rayon
        // RSA 验证 + 全服广播的入口。默认会话（expires_at 为 0）不在此列，
        // 仍须走完整校验以拒绝无效会话。
        {
            let current = player
                .chat_session
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let is_duplicate = current.expires_at > 0
                && current.session_id == session.session_id
                && current.expires_at == session.expires_at
                && current.public_key == session.public_key
                && current.signature == session.key_signature;
            drop(current);
            if is_duplicate {
                return;
            }
        }

        if let Err(err) = self.validate_chat_session(player, server, &session).await {
            log_at_level!(
                err.severity(),
                "{} (uuid {}) {}",
                player.gameprofile.name,
                player.gameprofile.id,
                err
            );
            if err.is_kick()
                && let Some(reason) = err.client_kick_reason()
            {
                self.kick(TextComponent::text(reason)).await;
            }
            return;
        }

        // 更新聊天会话字段
        *player
            .chat_session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = ChatSession::new(
            session.session_id,
            session.expires_at,
            session.public_key.clone(),
            session.key_signature.clone(),
        );

        server.broadcast_packet_all(&CPlayerInfoUpdate::new(
            PlayerInfoFlags::INITIALIZE_CHAT.bits(),
            &[papokin_protocol::java::client::play::Player {
                uuid: player.gameprofile.id,
                actions: &[PlayerAction::InitializeChat(Some(InitChat {
                    session_id: session.session_id,
                    expires_at: session.expires_at,
                    public_key: session.public_key.clone(),
                    signature: session.key_signature.clone(),
                }))],
            }],
        ));
    }

    /// 执行全部原版检查以确保玩家会话有效
    pub async fn validate_chat_session(
        &self,
        player: &Player,
        server: &Server,
        session: &SPlayerSession,
    ) -> Result<(), ChatError> {
        // 验证会话是否过期
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        if session.expires_at < now {
            return Err(ChatError::InvalidPublicKey);
        }

        let key_signature = RsaPkcs1v15Signature::try_from(session.key_signature.as_ref())
            .map_err(|_| ChatError::InvalidPublicKey)?;

        let mut signable = Vec::new();
        signable.extend_from_slice(player.gameprofile.id.as_bytes());
        signable.extend_from_slice(&session.expires_at.to_be_bytes());
        signable.extend_from_slice(&session.public_key);

        let public_keys = server.mojang_public_keys.load_full();

        let (tx, rx) = tokio::sync::oneshot::channel();
        rayon::spawn(move || {
            let is_valid = public_keys.iter().any(|key| {
                let verifying_key = VerifyingKey::<Sha1>::new(key.clone());
                verifying_key.verify(&signable, &key_signature).is_ok()
            });
            let _ = tx.send(is_valid);
        });
        let is_valid = rx.await.unwrap_or(false);

        // 验证可签名数据对 Mojang 的任一公钥均有效
        if !is_valid {
            return Err(ChatError::InvalidPublicKey);
        }

        Ok(())
    }
}
