use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use futures::future::join_all;
use papokin_util::math::vector2::Vector2;
use tokio::{
    join,
    sync::{OnceCell, RwLock, mpsc},
};
use tracing::{debug, error, trace, warn};

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
    /// 缓存字节预算（0 = 不设上限，恢复旧行为）。
    max_cache_bytes: usize,
    /// LRU 序号：每次取用加载器单调递增。
    usage_seq: AtomicU64,
    /// 字节预算执法累计驱逐的条目数（观测用）。
    evicted_total: AtomicU64,
}

/// `file_locks` 缓存条目数上限。
///
/// 条目通常在保存/读取完成后被 `maybe_evict` 驱逐（无注视者、无
/// 存活引用、无未落盘数据即移除），正常游玩远低于该值。上限兜住
/// 两类缓慢累积：驱逐瞬间恰有并发引用而滞留的条目，以及写盘
/// 失败后 `has_pending_writes` 挡住驱逐的条目。超限时在写锁内
/// 驱逐一切当前可安全移除的条目；找不到可驱逐条目时维持现状，
/// 下次插入再试。
const MAX_CACHED_SERIALIZERS: usize = 1024;

/// 把区块/实体数据映射到其在关卡目录下的磁盘文件。
///
/// 公开供基准与外部工具按真实路径布局驱动
/// `ChunkFileManager`（`benchmark/` 的内存基准即经此接口
/// 复现 region 目录组织）。
pub trait PathFromLevelFolder {
    fn file_path(folder: &LevelFolder, file_name: &str) -> PathBuf;
}

struct ChunkSerializerLazyLoader<S: ChunkSerializer<WriteBackend = PathBuf>> {
    path: PathBuf,
    /// 至多初始化一次；后续调用复用同一个 Arc。
    internal: OnceCell<Arc<RwLock<S>>>,
    /// 最近一次取用时的 LRU 序号（0 = 尚未取用）。
    last_used: AtomicU64,
}

impl<S: ChunkSerializer<WriteBackend = PathBuf> + 'static> ChunkSerializerLazyLoader<S> {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            internal: OnceCell::new(),
            last_used: AtomicU64::new(0),
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
                // 损坏保护：文件明显小于任何合法区域头（8 KiB）时，
                // 反序列化会"按空处理"，随后首次保存就会用空内存态
                // 整写覆盖原文件。先把可疑文件改名留档（*.corrupt），
                // 数据可手工恢复，而不是被静默清空。
                if bytes.len() < 8192 {
                    let backup = self.path.with_extension("corrupt");
                    warn!(
                        "文件 {} 仅 {} 字节（小于 8 KiB 区域头），疑似损坏；已改名为 {} 留档并按空文件处理",
                        self.path.display(),
                        bytes.len(),
                        backup.display()
                    );
                    let _ = tokio::fs::rename(&self.path, &backup).await;
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
    /// `max_cache_bytes` 为缓存字节预算（0 = 不设上限）。
    pub fn new(chunk_config: S::ChunkConfig, max_cache_bytes: usize) -> Self {
        Self {
            file_locks: RwLock::new(BTreeMap::new()),
            watchers: RwLock::new(BTreeMap::new()),
            chunk_config,
            max_cache_bytes,
            usage_seq: AtomicU64::new(0),
            evicted_total: AtomicU64::new(0),
        }
    }

    /// 记录一次取用，返回该加载器当前的 LRU 序号。
    fn touch(&self, loader: &ChunkSerializerLazyLoader<S>) {
        let seq = self.usage_seq.fetch_add(1, Ordering::Relaxed) + 1;
        loader.last_used.store(seq, Ordering::Relaxed);
    }

    /// 字节预算执法累计驱逐的缓存条目数。
    #[must_use]
    pub fn evicted_total(&self) -> u64 {
        self.evicted_total.load(Ordering::Relaxed)
    }

    /// 当前缓存条目的内存总账（字节；锁忙条目按 0 计入）。
    pub async fn cached_bytes_total(&self) -> usize {
        let locks = self.file_locks.read().await;
        locks
            .values()
            .map(|loader| {
                loader
                    .internal
                    .get()
                    .and_then(|arc| arc.try_read().ok())
                    .map_or(0, |serializer| serializer.cached_bytes())
            })
            .sum()
    }
}

