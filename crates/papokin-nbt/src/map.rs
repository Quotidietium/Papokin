//! NBT 复合标签的紧凑有序映射容器。
//!
//! 历史实现为 `HashMap<Box<str>, NbtTag>`：每个复合标签都要承担
//! 一张哈希表（表槽 + 控制字节 + 负载余量）与逐键独立堆分配，
//! 且 `NbtTag` 因内联 48 字节的哈希映射而膨胀到 56 字节。NBT
//! 复合标签在实践中是小映射（方块实体/实体存档通常 ≤20 键），
//! 插入序 `Vec` 承载同样的操作面时：
//!
//! - 非空复合标签的容器开销（表槽余量 + 控制字节）消失，
//!   仅保留逐条目的连续数组；
//! - `NbtTag` 收缩到 32 字节（`Vec` 变体重新成为最大变体），
//!   所有 `Vec<NbtTag>` 列表同步受益；
//! - 空复合标签从 48 字节内联降到 24 字节（实体/区块/世界的
//!   常驻 `custom_data` 字段即使为空也不再放大）。
//!
//! 查找为线性扫描：NBT 键为短字符串（memcmp 极快），条目数由
//! 解析层约束（网络侧 `custom_data` 另有 64 KiB 序列化总量上限，
//! 见 [`crate::compound::NbtCompound::estimated_serialized_size`]），
//! 最坏情况有界，不存在无界放大。
//!
//! 语义与哈希映射保持一致：
//! - 相等性是**键序无关**的（与 `HashMap` 的 `PartialEq` 语义
//!   对齐，手工实现而非派生）；
//! - `insert` 对已存在键**原位替换值**（保留首次插入的位置），
//!   与哈希映射「后者胜」的取值语义一致；
//! - 迭代顺序为插入序——哈希映射的迭代序本来就是进程内任意、
//!   NBT 线上格式亦不定义键序，插入序只是把「任意」固定为
//!   「确定」，所有既有读者不受影响。

use std::ops::Index;
use std::slice;

use crate::tag::NbtTag;

/// 有序紧凑的 NBT 复合标签条目表。
///
/// API 面对齐 `HashMap<Box<str>, NbtTag>` 的常用子集
/// （`get`/`get_mut`/`insert`/`remove`/`contains_key`/`keys`/
/// `values`/`iter`/`IntoIterator`/`Index`/`Extend`/`FromIterator`），
/// 以便既有调用点零改动迁移。
#[derive(Clone, Debug, Default)]
pub struct NbtMap {
    /// 键值对按插入序连续存储。
    entries: Vec<(Box<str>, NbtTag)>,
}

impl NbtMap {
    /// 创建一个空的映射。
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// 返回 `key` 对应的标签。
    #[inline]
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&NbtTag> {
        self.entries
            .iter()
            .find(|(k, _)| k.as_ref() == key)
            .map(|(_, v)| v)
    }

    /// 返回 `key` 对应的可变标签。
    #[inline]
    #[must_use]
    pub fn get_mut(&mut self, key: &str) -> Option<&mut NbtTag> {
        self.entries
            .iter_mut()
            .find(|(k, _)| k.as_ref() == key)
            .map(|(_, v)| v)
    }

    /// 插入或替换条目，返回被替换的旧值（若存在）。
    ///
    /// 对已存在的键原位替换值并保留其首次插入的位置（取值语义
    /// 与哈希映射的「后者胜」一致）。
    #[inline]
    pub fn insert(&mut self, key: Box<str>, value: NbtTag) -> Option<NbtTag> {
        self.insert_impl(key, value)
    }

    /// 以借用键插入或替换条目，返回被替换的旧值（若存在）。
    ///
    /// 与 [`NbtMap::insert`] 语义一致，但仅在新键时才分配键存储，
    /// 替换路径零分配。
    #[inline]
    pub fn insert_str(&mut self, key: &str, value: NbtTag) -> Option<NbtTag> {
        self.insert_impl(key, value)
    }

    /// 插入实现：`K: AsRef<str>` 统一替换与追加两条路径。
    fn insert_impl(&mut self, key: impl AsRef<str>, value: NbtTag) -> Option<NbtTag> {
        let key_ref = key.as_ref();
        if let Some((_, slot)) = self.entries.iter_mut().find(|(k, _)| k.as_ref() == key_ref) {
            return Some(std::mem::replace(slot, value));
        }
        self.entries.push((key_ref.into(), value));
        None
    }

