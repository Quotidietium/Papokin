use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use futures::future::join_all;
use papokin_util::math::vector2::Vector2;
use tokio::{
    join,
    sync::{OnceCell, RwLock, mpsc},
};
use tracing::{debug, error, trace};

use crate::{
    chunk::{ChunkReadingError, ChunkWritingError, io::Dirtiable},
    level::LevelFolder,
};

use super::{ChunkSerializer, FileIO, LoadedData, run_blocking};

/// `ChunkSerializer` trait 的一个简单实现，用于加载和保存数据
/// 使用并行和以文件路径为键的懒加载缓存写入磁盘。
///
/// ### Concurrency model
///
/// * `file_locks` — 每个磁盘文件各有一个 `Arc<RwLock<S>>`，延迟创建。
///   同一 Region 文件的所有读取器/写入器共享此锁，因此
///   同一文件绝不会有并发的写入者。
/// * `watchers` — 每个路径一个引用计数。当路径仍有活动的观察者时，
///   序列化器**不会**被逐出缓存，文件也**不会**被
///   刷新到磁盘（刷新生命周期由调用方负责）。
///
/// ### Lock ordering (must never be violated to avoid deadlocks)
///
/// 1. `file_locks`（外层）
/// 2. 每个加载器内部独立的 `RwLock<S>`（内层）
/// 3. `watchers`（独立——绝不与上面两者中的任何一个同时持有）
///
/// `watchers` 始终在其自己的临界区内获取，位于所有
/// 序列化器锁均已释放，因此它保持严格独立。
pub struct ChunkFileManager<S: ChunkSerializer<WriteBackend = PathBuf>> {
    file_locks: RwLock<BTreeMap<PathBuf, Arc<ChunkSerializerLazyLoader<S>>>>,
    watchers: RwLock<BTreeMap<PathBuf, usize>>,
    chunk_config: S::ChunkConfig,
}

pub(crate) trait PathFromLevelFolder {
    fn file_path(folder: &LevelFolder, file_name: &str) -> PathBuf;
}

struct ChunkSerializerLazyLoader<S: ChunkSerializer<WriteBackend = PathBuf>> {
    path: PathBuf,
    /// 至多初始化一次；后续调用复用同一个 Arc。
    internal: OnceCell<Arc<RwLock<S>>>,
}

impl<S: ChunkSerializer<WriteBackend = PathBuf> + 'static> ChunkSerializerLazyLoader<S> {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            internal: OnceCell::new(),
        }
    }

    ///只有当外部没有调用者仍持有此值的克隆时，才返回 `true`
    /// 加载器*或*内部序列化器。
    ///
    /// # Safety requirement
    /// **必须在持有父级 `file_locks` 映射的写锁时调用
    /// 持有。** 这保证了在我们运行期间不会发放新的 `Arc` 克隆
    /// 检查强引用计数。
    fn can_remove(loader: &Arc<Self>) -> bool {
        // 该映射本身持有 1 个强引用计数；任何超出都意味着
        // 活动调用方仍持有句柄。
        if Arc::strong_count(loader) > 1 {
            return false;
        }
        loader.internal.get().is_none_or(|arc| {
            Arc::strong_count(arc) == 1
                // 持有未写入数据的序列化器（例如在
                // 写入）绝不能被逐出：丢弃它会静默
                // 丢失该数据。锁忙视为待处理。
                && arc
                    .try_read()
                    .is_ok_and(|serializer| !serializer.has_pending_writes())
        })
    }

    /// 返回序列化器，首次调用时从磁盘初始化。
    async fn get(&self) -> Result<Arc<RwLock<S>>, ChunkReadingError> {
        self.internal
            .get_or_try_init(|| async {
                let serializer = self.read_from_disk().await?;
                Ok(Arc::new(RwLock::new(serializer)))
            })
            .await
            .cloned()
    }

    async fn read_from_disk(&self) -> Result<S, ChunkReadingError> {
        trace!("正在从磁盘打开文件: {}", self.path.display());

        match tokio::fs::read(&self.path).await {
            Ok(bytes) => {
                if bytes.is_empty() {
                    trace!("文件为空（0 字节），使用默认值: {}", self.path.display());
                    return Ok(S::default());
                }
                let path = self.path.clone();
                let value = run_blocking(move || S::read_at(bytes.into(), &path))
                    .await
                    .map_err(|_| {
                        ChunkReadingError::IoError(std::io::Error::other("区块反序列化任务失败"))
                    })??;
                trace!("已成功从磁盘读取文件: {}", self.path.display());
                Ok(value)
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                trace!("文件未找到，使用默认值: {}", self.path.display());
                Ok(S::default())
            }
            Err(err) => Err(ChunkReadingError::IoError(err)),
        }
    }
}

