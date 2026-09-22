use std::io::Write;

use crate::ser::NetworkWriteExt;
use crate::{ClientPacket, VarInt, WritingError};
use papokin_data::packet::clientbound::play::WAYPOINT;
use papokin_macros::java_packet;
use papokin_util::math::position::BlockPos;
use papokin_util::version::JavaMinecraftVersion;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum WaypointOperation {
    Track = 0,
    Untrack = 1,
    Update = 2,
}

#[derive(Clone, Debug)]
pub enum WaypointTarget {
    Position(BlockPos),
    Chunk { x: i32, z: i32 },
    Azimuth(f32),
    Empty,
}

#[derive(Clone, Debug)]
pub struct WaypointIcon<'a> {
    pub style: Option<&'a str>,
    pub color: i32,
}

#[derive(Clone, Debug)]
pub struct TrackedWaypoint<'a> {
    pub identifier: Uuid,
    pub icon: Option<WaypointIcon<'a>>,
    pub target: WaypointTarget,
}

impl TrackedWaypoint<'_> {
    #[must_use]
    pub const fn empty(identifier: Uuid) -> Self {
        Self {
            identifier,
            icon: None,
            target: WaypointTarget::Empty,
        }
    }

    #[must_use]
    pub const fn set_position(
        identifier: Uuid,
        icon: Option<WaypointIcon<'_>>,
        position: BlockPos,
    ) -> TrackedWaypoint<'_> {
        TrackedWaypoint {
            identifier,
            icon,
            target: WaypointTarget::Position(position),
        }
    }
}

/// 将追踪的路点（`ClientboundTrackedWaypointPacket`）同步到客户端。
#[java_packet(WAYPOINT)]
pub struct CWaypoint<'a> {
    pub operation: WaypointOperation,
    pub waypoint: TrackedWaypoint<'a>,
}

impl<'a> CWaypoint<'a> {
    #[must_use]
    pub const fn new(operation: WaypointOperation, waypoint: TrackedWaypoint<'a>) -> Self {
        Self {
            operation,
            waypoint,
        }
    }

    #[must_use]
    pub const fn remove(identifier: Uuid) -> Self {
        Self {
            operation: WaypointOperation::Untrack,
            waypoint: TrackedWaypoint::empty(identifier),
        }
    }

    #[must_use]
    pub const fn add_position(
        identifier: Uuid,
        icon: Option<WaypointIcon<'a>>,
        position: BlockPos,
    ) -> Self {
        Self {
            operation: WaypointOperation::Track,
            waypoint: TrackedWaypoint::set_position(identifier, icon, position),
        }
    }

    #[must_use]
    pub const fn update_position(
        identifier: Uuid,
        icon: Option<WaypointIcon<'a>>,
        position: BlockPos,
    ) -> Self {
        Self {
            operation: WaypointOperation::Update,
            waypoint: TrackedWaypoint::set_position(identifier, icon, position),
        }
    }
}

impl ClientPacket for CWaypoint<'_> {
    fn write_packet_data(
        &self,
        mut write: impl Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        // 1. 操作（TRACK = 0，UNTRACK = 1，UPDATE = 2）
        write.write_var_int(&VarInt(self.operation as i32))?;

        // 2. TrackedWaypoint 标识符（UUID）
        write.write_uuid(&self.waypoint.identifier)?;

        // 3. 路径点图标（可选）
        if let Some(ref icon) = self.waypoint.icon {
            write.write_bool(true)?;
            if let Some(style_str) = icon.style {
                write.write_bool(true)?;
                // 将样式格式化为有效的 ResourceLocation（例如 "minecraft:red" 或 "red"）
                if style_str.contains(':') {
                    write.write_string(style_str)?;
                } else {
                    let formatted = format!("minecraft:{style_str}");
                    write.write_string(&formatted)?;
                }
            } else {
                write.write_bool(false)?;
            }
            write.write_i32_be(icon.color)?;
        } else {
            write.write_bool(false)?;
        }

        // 4. 路径点目标载荷
        match &self.waypoint.target {
            WaypointTarget::Position(pos) => {
                write.write_var_int(&VarInt(0))?;
                write.write_block_pos(pos, version)?;
            }
            WaypointTarget::Chunk { x, z } => {
                write.write_var_int(&VarInt(1))?;
                write.write_i32_be(*x)?;
                write.write_i32_be(*z)?;
            }
            WaypointTarget::Azimuth(angle) => {
                write.write_var_int(&VarInt(2))?;
                write.write_f32_be(*angle)?;
            }
            WaypointTarget::Empty => {
                write.write_var_int(&VarInt(3))?;
            }
        }

        Ok(())
    }
}