    /// 移除 `key` 对应的条目并返回其值（保持其余条目的相对顺序）。
    #[inline]
    pub fn remove(&mut self, key: &str) -> Option<NbtTag> {
        let index = self.entries.iter().position(|(k, _)| k.as_ref() == key)?;
        Some(self.entries.remove(index).1)
    }

    /// 返回映射是否包含 `key`。
    #[inline]
    #[must_use]
    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k.as_ref() == key)
    }

    /// 返回条目数。
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// 返回映射是否为空。
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 按插入序迭代 `(&Box<str>, &NbtTag)`。
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&Box<str>, &NbtTag)> {
        self.entries.iter().map(|(k, v)| (k, v))
    }

    /// 按插入序可变迭代 `(&Box<str>, &mut NbtTag)`。
    #[inline]
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&Box<str>, &mut NbtTag)> {
        self.entries.iter_mut().map(|(k, v)| (&*k, v))
    }

    /// 按插入序迭代键。
    #[inline]
    pub fn keys(&self) -> impl Iterator<Item = &Box<str>> {
        self.entries.iter().map(|(k, _)| k)
    }

    /// 按插入序迭代值。
    #[inline]
    pub fn values(&self) -> impl Iterator<Item = &NbtTag> {
        self.entries.iter().map(|(_, v)| v)
    }

    /// 按插入序可变迭代值。
    #[inline]
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut NbtTag> {
        self.entries.iter_mut().map(|(_, v)| v)
    }
}

impl<'a> IntoIterator for &'a NbtMap {
    type Item = &'a (Box<str>, NbtTag);
    type IntoIter = slice::Iter<'a, (Box<str>, NbtTag)>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl<'a> IntoIterator for &'a mut NbtMap {
    type Item = &'a mut (Box<str>, NbtTag);
    type IntoIter = slice::IterMut<'a, (Box<str>, NbtTag)>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter_mut()
    }
}

impl IntoIterator for NbtMap {
    type Item = (Box<str>, NbtTag);
    type IntoIter = std::vec::IntoIter<(Box<str>, NbtTag)>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl Index<&str> for NbtMap {
    type Output = NbtTag;

    /// 返回 `key` 对应的标签；键不存在时 panic（与哈希映射的
    /// `Index` 语义一致）。
    ///
    /// # Panics
    ///
    /// 若 `key` 不存在于映射中。
    #[inline]
    #[allow(clippy::panic)] // Index 语义与 HashMap 一致：缺失键必须 panic
    fn index(&self, key: &str) -> &Self::Output {
        self.get(key)
            .unwrap_or_else(|| panic!("NBT 键不存在：{key}"))
    }
}

/// 键序无关的相等性（对齐 `HashMap` 的 `PartialEq` 语义）。
///
/// 双方条目数一致且每个键的值都相等即相等，与插入顺序无关。
impl PartialEq for NbtMap {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .entries
                .iter()
                .all(|(k, v)| other.get(k).is_some_and(|ov| v == ov))
    }
}

impl Extend<(Box<str>, NbtTag)> for NbtMap {
    fn extend<T: IntoIterator<Item = (Box<str>, NbtTag)>>(&mut self, iter: T) {
        for (key, value) in iter {
            self.insert(key, value);
        }
    }
}

impl Extend<(String, NbtTag)> for NbtMap {
    fn extend<T: IntoIterator<Item = (String, NbtTag)>>(&mut self, iter: T) {
        for (key, value) in iter {
            self.insert_str(&key, value);
        }
    }
}

impl FromIterator<(Box<str>, NbtTag)> for NbtMap {
    fn from_iter<T: IntoIterator<Item = (Box<str>, NbtTag)>>(iter: T) -> Self {
        let mut map = Self::new();
        map.extend(iter);
        map
    }
}

impl FromIterator<(String, NbtTag)> for NbtMap {
    fn from_iter<T: IntoIterator<Item = (String, NbtTag)>>(iter: T) -> Self {
        let mut map = Self::new();
        map.extend(iter);
        map
    }
}

