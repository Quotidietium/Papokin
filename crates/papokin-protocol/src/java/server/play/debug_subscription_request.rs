use crate::{
    ServerPacket, VarInt,
    ser::{NetworkReadExt, ReadingError},
};
use papokin_data::packet::serverbound::play::DEBUG_SUBSCRIPTION_REQUEST;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

#[java_packet(DEBUG_SUBSCRIPTION_REQUEST)]
pub struct SDebugSubscriptionRequest {
    pub sample_type: VarInt,
}

impl SDebugSubscriptionRequest {
    pub const TICK_TIME: i32 = 0;
}

impl<'a> ServerPacket<'a> for SDebugSubscriptionRequest {
    fn read(bytebuf: &mut &'a [u8], _version: &JavaMinecraftVersion) -> Result<Self, ReadingError> {
        Ok(Self {
            sample_type: bytebuf.get_var_int()?,
        })
    }
}

impl crate::ClientPacket for SDebugSubscriptionRequest {
    fn write_packet_data(
        &self,
        mut write: impl std::io::Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), crate::ser::WritingError> {
        use crate::ser::NetworkWriteExt;
        write.write_var_int(&self.sample_type)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::MultiVersionJavaPacket;

    // 1.21.9 将 debug_sample_subscription 更名为 debug_subscription_request，
    // 两个结构体各覆盖一个时代，拼起来才无缝；数值钉死为权威表 id
    #[test]
    fn id_matches_authoritative_table_per_era() {
        use super::super::debug_sample_subscription::SDebugSampleSubscription;
        assert_eq!(
            SDebugSampleSubscription::to_id(JavaMinecraftVersion::V_1_21_7),
            22
        );
        assert_eq!(
            SDebugSampleSubscription::to_id(JavaMinecraftVersion::V_1_21_9),
            -1
        );
        assert_eq!(
            SDebugSubscriptionRequest::to_id(JavaMinecraftVersion::V_1_21_7),
            -1
        );
        assert_eq!(
            SDebugSubscriptionRequest::to_id(JavaMinecraftVersion::V_1_21_11),
            22
        );
        assert_eq!(
            SDebugSubscriptionRequest::to_id(JavaMinecraftVersion::V_26_1),
            23
        );
    }
}