impl<S: ChunkSerializer<WriteBackend = PathBuf>> ChunkFileManager<S> {
    pub fn new(chunk_config: S::ChunkConfig) -> Self {
        Self {
            file_locks: RwLock::new(BTreeMap::new()),
            watchers: RwLock::new(BTreeMap::new()),
            chunk_config,
        }
    }
}

impl<S: ChunkSerializer<WriteBackend = PathBuf>> ChunkFileManager<S> {
    /// 返回 `path` 对应的序列化器，若不存在则插入一个惰性加载器。
    ///
    /// 使用乐观的“先读”模式：在常见情况（缓存命中）下
    /// 我们从不需要对该映射加写锁。
    async fn get_serializer(&self, path: &Path) -> Result<Arc<RwLock<S>>, ChunkReadingError> {
        {
            let locks = self.file_locks.read().await;
            if let Some(loader) = locks.get(path) {
                // 在释放锁*之前*克隆 Arc，使其保持存活。
                let loader = loader.clone();
                drop(locks);
                return loader.get().await;
            }
        }

        let loader = {
            let mut locks = self.file_locks.write().await;
            locks
                .entry(path.into())
                .or_insert_with(|| Arc::new(ChunkSerializerLazyLoader::new(path.into())))
                .clone()
            // 写锁在此处释放 —— `loader.get()` 可能因 I/O 而阻塞，且
            // 不得持有地图锁。
        };

        loader.get().await
    }

    /// 尝试逐出 `path` 对应的缓存序列化器。
    ///
    /// 只有当 *两个* 条件同时满足时才会移除该条目：
    /// 1. 没有监视器仍在引用该路径。
    /// 2. 没有其他存活的 `Arc` 克隆（由 `can_remove` 保证）。
    async fn maybe_evict(&self, path: &PathBuf) {
        // 独立于 file_locks 检查 watcher，以遵守锁顺序。
        let still_watched = {
            let watchers = self.watchers.read().await;
            watchers.get(path).is_some_and(|&c| c > 0)
        };

        if still_watched {
            return;
        }

        let mut locks = self.file_locks.write().await;
        let removable = locks
            .get(path)
            .is_some_and(ChunkSerializerLazyLoader::can_remove);

        if removable {
            locks.remove(path);
            trace!("已驱逐 {} 的序列化器缓存", path.display());
        } else {
            trace!("跳过 {} 的缓存驱逐——引用仍然存活", path.display());
        }
    }
}

