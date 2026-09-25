use std::{error, sync::Arc};

use bytes::Bytes;
use papokin_util::math::vector2::Vector2;
use tokio::io::AsyncWriteExt;

use super::{ChunkReadingError, ChunkWritingError};
use crate::level::LevelFolder;

pub mod file_manager;

pub(crate) async fn run_blocking<T, F>(task: F) -> Result<T, tokio::task::JoinError>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    tokio::task::spawn_blocking(task).await
}

/// 通过临时文件 + `sync_all` + 重命名将 `bytes` 原子写入 `path`。
///
/// `sync_all` 在重命名之前发出，因此本函数返回时数据已落盘：
/// 掉电场景下 rename 的元数据提交不会先于数据块持久化，
/// 进程崩溃/掉电都不会留下指向残缺内容的正式文件。
pub(crate) async fn atomic_write(
    path: &std::path::Path,
    bytes: &[u8],
) -> Result<(), std::io::Error> {
    let temp_path = path.with_extension("tmp_atomic");
    let mut file = tokio::fs::File::create(&temp_path).await?;
    file.write_all(bytes).await?;
    file.flush().await?;
    file.sync_all().await?;
    drop(file);
    tokio::fs::rename(&temp_path, path).await
}

/// 加载区块数据的结果。
///
/// 它可以是成功加载的数据、未找到的数据或错误
/// 附带区块坐标与所发生的错误。
#[derive(Debug)]
pub enum LoadedData<D: Send, Err: error::Error> {
    /// 区块数据已成功加载
    Loaded(D),

    /// 未找到区块数据
    Missing(Vector2<i32>),

    /// 加载区块数据时发生错误
    Error((Vector2<i32>, Err)),
}

impl<D: Send, E: error::Error> LoadedData<D, E> {
    pub fn map_loaded<D2: Send>(self, map: impl FnOnce(D) -> D2) -> LoadedData<D2, E> {
        match self {
            Self::Loaded(data) => LoadedData::Loaded(map(data)),
            Self::Missing(pos) => LoadedData::Missing(pos),
            Self::Error(err) => LoadedData::Error(err),
        }
    }
}

pub trait Dirtiable {
    fn is_dirty(&self) -> bool;
    fn mark_dirty(&self, flag: bool);
}

/// 用于处理区块 IO 的 trait
/// 用于加载和保存区块数据
/// 可针对不同类型的 IO 实现
/// 或使用不同的优化选项
///
/// `R` 类型是将被加载/保存的数据的类型
/// 例如 `ChunkData` 或 `EntityData`
pub trait FileIO
where
    Self: Send + Sync,
{
    type Data: Send + Sync + Sized;

    /// 加载区块数据
    fn fetch_chunks<'a>(
        &'a self,
        folder: &'a LevelFolder,
        chunk_coords: &'a [Vector2<i32>],
        stream: tokio::sync::mpsc::Sender<LoadedData<Self::Data, ChunkReadingError>>,
    ) -> impl Future<Output = ()> + Send + 'a;

    /// 持久化区块数据
    fn save_chunks<'a>(
        &'a self,
        folder: &'a LevelFolder,
        chunks_data: Vec<(Vector2<i32>, Self::Data)>,
    ) -> impl Future<Output = Result<(), ChunkWritingError>> + Send + 'a;

    /// 告知 `ChunkIO` 这些区块当前已加载到内存中
    fn watch_chunks<'a>(
        &'a self,
        folder: &'a LevelFolder,
        chunks: &'a [Vector2<i32>],
    ) -> impl Future<Output = ()> + Send + 'a;

    /// 告知 `ChunkIO` 这些区块不再加载在内存中
    fn unwatch_chunks<'a>(
        &'a self,
        folder: &'a LevelFolder,
        chunks: &'a [Vector2<i32>],
    ) -> impl Future<Output = ()> + Send + 'a;

    /// 告知 `ChunkIO` 不再有已加载到内存中的区块
    fn clear_watched_chunks(&self) -> impl Future<Output = ()> + Send + '_;

    /// 确保所有正在进行的操作都已完成
    fn block_and_await_ongoing_tasks(&self) -> impl Future<Output = ()> + Send + '_;
}

/// 用于将区块数据序列化为字节以及从字节反序列化的 trait。
///
/// `Data` 类型是将被更新或序列化/反序列化的数据的类型
/// 例如 `ChunkData` 或 `EntityData`
pub trait ChunkSerializer: Send + Sync + Default + 'static {
    type Data: Send + Sync + Sized + Dirtiable;
    type WriteBackend;

    type ChunkConfig;

    /// 获取区块的键（类似文件名）
    fn get_chunk_key(chunk: &Vector2<i32>) -> String;

    fn should_write(&self, is_watched: bool) -> bool;

    /// 将数据序列化为字节。
    fn write(
        &self,
        backend: &Self::WriteBackend,
    ) -> impl Future<Output = Result<(), std::io::Error>> + Send;

    /// 从字节创建新实例
    fn read(r: Bytes) -> Result<Self, ChunkReadingError>;

    /// 序列化器是否持有尚未写入磁盘的区块
    /// 还没有（例如写入失败之后）。
    ///
    /// 在此状态下逐出序列化器会静默丢失那些数据，因此
    /// 文件管理器会保留其缓存并重试写入。
    fn has_pending_writes(&self) -> bool {
        false
    }

    /// 从字节创建新实例，并知晓这些字节读取自哪个文件。
    ///
    /// 使用相对于区域文件的伴生文件进行格式化（原版外部
    /// `c.<x>.<z>.mcc` 区块、Paper 超大边车文件）会从
    /// `path`。默认实现会忽略该路径。
    fn read_at(r: Bytes, path: &std::path::Path) -> Result<Self, ChunkReadingError> {
        let _ = path;
        Self::read(r)
    }

    /// 将区块数据添加到序列化器
    fn update_chunk(
        &mut self,
        chunk_data: Arc<Self::Data>,
        chunk_config: &Self::ChunkConfig,
    ) -> impl Future<Output = Result<(), ChunkWritingError>> + Send;

    /// 从序列化器获取区块数据
    fn get_chunks(
        &self,
        chunks: Vec<Vector2<i32>>,
        stream: tokio::sync::mpsc::Sender<LoadedData<Self::Data, ChunkReadingError>>,
    ) -> impl Future<Output = ()> + Send;
}

#[cfg(test)]
mod tests {
    use super::atomic_write;

    #[tokio::test]
    async fn atomic_write_replaces_content_and_leaves_no_temp() {
        let dir = tempfile::tempdir().expect("临时目录");
        let path = dir.path().join("r.0.0.test");

        atomic_write(&path, b"first").await.expect("首次写入");
        assert_eq!(std::fs::read(&path).unwrap(), b"first");

        // 覆盖既有文件：rename 必须替换成功
        atomic_write(&path, b"second").await.expect("覆盖写入");
        assert_eq!(std::fs::read(&path).unwrap(), b"second");

        // 不留临时文件
        let entries: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(std::result::Result::ok)
            .map(|e| e.file_name())
            .collect();
        assert_eq!(entries, vec!["r.0.0.test"]);
    }
}