impl<S: ChunkSerializer<WriteBackend = PathBuf>> ChunkFileManager<S> {
    /// 在 `file_locks` 写锁内执行缓存字节预算驱逐。
    ///
    /// 只驱逐满足 `can_remove`（无存活引用、无未落盘更新——磁盘
    /// 已是最新状态）的条目，因此驱逐既不丢数据也不需要额外
    /// 写盘；被驱逐条目下次访问时从磁盘重读，状态完全一致。
    /// watched 条目同样可被驱逐（其语义仅是"保存先合并进内存"，
    /// 而干净条目内存与磁盘等价），但带有未落盘更改的 watched
    /// 条目被 `has_pending_writes` 挡下，留待自动保存清理。
    ///
    /// 为降低抖动采用滞回：超限时驱逐到预算的 80%。锁忙（正在
    /// 读写）的条目本轮按 0 字节计入并跳过，留给下一轮执法。
    fn enforce_byte_budget(
        &self,
        locks: &mut BTreeMap<PathBuf, Arc<ChunkSerializerLazyLoader<S>>>,
    ) {
        if self.max_cache_bytes == 0 {
            return;
        }

        let mut total_bytes = 0usize;
        let mut entries: Vec<(PathBuf, usize, u64)> = Vec::with_capacity(locks.len());
        for (path, loader) in locks.iter() {
            let bytes = loader
                .internal
                .get()
                .and_then(|arc| arc.try_read().ok())
                .map_or(0, |serializer| serializer.cached_bytes());
            total_bytes += bytes;
            entries.push((
                path.clone(),
                bytes,
                loader.last_used.load(Ordering::Relaxed),
            ));
        }

        if total_bytes <= self.max_cache_bytes {
            return;
        }

        let target = self.max_cache_bytes / 5 * 4;
        // 最近最少使用优先驱逐
        entries.sort_by_key(|(_, _, last_used)| *last_used);
        for (path, bytes, _) in entries {
            if total_bytes <= target {
                break;
            }
            if bytes == 0 {
                continue;
            }
            let removable = locks
                .get(&path)
                .is_some_and(ChunkSerializerLazyLoader::can_remove);
            if removable {
                locks.remove(&path);
                total_bytes -= bytes;
                self.evicted_total.fetch_add(1, Ordering::Relaxed);
                trace!(
                    "缓存字节超限，已驱逐 {} 的序列化器（{bytes} 字节，磁盘已是最新）",
                    path.display()
                );
            }
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
                self.touch(&loader);
                return loader.get().await;
            }
        }