impl<P, S> FileIO for ChunkFileManager<S>
where
    P: PathFromLevelFolder + Send + Sync + Sized + Dirtiable + 'static,
    S: ChunkSerializer<Data = P, WriteBackend = PathBuf>,
    S::ChunkConfig: Send + Sync,
{
    type Data = Arc<S::Data>;

    async fn watch_chunks<'a>(&'a self, folder: &'a LevelFolder, chunks: &'a [Vector2<i32>]) {
        let paths: Vec<_> = chunks
            .iter()
            .map(|c| P::file_path(folder, &S::get_chunk_key(c)))
            .collect();

        let mut watchers = self.watchers.write().await;
        for path in paths {
            *watchers.entry(path).or_insert(0) += 1;
        }
    }

    async fn unwatch_chunks<'a>(&'a self, folder: &'a LevelFolder, chunks: &'a [Vector2<i32>]) {
        let paths: Vec<_> = chunks
            .iter()
            .map(|c| P::file_path(folder, &S::get_chunk_key(c)))
            .collect();

        let mut paths_to_evict = Vec::new();
        {
            let mut watchers = self.watchers.write().await;
            for path in paths {
                if let std::collections::btree_map::Entry::Occupied(mut e) = watchers.entry(path) {
                    let count = e.get_mut();
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        let (path, _) = e.remove_entry();
                        paths_to_evict.push(path);
                    }
                }
            }
        }

        for path in paths_to_evict {
            self.maybe_evict(&path).await;
        }
    }

    async fn clear_watched_chunks(&self) {
        let paths: Vec<PathBuf> = {
            let mut watchers = self.watchers.write().await;
            let keys: Vec<_> = watchers.keys().cloned().collect();
            watchers.clear();
            keys
        };
        for path in paths {
            self.maybe_evict(&path).await;
        }
    }

    async fn fetch_chunks<'a>(
        &'a self,
        folder: &'a LevelFolder,
        chunk_coords: &'a [Vector2<i32>],
        stream: mpsc::Sender<LoadedData<Self::Data, ChunkReadingError>>,
    ) {
        // 按区域文件对请求的区块坐标分组。
        let mut regions_chunks: BTreeMap<String, Vec<Vector2<i32>>> = BTreeMap::new();
        for at in chunk_coords {
            regions_chunks
                .entry(S::get_chunk_key(at))
                .or_default()
                .push(*at);
        }

        let region_tasks = regions_chunks.into_iter().map(|(file_name, chunks)| {
            let task_stream = stream.clone();
            async move {
                let path = P::file_path(folder, &file_name);

                let chunk_serializer = match self.get_serializer(&path).await {
                    Ok(s) => s,
                    Err(ChunkReadingError::ChunkNotExist) => {
                        return;
                    }
                    Err(err) => {
                        // 尽力而为：报告批次中第一个坐标的错误。
                        let _ = task_stream.send(LoadedData::Error((chunks[0], err))).await;
                        return;
                    }
                };

                // 容量为 1 的有界通道在两者之间保持背压
                // 序列化器与调用方之间不会产生无限制的缓冲。
                let (send, mut recv) = mpsc::channel::<LoadedData<S::Data, ChunkReadingError>>(1);

                // 转发接收到的区块，将其包装在 `Arc` 中。
                // 捕获的 move 是有意为之——此处消费了 `task_stream`。
                let forward = async move {
                    while let Some(data) = recv.recv().await {
                        let wrapped = data.map_loaded(Arc::new);
                        if task_stream.send(wrapped).await.is_err() {
                            // 接收者已丢弃；提前中止以避免浪费工作。
                            return;
                        }
                    }
                };

                // 仅在 `get_chunks` 期间持有读锁。
                let read = async move {
                    let serializer = chunk_serializer.read().await;
                    serializer.get_chunks(chunks, send).await;
                };

                join!(forward, read);

                // 若未被监视且引用已释放，则逐出
                self.maybe_evict(&path).await;
            }
        });

        join_all(region_tasks).await;
    }

    async fn save_chunks<'a>(
        &'a self,
        folder: &'a LevelFolder,
        chunks_data: Vec<(Vector2<i32>, Self::Data)>,
    ) -> Result<(), ChunkWritingError> {
        // 按区域文件对区块分组。
        let mut regions_chunks: BTreeMap<String, Vec<Self::Data>> = BTreeMap::new();
        for (at, chunk) in chunks_data {
            regions_chunks
                .entry(S::get_chunk_key(&at))
                .or_default()
                .push(chunk);
        }

        let tasks = regions_chunks
            .into_iter()
            .map(|(file_name, chunk_locks)| async move {
                let path = P::file_path(folder, &file_name);
                trace!("正在将区块保存到 {}", path.display());

                let chunk_serializer = match self.get_serializer(&path).await {
                    Ok(s) => s,
                    Err(ChunkReadingError::ChunkNotExist) => {
                        return Err(ChunkWritingError::IoError(std::io::Error::other(
                            "get_serializer 返回了 ChunkNotExist",
                        )));
                    }
                    Err(ChunkReadingError::IoError(err)) => {
                        error!("写入前读取 Region 时发生 I/O 错误：{err}");
                        return Err(ChunkWritingError::IoError(err));
                    }
                    Err(err) => {
                        return Err(ChunkWritingError::IoError(std::io::Error::other(
                            err.to_string(),
                        )));
                    }
                };

                {
                    let mut writer = chunk_serializer.write().await;
                    for chunk in &chunk_locks {
                        // 先原子地快照并清除脏标记，再
                        // 写入，这样任何在本*次写入期间*竞态混入的修改
                        // 序列化轮次便能正确地再次将其标记为脏。
                        let was_dirty = chunk.is_dirty();
                        chunk.mark_dirty(false);

                        if was_dirty
                            && let Err(err) =
                                writer.update_chunk(chunk.clone(), &self.chunk_config).await
                        {
                            // 将区块交给序列化器失败：
                            // 重新将其标记为脏，让下一轮保存
                            // 重试而不是静默丢弃
                            // 变化。
                            chunk.mark_dirty(true);
                            return Err(err);
                        }
                    }
                    // 写锁在此处释放 —— 刷新可以在读锁下进行。
                }

                trace!("{} 的区块数据已更新", path.display());

                // 我们在释放写锁*之后*才检查观察者，以确保遵循
                // 锁顺序（序列化器锁 → 观察者，绝不反向）。
                let is_watched = {
                    let watchers = self.watchers.read().await;
                    watchers.get(&path).is_some_and(|&c| c > 0)
                };

                if !is_watched {
                    // `write()` 用读锁就够了，因为我们已经
                    // 已应用上述全部变更。
                    {
                        let serializer = chunk_serializer.read().await;
                        debug!("正在将 {} 写入磁盘", path.display());
                        serializer
                            .write(&path)
                            .await
                            .map_err(ChunkWritingError::IoError)?;
                        // 读锁在此释放。
                    };

                    // 丢弃我们的句柄，以便 `can_remove` 可以成功
                    drop(chunk_serializer);

                    // 不再需要时逐出该缓存条目
                    self.maybe_evict(&path).await;
                }

                Ok(())
            });

        // 收集所有 region 结果；上报遇到的第一个错误。
        let results: Vec<Result<(), ChunkWritingError>> = join_all(tasks).await;
        results.into_iter().find(Result::is_err).unwrap_or(Ok(()))
    }

    /// 阻塞直到所有进行中的序列化操作完成
    /// 在每个缓存条目上获取（并立即释放）写锁
    /// 序列化器。
    ///
    /// 这是一个线性化点：在此 future 完成后，不会再发生任何变更
    /// 之前启动的任务仍在运行。
    async fn block_and_await_ongoing_tasks(&self) {
        // 在读锁下对当前的加载器集合做快照，以便我们
        // 以免不必要地长时间阻塞新的插入。
        let loaders: Vec<Arc<ChunkSerializerLazyLoader<S>>> =
            { self.file_locks.read().await.values().cloned().collect() };

        // 对已初始化的每个加载器获取写锁
        // 并立即释放。这保证了任何并发
        // 正在进行的读或写操作已完成。
        let drain_tasks = loaders.into_iter().map(|loader| async move {
            if let Some(serializer_arc) = loader.internal.get() {
                // 获取写锁后立即释放，可充当
                // 屏障：只有在所有当前锁持有方
                // 已释放各自的锁守卫。
                let _guard = serializer_arc.write().await;
            }
        });

        join_all(drain_tasks).await;
    }
}

