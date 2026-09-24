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

        if let Err(err) = self
            .validate_chat_message(server, player, &chat_message)
            .await
        {
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
    pub async fn validate_chat_message(
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

            validate_chat_signature(player, chat_message).await?;
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

/// 校验 `last_seen` 窗口与消息签名的密码学有效性（仅安全聊天模式调用）。
async fn validate_chat_signature(
    player: &Arc<Player>,
    chat_message: &SChatMessage<'_>,
) -> Result<(), ChatError> {
    let offset = chat_message.message_count.0;
    if offset < 0 {
        return Err(ChatError::ChatValidationFailed);
    }

    // 验签与转播使用的 last_seen 必须与客户端实际签名的列表一致：
    // 取自校验器应用 offset 与位集后的结果，而不是服务器记录的全部
    // 已展示消息（客户端可以只确认其中一部分）。位集字段仅 1.19.3+
    // 存在，更早版本走纯 offset 分支且不做密码学验签（其签名体格式
    // 不同且旧版字段未完整映射，保留原有的存在性检查）。
    let mut validated_last_seen: Option<Vec<Box<[u8]>>> = None;
    {
        let mut cache = player
            .signature_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !chat_message.acknowledged.is_empty() {
            match cache
                .last_seen_validator
                .apply_update(offset as usize, chat_message.acknowledged)
            {
                Ok(entries) => validated_last_seen = Some(entries),
                Err(_) => return Err(ChatError::ChatValidationFailed),
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

    if let Some(entries) = validated_last_seen {
        // 让后续广播回放与客户端签名完全一致的 last_seen 列表
        player
            .signature_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .last_seen = crate::entity::player::LastSeen::from(entries.clone());

        // 密码学验签：签名体为 salt（8 字节大端）‖ timestamp（8 字节
        // 大端毫秒）‖ 消息内容 UTF-8 ‖ last_seen 各条签名原字节，
        // 用玩家会话公钥做 RSA/SHA-256 验签，与原版
        // PlayerChatMessage.verify 一致。此前只查签名存在性，
        // 伪造签名会被转播为「已签名」消息，破坏举报证据链。
        let Some(signature_bytes) = chat_message.signature else {
            return Err(ChatError::UnsignedChat);
        };
        let signature = RsaPkcs1v15Signature::try_from(signature_bytes)
            .map_err(|_| ChatError::ChatValidationFailed)?;
        let public_key_der = player
            .chat_session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .public_key
            .clone();
        let public_key = RsaPublicKey::from_public_key_der(&public_key_der)
            .map_err(|_| ChatError::ChatValidationFailed)?;

        let signable = chat_signable_body(
            chat_message.salt,
            chat_message.timestamp,
            chat_message.message,
            &entries,
        );

        // RSA 验签放到 rayon，避免阻塞网络任务线程
        let (tx, rx) = tokio::sync::oneshot::channel();
        rayon::spawn(move || {
            let is_valid = VerifyingKey::<Sha256>::new(public_key)
                .verify(&signable, &signature)
                .is_ok();
            let _ = tx.send(is_valid);
        });
        if !rx.await.unwrap_or(false) {
            return Err(ChatError::ChatValidationFailed);
        }
    }
    Ok(())
}

/// 构造原版聊天签名体：salt（8 字节大端）‖ timestamp（8 字节大端
/// 毫秒）‖ 消息内容 UTF-8 ‖ `last_seen` 各条签名原字节，与原版
/// `SignedMessageBody.updateSignature` 的字节布局一致。
fn chat_signable_body(salt: i64, timestamp: i64, message: &str, entries: &[Box<[u8]>]) -> Vec<u8> {
    let mut signable = Vec::with_capacity(16 + message.len() + entries.len() * 256);
    signable.extend_from_slice(&salt.to_be_bytes());
    signable.extend_from_slice(&timestamp.to_be_bytes());
    signable.extend_from_slice(message.as_bytes());
    for entry in entries {
        signable.extend_from_slice(entry);
    }
    signable
}

#[cfg(test)]
mod tests {
    /// 签名体字节布局必须与原版一致：字段顺序或端序错误会让所有
    /// 合法消息的验签失败（安全聊天服务器上的全员误踢）。
    #[test]
    fn signable_body_matches_vanilla_layout() {
        let entries: Vec<Box<[u8]>> = vec![
            vec![1u8; 256].into_boxed_slice(),
            vec![2u8; 256].into_boxed_slice(),
        ];
        let body =
            super::chat_signable_body(0x0102_0304_0506_0708, 0x1112_1314_1516_1718, "hi", &entries);
        assert_eq!(body.len(), 16 + 2 + 512);
        assert_eq!(&body[..8], &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(
            &body[8..16],
            &[0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18]
        );
        assert_eq!(&body[16..18], b"hi");
        assert!(body[18..18 + 256].iter().all(|&b| b == 1));
        assert!(body[18 + 256..].iter().all(|&b| b == 2));
    }
}
