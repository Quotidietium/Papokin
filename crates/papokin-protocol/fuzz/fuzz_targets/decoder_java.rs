#![no_main]
use libfuzzer_sys::fuzz_target;
use papokin_protocol::ServerPacket;
use papokin_protocol::java::{
    client::{
        config::CFinishConfig,
        login::CLoginDisconnect,
        play::{CBlockEntityData, CPlayerPosition, CSetEquipment, CUpdateEntityPosRot},
        status::{CPingResponse, CStatusResponse},
    },
    packet_decoder::TCPNetworkDecoder,
    server::{
        config::{
            SAcknowledgeFinishConfig, SClientInformationConfig, SConfigCookieResponse,
            SConfigResourcePack, SCustomClickAction as SConfigCustomClickAction,
            SKeepAlive as SConfigKeepAlive, SKnownPacks, SPluginMessage,
        },
        handshake::SHandShake,
        login::{
            SEncryptionResponse, SLoginAcknowledged, SLoginCookieResponse, SLoginPluginResponse,
            SLoginStart,
        },
        play::{
            SAttack, SBundleItemSelected, SChangeGameMode, SChatCommand, SChatMessage, SChunkBatch,
            SClickSlot, SClientCommand, SClientInformationPlay, SClientTickEnd, SCloseContainer,
            SCommandSuggestion, SConfirmTeleport, SContainerButtonClick, SCookieResponse,
            SCustomClickAction as SPlayCustomClickAction, SCustomPayload, SDebugSampleSubscription,
            SDebugSubscriptionRequest, SEditBook, SInteract, SJigsawGenerate,
            SKeepAlive as SPlayKeepAlive, SMoveVehicle, SPaddleBoat, SPickItemFromBlock,
            SPickItemFromEntity, SPlaceRecipe, SPlayPingRequest, SPlayerAbilities, SPlayerAction,
            SPlayerCommand, SPlayerInput, SPlayerLoaded, SPlayerPosition, SPlayerPositionRotation,
            SPlayerRotation, SPlayerSession, SRecipeBookChangeSettings, SRecipeBookSeenRecipe,
            SRenameItem, SSeenAdvancement, SSelectTrade, SSetBeacon, SSetCommandBlock,
            SSetCreativeSlot, SSetHeldItem, SSetJigsawBlock, SSetPlayerGround, SSetTestBlock,
            SSwingArm, STeleportToEntity, STestInstanceBlockAction, SUpdateSign, SUseItem,
            SUseItemOn,
        },
        status::{SStatusPingRequest, SStatusRequest},
    },
};
use papokin_util::version::JavaMinecraftVersion;
use std::io::Cursor;
use tokio::runtime::Runtime;

const TARGET_VERSION: JavaMinecraftVersion = JavaMinecraftVersion::V_26_1;

// ---------------------------------------------------------------------------
// 辅助函数：针对同一负载运行所有已知的 ServerPacket::read。
// 按 ServerPacket 签名的要求使用切片和 Version 枚举。
// ---------------------------------------------------------------------------
fn fuzz_all_deserializers(payload: &[u8]) {
    macro_rules! run_read {
        ($($packet:ty),* $(,)?) => {
            $(
                let mut slice = payload;
                let _ = <$packet>::read(&mut slice, &TARGET_VERSION);
            )*
        };
    }

    run_read!(
        // 握手
        SHandShake,
        // 状态
        SStatusPingRequest,
        SStatusRequest,
        // 登录
        SLoginStart,
        SEncryptionResponse,
        SLoginPluginResponse,
        SLoginCookieResponse,
        SLoginAcknowledged,
        // 配置
        SAcknowledgeFinishConfig,
        SClientInformationConfig,
        SConfigCookieResponse,
        SConfigCustomClickAction,
        SConfigKeepAlive,
        SKnownPacks,
        SPluginMessage,
        SConfigResourcePack,
        // 播放
        SAttack,
        SBundleItemSelected,
        SChangeGameMode,
        SChatCommand,
        SChatMessage,
        SChunkBatch,
        SClickSlot,
        SClientCommand,
        SClientInformationPlay,
        SClientTickEnd,
        SCloseContainer,
        SCommandSuggestion,
        SConfirmTeleport,
        SContainerButtonClick,
        SCookieResponse,
        SPlayCustomClickAction,
        SCustomPayload,
        SDebugSampleSubscription,
        SDebugSubscriptionRequest,
        SEditBook,
        SInteract,
        SJigsawGenerate,
        SPlayKeepAlive,
        SMoveVehicle,
        SPaddleBoat,
        SPickItemFromBlock,
        SPickItemFromEntity,
        SPlayPingRequest,
        SPlaceRecipe,
        SPlayerAbilities,
        SPlayerAction,
        SPlayerCommand,
        SPlayerInput,
        SPlayerLoaded,
        SPlayerPosition,
        SPlayerPositionRotation,
        SPlayerRotation,
        SPlayerSession,
        SRecipeBookChangeSettings,
        SRecipeBookSeenRecipe,
        SRenameItem,
        SSeenAdvancement,
        SSelectTrade,
        SSetBeacon,
        SSetCommandBlock,
        SSetCreativeSlot,
        SSetHeldItem,
        SSetJigsawBlock,
        SSetPlayerGround,
        SSetTestBlock,
        SSwingArm,
        STeleportToEntity,
        STestInstanceBlockAction,
        SUpdateSign,
        SUseItem,
        SUseItemOn,
        // 实现 ServerPacket 的 Clientbound 数据包
        CBlockEntityData,
        CFinishConfig,
        CLoginDisconnect,
        CPlayerPosition,
        CSetEquipment,
        CUpdateEntityPosRot,
        CPingResponse,
        CStatusResponse,
    );
}

// ---------------------------------------------------------------------------
// 模糊测试目标
// ---------------------------------------------------------------------------
fuzz_target!(|data: &[u8]| {
    if data.len() < 18 {
        return;
    }

    let mode = data[0] % 4;
    let key = &data[2..18];
    let rest = &data[18..];

    let split = if rest.is_empty() {
        0
    } else {
        (data[1] as usize) % rest.len()
    };
    let (decoder_bytes, deser_bytes) = rest.split_at(split);

    // --- 路径 1：解码器（组帧 / 加密 / 压缩）--------------
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let mut decoder = TCPNetworkDecoder::new(Cursor::new(decoder_bytes));
        match mode {
            1 => {
                decoder.set_compression(256);
            }
            2 => {
                let mut aes_key = [0u8; 16];
                aes_key.copy_from_slice(key);
                let _ = decoder.set_encryption(&aes_key);
            }
            3 => {
                decoder.set_compression(256);
                let mut aes_key = [0u8; 16];
                aes_key.copy_from_slice(key);
                let _ = decoder.set_encryption(&aes_key);
            }
            _ => {}
        }
        let _ = decoder.get_raw_packet().await;
    });

    // --- 路径 2：单个数据包反序列化器 ---------------------------
    fuzz_all_deserializers(deser_bytes);
});
