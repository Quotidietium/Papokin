use arc_swap::ArcSwap;
use papokin_data::translation;
use papokin_protocol::{
    ConnectionState, Label, Link, LinkType,
    java::client::{
        config::{
            CConfigAddResourcePack, CConfigServerLinks, CFeatureFlags, CFinishConfig, CKnownPacks,
            CRegistryData, CUpdateTags,
        },
        login::{CLoginSuccess, CSetCompression},
    },
    java::server::login::{
        SEncryptionResponse, SLoginCookieResponse, SLoginPluginResponse, SLoginStart,
    },
};
use papokin_util::{text::TextComponent, version::JavaMinecraftVersion};
use std::sync::Arc;
use tracing::debug;
use uuid::Uuid;

use crate::{
    net::{
        EncryptionError, GameProfile, PacketHandlerResult,
        authentication::{self, AuthError},
        can_not_join, is_valid_player_name,
        java::pending::PendingConnection,
        offline_uuid,
        proxy::{bungeecord, velocity, vine},
    },
    plugin::player::player_pre_login::PlayerPreLoginEvent,
    server::Server,
};

pub mod cookie_response;
pub mod encryption_response;
pub mod known_packs;
pub mod login_acknowledged;
pub mod login_start;
pub mod plugin_response;
