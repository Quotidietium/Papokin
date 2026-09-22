//! 合并后的标签映射，交给标签同步数据包（`CUpdateTags` /
//! `CUpdateTagsPlay`）由服务器发送。
//!
//! 标签通常直接从静态的编译期
//! 表中。此外，插件还可以向现有
//! 标签添加条目、移除条目，或通过服务器的标签
//! 管理器。服务器会将其覆盖层与静态表合并，并把
//! 每个条目解析为连接客户端所期望的网络 id（包括
//! 跨版本重映射与自定义注册表条目），并将
//! 结果以 [`MergedTags`] 的形式放进数据包。当某个注册表键没有
//! 覆盖层时，数据包会原样回退到静态表路径。

use std::collections::HashMap;

use papokin_data::tag::RegistryKey;

/// 针对单个客户端协议版本的完全合并标签映射。
///
/// 这是静态表加上服务器的插件覆盖层，其中每个
/// 条目已解析为最终的网络 id（版本重映射已经
/// 在需要的地方应用）。
#[derive(Clone, Debug, Default)]
pub struct MergedTags {
    /// 每个注册表键对应的合并标签：标签名 -> 按网络顺序排列的条目 ID。
    /// 只有其静态表被覆盖层修改过的注册表键
    /// (或仅通过覆盖层才存在的条目) 都会出现。
    pub maps: HashMap<RegistryKey, Vec<(String, Vec<u16>)>>,
}

impl MergedTags {
    /// 某个注册表键合并后的标签（如果叠加层触及过它）。
    #[must_use]
    pub fn get(&self, key: RegistryKey) -> Option<&[(String, Vec<u16>)]> {
        self.maps.get(&key).map(Vec::as_slice)
    }

    /// 叠加层是否为 `key` 生成了任何合并后的标签。
    #[must_use]
    pub fn contains_key(&self, key: RegistryKey) -> bool {
        self.maps.contains_key(&key)
    }

    /// 叠加层是否完全没有触及任何注册表键。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.maps.is_empty()
    }
}
