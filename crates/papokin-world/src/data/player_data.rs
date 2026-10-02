use dashmap::DashMap;
use papokin_nbt::compound::NbtCompound;
use std::fs::{File, create_dir_all};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
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
    /// 每玩家一把保存锁：周期保存（rayon 异步任务）与退出保存
    /// （tokio 连接任务同步调用）可能并发写同一玩家，此前二者
    /// 共用固定名临时文件会互相截断，产生损坏的 gzip 存档。
    save_locks: DashMap<Uuid, Arc<Mutex<()>>>,
    /// 临时文件名计数器，保证并发写各自使用独立临时文件。
    tmp_counter: AtomicU64,
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
            save_locks: DashMap::new(),
            tmp_counter: AtomicU64::new(0),
        }
    }

    /// 获取（或创建）指定玩家的保存锁。
    ///
    /// 必须先克隆出 `Arc` 再锁：`entry()` 返回的分片守卫若在等锁
    /// 期间一直持有，会连带阻塞其他玩家在该分片上的锁获取。
    fn save_lock_for(&self, uuid: &Uuid) -> Arc<Mutex<()>> {
        if let Some(lock) = self.save_locks.get(uuid) {
            return lock.value().clone();
        }
        self.save_locks
            .entry(*uuid)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .value()
            .clone()
    }

    /// 尝试回收该玩家的保存锁条目。
    ///
    /// `save_locks` 只插不删会在长期高换手率的服务器上缓慢无界
    /// 增长（每 UUID 数十字节）。仅当锁的强引用计数为 1（map 是
    /// 唯一持有者，无任何在途保存/加载）时移除：`remove_if` 的
    /// 谓词在分片独占锁内执行，与 `save_lock_for` 的取值互斥，
    /// 不存在"刚取走 Arc 又被移除、后继者另建新锁"的分裂窗口。
    /// 调用方必须已释放自身持有的锁与守卫。
    fn prune_save_lock(&self, uuid: &Uuid) {
        self.save_locks
            .remove_if(uuid, |_, lock| Arc::strong_count(lock) == 1);
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

        // 与退出/周期保存互斥：断线后快速重连的加载可能先于退出
        // 保存（gzip+fsync）完成，读到旧档后以其内存状态继续游戏并
        // 最终覆盖落盘 = 回档/复制。加载与保存共用同一把 per-UUID 锁。
        let result = {
            let load_lock = self.save_lock_for(uuid);
            let _load_guard = load_lock
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            // 如果不在缓存中，则从磁盘加载
            // 注意：所有分支都必须落到末尾的 `prune_save_lock`，
            // 提前 return 会把 per-UUID 锁条目永久留在 `save_locks`
            // 里（无档新玩家每进一人就漏一条）。
            let path = self.get_player_data_path(uuid);
            if path.exists() {
                match File::open(&path) {
                    Ok(file) => match papokin_nbt::nbt_compress::read_gzip_compound_tag(file) {
                        Ok(nbt) => {
                            debug!("已从磁盘加载玩家 {uuid} 的数据");
                            Ok((true, nbt))
                        }
                        Err(e) => {
                            error!("读取玩家 {uuid} 的数据失败：{e}");
                            Err(PlayerDataError::Nbt(e.to_string()))
                        }
                    },
                    Err(e) => {
                        error!("打开玩家 {uuid} 的数据文件失败：{e}");
                        Err(PlayerDataError::Io(e))
                    }
                }
            } else {
                debug!("未找到玩家 {uuid} 的数据文件");
                Ok((false, NbtCompound::new()))
            }
        };
        self.prune_save_lock(uuid);
        result
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
        if !self.is_save_enabled() {
            return Ok(());
        }
        let result = {
            let lock = self.save_lock_for(uuid);
            let _guard = lock
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            self.save_player_data_locked(uuid, data)
        };
        self.prune_save_lock(uuid);
        result
    }

    /// 带过期检查的保存：拿到 per-uuid 锁后在锁内执行 `is_stale`，
    /// 为真则放弃本次写入。
    ///
    /// 供周期保存使用：快照与落盘之间玩家可能已退出，退出保存
    /// （更新、更完整的数据）已写盘或正在写。此时旧快照再覆盖
    /// 会造成物品回档复制，故直接丢弃。
    pub fn save_player_data_unless(
        &self,
        uuid: &Uuid,
        data: NbtCompound,
        is_stale: &dyn Fn() -> bool,
    ) -> Result<(), PlayerDataError> {
        if !self.is_save_enabled() {
            return Ok(());
        }
        let result = {
            let lock = self.save_lock_for(uuid);
            let _guard = lock
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if is_stale() {
                debug!("跳过玩家 {uuid} 的过期周期保存快照");
                Ok(())
            } else {
                self.save_player_data_locked(uuid, data)
            }
        };
        self.prune_save_lock(uuid);
        result
    }

    /// 实际写盘逻辑，调用方必须已持有该玩家的保存锁。
    fn save_player_data_locked(
        &self,
        uuid: &Uuid,
        data: NbtCompound,
    ) -> Result<(), PlayerDataError> {
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
        // 像原版那样保留为 `.dat_old` 备份。临时名带
        // 单调后缀：即使两个线程同时保存同一玩家（被
        // 保存锁意外放行的场景）也不会共用一个临时文件
        // 互相截断。进程崩溃可能残留孤儿 `*.dat_new.*`，
        // 不影响读档，可手动清理。
        let tmp_path = path.with_extension(format!(
            "dat_new.{}",
            self.tmp_counter.fetch_add(1, Ordering::Relaxed)
        ));

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