        let loader = {
            let mut locks = self.file_locks.write().await;
            let path_key: PathBuf = path.into();
            let loader = locks
                .entry(path_key)
                .or_insert_with(|| Arc::new(ChunkSerializerLazyLoader::new(path.into())))
                .clone();
            self.touch(&loader);
            if locks.len() > MAX_CACHED_SERIALIZERS {
                // 超出缓存上限：在写锁内驱逐一切当前可安全移除的
                // 条目（判定与 maybe_evict 一致；刚插入的条目持有
                // 本地 Arc 克隆，can_remove 必然不放行，不会被误删）。
                let evictable: Vec<PathBuf> = locks
                    .iter()
                    .filter(|(_, candidate)| ChunkSerializerLazyLoader::can_remove(candidate))
                    .map(|(p, _)| p.clone())
                    .take(locks.len() - MAX_CACHED_SERIALIZERS)
                    .collect();
                for p in evictable {
                    locks.remove(&p);
                    trace!("缓存超限，已驱逐 {} 的序列化器", p.display());
                }
            }
            self.enforce_byte_budget(&mut locks);
            loader
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

    /// 将区块数据移入序列化器缓存，并按注视状态决定是否写盘。
    ///
    /// `force = true` 时无视注视状态强制写盘（仍不驱逐有注视者的
    /// 缓存条目），供 `/save-all` 与自动保存的实体区块刷新使用。
    async fn save_chunks_with<'a, P>(
        &'a self,
        folder: &'a LevelFolder,
        chunks_data: Vec<(Vector2<i32>, Arc<S::Data>)>,
        force: bool,
    ) -> Result<(), ChunkWritingError>
    where
        P: PathFromLevelFolder + Send + Sync + Sized + Dirtiable + 'static,
        S: Send + Sync,
        S::ChunkConfig: Send + Sync,
    {
        // 按区域文件对区块分组。
        let mut regions_chunks: BTreeMap<String, Vec<Arc<S::Data>>> = BTreeMap::new();
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

                if force || !is_watched {
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

                    if !is_watched {
                        // 不再需要时逐出该缓存条目
                        self.maybe_evict(&path).await;
                    }
                }

                Ok(())
            });

        // 收集所有 region 结果；上报遇到的第一个错误。
        let results: Vec<Result<(), ChunkWritingError>> = join_all(tasks).await;
        let first_err = results.into_iter().find(Result::is_err);

        // 保存会让序列化器吞入新区块字节（watched 区域先合并进
        // 内存），是缓存增长的另一条路径；在全部区域任务收尾后
        // 统一做一次预算执法，避免并发任务间的锁车队。
        let mut locks = self.file_locks.write().await;
        self.enforce_byte_budget(&mut locks);
        drop(locks);

        first_err.unwrap_or(Ok(()))
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
                        // 必须为批次内的每个坐标回执：消费方按坐标数
                        // 收取消息，少发会让调度器的 running_task_count
                        // 永久泄漏，在途配额耗尽后区块系统整体停摆。
                        // 错误详情随首坐标回报，其余按 Missing 回执
                        // （下游两者同样走重新生成）。
                        let _ = task_stream.send(LoadedData::Error((chunks[0], err))).await;
                        for pos in chunks.iter().skip(1).copied() {
                            if task_stream.send(LoadedData::Missing(pos)).await.is_err() {
                                break;
                            }
                        }
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
        self.save_chunks_with::<P>(folder, chunks_data, false).await
    }

    async fn save_chunks_forced<'a>(
        &'a self,
        folder: &'a LevelFolder,
        chunks_data: Vec<(Vector2<i32>, Self::Data)>,
    ) -> Result<(), ChunkWritingError> {
        self.save_chunks_with::<P>(folder, chunks_data, true).await
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

    /// 关停兜底：将所有仍持有未落盘更新（`has_pending_writes`）
    /// 的序列化器强制写盘。见 `FileIO::flush_pending_writes`。
    async fn flush_pending_writes(&self) {
        let loaders: Vec<Arc<ChunkSerializerLazyLoader<S>>> =
            { self.file_locks.read().await.values().cloned().collect() };

        for loader in loaders {
            let Some(serializer_arc) = loader.internal.get() else {
                continue;
            };
            let serializer = serializer_arc.read().await;
            if !serializer.has_pending_writes() {
                continue;
            }
            if let Err(err) = serializer.write(&loader.path).await {
                error!(
                    "关停前刷写 {} 失败（该文件未落盘的更新丢失）：{err}",
                    loader.path.display()
                );
            }
        }
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

    async fn save_chunks_forced<'a>(
        &'a self,
        folder: &'a LevelFolder,
        chunks_data: Vec<(Vector2<i32>, Self::Data)>,
    ) -> Result<(), ChunkWritingError> {
        match self {
            Self::Linear(io) => io.save_chunks_forced(folder, chunks_data).await,
            Self::Anvil(io) => io.save_chunks_forced(folder, chunks_data).await,
            Self::Pump(io) => io.save_chunks_forced(folder, chunks_data).await,
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

    async fn flush_pending_writes(&self) {
        match self {
            Self::Linear(io) => io.flush_pending_writes().await,
            Self::Anvil(io) => io.flush_pending_writes().await,
            Self::Pump(io) => io.flush_pending_writes().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::ChunkSerializingError;
    use crate::chunk::format::anvil::{AnvilChunkFile, SingleChunkDataSerializer};
    use bytes::Bytes;
    use papokin_config::chunk::AnvilChunkConfig;
    use std::sync::atomic::AtomicBool;

    /// 纯逻辑测试用的占位区块数据。
    struct MockData;

    impl Dirtiable for MockData {
        fn is_dirty(&self) -> bool {
            false
        }
        fn mark_dirty(&self, _flag: bool) {}
    }

    /// 字节量与待写状态可由测试直接操控的模拟序列化器。
    #[derive(Default)]
    struct MockSerializer {
        bytes: usize,
        pending: bool,
    }

    impl ChunkSerializer for MockSerializer {
        type Data = MockData;
        type WriteBackend = PathBuf;
        type ChunkConfig = ();

        fn get_chunk_key(chunk: &Vector2<i32>) -> String {
            format!("./r.{}.{}.mock", chunk.x >> 5, chunk.y >> 5)
        }

        async fn write(&self, _backend: &PathBuf) -> Result<(), std::io::Error> {
            Ok(())
        }

        fn read(_r: Bytes) -> Result<Self, ChunkReadingError> {
            Ok(Self::default())
        }

        fn has_pending_writes(&self) -> bool {
            self.pending
        }

        fn cached_bytes(&self) -> usize {
            self.bytes
        }

        async fn update_chunk(
            &mut self,
            _chunk_data: Arc<Self::Data>,
            _chunk_config: &Self::ChunkConfig,
        ) -> Result<(), ChunkWritingError> {
            Ok(())
        }

        async fn get_chunks(
            &self,
            _chunks: Vec<Vector2<i32>>,
            _stream: mpsc::Sender<LoadedData<Self::Data, ChunkReadingError>>,
        ) {
        }
    }

    async fn insert_with_bytes(
        manager: &ChunkFileManager<MockSerializer>,
        dir: &Path,
        name: &str,
        bytes: usize,
        pending: bool,
    ) {
        let path = dir.join(name);
        let serializer = manager.get_serializer(&path).await.expect("取序列化器");
        let mut guard = serializer.write().await;
        guard.bytes = bytes;
        guard.pending = pending;
        drop(guard);
        // 必须丢弃句柄，否则存活引用会挡住驱逐
        drop(serializer);
    }

    async fn cached_names(manager: &ChunkFileManager<MockSerializer>) -> Vec<String> {
        manager
            .file_locks
            .read()
            .await
            .keys()
            .map(|p| {
                p.file_name()
                    .expect("文件名")
                    .to_string_lossy()
                    .into_owned()
            })
            .collect()
    }

    #[tokio::test]
    async fn budget_evicts_least_recently_used_clean_entries() {
        let dir = tempfile::tempdir().expect("临时目录");
        let manager = ChunkFileManager::<MockSerializer>::new((), 100);

        // 逐个插入；每次插入都会在写锁内做预算执法，
        // 预期最旧的干净条目依次被驱逐、最新三条留存。
        for (name, bytes) in [("f0", 40), ("f1", 40), ("f2", 40), ("f3", 40), ("f4", 40)] {
            insert_with_bytes(&manager, dir.path(), name, bytes, false).await;
        }

        let mut names = cached_names(&manager).await;
        names.sort_unstable();
        assert_eq!(names, vec!["f2", "f3", "f4"]);
    }

    #[tokio::test]
    async fn budget_never_evicts_pending_writes() {
        let dir = tempfile::tempdir().expect("临时目录");
        let manager = ChunkFileManager::<MockSerializer>::new((), 100);

        // f0 体积超预算但持有未落盘更新：绝不可驱逐
        insert_with_bytes(&manager, dir.path(), "f0", 200, true).await;
        insert_with_bytes(&manager, dir.path(), "f1", 50, false).await;
        insert_with_bytes(&manager, dir.path(), "f2", 0, false).await;

        let mut names = cached_names(&manager).await;
        names.sort_unstable();
        assert_eq!(names, vec!["f0", "f2"]);
    }

    #[tokio::test]
    async fn zero_budget_disables_enforcement() {
        let dir = tempfile::tempdir().expect("临时目录");
        let manager = ChunkFileManager::<MockSerializer>::new((), 0);

        for i in 0..4 {
            insert_with_bytes(&manager, dir.path(), &format!("f{i}"), 1_000_000, false).await;
        }

        assert_eq!(cached_names(&manager).await.len(), 4);
    }

    /// 真实 Anvil 格式下的端到端往返：干净区域被驱逐后，
    /// 数据仍可从磁盘原样读回；驱逐后再写入能正确重读合并。
    #[derive(Debug)]
    struct PayloadChunk {
        x: i32,
        z: i32,
        payload: Vec<u8>,
        dirty: AtomicBool,
    }

    impl PayloadChunk {
        fn new(x: i32, z: i32, payload: Vec<u8>) -> Arc<Self> {
            Arc::new(Self {
                x,
                z,
                payload,
                dirty: AtomicBool::new(true),
            })
        }
    }

    impl Dirtiable for PayloadChunk {
        fn is_dirty(&self) -> bool {
            self.dirty.load(Ordering::Relaxed)
        }
        fn mark_dirty(&self, flag: bool) {
            self.dirty.store(flag, Ordering::Relaxed);
        }
    }

    impl SingleChunkDataSerializer for PayloadChunk {
        fn to_bytes(&self) -> Result<Bytes, ChunkSerializingError> {
            Ok(Bytes::copy_from_slice(&self.payload))
        }

        fn from_bytes(bytes: &Bytes, pos: Vector2<i32>) -> Result<Self, ChunkReadingError> {
            Ok(Self {
                x: pos.x,
                z: pos.y,
                payload: bytes.to_vec(),
                dirty: AtomicBool::new(false),
            })
        }

        fn position(&self) -> (i32, i32) {
            (self.x, self.z)
        }
    }

    impl PathFromLevelFolder for PayloadChunk {
        fn file_path(folder: &LevelFolder, file_name: &str) -> PathBuf {
            folder.region_folder.join(file_name)
        }
    }

    /// 不可压缩的伪随机负载（压缩后体积≈原文，便于控制预算账）。
    fn pseudo_random_bytes(len: usize, seed: u64) -> Vec<u8> {
        let mut state = seed | 1;
        (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                (state >> 32) as u8
            })
            .collect()
    }

    fn test_folder(dir: &Path) -> Arc<LevelFolder> {
        let region_folder = dir.join("region");
        std::fs::create_dir_all(&region_folder).expect("建 region 目录");
        Arc::new(LevelFolder {
            root_folder: dir.to_path_buf(),
            dim_folder: dir.to_path_buf(),
            region_folder,
            entities_folder: dir.join("entities"),
            poi_folder: dir.join("poi"),
        })
    }

    type PayloadManager = ChunkFileManager<AnvilChunkFile<PayloadChunk>>;

    async fn save_region(
        manager: &PayloadManager,
        folder: &Arc<LevelFolder>,
        region_x: i32,
        seeds: &[u64],
    ) {
        let chunks: Vec<(Vector2<i32>, Arc<PayloadChunk>)> = seeds
            .iter()
            .enumerate()
            .map(|(i, seed)| {
                let at = Vector2::new(region_x * 32 + i as i32, 0);
                (
                    at,
                    PayloadChunk::new(at.x, at.y, pseudo_random_bytes(600, *seed)),
                )
            })
            .collect();
        manager
            .save_chunks_forced(folder, chunks)
            .await
            .expect("强制保存");
    }

    async fn fetch_payloads(
        manager: &PayloadManager,
        folder: &Arc<LevelFolder>,
        coords: &[Vector2<i32>],
    ) -> Vec<(Vector2<i32>, Vec<u8>)> {
        let (send, mut recv) = mpsc::channel(64);
        manager.fetch_chunks(folder, coords, send).await;
        let mut out = Vec::new();
        while let Some(data) = recv.recv().await {
            match data {
                LoadedData::Loaded(chunk) => {
                    out.push((Vector2::new(chunk.x, chunk.z), chunk.payload.clone()));
                }
                other => panic!("期望 Loaded，得到 {other:?}"),
            }
        }
        out.sort_by_key(|(pos, _)| (pos.x, pos.y));
        out
    }

    #[tokio::test]
    async fn evicted_clean_region_roundtrips_from_disk() {
        let dir = tempfile::tempdir().expect("临时目录");
        let folder = test_folder(dir.path());
        // 预算 2 KiB：每区域约 1.3 KiB（两块 × 600 字节级压缩负载）
        let manager = PayloadManager::new(AnvilChunkConfig::default(), 2048);

        // 模拟运行中服务器：三个区域的区块都处于加载（watched）
        // 状态，强制保存落盘后序列化器转为干净但仍驻留内存——
        // 这正是预算执法要收口的内存。
        for region_x in 0..3 {
            let coords: Vec<Vector2<i32>> =
                (0..2).map(|i| Vector2::new(region_x * 32 + i, 0)).collect();
            manager.watch_chunks(&folder, &coords).await;
        }

        save_region(&manager, &folder, 0, &[11, 12]).await;
        save_region(&manager, &folder, 1, &[21, 22]).await;
        // 第三次保存后总账超预算：执法按 LRU 驱逐最旧的干净区域
        save_region(&manager, &folder, 2, &[31, 32]).await;

        let (names, total) = {
            let locks = manager.file_locks.read().await;
            let names: Vec<String> = locks
                .keys()
                .map(|p| {
                    p.file_name()
                        .expect("文件名")
                        .to_string_lossy()
                        .into_owned()
                })
                .collect();
            let mut total = 0usize;
            for loader in locks.values() {
                if let Some(arc) = loader.internal.get()
                    && let Ok(serializer) = arc.try_read()
                {
                    total += serializer.cached_bytes();
                }
            }
            (names, total)
        };
        // 不变式 1：最旧的区域 0 必须已被驱逐
        assert!(
            !names.contains(&"r.0.0.mca".to_string()),
            "区域 0 应已被驱逐: {names:?}"
        );
        // 不变式 2：最新的区域 2 必须仍在缓存
        assert!(
            names.contains(&"r.2.0.mca".to_string()),
            "区域 2 应仍在缓存: {names:?}"
        );
        // 不变式 3：缓存总账不得超预算
        assert!(total <= 2048, "缓存总账 {total} 超预算");

        // 被驱逐区域的磁盘数据必须原样可读
        let got =
            fetch_payloads(&manager, &folder, &[Vector2::new(0, 0), Vector2::new(1, 0)]).await;
        assert_eq!(
            got,
            vec![
                (Vector2::new(0, 0), pseudo_random_bytes(600, 11)),
                (Vector2::new(1, 0), pseudo_random_bytes(600, 12)),
            ]
        );

        // 向已被驱逐的区域再写入：管理器重读磁盘并合并，
        // 新旧区块都必须完好
        save_region(&manager, &folder, 0, &[13]).await;
        let got =
            fetch_payloads(&manager, &folder, &[Vector2::new(0, 0), Vector2::new(1, 0)]).await;
        assert_eq!(
            got,
            vec![
                (Vector2::new(0, 0), pseudo_random_bytes(600, 13)),
                (Vector2::new(1, 0), pseudo_random_bytes(600, 12)),
            ]
        );
    }

    #[tokio::test]
    async fn dirty_watched_region_is_never_evicted_before_flush() {
        let dir = tempfile::tempdir().expect("临时目录");
        let folder = test_folder(dir.path());
        let manager = PayloadManager::new(AnvilChunkConfig::default(), 100);

        // 模拟运行中服务器：区块被 watch（加载中），普通保存只
        // 合并进内存不写盘 → 序列化器带未落盘更新
        let at = Vector2::new(0, 0);
        manager.watch_chunks(&folder, &[at]).await;
        manager
            .save_chunks(
                &folder,
                vec![(at, PayloadChunk::new(0, 0, pseudo_random_bytes(600, 7)))],
            )
            .await
            .expect("保存");
        // 再触碰另一文件触发执法；带 pending 的条目必须存活
        let other = dir.path().join("region").join("r.9.9.mca");
        let _ = manager.get_serializer(&other).await.expect("取序列化器");

        assert!(
            manager
                .file_locks
                .read()
                .await
                .keys()
                .any(|p| p.ends_with("r.0.0.mca")),
            "带未落盘更新的条目绝不可被驱逐"
        );
        manager.unwatch_chunks(&folder, &[at]).await;
    }
}
