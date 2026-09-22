use std::io::Write;

use papokin_data::packet::clientbound::play::RECIPE_BOOK_SETTINGS;
use papokin_macros::java_packet;
use papokin_util::version::JavaMinecraftVersion;

use crate::{ClientPacket, WritingError, ser::NetworkWriteExt};

/// 由服务器发送，用于更新玩家配方书的打开/筛选状态。
///
/// 传输格式：4 对 `TypeSettings`（合成、熔炉、`blast_furnace`、烟熏炉），
/// 每一对是（`is_open`: bool、`is_filtering`: bool）。
#[java_packet(RECIPE_BOOK_SETTINGS)]
pub struct CRecipeBookSettings {
    pub crafting_open: bool,
    pub crafting_filtering: bool,
    pub furnace_open: bool,
    pub furnace_filtering: bool,
    pub blast_furnace_open: bool,
    pub blast_furnace_filtering: bool,
    pub smoker_open: bool,
    pub smoker_filtering: bool,
}

impl CRecipeBookSettings {
    #[must_use]
    pub const fn default_closed() -> Self {
        Self {
            crafting_open: false,
            crafting_filtering: false,
            furnace_open: false,
            furnace_filtering: false,
            blast_furnace_open: false,
            blast_furnace_filtering: false,
            smoker_open: false,
            smoker_filtering: false,
        }
    }
}

impl ClientPacket for CRecipeBookSettings {
    fn write_packet_data(
        &self,
        write: impl Write,
        _version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        let mut write = write;
        write.write_bool(self.crafting_open)?;
        write.write_bool(self.crafting_filtering)?;
        write.write_bool(self.furnace_open)?;
        write.write_bool(self.furnace_filtering)?;
        write.write_bool(self.blast_furnace_open)?;
        write.write_bool(self.blast_furnace_filtering)?;
        write.write_bool(self.smoker_open)?;
        write.write_bool(self.smoker_filtering)?;
        Ok(())
    }
}
