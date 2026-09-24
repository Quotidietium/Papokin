// 由 papokin-codegen 生成。请勿编辑。
#![allow(clippy::all)]
#![allow(clippy::pedantic)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
use crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::{
    ClientboundPacket, ServerboundPacket,
};
use bytes::Bytes;
use papokin_protocol::codec::var_int::VarInt;
use papokin_protocol::packet::MultiVersionJavaPacket;
use papokin_protocol::packet::Packet;
use papokin_util::version::JavaMinecraftVersion;
use std::any::Any;

trait SaturateInto<T> {
    fn saturate_into(self) -> T;
}

macro_rules! impl_saturate_into {
    ($($src_ty:ty => $($dst_ty:ty),+);+ $(;)?) => {
        $($(impl SaturateInto<$dst_ty> for $src_ty {
            fn saturate_into(self) -> $dst_ty {
                // 经 i128 中转做钳制，跨符号序对也能饱和而非回绕
                let widened = self as i128;
                widened.clamp(<$dst_ty>::MIN as i128, <$dst_ty>::MAX as i128) as $dst_ty
            }
        })+)+
    };
}
impl_saturate_into! {
    i8 => i8, u8, i16, u16, i32, u32, i64, u64;
    u8 => i8, u8, i16, u16, i32, u32, i64, u64;
    i16 => i8, u8, i16, u16, i32, u32, i64, u64;
    u16 => i8, u8, i16, u16, i32, u32, i64, u64;
    i32 => i8, u8, i16, u16, i32, u32, i64, u64;
    u32 => i8, u8, i16, u16, i32, u32, i64, u64;
    i64 => i8, u8, i16, u16, i32, u32, i64, u64;
    u64 => i8, u8, i16, u16, i32, u32, i64, u64;
}
impl SaturateInto<Self> for f32 {
    fn saturate_into(self) -> Self {
        self
    }
}
impl SaturateInto<Self> for bool {
    fn saturate_into(self) -> Self {
        self
    }
}
impl SaturateInto<Self> for f64 {
    fn saturate_into(self) -> Self {
        self
    }
}
impl SaturateInto<f64> for f32 {
    fn saturate_into(self) -> f64 {
        self as f64
    }
}
impl SaturateInto<f32> for f64 {
    fn saturate_into(self) -> f32 {
        // 浮点 as 转换本身即饱和语义
        self as f32
    }
}