pub enum LevelFileIO<Linear, Anvil, Pump>
where
    Linear: ChunkSerializer<WriteBackend = PathBuf>,
    Anvil: ChunkSerializer<WriteBackend = PathBuf>,
    Pump: ChunkSerializer<WriteBackend = PathBuf>,
{
    Linear(ChunkFileManager<Linear>),
    Anvil(ChunkFileManager<Anvil>),
    Pump(ChunkFileManager<Pump>),
}

impl<P, Linear, Anvil, Pump> FileIO for LevelFileIO<Linear, Anvil, Pump>
where
    P: PathFromLevelFolder + Send + Sync + Sized + Dirtiable + 'static,
    Linear: ChunkSerializer<Data = P, WriteBackend = PathBuf>,
    Anvil: ChunkSerializer<Data = P, WriteBackend = PathBuf>,
    Pump: ChunkSerializer<Data = P, WriteBackend = PathBuf>,
    Linear::ChunkConfig: Send + Sync,
    Anvil::ChunkConfig: Send + Sync,
    Pump::ChunkConfig: Send + Sync,
{
    type Data = Arc<P>;

    async fn fetch_chunks<'a>(
        &'a self,
        folder: &'a LevelFolder,
        chunk_coords: &'a [Vector2<i32>],
        stream: tokio::sync::mpsc::Sender<LoadedData<Self::Data, ChunkReadingError>>,
    ) {
        match self {
            Self::Linear(io) => io.fetch_chunks(folder, chunk_coords, stream).await,
            Self::Anvil(io) => io.fetch_chunks(folder, chunk_coords, stream).await,
            Self::Pump(io) => io.fetch_chunks(folder, chunk_coords, stream).await,
        }
    }

    async fn save_chunks<'a>(
        &'a self,
        folder: &'a LevelFolder,
        chunks_data: Vec<(Vector2<i32>, Self::Data)>,
    ) -> Result<(), ChunkWritingError> {
        match self {
            Self::Linear(io) => io.save_chunks(folder, chunks_data).await,
            Self::Anvil(io) => io.save_chunks(folder, chunks_data).await,
            Self::Pump(io) => io.save_chunks(folder, chunks_data).await,
        }
    }

    async fn watch_chunks<'a>(&'a self, folder: &'a LevelFolder, chunks: &'a [Vector2<i32>]) {
        match self {
            Self::Linear(io) => io.watch_chunks(folder, chunks).await,
            Self::Anvil(io) => io.watch_chunks(folder, chunks).await,
            Self::Pump(io) => io.watch_chunks(folder, chunks).await,
        }
    }

    async fn unwatch_chunks<'a>(&'a self, folder: &'a LevelFolder, chunks: &'a [Vector2<i32>]) {
        match self {
            Self::Linear(io) => io.unwatch_chunks(folder, chunks).await,
            Self::Anvil(io) => io.unwatch_chunks(folder, chunks).await,
            Self::Pump(io) => io.unwatch_chunks(folder, chunks).await,
        }
    }

    async fn clear_watched_chunks(&self) {
        match self {
            Self::Linear(io) => io.clear_watched_chunks().await,
            Self::Anvil(io) => io.clear_watched_chunks().await,
            Self::Pump(io) => io.clear_watched_chunks().await,
        }
    }

    async fn block_and_await_ongoing_tasks(&self) {
        match self {
            Self::Linear(io) => io.block_and_await_ongoing_tasks().await,
            Self::Anvil(io) => io.block_and_await_ongoing_tasks().await,
            Self::Pump(io) => io.block_and_await_ongoing_tasks().await,
        }
    }
}
