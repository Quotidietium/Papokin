use std::sync::Arc;
use std::time::Duration;

use papokin_protocol::codec::bit_set::BitSet;
use papokin_protocol::codec::var_int::VarInt;
use papokin_protocol::java::client::play::{CDisguisedChatMessage, CPlayerChatMessage, FilterType};
use papokin_util::text::TextComponent;
use uuid::Uuid;

use crate::entity::player::Player;

/// 指定如何为接收方客户端过滤消息的掩码。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilterMask {
    PassThrough,
    FullyFiltered,
    PartiallyFiltered(BitSet),
}

impl FilterMask {
    pub const PASS_THROUGH: Self = Self::PassThrough;
    pub const FULLY_FILTERED: Self = Self::FullyFiltered;

    #[must_use]
    pub const fn is_pass_through(&self) -> bool {
        matches!(self, Self::PassThrough)
    }

    #[must_use]
    pub const fn is_fully_filtered(&self) -> bool {
        matches!(self, Self::FullyFiltered)
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.is_pass_through()
    }

    #[must_use]
    pub fn to_filter_type(&self) -> FilterType {
        match self {
            Self::PassThrough => FilterType::PassThrough,
            Self::FullyFiltered => FilterType::FullyFiltered,
            Self::PartiallyFiltered(bits) => FilterType::PartiallyFiltered(bits.clone()),
        }
    }

    #[must_use]
    pub fn from_filter_type(filter_type: FilterType) -> Self {
        match filter_type {
            FilterType::PassThrough => Self::PassThrough,
            FilterType::FullyFiltered => Self::FullyFiltered,
            FilterType::PartiallyFiltered(bits) => Self::PartiallyFiltered(bits),
        }
    }

    #[must_use]
    pub fn apply(&self, text: &str) -> Option<String> {
        match self {
            Self::PassThrough => Some(text.to_string()),
            Self::FullyFiltered => None,
            Self::PartiallyFiltered(bits) => {
                let mut chars = Vec::with_capacity(text.len());
                for (i, c) in text.chars().enumerate() {
                    if bits.get_bit(i) {
                        chars.push('#');
                    } else {
                        chars.push(c);
                    }
                }
                Some(chars.into_iter().collect())
            }
        }
    }

    #[must_use]
    pub fn filter_component(&self, component: &TextComponent) -> Option<TextComponent> {
        match self {
            Self::PassThrough => Some(component.clone()),
            Self::FullyFiltered => None,
            Self::PartiallyFiltered(_) => {
                let text = component.clone().get_text();
                self.apply(&text).map(|filtered_str| {
                    let mut comp = component.clone();
                    comp.0.content = Box::new(papokin_util::text::TextContent::Text {
                        text: filtered_str.into(),
                    });
                    comp
                })
            }
        }
    }
}

/// 标识安全聊天链中聊天消息的发送者、会话和索引。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedMessageLink {
    pub index: i32,
    pub sender: Uuid,
    pub session_id: Uuid,
}

impl SignedMessageLink {
    #[must_use]
    pub const fn unsigned(sender: Uuid) -> Self {
        Self {
            index: 0,
            sender,
            session_id: Uuid::nil(),
        }
    }

    #[must_use]
    pub const fn new(index: i32, sender: Uuid, session_id: Uuid) -> Self {
        Self {
            index,
            sender,
            session_id,
        }
    }
}

/// 聊天消息的签名正文。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedMessageBody {
    pub content: String,
    pub time_stamp: i64,
    pub salt: i64,
    pub last_seen: Vec<Box<[u8]>>,
}

impl SignedMessageBody {
    #[must_use]
    pub const fn unsigned(content: String) -> Self {
        Self {
            content,
            time_stamp: 0,
            salt: 0,
            last_seen: Vec::new(),
        }
    }

    #[must_use]
    pub const fn new(
        content: String,
        time_stamp: i64,
        salt: i64,
        last_seen: Vec<Box<[u8]>>,
    ) -> Self {
        Self {
            content,
            time_stamp,
            salt,
            last_seen,
        }
    }
}

/// 表示由玩家或系统发送的聊天消息，可带数字签名和过滤掩码。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerChatMessage {
    pub link: SignedMessageLink,
    pub signature: Option<Box<[u8]>>,
    pub signed_body: SignedMessageBody,
    pub unsigned_content: Option<TextComponent>,
    pub filter_mask: FilterMask,
}

impl PlayerChatMessage {
    pub const SYSTEM_SENDER: Uuid = Uuid::nil();
    pub const MESSAGE_EXPIRES_AFTER_SERVER: Duration = Duration::from_mins(5);
    pub const MESSAGE_EXPIRES_AFTER_CLIENT: Duration = Duration::from_mins(7);

    #[must_use]
    pub const fn system(content: String) -> Self {
        Self::unsigned(Self::SYSTEM_SENDER, content)
    }

    #[must_use]
    pub const fn unsigned(sender: Uuid, content: String) -> Self {
        let body = SignedMessageBody::unsigned(content);
        let link = SignedMessageLink::unsigned(sender);
        Self {
            link,
            signature: None,
            signed_body: body,
            unsigned_content: None,
            filter_mask: FilterMask::PassThrough,
        }
    }

    #[must_use]
    pub const fn new(
        link: SignedMessageLink,
        signature: Option<Box<[u8]>>,
        signed_body: SignedMessageBody,
        unsigned_content: Option<TextComponent>,
        filter_mask: FilterMask,
    ) -> Self {
        Self {
            link,
            signature,
            signed_body,
            unsigned_content,
            filter_mask,
        }
    }

