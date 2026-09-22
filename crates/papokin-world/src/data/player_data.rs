use papokin_nbt::compound::NbtCompound;
use std::fs::{File, create_dir_all};
use std::io::{self, Write};
use std::path::PathBuf;
use tracing::{debug, error};
use uuid::Uuid;

/// 管理玩家数据在磁盘与内存缓存中的存储与读取。
///
/// 此结构体提供向 NBT 文件保存以及从中加载玩家数据的函数，
/// 配备内存缓存以临时应对玩家断线。
pub struct PlayerDataStorage {
    /// 玩家数据存储目录的路径
    data_path: PathBuf,
    /// 是否启用玩家数据保存
    save_enabled: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum PlayerDataError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("NBT error: {0}")]
    Nbt(String),
}

impl PlayerDataStorage {
    /// 使用指定的数据路径和缓存过期时间创建新的 `PlayerDataStorage`。
    pub fn new(data_path: impl Into<PathBuf>, enabled: bool) -> Self {
        let path = data_path.into();
        if !path.exists()
            && let Err(e) = create_dir_all(&path)
        {
            error!("创建玩家数据目录 {} 失败：{e}", path.display());
        }

        Self {
            data_path: path,
            save_enabled: enabled,
        }
    }

    #[must_use]
    pub const fn get_data_path(&self) -> &PathBuf {
        &self.data_path
    }

    #[must_use]
    pub const fn is_save_enabled(&self) -> bool {
        self.save_enabled
    }

    pub const fn set_save_enabled(&mut self, enabled: bool) {
        self.save_enabled = enabled;
    }

    /// 根据玩家的 UUID 返回其数据文件的路径。
    #[must_use]
    pub fn get_player_data_path(&self, uuid: &Uuid) -> PathBuf {
        self.get_data_path().join(format!("{uuid}.dat"))
    }

    /// 从 NBT 文件或缓存加载玩家数据。
    ///
    /// 此函数首先检查缓存中是否存在玩家数据。
    /// 否则，它会尝试从磁盘上的 .dat 文件加载数据。
    ///
    /// # Arguments
    ///
    /// * `uuid` - 要为其加载数据的玩家的 UUID。
    ///
    /// # Returns
    ///
    /// 一个 Result，包含玩家的 NBT 数据或错误。
    pub fn load_player_data(&self, uuid: &Uuid) -> Result<(bool, NbtCompound), PlayerDataError> {
        // 如果玩家数据保存被禁用，则返回空数据
        if !self.is_save_enabled() {
            return Ok((false, NbtCompound::new()));
        }

        // 如果不在缓存中，则从磁盘加载
        let path = self.get_player_data_path(uuid);
        if !path.exists() {
            debug!("未找到玩家 {uuid} 的数据文件");
            return Ok((false, NbtCompound::new()));
        }

        let file = match File::open(&path) {
            Ok(file) => file,
            Err(e) => {
                error!("打开玩家 {uuid} 的数据文件失败：{e}");
                return Err(PlayerDataError::Io(e));
            }
        };

        match papokin_nbt::nbt_compress::read_gzip_compound_tag(file) {
            Ok(nbt) => {
                debug!("已从磁盘加载玩家 {uuid} 的数据");
                Ok((true, nbt))
            }
            Err(e) => {
                error!("读取玩家 {uuid} 的数据失败：{e}");
                Err(PlayerDataError::Nbt(e.to_string()))
            }
        }
    }

    /// 将玩家数据保存到 NBT 文件并更新缓存。
    ///
    /// 此函数将玩家数据保存到磁盘上的 .dat 文件中，并且还会
    /// 用最新数据更新内存缓存。
    ///
    /// # Arguments
    ///
    /// * `uuid` - 要为其保存数据的玩家的 UUID。
    /// * `data` - 要保存的 NBT 复合数据。
    ///
    /// # Returns
    ///
    /// 一个 Result，表示成功或发生的错误。
    pub fn save_player_data(&self, uuid: &Uuid, data: NbtCompound) -> Result<(), PlayerDataError> {
        // 若在配置中禁用了保存则跳过
        if !self.is_save_enabled() {
            return Ok(());
        }

        let path = self.get_player_data_path(uuid);

        // 确保父目录存在
        if let Some(parent) = path.parent()
            && let Err(e) = create_dir_all(parent)
        {
            error!("为玩家 {uuid} 创建数据目录失败：{e}");
            return Err(PlayerDataError::Io(e));
        }

        // 先写入临时文件再原子换入，这样崩溃时
        // 写入中途也绝不会破坏之前的保存。旧文件会
        // 像原版那样保留为 `.dat_old` 备份。
        let tmp_path = path.with_extension("dat_new");

        let write_result: Result<(), PlayerDataError> = (|| {
            let file = File::create(&tmp_path)?;
            let mut writer = io::BufWriter::new(file);
            papokin_nbt::nbt_compress::write_gzip_compound_tag(data, &mut writer)
                .map_err(|e| PlayerDataError::Nbt(e.to_string()))?;
            writer.flush()?;
            writer.get_ref().sync_all()?;
            Ok(())
        })();
        if let Err(e) = write_result {
            error!("写入玩家 {uuid} 的压缩数据失败：{e}");
            let _ = std::fs::remove_file(&tmp_path);
            return Err(e);
        }

        // 保留上一份存档作为备份，类似原版的 `.dat_old`。
        if path.exists() {
            let old_path = path.with_extension("dat_old");
            let _ = std::fs::remove_file(&old_path);
            let _ = std::fs::rename(&path, &old_path);
        }

        if let Err(e) = std::fs::rename(&tmp_path, &path) {
            // 例如目标在 Windows 上被锁定：回退到
            // 原地复制，而不是丢弃新数据。
            error!("将玩家 {uuid} 的新数据文件移入目标位置失败：{e}");
            std::fs::copy(&tmp_path, &path)?;
            let _ = std::fs::remove_file(&tmp_path);
        }

        debug!("已将玩家 {uuid} 的数据保存到磁盘");
        Ok(())
    }
}