#[must_use]
pub fn serialize_java_packet(
    packet: &ClientboundPacket,
    version: JavaMinecraftVersion,
) -> Option<Bytes> {
    match packet {
        ClientboundPacket::ConfigCCodeOfConduct(data) => {
            let p = papokin_protocol::java::client::config::CCodeOfConduct {
                code_of_conduct: &data.code_of_conduct,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::ConfigCConfigDisconnect(data) => {
            let p = papokin_protocol::java::client::config::CConfigDisconnect {
                reason: &data.reason,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::ConfigCFeatureFlags(data) => {
            let vec_features: Vec<&str> = data.features.iter().map(|s| s.as_str()).collect();
            let p = papokin_protocol::java::client::config::CFeatureFlags {
                features: &vec_features,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::ConfigCConfigPing(data) => {
            let p = papokin_protocol::java::client::config::CConfigPing {
                id: data.id.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::ConfigCPluginMessage(data) => {
            let p = papokin_protocol::java::client::config::CPluginMessage {
                channel: &data.channel,
                data: &data.data,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::ConfigCTransfer(data) => {
            let var_int_port = VarInt(data.port);
            let p = papokin_protocol::java::client::config::CTransfer {
                host: &data.host,
                port: &var_int_port,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::LoginCLoginDisconnect(data) => {
            let p = papokin_protocol::java::client::login::CLoginDisconnect {
                json_reason: data.json_reason.clone(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::LoginCLoginPluginRequest(data) => {
            let p = papokin_protocol::java::client::login::CLoginPluginRequest {
                message_id: VarInt(data.message_id),
                channel: &data.channel,
                data: &data.data,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::LoginCSetCompression(data) => {
            let p = papokin_protocol::java::client::login::CSetCompression {
                threshold: VarInt(data.threshold),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CAcknowledgeBlockChange(data) => {
            let p = papokin_protocol::java::client::play::CAcknowledgeBlockChange {
                sequence_id: VarInt(data.sequence_id),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CActionBar(data) => {
            let component_action_bar =
                papokin_util::text::TextComponent::text(data.action_bar.clone());
            let p = papokin_protocol::java::client::play::CActionBar {
                action_bar: &component_action_bar,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CSetBlockDestroyStage(data) => {
            let p = papokin_protocol::java::client::play::CSetBlockDestroyStage {
                entity_id: VarInt(data.entity_id),
                location: papokin_util::math::position::BlockPos::new(
                    data.location.0,
                    data.location.1,
                    data.location.2,
                ),
                destroy_stage: data.destroy_stage.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CBlockEntityData(data) => {
            let p = papokin_protocol::java::client::play::CBlockEntityData {
                location: papokin_util::math::position::BlockPos::new(
                    data.location.0,
                    data.location.1,
                    data.location.2,
                ),
                r#type: VarInt(data.r_type),
                nbt_data: data.nbt_data.clone().into_boxed_slice(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CBlockEvent(data) => {
            let p = papokin_protocol::java::client::play::CBlockEvent {
                location: papokin_util::math::position::BlockPos::new(
                    data.location.0,
                    data.location.1,
                    data.location.2,
                ),
                action_id: data.action_id.saturate_into(),
                action_parameter: data.action_parameter.saturate_into(),
                block_type: VarInt(data.block_type),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CBlockUpdate(data) => {
            let p = papokin_protocol::java::client::play::CBlockUpdate {
                location: papokin_util::math::position::BlockPos::new(
                    data.location.0,
                    data.location.1,
                    data.location.2,
                ),
                state_id: VarInt(data.state_id),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CCenterChunk(data) => {
            let p = papokin_protocol::java::client::play::CCenterChunk {
                chunk_x: VarInt(data.chunk_x),
                chunk_z: VarInt(data.chunk_z),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CChangeDifficulty(data) => {
            let p = papokin_protocol::java::client::play::CChangeDifficulty {
                difficulty: data.difficulty.saturate_into(),
                locked: data.locked.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CChunkBatchEnd(data) => {
            let p = papokin_protocol::java::client::play::CChunkBatchEnd {
                batch_size: VarInt(data.batch_size),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CClearTitle(data) => {
            let p = papokin_protocol::java::client::play::CClearTitle {
                reset: data.reset.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CCloseContainer(data) => {
            let p = papokin_protocol::java::client::play::CCloseContainer {
                sync_id: VarInt(data.sync_id),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CCombatDeath(data) => {
            let component_message = papokin_util::text::TextComponent::text(data.message.clone());
            let p = papokin_protocol::java::client::play::CCombatDeath {
                player_id: VarInt(data.player_id),
                message: &component_message,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CCombatEnd(data) => {
            let p = papokin_protocol::java::client::play::CCombatEnd {
                duration_ticks: VarInt(data.duration_ticks),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CCustomChatCompletions(data) => {
            let vec_entries: Vec<&str> = data.entries.iter().map(|s| s.as_str()).collect();
            let p = papokin_protocol::java::client::play::CCustomChatCompletions {
                action: VarInt(data.action),
                entries: &vec_entries,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CCustomPayload(data) => {
            let p = papokin_protocol::java::client::play::CCustomPayload {
                channel: &data.channel,
                data: &data.data,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CDebugBlockValue(data) => {
            let p = papokin_protocol::java::client::play::CDebugBlockValue {
                pos: papokin_util::math::position::BlockPos::new(
                    data.pos.0, data.pos.1, data.pos.2,
                ),
                name: &data.name,
                value: &data.value,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CDebugEntityValue(data) => {
            let p = papokin_protocol::java::client::play::CDebugEntityValue {
                entity_id: VarInt(data.entity_id),
                name: &data.name,
                value: &data.value,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CDebugEvent(data) => {
            let p = papokin_protocol::java::client::play::CDebugEvent {
                name: &data.name,
                data: &data.data,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CDebugSample(data) => {
            let vec_sample: Vec<_> = data.sample.iter().map(|v| *v as _).collect();
            let p = papokin_protocol::java::client::play::CDebugSample {
                sample: &vec_sample,
                sample_type: VarInt(data.sample_type),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CPlayDisconnect(data) => {
            let component_reason = papokin_util::text::TextComponent::text(data.reason.clone());
            let p = papokin_protocol::java::client::play::CPlayDisconnect {
                reason: &component_reason,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CDisplayObjective(data) => {
            let p = papokin_protocol::java::client::play::CDisplayObjective {
                position: VarInt(data.position),
                score_name: data.score_name.clone(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CEntityAnimation(data) => {
            let p = papokin_protocol::java::client::play::CEntityAnimation {
                entity_id: VarInt(data.entity_id),
                animation: data.animation.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CSetEntityMetadata(data) => {
            let p = papokin_protocol::java::client::play::CSetEntityMetadata {
                entity_id: VarInt(data.entity_id),
                metadata: data.metadata.clone().into_boxed_slice(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CEntityStatus(data) => {
            let p = papokin_protocol::java::client::play::CEntityStatus {
                entity_id: data.entity_id.saturate_into(),
                entity_status: data.entity_status.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CEntityVelocity(data) => {
            let parsed_velocity: [f64; 3] = serde_json::from_str(&data.velocity).ok()?;
            let p = papokin_protocol::java::client::play::CEntityVelocity {
                entity_id: VarInt(data.entity_id),
                velocity: papokin_protocol::codec::lp_vector_3d::LpVector3d(
                    papokin_util::math::vector3::Vector3::new(
                        parsed_velocity[0],
                        parsed_velocity[1],
                        parsed_velocity[2],
                    ),
                ),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CGameEvent(data) => {
            let p = papokin_protocol::java::client::play::CGameEvent {
                event: data.event.saturate_into(),
                value: data.value.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CGameTestHighlightPos(data) => {
            let p = papokin_protocol::java::client::play::CGameTestHighlightPos {
                pos: papokin_util::math::position::BlockPos::new(
                    data.pos.0, data.pos.1, data.pos.2,
                ),
                color: data.color.saturate_into(),
                label: &data.label,
                duration_ms: data.duration_ms.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CHeadRot(data) => {
            let p = papokin_protocol::java::client::play::CHeadRot {
                entity_id: VarInt(data.entity_id),
                head_yaw: data.head_yaw.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CHurtAnimation(data) => {
            let p = papokin_protocol::java::client::play::CHurtAnimation {
                entity_id: VarInt(data.entity_id),
                yaw: data.yaw.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CInitializeWorldBorder(data) => {
            let p = papokin_protocol::java::client::play::CInitializeWorldBorder {
                x: data.x.saturate_into(),
                z: data.z.saturate_into(),
                old_diameter: data.old_diameter.saturate_into(),
                new_diameter: data.new_diameter.saturate_into(),
                speed: papokin_protocol::codec::var_long::VarLong(data.speed.saturate_into()),
                portal_teleport_boundary: VarInt(data.portal_teleport_boundary),
                warning_blocks: VarInt(data.warning_blocks),
                warning_time: VarInt(data.warning_time),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CItemCooldown(data) => {
            let p = papokin_protocol::java::client::play::CItemCooldown {
                group: data.group.clone(),
                cooldown: VarInt(data.cooldown),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CKeepAlive(data) => {
            let p = papokin_protocol::java::client::play::CKeepAlive {
                keep_alive_id: data.keep_alive_id.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CLevelEvent(data) => {
            let p = papokin_protocol::java::client::play::CLevelEvent {
                event: data.event.saturate_into(),
                location: papokin_util::math::position::BlockPos::new(
                    data.location.0,
                    data.location.1,
                    data.location.2,
                ),
                data: data.data.saturate_into(),
                disable_relative_volume: data.disable_relative_volume.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CMoveVehicle(data) => {
            let p = papokin_protocol::java::client::play::CMoveVehicle {
                x: data.x.saturate_into(),
                y: data.y.saturate_into(),
                z: data.z.saturate_into(),
                yaw: data.yaw.saturate_into(),
                pitch: data.pitch.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::COpenBook(data) => {
            let p = papokin_protocol::java::client::play::COpenBook {
                hand: VarInt(data.hand),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::COpenMountScreen(data) => {
            let p = papokin_protocol::java::client::play::COpenMountScreen {
                window_id: data.window_id.saturate_into(),
                slot_count: VarInt(data.slot_count),
                entity_id: data.entity_id.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::COpenScreen(data) => {
            let component_window_title =
                papokin_util::text::TextComponent::text(data.window_title.clone());
            let p = papokin_protocol::java::client::play::COpenScreen {
                sync_id: VarInt(data.sync_id),
                window_type: VarInt(data.window_type),
                window_title: &component_window_title,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::COpenSignEditor(data) => {
            let p = papokin_protocol::java::client::play::COpenSignEditor {
                location: papokin_util::math::position::BlockPos::new(
                    data.location.0,
                    data.location.1,
                    data.location.2,
                ),
                is_front_text: data.is_front_text.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CParticle(data) => {
            let p = papokin_protocol::java::client::play::CParticle {
                force_spawn: data.force_spawn.saturate_into(),
                important: data.important.saturate_into(),
                position: papokin_util::math::vector3::Vector3::new(
                    data.position.0 as _,
                    data.position.1 as _,
                    data.position.2 as _,
                ),
                offset: papokin_util::math::vector3::Vector3::new(
                    data.offset.0 as _,
                    data.offset.1 as _,
                    data.offset.2 as _,
                ),
                max_speed: data.max_speed.saturate_into(),
                particle_count: data.particle_count.saturate_into(),
                particle_id: VarInt(data.particle_id),
                data: &data.data,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CPlayPing(data) => {
            let p = papokin_protocol::java::client::play::CPlayPing {
                id: data.id.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CPingResponse(data) => {
            let p = papokin_protocol::java::client::play::CPingResponse {
                payload: data.payload.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CPlaceGhostRecipe(data) => {
            let p = papokin_protocol::java::client::play::CPlaceGhostRecipe {
                window_id: data.window_id.saturate_into(),
                recipe_id: &data.recipe_id,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CPlayerAbilities(data) => {
            let p = papokin_protocol::java::client::play::CPlayerAbilities {
                flags: data.flags.saturate_into(),
                flying_speed: data.flying_speed.saturate_into(),
                field_of_view: data.field_of_view.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CRemovePlayerInfo(data) => {
            let vec_players = data
                .players
                .iter()
                .map(|u| uuid::Uuid::from_u64_pair(u.high, u.low))
                .collect::<Vec<_>>();
            let p = papokin_protocol::java::client::play::CRemovePlayerInfo {
                players: &vec_players,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CPlayerRotation(data) => {
            let p = papokin_protocol::java::client::play::CPlayerRotation {
                yaw: data.yaw.saturate_into(),
                pitch: data.pitch.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CPlayerSpawnPosition(data) => {
            let p = papokin_protocol::java::client::play::CPlayerSpawnPosition {
                dimension_name: data.dimension_name.clone(),
                location: papokin_util::math::position::BlockPos::new(
                    data.location.0,
                    data.location.1,
                    data.location.2,
                ),
                yaw: data.yaw.saturate_into(),
                pitch: data.pitch.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CProjectilePower(data) => {
            let p = papokin_protocol::java::client::play::CProjectilePower {
                entity_id: VarInt(data.entity_id),
                x_power: data.x_power.saturate_into(),
                y_power: data.y_power.saturate_into(),
                z_power: data.z_power.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CRecipeBookAdd(data) => {
            let p = papokin_protocol::java::client::play::CRecipeBookAdd {
                replace: data.replace.saturate_into(),
                dynamic_recipes: &[],
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CRecipeBookRemove(data) => {
            let vec_recipes: Vec<VarInt> = data.recipes.iter().map(|v| VarInt(*v)).collect();
            let p = papokin_protocol::java::client::play::CRecipeBookRemove {
                recipes: &vec_recipes,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CRecipeBookSettings(data) => {
            let p = papokin_protocol::java::client::play::CRecipeBookSettings {
                crafting_open: data.crafting_open.saturate_into(),
                crafting_filtering: data.crafting_filtering.saturate_into(),
                furnace_open: data.furnace_open.saturate_into(),
                furnace_filtering: data.furnace_filtering.saturate_into(),
                blast_furnace_open: data.blast_furnace_open.saturate_into(),
                blast_furnace_filtering: data.blast_furnace_filtering.saturate_into(),
                smoker_open: data.smoker_open.saturate_into(),
                smoker_filtering: data.smoker_filtering.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CRemoveEntities(data) => {
            let vec_entity_ids: Vec<VarInt> = data.entity_ids.iter().map(|v| VarInt(*v)).collect();
            let p = papokin_protocol::java::client::play::CRemoveEntities {
                entity_ids: &vec_entity_ids,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CRemoveMobEffect(data) => {
            let p = papokin_protocol::java::client::play::CRemoveMobEffect {
                entity_id: VarInt(data.entity_id),
                effect_id: VarInt(data.effect_id),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CSetBorderCenter(data) => {
            let p = papokin_protocol::java::client::play::CSetBorderCenter {
                x: data.x.saturate_into(),
                z: data.z.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CSetBorderLerpSize(data) => {
            let p = papokin_protocol::java::client::play::CSetBorderLerpSize {
                old_diameter: data.old_diameter.saturate_into(),
                new_diameter: data.new_diameter.saturate_into(),
                speed: papokin_protocol::codec::var_long::VarLong(data.speed.saturate_into()),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CSetBorderSize(data) => {
            let p = papokin_protocol::java::client::play::CSetBorderSize {
                diameter: data.diameter.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CSetBorderWarningDelay(data) => {
            let p = papokin_protocol::java::client::play::CSetBorderWarningDelay {
                warning_time: VarInt(data.warning_time),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CSetBorderWarningDistance(data) => {
            let p = papokin_protocol::java::client::play::CSetBorderWarningDistance {
                warning_blocks: VarInt(data.warning_blocks),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CSetCamera(data) => {
            let p = papokin_protocol::java::client::play::CSetCamera {
                camera_id: VarInt(data.camera_id),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CSetChunkCacheRadius(data) => {
            let p = papokin_protocol::java::client::play::CSetChunkCacheRadius {
                radius: VarInt(data.radius),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CSetContainerProperty(data) => {
            let p = papokin_protocol::java::client::play::CSetContainerProperty {
                window_id: VarInt(data.window_id),
                property: data.property.saturate_into(),
                value: data.value.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CSetEntityLink(data) => {
            let p = papokin_protocol::java::client::play::CSetEntityLink {
                attached_entity_id: data.attached_entity_id.saturate_into(),
                holding_entity_id: data.holding_entity_id.saturate_into(),
                leash: data.leash.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CSetExperience(data) => {
            let p = papokin_protocol::java::client::play::CSetExperience {
                progress: data.progress.saturate_into(),
                level: VarInt(data.level),
                total_experience: VarInt(data.total_experience),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CSetHealth(data) => {
            let p = papokin_protocol::java::client::play::CSetHealth {
                health: data.health.saturate_into(),
                food: VarInt(data.food),
                food_saturation: data.food_saturation.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CSetPassengers(data) => {
            let vec_passengers: Vec<VarInt> = data.passengers.iter().map(|v| VarInt(*v)).collect();
            let p = papokin_protocol::java::client::play::CSetPassengers {
                entity_id: VarInt(data.entity_id),
                passengers: &vec_passengers,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CSetSimulationDistance(data) => {
            let p = papokin_protocol::java::client::play::CSetSimulationDistance {
                simulation_distance: VarInt(data.simulation_distance),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CTitleText(data) => {
            let component_title = papokin_util::text::TextComponent::text(data.title.clone());
            let p = papokin_protocol::java::client::play::CTitleText {
                title: &component_title,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CTitleAnimation(data) => {
            let p = papokin_protocol::java::client::play::CTitleAnimation {
                fade_in_ticks: data.fade_in_ticks.saturate_into(),
                stay_ticks: data.stay_ticks.saturate_into(),
                fade_out_ticks: data.fade_out_ticks.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CSpawnEntity(data) => {
            let uuid_entity_uuid =
                uuid::Uuid::from_u64_pair(data.entity_uuid.high, data.entity_uuid.low);
            let parsed_velocity: [f64; 3] = serde_json::from_str(&data.velocity).ok()?;
            let p = papokin_protocol::java::client::play::CSpawnEntity {
                entity_id: VarInt(data.entity_id),
                entity_uuid: uuid_entity_uuid,
                r#type: VarInt(data.r_type),
                position: papokin_util::math::vector3::Vector3::new(
                    data.position.0 as _,
                    data.position.1 as _,
                    data.position.2 as _,
                ),
                velocity: papokin_protocol::codec::lp_vector_3d::LpVector3d(
                    papokin_util::math::vector3::Vector3::new(
                        parsed_velocity[0],
                        parsed_velocity[1],
                        parsed_velocity[2],
                    ),
                ),
                pitch: data.pitch.saturate_into(),
                yaw: data.yaw.saturate_into(),
                head_yaw: data.head_yaw.saturate_into(),
                data: VarInt(data.data),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CSpawnPainting(data) => {
            let uuid_uuid = uuid::Uuid::from_u64_pair(data.uuid.high, data.uuid.low);
            let p = papokin_protocol::java::client::play::CSpawnPainting {
                entity_id: VarInt(data.entity_id),
                uuid: uuid_uuid,
                title: data.title.clone(),
                variant: VarInt(data.variant),
                location: papokin_util::math::position::BlockPos::new(
                    data.location.0,
                    data.location.1,
                    data.location.2,
                ),
                direction: data.direction.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CSubtitle(data) => {
            let component_subtitle = papokin_util::text::TextComponent::text(data.subtitle.clone());
            let p = papokin_protocol::java::client::play::CSubtitle {
                subtitle: &component_subtitle,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CTabList(data) => {
            let component_header = papokin_util::text::TextComponent::text(data.header.clone());
            let component_footer = papokin_util::text::TextComponent::text(data.footer.clone());
            let p = papokin_protocol::java::client::play::CTabList {
                header: &component_header,
                footer: &component_footer,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CTagQueryResponse(data) => {
            let p = papokin_protocol::java::client::play::CTagQueryResponse {
                transaction_id: VarInt(data.transaction_id),
                nbt_bytes: &data.nbt_bytes,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CTakeItemEntity(data) => {
            let p = papokin_protocol::java::client::play::CTakeItemEntity {
                entity_id: VarInt(data.entity_id),
                collector_entity_id: VarInt(data.collector_entity_id),
                stack_amount: VarInt(data.stack_amount),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CTickingState(data) => {
            let p = papokin_protocol::java::client::play::CTickingState {
                tick_rate: data.tick_rate.saturate_into(),
                is_frozen: data.is_frozen.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CTickingStep(data) => {
            let p = papokin_protocol::java::client::play::CTickingStep {
                tick_steps: VarInt(data.tick_steps),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CTransfer(data) => {
            let p = papokin_protocol::java::client::play::CTransfer {
                host: &data.host,
                port: VarInt(data.port),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CUnloadChunk(data) => {
            let p = papokin_protocol::java::client::play::CUnloadChunk {
                x: data.x.saturate_into(),
                z: data.z.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CUpdateEntityPos(data) => {
            let p = papokin_protocol::java::client::play::CUpdateEntityPos {
                entity_id: VarInt(data.entity_id),
                delta: papokin_util::math::vector3::Vector3::new(
                    data.delta.0 as _,
                    data.delta.1 as _,
                    data.delta.2 as _,
                ),
                on_ground: data.on_ground.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CUpdateEntityPosRot(data) => {
            let p = papokin_protocol::java::client::play::CUpdateEntityPosRot {
                entity_id: VarInt(data.entity_id),
                delta: papokin_util::math::vector3::Vector3::new(
                    data.delta.0 as _,
                    data.delta.1 as _,
                    data.delta.2 as _,
                ),
                yaw: data.yaw.saturate_into(),
                pitch: data.pitch.saturate_into(),
                on_ground: data.on_ground.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CUpdateEntityRot(data) => {
            let p = papokin_protocol::java::client::play::CUpdateEntityRot {
                entity_id: VarInt(data.entity_id),
                yaw: data.yaw.saturate_into(),
                pitch: data.pitch.saturate_into(),
                on_ground: data.on_ground.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CUpdateMobEffect(data) => {
            let p = papokin_protocol::java::client::play::CUpdateMobEffect {
                entity_id: VarInt(data.entity_id),
                effect_id: VarInt(data.effect_id),
                amplifier: VarInt(data.amplifier),
                duration: VarInt(data.duration),
                flags: data.flags.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CUpdateRecipes(data) => {
            let p = papokin_protocol::java::client::play::CUpdateRecipes {
                raw_data: &data.raw_data,
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CUseBed(data) => {
            let p = papokin_protocol::java::client::play::CUseBed {
                entity_id: VarInt(data.entity_id),
                location: papokin_util::math::position::BlockPos::new(
                    data.location.0,
                    data.location.1,
                    data.location.2,
                ),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::CWorldEvent(data) => {
            let p = papokin_protocol::java::client::play::CWorldEvent {
                event: data.event.saturate_into(),
                location: papokin_util::math::position::BlockPos::new(
                    data.location.0,
                    data.location.1,
                    data.location.2,
                ),
                data: data.data.saturate_into(),
                disable_relative_volume: data.disable_relative_volume.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::StatusCPingResponse(data) => {
            let p = papokin_protocol::java::client::status::CPingResponse {
                payload: data.payload.saturate_into(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        ClientboundPacket::StatusCStatusResponse(data) => {
            let p = papokin_protocol::java::client::status::CStatusResponse {
                json_response: data.json_response.clone(),
            };
            let mut buf = Vec::new();
            crate::net::java::JavaClient::write_packet_for_version(&p, version, &mut buf).ok()?;
            Some(buf.into())
        }
        _ => None,
    }
}

#[must_use]
pub fn deserialize_java_serverbound_packet(
    id: i32,
    mut payload: &[u8],
    version: JavaMinecraftVersion,
) -> Option<ServerboundPacket> {
    match id {
        id if id
            == papokin_protocol::java::server::config::SClientInformationConfig::to_id(version) =>
        {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::config::SClientInformationConfig as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::ConfigSClientInformationConfig(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::ConfigSClientInformationConfig {
                locale: p.locale.into(),
                view_distance: p.view_distance.saturate_into(),
                chat_mode: p.chat_mode.0.saturate_into(),
                chat_colors: p.chat_colors.saturate_into(),
                skin_parts: p.skin_parts.saturate_into(),
                main_hand: p.main_hand.0.saturate_into(),
                text_filtering: p.text_filtering.saturate_into(),
                server_listing: p.server_listing.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::config::SKeepAlive::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::config::SKeepAlive as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::ConfigSKeepAlive(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::ConfigSKeepAlive {
                keep_alive_id: p.keep_alive_id.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::config::SPluginMessage::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::config::SPluginMessage as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::ConfigSPluginMessage(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::ConfigSPluginMessage {
                channel: p.channel.into(),
                data: p.data.iter().map(|v| *v as _).collect(),
            }))
        }
        id if id == papokin_protocol::java::server::config::SConfigPong::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::config::SConfigPong as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::ConfigSConfigPong(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::ConfigSConfigPong {
                id: p.id.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::config::SConfigResourcePack::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::config::SConfigResourcePack as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::ConfigSConfigResourcePack(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::ConfigSConfigResourcePack {
                uuid: crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::uuid::Uuid { high: p.uuid.as_u64_pair().1, low: p.uuid.as_u64_pair().0 },
                result: p.result.0.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SAttack::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SAttack as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SAttack(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SAttack {
                entity_id: p.entity_id.0.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SBlockEntityTagQuery::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SBlockEntityTagQuery as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SBlockEntityTagQuery(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SBlockEntityTagQuery {
                transaction_id: p.transaction_id.0.saturate_into(),
                location: (p.location.0.x, p.location.0.y, p.location.0.z),
            }))
        }
        id if id == papokin_protocol::java::server::play::SBundleItemSelected::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SBundleItemSelected as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SBundleItemSelected(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SBundleItemSelected {
                slot_id: p.slot_id.0.saturate_into(),
                selected_item_index: p.selected_item_index.0.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SChatAck::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SChatAck as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SChatAck(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SChatAck {
                offset: p.offset.0.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SChatCommand::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SChatCommand as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SChatCommand(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SChatCommand {
                command: p.command.into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SChunkBatch::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SChunkBatch as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SChunkBatch(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SChunkBatch {
                chunks_per_tick: p.chunks_per_tick.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SClientCommand::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SClientCommand as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SClientCommand(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SClientCommand {
                action_id: p.action_id.0.saturate_into(),
            }))
        }
        id if id
            == papokin_protocol::java::server::play::SClientInformationPlay::to_id(version) =>
        {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SClientInformationPlay as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SClientInformationPlay(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SClientInformationPlay {
                locale: p.locale.into(),
                view_distance: p.view_distance.saturate_into(),
                chat_mode: p.chat_mode.0.saturate_into(),
                chat_colors: p.chat_colors.saturate_into(),
                skin_parts: p.skin_parts.saturate_into(),
                main_hand: p.main_hand.0.saturate_into(),
                text_filtering: p.text_filtering.saturate_into(),
                server_listing: p.server_listing.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SCloseContainer::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SCloseContainer as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SCloseContainer(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SCloseContainer {
                window_id: p.window_id.0.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SCommandSuggestion::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SCommandSuggestion as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SCommandSuggestion(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SCommandSuggestion {
                id: p.id.0.saturate_into(),
                command: p.command.into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SContainerButtonClick::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SContainerButtonClick as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SContainerButtonClick(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SContainerButtonClick {
                window_id: p.window_id.0.saturate_into(),
                button_id: p.button_id.0.saturate_into(),
            }))
        }
        id if id
            == papokin_protocol::java::server::play::SContainerSlotStateChanged::to_id(version) =>
        {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SContainerSlotStateChanged as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SContainerSlotStateChanged(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SContainerSlotStateChanged {
                slot_id: p.slot_id.0.saturate_into(),
                container_id: p.container_id.0.saturate_into(),
                new_state: p.new_state.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SCustomPayload::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SCustomPayload as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SCustomPayload(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SCustomPayload {
                channel: p.channel.into(),
                data: p.data.iter().map(|v| *v as _).collect(),
            }))
        }
        id if id
            == papokin_protocol::java::server::play::SDebugSampleSubscription::to_id(version) =>
        {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SDebugSampleSubscription as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SDebugSampleSubscription(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SDebugSampleSubscription {
                sample_type: p.sample_type.0.saturate_into(),
            }))
        }
        id if id
            == papokin_protocol::java::server::play::SDebugSubscriptionRequest::to_id(version) =>
        {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SDebugSubscriptionRequest as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SDebugSubscriptionRequest(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SDebugSubscriptionRequest {
                sample_type: p.sample_type.0.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SEntityTagQuery::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SEntityTagQuery as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SEntityTagQuery(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SEntityTagQuery {
                transaction_id: p.transaction_id.0.saturate_into(),
                entity_id: p.entity_id.0.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SJigsawGenerate::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SJigsawGenerate as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SJigsawGenerate(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SJigsawGenerate {
                pos: (p.pos.0.x, p.pos.0.y, p.pos.0.z),
                levels: p.levels.0.saturate_into(),
                keep_jigsaws: p.keep_jigsaws.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SKeepAlive::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SKeepAlive as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SKeepAlive(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SKeepAlive {
                keep_alive_id: p.keep_alive_id.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SLockDifficulty::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SLockDifficulty as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SLockDifficulty(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SLockDifficulty {
                locked: p.locked.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SMoveVehicle::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SMoveVehicle as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SMoveVehicle(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SMoveVehicle {
                x: p.x.saturate_into(),
                y: p.y.saturate_into(),
                z: p.z.saturate_into(),
                yaw: p.yaw.saturate_into(),
                pitch: p.pitch.saturate_into(),
                on_ground: p.on_ground.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SPaddleBoat::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SPaddleBoat as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SPaddleBoat(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SPaddleBoat {
                left_paddle: p.left_paddle.saturate_into(),
                right_paddle: p.right_paddle.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SPickItemFromBlock::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SPickItemFromBlock as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SPickItemFromBlock(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SPickItemFromBlock {
                pos: (p.pos.0.x, p.pos.0.y, p.pos.0.z),
                include_data: p.include_data.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SPickItemFromEntity::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SPickItemFromEntity as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SPickItemFromEntity(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SPickItemFromEntity {
                id: p.id.0.saturate_into(),
                include_data: p.include_data.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SPlayPingRequest::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SPlayPingRequest as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SPlayPingRequest(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SPlayPingRequest {
                payload: p.payload.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SPlaceRecipe::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SPlaceRecipe as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SPlaceRecipe(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SPlaceRecipe {
                container_id: p.container_id.saturate_into(),
                recipe_display_id: p.recipe_display_id.0.saturate_into(),
                use_max_items: p.use_max_items.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SPlayerAction::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SPlayerAction as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SPlayerAction(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SPlayerAction {
                status: p.status.0.saturate_into(),
                position: (p.position.0.x, p.position.0.y, p.position.0.z),
                face: p.face.saturate_into(),
                sequence: p.sequence.0.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SSetPlayerGround::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SSetPlayerGround as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SSetPlayerGround(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SSetPlayerGround {
                on_ground: p.on_ground.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SPlayerInput::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SPlayerInput as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SPlayerInput(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SPlayerInput {
                input: p.input.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SPlayerPosition::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SPlayerPosition as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SPlayerPosition(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SPlayerPosition {
                position: (p.position.x as _, p.position.y as _, p.position.z as _),
                collision: p.collision.saturate_into(),
            }))
        }
        id if id
            == papokin_protocol::java::server::play::SPlayerPositionRotation::to_id(version) =>
        {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SPlayerPositionRotation as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SPlayerPositionRotation(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SPlayerPositionRotation {
                position: (p.position.x as _, p.position.y as _, p.position.z as _),
                yaw: p.yaw.saturate_into(),
                pitch: p.pitch.saturate_into(),
                collision: p.collision.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SPlayerRotation::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SPlayerRotation as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SPlayerRotation(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SPlayerRotation {
                yaw: p.yaw.saturate_into(),
                pitch: p.pitch.saturate_into(),
                ground: p.ground.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SPlayerSession::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SPlayerSession as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SPlayerSession(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SPlayerSession {
                session_id: crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::uuid::Uuid { high: p.session_id.as_u64_pair().1, low: p.session_id.as_u64_pair().0 },
                expires_at: p.expires_at.saturate_into(),
                public_key: p.public_key.to_vec(),
                key_signature: p.key_signature.to_vec(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SPlayPong::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SPlayPong as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SPlayPong(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SPlayPong {
                id: p.id.saturate_into(),
            }))
        }
        id if id
            == papokin_protocol::java::server::play::SRecipeBookChangeSettings::to_id(version) =>
        {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SRecipeBookChangeSettings as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SRecipeBookChangeSettings(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SRecipeBookChangeSettings {
                book_type: p.book_type.0.saturate_into(),
                is_open: p.is_open.saturate_into(),
                is_filtering: p.is_filtering.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SRecipeBookSeenRecipe::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SRecipeBookSeenRecipe as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SRecipeBookSeenRecipe(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SRecipeBookSeenRecipe {
                recipe_display_id: p.recipe_display_id.0.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SRenameItem::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SRenameItem as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SRenameItem(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SRenameItem {
                item_name: p.item_name.into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SPlayResourcePack::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SPlayResourcePack as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SPlayResourcePack(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SPlayResourcePack {
                uuid: crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::uuid::Uuid { high: p.uuid.as_u64_pair().1, low: p.uuid.as_u64_pair().0 },
                result: p.result.0.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SSeenAdvancement::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SSeenAdvancement as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(match p {
                papokin_protocol::java::server::play::SSeenAdvancement::OpenTab(identifier) => ServerboundPacket::SSeenAdvancement(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SSeenAdvancement::OpenTab(identifier.to_string())),
                papokin_protocol::java::server::play::SSeenAdvancement::CloseTab => ServerboundPacket::SSeenAdvancement(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SSeenAdvancement::CloseTab),
            })
        }
        id if id == papokin_protocol::java::server::play::SSelectTrade::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SSelectTrade as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SSelectTrade(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SSelectTrade {
                selected_slot: p.selected_slot.0.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SSetCommandBlock::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SSetCommandBlock as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SSetCommandBlock(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SSetCommandBlock {
                pos: (p.pos.0.x, p.pos.0.y, p.pos.0.z),
                command: p.command.into(),
                mode: p.mode.0.saturate_into(),
                flags: p.flags.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SSetCommandMinecart::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SSetCommandMinecart as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SSetCommandMinecart(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SSetCommandMinecart {
                entity_id: p.entity_id.0.saturate_into(),
                command: p.command.into(),
                track_output: p.track_output.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SSetHeldItem::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SSetHeldItem as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SSetHeldItem(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SSetHeldItem {
                slot: p.slot.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SSetJigsawBlock::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SSetJigsawBlock as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SSetJigsawBlock(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SSetJigsawBlock {
                pos: (p.pos.0.x, p.pos.0.y, p.pos.0.z),
                name: p.name.into(),
                target: p.target.into(),
                pool: p.pool.into(),
                final_state: p.final_state.into(),
                joint: p.joint.into(),
                selection_priority: p.selection_priority.0.saturate_into(),
                placement_priority: p.placement_priority.0.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SSetStructureBlock::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SSetStructureBlock as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SSetStructureBlock(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SSetStructureBlock {
                location: (p.location.0.x, p.location.0.y, p.location.0.z),
                action: p.action.0.saturate_into(),
                mode: p.mode.0.saturate_into(),
                name: p.name.into(),
                offset_x: p.offset_x.saturate_into(),
                offset_y: p.offset_y.saturate_into(),
                offset_z: p.offset_z.saturate_into(),
                size_x: p.size_x.saturate_into(),
                size_y: p.size_y.saturate_into(),
                size_z: p.size_z.saturate_into(),
                mirror: p.mirror.0.saturate_into(),
                rotation: p.rotation.0.saturate_into(),
                metadata: p.metadata.into(),
                integrity: p.integrity.saturate_into(),
                seed: p.seed.0.saturate_into(),
                flags: p.flags.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SSpectateEntity::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SSpectateEntity as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SSpectateEntity(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SSpectateEntity {
                target: crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::uuid::Uuid { high: p.target.as_u64_pair().1, low: p.target.as_u64_pair().0 },
            }))
        }
        id if id == papokin_protocol::java::server::play::STeleportToEntity::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::STeleportToEntity as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::STeleportToEntity(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::STeleportToEntity {
                target: crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::uuid::Uuid { high: p.target.as_u64_pair().1, low: p.target.as_u64_pair().0 },
            }))
        }
        id if id == papokin_protocol::java::server::play::SUpdateSign::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SUpdateSign as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SUpdateSign(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SUpdateSign {
                location: (p.location.0.x, p.location.0.y, p.location.0.z),
                is_front_text: p.is_front_text.saturate_into(),
                line_1: p.line_1.into(),
                line_2: p.line_2.into(),
                line_3: p.line_3.into(),
                line_4: p.line_4.into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SUseItem::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SUseItem as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SUseItem(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SUseItem {
                hand: p.hand.0.saturate_into(),
                sequence: p.sequence.0.saturate_into(),
                yaw: p.yaw.saturate_into(),
                pitch: p.pitch.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::play::SUseItemOn::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::play::SUseItemOn as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::SUseItemOn(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::SUseItemOn {
                hand: p.hand.0.saturate_into(),
                position: (p.position.0.x, p.position.0.y, p.position.0.z),
                face: p.face.0.saturate_into(),
                cursor_pos: (p.cursor_pos.x as _, p.cursor_pos.y as _, p.cursor_pos.z as _),
                inside_block: p.inside_block.saturate_into(),
                is_against_world_border: p.is_against_world_border.saturate_into(),
                sequence: p.sequence.0.saturate_into(),
            }))
        }
        id if id == papokin_protocol::java::server::status::SStatusPingRequest::to_id(version) => {
            use papokin_protocol::ServerPacket;
            let p = <papokin_protocol::java::server::status::SStatusPingRequest as papokin_protocol::ServerPacket>::read(&mut payload, &version).ok()?;
            Some(ServerboundPacket::StatusSStatusPingRequest(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::StatusSStatusPingRequest {
                payload: p.payload.saturate_into(),
            }))
        }
        _ => None,
    }
}

pub trait ToWitClientboundJava {
    fn to_wit(&self) -> ClientboundPacket;
}

impl ToWitClientboundJava for papokin_protocol::java::client::config::CCodeOfConduct<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::ConfigCCodeOfConduct(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::ConfigCCodeOfConduct {
                code_of_conduct: self.code_of_conduct.to_string(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::config::CConfigDisconnect<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::ConfigCConfigDisconnect(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::ConfigCConfigDisconnect {
                reason: self.reason.to_string(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::config::CFeatureFlags<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::ConfigCFeatureFlags(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::ConfigCFeatureFlags {
                features: self.features.iter().map(|s| s.to_string()).collect(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::config::CConfigPing {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::ConfigCConfigPing(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::ConfigCConfigPing {
                id: self.id.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::config::CPluginMessage<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::ConfigCPluginMessage(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::ConfigCPluginMessage {
                channel: self.channel.to_string(),
                data: self.data.iter().map(|v| *v as _).collect(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::config::CTransfer<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::ConfigCTransfer(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::ConfigCTransfer {
                host: self.host.to_string(),
                port: self.port.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::login::CLoginDisconnect {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::LoginCLoginDisconnect(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::LoginCLoginDisconnect {
                json_reason: self.json_reason.to_string(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::login::CLoginPluginRequest<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::LoginCLoginPluginRequest(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::LoginCLoginPluginRequest {
                message_id: self.message_id.0.saturate_into(),
                channel: self.channel.to_string(),
                data: self.data.iter().map(|v| *v as _).collect(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::login::CSetCompression {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::LoginCSetCompression(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::LoginCSetCompression {
                threshold: self.threshold.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CAcknowledgeBlockChange {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CAcknowledgeBlockChange(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CAcknowledgeBlockChange {
                sequence_id: self.sequence_id.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CActionBar<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CActionBar(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CActionBar {
                action_bar: serde_json::to_string(&self.action_bar).unwrap_or_default(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CSetBlockDestroyStage {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CSetBlockDestroyStage(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CSetBlockDestroyStage {
                entity_id: self.entity_id.0.saturate_into(),
                location: (self.location.0.x, self.location.0.y, self.location.0.z),
                destroy_stage: self.destroy_stage.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CBlockEntityData {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CBlockEntityData(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CBlockEntityData {
                location: (self.location.0.x, self.location.0.y, self.location.0.z),
                r_type: self.r#type.0.saturate_into(),
                nbt_data: self.nbt_data.to_vec(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CBlockEvent {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CBlockEvent(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CBlockEvent {
                location: (self.location.0.x, self.location.0.y, self.location.0.z),
                action_id: self.action_id.saturate_into(),
                action_parameter: self.action_parameter.saturate_into(),
                block_type: self.block_type.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CBlockUpdate {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CBlockUpdate(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CBlockUpdate {
                location: (self.location.0.x, self.location.0.y, self.location.0.z),
                state_id: self.state_id.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CCenterChunk {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CCenterChunk(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CCenterChunk {
                chunk_x: self.chunk_x.0.saturate_into(),
                chunk_z: self.chunk_z.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CChangeDifficulty {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CChangeDifficulty(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CChangeDifficulty {
                difficulty: self.difficulty.saturate_into(),
                locked: self.locked.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CChunkBatchEnd {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CChunkBatchEnd(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CChunkBatchEnd {
                batch_size: self.batch_size.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CClearTitle {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CClearTitle(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CClearTitle {
                reset: self.reset.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CCloseContainer {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CCloseContainer(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CCloseContainer {
                sync_id: self.sync_id.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CCombatDeath<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CCombatDeath(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CCombatDeath {
                player_id: self.player_id.0.saturate_into(),
                message: serde_json::to_string(&self.message).unwrap_or_default(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CCombatEnd {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CCombatEnd(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CCombatEnd {
                duration_ticks: self.duration_ticks.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CCustomChatCompletions<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CCustomChatCompletions(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CCustomChatCompletions {
                action: self.action.0.saturate_into(),
                entries: self.entries.iter().map(|s| s.to_string()).collect(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CCustomPayload<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CCustomPayload(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CCustomPayload {
                channel: self.channel.to_string(),
                data: self.data.iter().map(|v| *v as _).collect(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CDebugBlockValue<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CDebugBlockValue(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CDebugBlockValue {
                pos: (self.pos.0.x, self.pos.0.y, self.pos.0.z),
                name: self.name.to_string(),
                value: self.value.to_string(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CDebugEntityValue<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CDebugEntityValue(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CDebugEntityValue {
                entity_id: self.entity_id.0.saturate_into(),
                name: self.name.to_string(),
                value: self.value.to_string(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CDebugEvent<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CDebugEvent(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CDebugEvent {
                name: self.name.to_string(),
                data: self.data.iter().map(|v| *v as _).collect(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CDebugSample<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CDebugSample(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CDebugSample {
                sample: self.sample.iter().map(|v| *v as _).collect(),
                sample_type: self.sample_type.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CPlayDisconnect<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CPlayDisconnect(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CPlayDisconnect {
                reason: serde_json::to_string(&self.reason).unwrap_or_default(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CDisplayObjective {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CDisplayObjective(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CDisplayObjective {
                position: self.position.0.saturate_into(),
                score_name: self.score_name.to_string(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CEntityAnimation {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CEntityAnimation(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CEntityAnimation {
                entity_id: self.entity_id.0.saturate_into(),
                animation: self.animation.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CSetEntityMetadata {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CSetEntityMetadata(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CSetEntityMetadata {
                entity_id: self.entity_id.0.saturate_into(),
                metadata: self.metadata.to_vec(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CEntityStatus {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CEntityStatus(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CEntityStatus {
                entity_id: self.entity_id.saturate_into(),
                entity_status: self.entity_status.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CEntityVelocity {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CEntityVelocity(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CEntityVelocity {
                entity_id: self.entity_id.0.saturate_into(),
                velocity: serde_json::to_string(&[self.velocity.0.x, self.velocity.0.y, self.velocity.0.z]).unwrap_or_default(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CGameEvent {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CGameEvent(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CGameEvent {
                event: self.event.saturate_into(),
                value: self.value.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CGameTestHighlightPos<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CGameTestHighlightPos(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CGameTestHighlightPos {
                pos: (self.pos.0.x, self.pos.0.y, self.pos.0.z),
                color: self.color.saturate_into(),
                label: self.label.to_string(),
                duration_ms: self.duration_ms.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CHeadRot {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CHeadRot(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CHeadRot {
                entity_id: self.entity_id.0.saturate_into(),
                head_yaw: self.head_yaw.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CHurtAnimation {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CHurtAnimation(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CHurtAnimation {
                entity_id: self.entity_id.0.saturate_into(),
                yaw: self.yaw.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CInitializeWorldBorder {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CInitializeWorldBorder(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CInitializeWorldBorder {
                x: self.x.saturate_into(),
                z: self.z.saturate_into(),
                old_diameter: self.old_diameter.saturate_into(),
                new_diameter: self.new_diameter.saturate_into(),
                speed: self.speed.0.saturate_into(),
                portal_teleport_boundary: self.portal_teleport_boundary.0.saturate_into(),
                warning_blocks: self.warning_blocks.0.saturate_into(),
                warning_time: self.warning_time.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CItemCooldown {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CItemCooldown(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CItemCooldown {
                group: self.group.to_string(),
                cooldown: self.cooldown.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CKeepAlive {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CKeepAlive(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CKeepAlive {
                keep_alive_id: self.keep_alive_id.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CLevelEvent {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CLevelEvent(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CLevelEvent {
                event: self.event.saturate_into(),
                location: (self.location.0.x, self.location.0.y, self.location.0.z),
                data: self.data.saturate_into(),
                disable_relative_volume: self.disable_relative_volume.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CMoveVehicle {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CMoveVehicle(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CMoveVehicle {
                x: self.x.saturate_into(),
                y: self.y.saturate_into(),
                z: self.z.saturate_into(),
                yaw: self.yaw.saturate_into(),
                pitch: self.pitch.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::COpenBook {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::COpenBook(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::COpenBook {
                hand: self.hand.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::COpenMountScreen {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::COpenMountScreen(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::COpenMountScreen {
                window_id: self.window_id.saturate_into(),
                slot_count: self.slot_count.0.saturate_into(),
                entity_id: self.entity_id.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::COpenScreen<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::COpenScreen(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::COpenScreen {
                sync_id: self.sync_id.0.saturate_into(),
                window_type: self.window_type.0.saturate_into(),
                window_title: serde_json::to_string(&self.window_title).unwrap_or_default(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::COpenSignEditor {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::COpenSignEditor(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::COpenSignEditor {
                location: (self.location.0.x, self.location.0.y, self.location.0.z),
                is_front_text: self.is_front_text.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CParticle<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CParticle(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CParticle {
                force_spawn: self.force_spawn.saturate_into(),
                important: self.important.saturate_into(),
                position: (self.position.x as _, self.position.y as _, self.position.z as _),
                offset: (self.offset.x as _, self.offset.y as _, self.offset.z as _),
                max_speed: self.max_speed.saturate_into(),
                particle_count: self.particle_count.saturate_into(),
                particle_id: self.particle_id.0.saturate_into(),
                data: self.data.iter().map(|v| *v as _).collect(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CPlayPing {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CPlayPing(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CPlayPing {
                id: self.id.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CPingResponse {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CPingResponse(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CPingResponse {
                payload: self.payload.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CPlaceGhostRecipe<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CPlaceGhostRecipe(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CPlaceGhostRecipe {
                window_id: self.window_id.saturate_into(),
                recipe_id: self.recipe_id.to_string(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CPlayerAbilities {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CPlayerAbilities(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CPlayerAbilities {
                flags: self.flags.saturate_into(),
                flying_speed: self.flying_speed.saturate_into(),
                field_of_view: self.field_of_view.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CRemovePlayerInfo<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CRemovePlayerInfo(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CRemovePlayerInfo {
                players: self.players.iter().map(|u| crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::uuid::Uuid { high: u.as_u64_pair().1, low: u.as_u64_pair().0 }).collect(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CPlayerRotation {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CPlayerRotation(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CPlayerRotation {
                yaw: self.yaw.saturate_into(),
                pitch: self.pitch.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CPlayerSpawnPosition {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CPlayerSpawnPosition(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CPlayerSpawnPosition {
                dimension_name: self.dimension_name.to_string(),
                location: (self.location.0.x, self.location.0.y, self.location.0.z),
                yaw: self.yaw.saturate_into(),
                pitch: self.pitch.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CProjectilePower {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CProjectilePower(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CProjectilePower {
                entity_id: self.entity_id.0.saturate_into(),
                x_power: self.x_power.saturate_into(),
                y_power: self.y_power.saturate_into(),
                z_power: self.z_power.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CRecipeBookRemove<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CRecipeBookRemove(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CRecipeBookRemove {
                recipes: self.recipes.iter().map(|v| v.0 as _).collect(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CRecipeBookSettings {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CRecipeBookSettings(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CRecipeBookSettings {
                crafting_open: self.crafting_open.saturate_into(),
                crafting_filtering: self.crafting_filtering.saturate_into(),
                furnace_open: self.furnace_open.saturate_into(),
                furnace_filtering: self.furnace_filtering.saturate_into(),
                blast_furnace_open: self.blast_furnace_open.saturate_into(),
                blast_furnace_filtering: self.blast_furnace_filtering.saturate_into(),
                smoker_open: self.smoker_open.saturate_into(),
                smoker_filtering: self.smoker_filtering.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CRemoveEntities<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CRemoveEntities(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CRemoveEntities {
                entity_ids: self.entity_ids.iter().map(|v| v.0 as _).collect(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CRemoveMobEffect {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CRemoveMobEffect(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CRemoveMobEffect {
                entity_id: self.entity_id.0.saturate_into(),
                effect_id: self.effect_id.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CSetBorderCenter {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CSetBorderCenter(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CSetBorderCenter {
                x: self.x.saturate_into(),
                z: self.z.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CSetBorderLerpSize {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CSetBorderLerpSize(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CSetBorderLerpSize {
                old_diameter: self.old_diameter.saturate_into(),
                new_diameter: self.new_diameter.saturate_into(),
                speed: self.speed.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CSetBorderSize {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CSetBorderSize(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CSetBorderSize {
                diameter: self.diameter.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CSetBorderWarningDelay {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CSetBorderWarningDelay(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CSetBorderWarningDelay {
                warning_time: self.warning_time.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CSetBorderWarningDistance {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CSetBorderWarningDistance(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CSetBorderWarningDistance {
                warning_blocks: self.warning_blocks.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CSetCamera {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CSetCamera(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CSetCamera {
                camera_id: self.camera_id.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CSetChunkCacheRadius {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CSetChunkCacheRadius(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CSetChunkCacheRadius {
                radius: self.radius.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CSetContainerProperty {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CSetContainerProperty(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CSetContainerProperty {
                window_id: self.window_id.0.saturate_into(),
                property: self.property.saturate_into(),
                value: self.value.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CSetEntityLink {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CSetEntityLink(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CSetEntityLink {
                attached_entity_id: self.attached_entity_id.saturate_into(),
                holding_entity_id: self.holding_entity_id.saturate_into(),
                leash: self.leash.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CSetExperience {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CSetExperience(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CSetExperience {
                progress: self.progress.saturate_into(),
                level: self.level.0.saturate_into(),
                total_experience: self.total_experience.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CSetHealth {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CSetHealth(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CSetHealth {
                health: self.health.saturate_into(),
                food: self.food.0.saturate_into(),
                food_saturation: self.food_saturation.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CSetPassengers<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CSetPassengers(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CSetPassengers {
                entity_id: self.entity_id.0.saturate_into(),
                passengers: self.passengers.iter().map(|v| v.0 as _).collect(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CSetSimulationDistance {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CSetSimulationDistance(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CSetSimulationDistance {
                simulation_distance: self.simulation_distance.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CTitleText<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CTitleText(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CTitleText {
                title: serde_json::to_string(&self.title).unwrap_or_default(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CTitleAnimation {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CTitleAnimation(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CTitleAnimation {
                fade_in_ticks: self.fade_in_ticks.saturate_into(),
                stay_ticks: self.stay_ticks.saturate_into(),
                fade_out_ticks: self.fade_out_ticks.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CSpawnEntity {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CSpawnEntity(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CSpawnEntity {
                entity_id: self.entity_id.0.saturate_into(),
                entity_uuid: crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::uuid::Uuid { high: self.entity_uuid.as_u64_pair().1, low: self.entity_uuid.as_u64_pair().0 },
                r_type: self.r#type.0.saturate_into(),
                position: (self.position.x as _, self.position.y as _, self.position.z as _),
                velocity: serde_json::to_string(&[self.velocity.0.x, self.velocity.0.y, self.velocity.0.z]).unwrap_or_default(),
                pitch: self.pitch.saturate_into(),
                yaw: self.yaw.saturate_into(),
                head_yaw: self.head_yaw.saturate_into(),
                data: self.data.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CSpawnPainting {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CSpawnPainting(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CSpawnPainting {
                entity_id: self.entity_id.0.saturate_into(),
                uuid: crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::uuid::Uuid { high: self.uuid.as_u64_pair().1, low: self.uuid.as_u64_pair().0 },
                title: self.title.to_string(),
                variant: self.variant.0.saturate_into(),
                location: (self.location.0.x, self.location.0.y, self.location.0.z),
                direction: self.direction.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CSubtitle<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CSubtitle(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CSubtitle {
                subtitle: serde_json::to_string(&self.subtitle).unwrap_or_default(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CTabList<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CTabList(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CTabList {
                header: serde_json::to_string(&self.header).unwrap_or_default(),
                footer: serde_json::to_string(&self.footer).unwrap_or_default(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CTagQueryResponse<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CTagQueryResponse(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CTagQueryResponse {
                transaction_id: self.transaction_id.0.saturate_into(),
                nbt_bytes: self.nbt_bytes.iter().map(|v| *v as _).collect(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CTakeItemEntity {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CTakeItemEntity(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CTakeItemEntity {
                entity_id: self.entity_id.0.saturate_into(),
                collector_entity_id: self.collector_entity_id.0.saturate_into(),
                stack_amount: self.stack_amount.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CTickingState {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CTickingState(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CTickingState {
                tick_rate: self.tick_rate.saturate_into(),
                is_frozen: self.is_frozen.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CTickingStep {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CTickingStep(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CTickingStep {
                tick_steps: self.tick_steps.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CTransfer<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CTransfer(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CTransfer {
                host: self.host.to_string(),
                port: self.port.0.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CUnloadChunk {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CUnloadChunk(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CUnloadChunk {
                x: self.x.saturate_into(),
                z: self.z.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CUpdateEntityPos {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CUpdateEntityPos(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CUpdateEntityPos {
                entity_id: self.entity_id.0.saturate_into(),
                delta: (self.delta.x as _, self.delta.y as _, self.delta.z as _),
                on_ground: self.on_ground.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CUpdateEntityPosRot {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CUpdateEntityPosRot(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CUpdateEntityPosRot {
                entity_id: self.entity_id.0.saturate_into(),
                delta: (self.delta.x as _, self.delta.y as _, self.delta.z as _),
                yaw: self.yaw.saturate_into(),
                pitch: self.pitch.saturate_into(),
                on_ground: self.on_ground.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CUpdateEntityRot {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CUpdateEntityRot(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CUpdateEntityRot {
                entity_id: self.entity_id.0.saturate_into(),
                yaw: self.yaw.saturate_into(),
                pitch: self.pitch.saturate_into(),
                on_ground: self.on_ground.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CUpdateMobEffect {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CUpdateMobEffect(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CUpdateMobEffect {
                entity_id: self.entity_id.0.saturate_into(),
                effect_id: self.effect_id.0.saturate_into(),
                amplifier: self.amplifier.0.saturate_into(),
                duration: self.duration.0.saturate_into(),
                flags: self.flags.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CUpdateRecipes<'_> {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CUpdateRecipes(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CUpdateRecipes {
                raw_data: self.raw_data.iter().map(|v| *v as _).collect(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CUseBed {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CUseBed(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CUseBed {
                entity_id: self.entity_id.0.saturate_into(),
                location: (self.location.0.x, self.location.0.y, self.location.0.z),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::play::CWorldEvent {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::CWorldEvent(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::CWorldEvent {
                event: self.event.saturate_into(),
                location: (self.location.0.x, self.location.0.y, self.location.0.z),
                data: self.data.saturate_into(),
                disable_relative_volume: self.disable_relative_volume.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::status::CPingResponse {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::StatusCPingResponse(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::StatusCPingResponse {
                payload: self.payload.saturate_into(),
        })
    }
}

impl ToWitClientboundJava for papokin_protocol::java::client::status::CStatusResponse {
    fn to_wit(&self) -> ClientboundPacket {
        ClientboundPacket::StatusCStatusResponse(crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::java_packets::StatusCStatusResponse {
                json_response: self.json_response.to_string(),
        })
    }
}

#[must_use]
pub fn clientbound_java_any_to_wit(any: &dyn Any) -> Option<ClientboundPacket> {
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::config::CCodeOfConduct>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::config::CConfigDisconnect>()
    {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::config::CFeatureFlags>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::config::CConfigPing>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::config::CPluginMessage>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::config::CTransfer>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::login::CLoginDisconnect>() {
        return Some(p.to_wit());
    }
    if let Some(p) =
        any.downcast_ref::<papokin_protocol::java::client::login::CLoginPluginRequest>()
    {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::login::CSetCompression>() {
        return Some(p.to_wit());
    }
    if let Some(p) =
        any.downcast_ref::<papokin_protocol::java::client::play::CAcknowledgeBlockChange>()
    {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CActionBar>() {
        return Some(p.to_wit());
    }
    if let Some(p) =
        any.downcast_ref::<papokin_protocol::java::client::play::CSetBlockDestroyStage>()
    {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CBlockEntityData>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CBlockEvent>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CBlockUpdate>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CCenterChunk>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CChangeDifficulty>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CChunkBatchEnd>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CClearTitle>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CCloseContainer>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CCombatDeath>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CCombatEnd>() {
        return Some(p.to_wit());
    }
    if let Some(p) =
        any.downcast_ref::<papokin_protocol::java::client::play::CCustomChatCompletions>()
    {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CCustomPayload>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CDebugBlockValue>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CDebugEntityValue>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CDebugEvent>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CDebugSample>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CPlayDisconnect>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CDisplayObjective>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CEntityAnimation>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CSetEntityMetadata>()
    {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CEntityStatus>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CEntityVelocity>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CGameEvent>() {
        return Some(p.to_wit());
    }
    if let Some(p) =
        any.downcast_ref::<papokin_protocol::java::client::play::CGameTestHighlightPos>()
    {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CHeadRot>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CHurtAnimation>() {
        return Some(p.to_wit());
    }
    if let Some(p) =
        any.downcast_ref::<papokin_protocol::java::client::play::CInitializeWorldBorder>()
    {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CItemCooldown>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CKeepAlive>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CLevelEvent>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CMoveVehicle>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::COpenBook>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::COpenMountScreen>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::COpenScreen>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::COpenSignEditor>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CParticle>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CPlayPing>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CPingResponse>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CPlaceGhostRecipe>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CPlayerAbilities>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CRemovePlayerInfo>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CPlayerRotation>() {
        return Some(p.to_wit());
    }
    if let Some(p) =
        any.downcast_ref::<papokin_protocol::java::client::play::CPlayerSpawnPosition>()
    {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CProjectilePower>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CRecipeBookRemove>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CRecipeBookSettings>()
    {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CRemoveEntities>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CRemoveMobEffect>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CSetBorderCenter>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CSetBorderLerpSize>()
    {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CSetBorderSize>() {
        return Some(p.to_wit());
    }
    if let Some(p) =
        any.downcast_ref::<papokin_protocol::java::client::play::CSetBorderWarningDelay>()
    {
        return Some(p.to_wit());
    }
    if let Some(p) =
        any.downcast_ref::<papokin_protocol::java::client::play::CSetBorderWarningDistance>()
    {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CSetCamera>() {
        return Some(p.to_wit());
    }
    if let Some(p) =
        any.downcast_ref::<papokin_protocol::java::client::play::CSetChunkCacheRadius>()
    {
        return Some(p.to_wit());
    }
    if let Some(p) =
        any.downcast_ref::<papokin_protocol::java::client::play::CSetContainerProperty>()
    {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CSetEntityLink>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CSetExperience>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CSetHealth>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CSetPassengers>() {
        return Some(p.to_wit());
    }
    if let Some(p) =
        any.downcast_ref::<papokin_protocol::java::client::play::CSetSimulationDistance>()
    {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CTitleText>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CTitleAnimation>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CSpawnEntity>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CSpawnPainting>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CSubtitle>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CTabList>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CTagQueryResponse>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CTakeItemEntity>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CTickingState>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CTickingStep>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CTransfer>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CUnloadChunk>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CUpdateEntityPos>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CUpdateEntityPosRot>()
    {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CUpdateEntityRot>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CUpdateMobEffect>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CUpdateRecipes>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CUseBed>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::play::CWorldEvent>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::status::CPingResponse>() {
        return Some(p.to_wit());
    }
    if let Some(p) = any.downcast_ref::<papokin_protocol::java::client::status::CStatusResponse>() {
        return Some(p.to_wit());
    }
    None
}

#[cfg(test)]
mod saturate_into_tests {
    use super::SaturateInto;

    #[test]
    fn 有符号收窄时钳制到边界() {
        let v: i8 = 300i32.saturate_into();
        assert_eq!(v, 127);
        let v: i8 = (-300i32).saturate_into();
        assert_eq!(v, -128);
        let v: i8 = 100i32.saturate_into();
        assert_eq!(v, 100);
    }

    #[test]
    fn 跨符号收窄时钳制到非负区间() {
        let v: u8 = (-5i8).saturate_into();
        assert_eq!(v, 0);
        let v: i8 = u32::MAX.saturate_into();
        assert_eq!(v, 127);
    }

    #[test]
    fn 宽源截断到目标宽度前先钳制() {
        let v: u32 = 0x1_0000_0001u64.saturate_into();
        assert_eq!(v, u32::MAX);
        let v: u32 = i64::MIN.saturate_into();
        assert_eq!(v, 0);
    }

    #[test]
    fn 同宽与浮点布尔保持恒等() {
        let v: u32 = 42u32.saturate_into();
        assert_eq!(v, 42);
        let v: f64 = 1.5f64.saturate_into();
        assert_eq!(v, 1.5);
        let v: bool = true.saturate_into();
        assert!(v);
    }
}