    #[must_use]
    pub fn with_unsigned_content(mut self, content: TextComponent) -> Self {
        let is_same = match &*content.0.content {
            papokin_util::text::TextContent::Text { text } => text == &self.signed_body.content,
            _ => false,
        };
        self.unsigned_content = if is_same { None } else { Some(content) };
        self
    }

    #[must_use]
    pub fn remove_unsigned_content(mut self) -> Self {
        self.unsigned_content = None;
        self
    }

    #[must_use]
    pub fn filter(&self, filter_mask: FilterMask) -> Self {
        if self.filter_mask == filter_mask {
            self.clone()
        } else {
            Self {
                link: self.link.clone(),
                signature: self.signature.clone(),
                signed_body: self.signed_body.clone(),
                unsigned_content: self.unsigned_content.clone(),
                filter_mask,
            }
        }
    }

    #[must_use]
    pub fn filter_by_bool(&self, filtered: bool) -> Self {
        self.filter(if filtered {
            self.filter_mask.clone()
        } else {
            FilterMask::PassThrough
        })
    }

    #[must_use]
    pub fn remove_signature(&self) -> Self {
        let body = SignedMessageBody::unsigned(self.signed_content().to_string());
        let link = SignedMessageLink::unsigned(self.sender());
        Self {
            link,
            signature: None,
            signed_body: body,
            unsigned_content: self.unsigned_content.clone(),
            filter_mask: self.filter_mask.clone(),
        }
    }

    #[must_use]
    pub fn signed_content(&self) -> &str {
        &self.signed_body.content
    }

    #[must_use]
    pub fn decorated_content(&self) -> TextComponent {
        self.unsigned_content
            .clone()
            .unwrap_or_else(|| TextComponent::text(self.signed_content().to_string()))
    }

    #[must_use]
    pub const fn timestamp(&self) -> i64 {
        self.signed_body.time_stamp
    }

    #[must_use]
    pub const fn salt(&self) -> i64 {
        self.signed_body.salt
    }

    #[must_use]
    pub const fn sender(&self) -> Uuid {
        self.link.sender
    }

    #[must_use]
    pub fn is_system(&self) -> bool {
        self.sender() == Self::SYSTEM_SENDER
    }

    #[must_use]
    pub const fn has_signature(&self) -> bool {
        self.signature.is_some()
    }

    #[must_use]
    pub fn has_signature_from(&self, profile_id: &Uuid) -> bool {
        self.has_signature() && self.sender() == *profile_id
    }

    #[must_use]
    pub const fn is_fully_filtered(&self) -> bool {
        self.filter_mask.is_fully_filtered()
    }
}

/// 传出聊天消息的抽象，可发送伪装的系统聊天或玩家签名的聊天。
#[derive(Clone, Debug)]
pub enum OutgoingChatMessage {
    Disguised { content: TextComponent },
    Player { message: PlayerChatMessage },
}

impl OutgoingChatMessage {
    #[must_use]
    pub fn create(message: PlayerChatMessage) -> Self {
        if message.is_system() {
            Self::Disguised {
                content: message.decorated_content(),
            }
        } else {
            Self::Player { message }
        }
    }

    #[must_use]
    pub fn content(&self) -> TextComponent {
        match self {
            Self::Disguised { content } => content.clone(),
            Self::Player { message } => message.decorated_content(),
        }
    }

    pub fn send_to_player(
        &self,
        player: &Arc<Player>,
        filtered: bool,
        chat_type: VarInt,
        sender_name: &TextComponent,
        target_name: Option<&TextComponent>,
    ) {
        match self {
            Self::Disguised { content } => {
                let packet =
                    CDisguisedChatMessage::new(content, chat_type, sender_name, target_name);
                player.client.try_send_packet(&packet);
            }
            Self::Player { message } => {
                let filtered_message = message.filter_by_bool(filtered);
                if !filtered_message.is_fully_filtered() {
                    let messages_sent = VarInt(filtered_message.link.index);
                    let messages_received: i32 = player
                        .chat_session
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .messages_received;

                    let sender_last_seen = {
                        let cache = player
                            .signature_cache
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        cache.last_seen.indexed_for(player)
                    };

                    let je_packet = CPlayerChatMessage::new(
                        VarInt(messages_received),
                        filtered_message.sender(),
                        messages_sent,
                        filtered_message.signature.clone(),
                        filtered_message.signed_content().into(),
                        filtered_message.timestamp(),
                        filtered_message.salt(),
                        sender_last_seen,
                        filtered_message.unsigned_content.clone(),
                        filtered_message.filter_mask.to_filter_type(),
                        chat_type,
                        sender_name.clone(),
                        target_name.cloned(),
                    );

                    player.client.try_send_packet(&je_packet);

                    if let Some(signature) = &filtered_message.signature {
                        let mut cache = player
                            .signature_cache
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        cache.add_seen_signature(signature);
                        cache.last_seen_validator.add_pending(signature);
                        let tracked_count = cache.last_seen_validator.tracked_messages_count();
                        drop(cache);

                        if tracked_count > 4096 {
                            player.kick(&TextComponent::translate(
                                papokin_data::translation::java::MULTIPLAYER_DISCONNECT_TOO_MANY_PENDING_CHATS,
                                []),
                            );
                        }
                    }

                    if player.gameprofile.id != filtered_message.sender() {
                        player
                            .signature_cache
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .cache_signatures(&filtered_message.signed_body.last_seen);
                    }
                    player
                        .chat_session
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .messages_received += 1;
                }
            }
        }
    }
}
