use papokin_util::text::TextComponent;

use crate::{Property, VarInt};

pub enum PlayerAction<'a> {
    AddPlayer {
        name: &'a str,
        properties: &'a [Property],
    },
    InitializeChat(Option<InitChat>),
    UpdateGameMode(VarInt),
    UpdateListed(bool),
    UpdateLatency(VarInt),
    UpdateDisplayName(Option<&'a TextComponent>),
    /// 于 1.21.2 加入
    UpdateListOrder(VarInt),
    /// 于 1.21.4 加入
    /// 切换玩家帽子层（第二皮肤层）的可见性。
    UpdateHat(bool),
}

pub struct InitChat {
    pub session_id: uuid::Uuid,
    pub expires_at: i64,
    pub public_key: Box<[u8]>,
    pub signature: Box<[u8]>,
}
