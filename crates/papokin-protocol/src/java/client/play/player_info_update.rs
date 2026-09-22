use std::io::Write;

use bitflags::bitflags;
use papokin_data::packet::clientbound::play::PLAYER_INFO_UPDATE;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

use crate::{ClientPacket, Property, WritingError, ser::NetworkWriteExt};

use super::PlayerAction;

bitflags! {
    /// 定义 Player Info Update 数据包中包含哪些字段。
    ///
    /// 此位掩码允许服务器更新玩家的多个方面
    /// 在 Tab 列表（及全局状态）中的存在状态整合进单个数据包。
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PlayerInfoFlags: u8 {
        /// 将玩家添加到客户端的内部玩家列表（注册表）中。
        const ADD_PLAYER            = 0x01;
        /// 为安全聊天初始化聊天签名会话。
        const INITIALIZE_CHAT       = 0x02;
        /// 更改玩家在 Tab 列表中显示的游戏模式。
        const UPDATE_GAME_MODE      = 0x04;
        /// 判断玩家是否在 Tab 列表中可见。
        const UPDATE_LISTED         = 0x08;
        /// 更新 ping/延迟指示条。
        const UPDATE_LATENCY        = 0x10;
        /// 更改 Tab 列表中显示的名称（支持格式化）。
        const UPDATE_DISPLAY_NAME   = 0x20;
        /// 设置 Tab 列表中的排序顺序（1.21.2 新增）。
        const UPDATE_LIST_PRIORITY  = 0x40;
        /// 切换玩家帽子层的可见性（1.21.4 新增）。
        const UPDATE_HAT            = 0x80;
    }
}

/// 在客户端上更新一个或多个玩家的信息。
///
/// 此数据包取代了旧版“玩家信息”数据包，采用更高效的方式
/// 基于位掩码的方式。它并不每次都发送完整数据，而是
/// 服务器只发送 `actions` 位掩码中指定的字段。
#[java_packet(PLAYER_INFO_UPDATE)]
pub struct CPlayerInfoUpdate<'a> {
    /// 决定后续包含哪些数据的位掩码（`PlayerInfoFlags`）。
    pub actions: u8,
    /// 正在更新的玩家列表。每个玩家条目包含
    /// 数据字段，按其在位掩码中出现的顺序排列。
    pub players: &'a [Player<'a>],
}

pub struct Player<'a> {
    pub uuid: uuid::Uuid,
    pub actions: &'a [PlayerAction<'a>],
}

impl<'a> CPlayerInfoUpdate<'a> {
    #[must_use]
    pub const fn new(actions: u8, players: &'a [Player<'a>]) -> Self {
        Self { actions, players }
    }
}

// TODO: 检查是否需要这个自定义实现
impl ClientPacket for CPlayerInfoUpdate<'_> {
    fn write_packet_data(
        &self,
        write: impl Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        let mut write = write;

        // UPDATE_LIST_PRIORITY 在 1.21.2 中加入，UPDATE_HAT 在 1.21.4 中加入。
        // 屏蔽不支持的位并省略其数据，以便旧版客户端能解析数据包。
        let mut effective_actions = self.actions;
        if *version < JavaMinecraftVersion::V_1_21_2 {
            effective_actions &= !PlayerInfoFlags::UPDATE_LIST_PRIORITY.bits();
        }
        if *version < JavaMinecraftVersion::V_1_21_4 {
            effective_actions &= !PlayerInfoFlags::UPDATE_HAT.bits();
        }

        write.write_u8(effective_actions)?;
        write.write_list::<Player>(self.players, |p, v| {
            p.write_uuid(&v.uuid)?;
            for action in v.actions {
                match action {
                    PlayerAction::AddPlayer { name, properties } => {
                        p.write_string(name)?;
                        p.write_list::<Property>(properties, |p, v| {
                            p.write_string(&v.name)?;
                            p.write_string(&v.value)?;
                            p.write_option(&v.signature, |p, v| p.write_string(v))
                        })?;
                    }
                    PlayerAction::InitializeChat(init_chat) => {
                        p.write_option(init_chat, |p, v| {
                            p.write_uuid(&v.session_id)?;
                            p.write_i64_be(v.expires_at)?;
                            p.write_var_int(&v.public_key.len().try_into().map_err(|_| {
                                WritingError::Message(format!(
                                    "{} isn't representable as a VarInt",
                                    v.public_key.len()
                                ))
                            })?)?;
                            p.write_slice(&v.public_key)?;
                            p.write_var_int(&v.signature.len().try_into().map_err(|_| {
                                WritingError::Message(format!(
                                    "{} isn't representable as a VarInt",
                                    v.signature.len()
                                ))
                            })?)?;
                            p.write_slice(&v.signature)
                        })?;
                    }
                    PlayerAction::UpdateGameMode(gamemode) => p.write_var_int(gamemode)?,
                    PlayerAction::UpdateListed(listed) => p.write_bool(*listed)?,
                    PlayerAction::UpdateLatency(latency) => p.write_var_int(latency)?,
                    PlayerAction::UpdateDisplayName(display_name) => {
                        p.write_option(display_name, |w, text_component| {
                            w.write_component(text_component, version)
                        })?;
                    }
                    PlayerAction::UpdateListOrder(order) => {
                        // 1.21.2 新增
                        if effective_actions & PlayerInfoFlags::UPDATE_LIST_PRIORITY.bits() != 0 {
                            p.write_var_int(order)?;
                        }
                    }
                    PlayerAction::UpdateHat(show_hat) => {
                        // 1.21.4 新增
                        if effective_actions & PlayerInfoFlags::UPDATE_HAT.bits() != 0 {
                            p.write_bool(*show_hat)?;
                        }
                    }
                }
            }

            Ok(())
        })
    }
}