#[cfg(test)]
mod tests {
    use super::NbtMap;
    use crate::tag::NbtTag;

    fn sample() -> NbtMap {
        let mut map = NbtMap::new();
        map.insert_str("Pos", NbtTag::Int(1));
        map.insert_str("Motion", NbtTag::Int(2));
        map.insert_str("Health", NbtTag::Float(3.5));
        map
    }

    #[test]
    fn 基本读写与长度() {
        let map = sample();
        assert_eq!(map.len(), 3);
        assert!(!map.is_empty());
        assert_eq!(map.get("Pos"), Some(&NbtTag::Int(1)));
        assert_eq!(map.get("missing"), None);
        assert!(map.contains_key("Health"));
        assert!(!map.contains_key("missing"));
    }

    #[test]
    fn 替换保留位置且后者胜() {
        let mut map = sample();
        assert_eq!(map.insert_str("Pos", NbtTag::Int(99)), Some(NbtTag::Int(1)));
        assert_eq!(map.len(), 3);
        // 替换后位置仍在首位（插入序保留）
        assert_eq!(
            map.keys().next().map(std::convert::AsRef::as_ref),
            Some("Pos")
        );
        assert_eq!(map.get("Pos"), Some(&NbtTag::Int(99)));

        // 新键追加到尾部
        map.insert_str("Fire", NbtTag::Short(-1));
        assert_eq!(map.len(), 4);
        assert_eq!(
            map.keys().last().map(std::convert::AsRef::as_ref),
            Some("Fire")
        );
    }

    #[test]
    fn 移除保持其余顺序() {
        let mut map = sample();
        assert_eq!(map.remove("Motion"), Some(NbtTag::Int(2)));
        assert_eq!(map.remove("Motion"), None);
        assert_eq!(map.len(), 2);
        let keys: Vec<&str> = map.keys().map(std::convert::AsRef::as_ref).collect();
        assert_eq!(keys, ["Pos", "Health"]);
    }

    #[test]
    fn 键序无关相等() {
        let mut a = sample();
        let mut b = NbtMap::new();
        // 逆序构建同样的内容
        b.insert_str("Health", NbtTag::Float(3.5));
        b.insert_str("Motion", NbtTag::Int(2));
        b.insert_str("Pos", NbtTag::Int(1));
        assert_eq!(a, b);

        a.insert_str("extra", NbtTag::Byte(0));
        assert_ne!(a, b);

        b.insert_str("extra", NbtTag::Byte(0));
        assert_eq!(a, b);
        b.insert_str("Pos2", NbtTag::Int(1));
        assert_ne!(a, b);
    }

    #[test]
    fn 索引访问() {
        let map = sample();
        assert_eq!(map["Health"], NbtTag::Float(3.5));
    }

    #[test]
    #[should_panic(expected = "NBT 键不存在")]
    fn 索引缺失键时_panic() {
        let map = sample();
        let _ = &map["missing"];
    }

    #[test]
    fn 迭代为插入序() {
        let map = sample();
        let keys: Vec<&str> = map.iter().map(|(k, _)| k.as_ref()).collect();
        assert_eq!(keys, ["Pos", "Motion", "Health"]);

        let owned: Vec<(Box<str>, NbtTag)> = map.into_iter().collect();
        assert_eq!(owned[0].0.as_ref(), "Pos");
    }

    #[test]
    fn 收集与扩展() {
        let entries = vec![
            (String::from("a"), NbtTag::Int(1)),
            (String::from("b"), NbtTag::Int(2)),
        ];
        let mut map: NbtMap = entries.into_iter().collect();
        assert_eq!(map.len(), 2);

        map.extend([(Box::from("c"), NbtTag::Int(3))]);
        assert_eq!(map.len(), 3);
        assert_eq!(map.get("c"), Some(&NbtTag::Int(3)));
    }

    #[test]
    fn 尺寸收缩达标() {
        // 轮次 40 的核心收益之一：NbtTag 从 56 字节（内联哈希映射）
        // 收缩到 32 字节，NbtMap/复合标签内联 24 字节。
        assert_eq!(std::mem::size_of::<NbtTag>(), 32);
        assert_eq!(std::mem::size_of::<NbtMap>(), 24);
    }
}
