use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use bytes::BufMut;
use papokin_data::meta_data_type::MetaDataType;
use papokin_data::tracked_data::{TrackedData, TrackedId};
use papokin_protocol::java::client::play::{Metadata, MetadataSerializer};
use papokin_protocol::ser::WritingError;
use papokin_util::version::JavaMinecraftVersion;

pub trait ErasedSerializer: Send + Sync {
    fn write(
        &self,
        index: TrackedId,
        r#type: MetaDataType,
        writer: &mut dyn std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError>;

    fn write_canonical(&self, index: TrackedId, r#type: MetaDataType) -> Vec<u8>;
}

struct SerializerHolder<T> {
    value: T,
}

impl<T: MetadataSerializer + Clone + Send + Sync + 'static> ErasedSerializer
    for SerializerHolder<T>
{
    fn write(
        &self,
        index: TrackedId,
        r#type: MetaDataType,
        writer: &mut dyn std::io::Write,
        version: &JavaMinecraftVersion,
    ) -> Result<(), WritingError> {
        let meta = Metadata::new_raw(index, r#type, &self.value);
        meta.write(writer, version)
    }

    fn write_canonical(&self, index: TrackedId, r#type: MetaDataType) -> Vec<u8> {
        let mut buf = Vec::new();
        let meta = Metadata::new_raw(index, r#type, &self.value);
        let _ = meta.write(&mut buf, &JavaMinecraftVersion::V_26_3);
        buf
    }
}

pub struct DataItem {
    pub tracked: TrackedData,
    pub serializer: Box<dyn ErasedSerializer>,
    pub canonical_bytes: Vec<u8>,
    pub dirty: bool,
    pub is_default: bool,
}

pub struct SynchedEntityData {
    items: Mutex<HashMap<TrackedData, DataItem>>,
    is_dirty: AtomicBool,
}

impl Default for SynchedEntityData {
    fn default() -> Self {
        Self::new()
    }
}

impl SynchedEntityData {
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: Mutex::new(HashMap::new()),
            is_dirty: AtomicBool::new(false),
        }
    }

    pub fn define<T: MetadataSerializer + Clone + Send + Sync + 'static>(
        &self,
        tracked: TrackedData,
        value: T,
    ) {
        let holder = SerializerHolder { value };
        let canonical_bytes = holder.write_canonical(tracked.id, tracked.r#type);
        let mut items = self
            .items
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        items.insert(
            tracked,
            DataItem {
                tracked,
                serializer: Box::new(holder),
                canonical_bytes,
                dirty: false,
                is_default: true,
            },
        );
    }

    pub fn set<T: MetadataSerializer + Clone + Send + Sync + 'static>(
        &self,
        tracked: TrackedData,
        value: T,
    ) -> bool {
        let holder = SerializerHolder { value };
        let new_canonical = holder.write_canonical(tracked.id, tracked.r#type);

        let mut items = self
            .items
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(item) = items.get_mut(&tracked) {
            if item.canonical_bytes == new_canonical {
                return false;
            }
            item.canonical_bytes = new_canonical;
            item.serializer = Box::new(holder);
            item.dirty = true;
            item.is_default = false;
        } else {
            items.insert(
                tracked,
                DataItem {
                    tracked,
                    serializer: Box::new(holder),
                    canonical_bytes: new_canonical,
                    dirty: true,
                    is_default: false,
                },
            );
        }
        self.is_dirty.store(true, Ordering::Release);
        true
    }

    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.is_dirty.load(Ordering::Acquire)
    }

    /// 在同一把锁内为每个在线客户端版本打包脏项并清脏。
    ///
    /// 分离的“打包后清脏”（先 `pack_dirty_for_version` 再
    /// `clear_dirty`）存在窗口：两步之间并发 [`Self::set`] 写入的
    /// 新脏项会被清脏一并吞掉，该次元数据更新将永远不会广播
    /// （高并发 tick 下的静默数据丢失）。此方法持有同一把
    /// `items` 锁完成打包与清脏，`set` 只能在其前后生效。
    pub fn pack_dirty_for_versions(
        &self,
        versions: &[JavaMinecraftVersion],
    ) -> Vec<(JavaMinecraftVersion, Box<[u8]>)> {
        let mut items = self
            .items
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        let mut out = Vec::new();
        for version in versions {
            let mut buf = Vec::new();
            let mut has_any = false;
            for item in items.values() {
                if item.dirty {
                    let before_len = buf.len();
                    if item
                        .serializer
                        .write(item.tracked.id, item.tracked.r#type, &mut buf, version)
                        .is_ok()
                        && buf.len() > before_len
                    {
                        has_any = true;
                    }
                }
            }
            if has_any {
                buf.put_u8(255);
                out.push((*version, buf.into_boxed_slice()));
            }
        }

        for item in items.values_mut() {
            item.dirty = false;
        }
        self.is_dirty.store(false, Ordering::Release);
        out
    }

    pub fn get_non_default_values_for_version(
        &self,
        version: &JavaMinecraftVersion,
    ) -> Option<Box<[u8]>> {
        let items = self
            .items
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut buf = Vec::new();
        let mut has_any = false;

        for item in items.values() {
            if !item.is_default {
                let before_len = buf.len();
                if item
                    .serializer
                    .write(item.tracked.id, item.tracked.r#type, &mut buf, version)
                    .is_ok()
                    && buf.len() > before_len
                {
                    has_any = true;
                }
            }
        }

        if !has_any {
            return None;
        }

        buf.put_u8(255);
        Some(buf.into_boxed_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use papokin_data::tracked_data;

    /// 打包与清脏在同一临界区：每个版本的缓冲以终止符结尾，
    /// 打包后脏标记清零，二次打包为空。
    #[test]
    fn pack_dirty_for_versions_packs_all_versions_and_clears() {
        let data = SynchedEntityData::new();
        data.define(tracked_data::living_entity::DATA_HEALTH_ID, 20.0f32);
        assert!(data.set(tracked_data::living_entity::DATA_HEALTH_ID, 15.5f32));
        assert!(data.is_dirty());

        let versions = [
            JavaMinecraftVersion::V_1_21_11,
            JavaMinecraftVersion::V_26_3,
        ];
        let packed = data.pack_dirty_for_versions(&versions);

        assert_eq!(packed.len(), 2);
        for (version, buf) in &packed {
            // 终止符 0xFF 必须存在，且两个版本键互不相同
            assert_eq!(*buf.last().unwrap(), 255);
            assert!(versions.contains(version));
        }
        assert!(!data.is_dirty());

        // 清脏后重复打包不得再产生任何包
        assert!(data.pack_dirty_for_versions(&versions).is_empty());
    }

    /// 并发窗口语义：打包完成后新的 set 写入不会被清脏吞掉。
    #[test]
    fn set_after_pack_marks_dirty_again() {
        let data = SynchedEntityData::new();
        data.define(tracked_data::living_entity::DATA_HEALTH_ID, 20.0f32);
        data.set(tracked_data::living_entity::DATA_HEALTH_ID, 15.5f32);
        let versions = [JavaMinecraftVersion::V_1_21_11];
        assert_eq!(data.pack_dirty_for_versions(&versions).len(), 1);

        // 模拟打包后到达的新写入：必须再次变脏等待下一轮广播
        assert!(data.set(tracked_data::living_entity::DATA_HEALTH_ID, 12.0f32));
        assert!(data.is_dirty());
        assert_eq!(data.pack_dirty_for_versions(&versions).len(), 1);
    }
}
