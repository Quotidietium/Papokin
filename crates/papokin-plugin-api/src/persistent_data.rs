//! 持久化自定义数据容器 API（Bukkit 风格的 `PersistentDataHolder`）。
//!
//! 在实体、方块实体、区块、
//! 世界和物品堆上提供带命名空间的持久化数据存储，并自动以 NBT 格式持久化到磁盘。

use crate::wit::papokin::plugin::block_entity::BlockEntity;
use crate::wit::papokin::plugin::common::{NbtTag, NbtTree};
use crate::wit::papokin::plugin::item_stack::ItemStack;
use crate::wit::papokin::plugin::player::Player;
use crate::wit::papokin::plugin::world::{Chunk, Entity, World};

/// 构造包含单个字符串值的 `NbtTree`。
pub fn string_tree(val: &str) -> NbtTree {
    NbtTree {
        root: 0,
        tags: vec![NbtTag::StringTag(val.to_string())],
    }
}

/// 构造包含单个 32 位整数值的 `NbtTree`。
pub fn int_tree(val: i32) -> NbtTree {
    NbtTree {
        root: 0,
        tags: vec![NbtTag::Int(val)],
    }
}

/// 构造包含单个 64 位整数值的 `NbtTree`。
pub fn long_tree(val: i64) -> NbtTree {
    NbtTree {
        root: 0,
        tags: vec![NbtTag::Long(val)],
    }
}

/// 构造包含单个 16 位 short 值的 `NbtTree`。
pub fn short_tree(val: i16) -> NbtTree {
    NbtTree {
        root: 0,
        tags: vec![NbtTag::Short(val)],
    }
}

/// 构造包含单个字节（8 位）值的 `NbtTree`。
pub fn byte_tree(val: i8) -> NbtTree {
    NbtTree {
        root: 0,
        tags: vec![NbtTag::Byte(val)],
    }
}

/// 构造包含单个布尔值的 `NbtTree`。
pub fn bool_tree(val: bool) -> NbtTree {
    NbtTree {
        root: 0,
        tags: vec![NbtTag::Byte(if val { 1 } else { 0 })],
    }
}

/// 构造包含单个 32 位 float 值的 `NbtTree`。
pub fn float_tree(val: f32) -> NbtTree {
    NbtTree {
        root: 0,
        tags: vec![NbtTag::Float(val)],
    }
}

/// 构造包含单个 64 位 double 值的 `NbtTree`。
pub fn double_tree(val: f64) -> NbtTree {
    NbtTree {
        root: 0,
        tags: vec![NbtTag::Double(val)],
    }
}

/// 构造包含字节数组值的 `NbtTree`。
pub fn byte_array_tree(val: Vec<i8>) -> NbtTree {
    NbtTree {
        root: 0,
        tags: vec![NbtTag::ByteArray(val)],
    }
}

/// 构造包含整型数组值的 `NbtTree`。
pub fn int_array_tree(val: Vec<i32>) -> NbtTree {
    NbtTree {
        root: 0,
        tags: vec![NbtTag::IntArray(val)],
    }
}

/// 构造包含长整型数组值的 `NbtTree`。
pub fn long_array_tree(val: Vec<i64>) -> NbtTree {
    NbtTree {
        root: 0,
        tags: vec![NbtTag::LongArray(val)],
    }
}

/// 用于可持有持久化、带命名空间的自定义 NBT 数据的对象的 trait。
///
/// 类比 Bukkit 的 `PersistentDataHolder` 接口。
pub trait PersistentDataHolder {
    /// 在指定的命名空间和键下设置原始 NBT 树。
    fn set_custom_data(&self, namespace: &str, key: &str, value: &NbtTree);

    /// 获取指定命名空间与键下的原始 NBT 树（如果存在）。
    fn get_custom_data(&self, namespace: &str, key: &str) -> Option<NbtTree>;

    /// 移除存储在指定命名空间和键下的自定义数据。
    fn remove_custom_data(&self, namespace: &str, key: &str);

    ///若自定义数据存储在指定的命名空间和键之下，则返回 `true`。
    fn has_custom_data(&self, namespace: &str, key: &str) -> bool;

    /// 设置字符串值。
    fn set_string(&self, namespace: &str, key: &str, value: &str) {
        self.set_custom_data(namespace, key, &string_tree(value));
    }

    /// 获取字符串值。
    fn get_string(&self, namespace: &str, key: &str) -> Option<String> {
        let tree = self.get_custom_data(namespace, key)?;
        match tree.tags.get(tree.root as usize)? {
            NbtTag::StringTag(s) => Some(s.clone()),
            _ => None,
        }
    }

    /// 设置 32 位整数值。
    fn set_int(&self, namespace: &str, key: &str, value: i32) {
        self.set_custom_data(namespace, key, &int_tree(value));
    }

    /// 获取 32 位整数值。
    fn get_int(&self, namespace: &str, key: &str) -> Option<i32> {
        let tree = self.get_custom_data(namespace, key)?;
        match tree.tags.get(tree.root as usize)? {
            NbtTag::Int(v) => Some(*v),
            NbtTag::Byte(v) => Some(i32::from(*v)),
            NbtTag::Short(v) => Some(i32::from(*v)),
            _ => None,
        }
    }

    /// 设置 64 位整数值。
    fn set_long(&self, namespace: &str, key: &str, value: i64) {
        self.set_custom_data(namespace, key, &long_tree(value));
    }

    /// 获取 64 位整数值。
    fn get_long(&self, namespace: &str, key: &str) -> Option<i64> {
        let tree = self.get_custom_data(namespace, key)?;
        match tree.tags.get(tree.root as usize)? {
            NbtTag::Long(v) => Some(*v),
            NbtTag::Int(v) => Some(i64::from(*v)),
            NbtTag::Byte(v) => Some(i64::from(*v)),
            NbtTag::Short(v) => Some(i64::from(*v)),
            _ => None,
        }
    }

    /// 设置 16 位整数值。
    fn set_short(&self, namespace: &str, key: &str, value: i16) {
        self.set_custom_data(namespace, key, &short_tree(value));
    }

    /// 获取 16 位整数值。
    fn get_short(&self, namespace: &str, key: &str) -> Option<i16> {
        let tree = self.get_custom_data(namespace, key)?;
        match tree.tags.get(tree.root as usize)? {
            NbtTag::Short(v) => Some(*v),
            NbtTag::Byte(v) => Some(i16::from(*v)),
            _ => None,
        }
    }

    /// 设置 8 位字节值。
    fn set_byte(&self, namespace: &str, key: &str, value: i8) {
        self.set_custom_data(namespace, key, &byte_tree(value));
    }

    /// 获取 8 位字节值。
    fn get_byte(&self, namespace: &str, key: &str) -> Option<i8> {
        let tree = self.get_custom_data(namespace, key)?;
        match tree.tags.get(tree.root as usize)? {
            NbtTag::Byte(v) => Some(*v),
            _ => None,
        }
    }

    /// 设置布尔值（以 NBT 字节存储：1 或 0）。
    fn set_bool(&self, namespace: &str, key: &str, value: bool) {
        self.set_custom_data(namespace, key, &bool_tree(value));
    }

    /// 获取布尔值。
    fn get_bool(&self, namespace: &str, key: &str) -> Option<bool> {
        let tree = self.get_custom_data(namespace, key)?;
        match tree.tags.get(tree.root as usize)? {
            NbtTag::Byte(v) => Some(*v != 0),
            _ => None,
        }
    }

    /// 设置 32 位浮点值。
    fn set_float(&self, namespace: &str, key: &str, value: f32) {
        self.set_custom_data(namespace, key, &float_tree(value));
    }

    /// 获取 32 位浮点值。
    fn get_float(&self, namespace: &str, key: &str) -> Option<f32> {
        let tree = self.get_custom_data(namespace, key)?;
        match tree.tags.get(tree.root as usize)? {
            NbtTag::Float(v) => Some(*v),
            _ => None,
        }
    }

    /// 设置 64 位双精度浮点值。
    fn set_double(&self, namespace: &str, key: &str, value: f64) {
        self.set_custom_data(namespace, key, &double_tree(value));
    }

    /// 获取 64 位双精度值。
    fn get_double(&self, namespace: &str, key: &str) -> Option<f64> {
        let tree = self.get_custom_data(namespace, key)?;
        match tree.tags.get(tree.root as usize)? {
            NbtTag::Double(v) => Some(*v),
            NbtTag::Float(v) => Some(f64::from(*v)),
            _ => None,
        }
    }

    /// 设置字节数组值。
    fn set_byte_array(&self, namespace: &str, key: &str, value: Vec<i8>) {
        self.set_custom_data(namespace, key, &byte_array_tree(value));
    }

    /// 获取字节数组值。
    fn get_byte_array(&self, namespace: &str, key: &str) -> Option<Vec<i8>> {
        let tree = self.get_custom_data(namespace, key)?;
        match tree.tags.get(tree.root as usize)? {
            NbtTag::ByteArray(v) => Some(v.clone()),
            _ => None,
        }
    }

    /// 设置整型数组值。
    fn set_int_array(&self, namespace: &str, key: &str, value: Vec<i32>) {
        self.set_custom_data(namespace, key, &int_array_tree(value));
    }

    /// 获取整型数组值。
    fn get_int_array(&self, namespace: &str, key: &str) -> Option<Vec<i32>> {
        let tree = self.get_custom_data(namespace, key)?;
        match tree.tags.get(tree.root as usize)? {
            NbtTag::IntArray(v) => Some(v.clone()),
            _ => None,
        }
    }

    /// 设置长整型数组值。
    fn set_long_array(&self, namespace: &str, key: &str, value: Vec<i64>) {
        self.set_custom_data(namespace, key, &long_array_tree(value));
    }

    /// 获取长整型数组值。
    fn get_long_array(&self, namespace: &str, key: &str) -> Option<Vec<i64>> {
        let tree = self.get_custom_data(namespace, key)?;
        match tree.tags.get(tree.root as usize)? {
            NbtTag::LongArray(v) => Some(v.clone()),
            _ => None,
        }
    }
}

impl PersistentDataHolder for ItemStack {
    fn set_custom_data(&self, namespace: &str, key: &str, value: &NbtTree) {
        self.set_custom_data(namespace, key, value);
    }

    fn get_custom_data(&self, namespace: &str, key: &str) -> Option<NbtTree> {
        self.get_custom_data(namespace, key)
    }

    fn remove_custom_data(&self, namespace: &str, key: &str) {
        self.remove_custom_data(namespace, key);
    }

    fn has_custom_data(&self, namespace: &str, key: &str) -> bool {
        self.has_custom_data(namespace, key)
    }
}

impl PersistentDataHolder for Entity {
    fn set_custom_data(&self, namespace: &str, key: &str, value: &NbtTree) {
        self.set_custom_data(namespace, key, value);
    }

    fn get_custom_data(&self, namespace: &str, key: &str) -> Option<NbtTree> {
        self.get_custom_data(namespace, key)
    }

    fn remove_custom_data(&self, namespace: &str, key: &str) {
        self.remove_custom_data(namespace, key);
    }

    fn has_custom_data(&self, namespace: &str, key: &str) -> bool {
        self.has_custom_data(namespace, key)
    }
}

impl PersistentDataHolder for BlockEntity {
    fn set_custom_data(&self, namespace: &str, key: &str, value: &NbtTree) {
        self.set_custom_data(namespace, key, value);
    }

    fn get_custom_data(&self, namespace: &str, key: &str) -> Option<NbtTree> {
        self.get_custom_data(namespace, key)
    }

    fn remove_custom_data(&self, namespace: &str, key: &str) {
        self.remove_custom_data(namespace, key);
    }

    fn has_custom_data(&self, namespace: &str, key: &str) -> bool {
        self.has_custom_data(namespace, key)
    }
}

impl PersistentDataHolder for Chunk {
    fn set_custom_data(&self, namespace: &str, key: &str, value: &NbtTree) {
        self.set_custom_data(namespace, key, value);
    }

    fn get_custom_data(&self, namespace: &str, key: &str) -> Option<NbtTree> {
        self.get_custom_data(namespace, key)
    }

    fn remove_custom_data(&self, namespace: &str, key: &str) {
        self.remove_custom_data(namespace, key);
    }

    fn has_custom_data(&self, namespace: &str, key: &str) -> bool {
        self.has_custom_data(namespace, key)
    }
}

impl PersistentDataHolder for World {
    fn set_custom_data(&self, namespace: &str, key: &str, value: &NbtTree) {
        self.set_custom_data(namespace, key, value);
    }

    fn get_custom_data(&self, namespace: &str, key: &str) -> Option<NbtTree> {
        self.get_custom_data(namespace, key)
    }

    fn remove_custom_data(&self, namespace: &str, key: &str) {
        self.remove_custom_data(namespace, key);
    }

    fn has_custom_data(&self, namespace: &str, key: &str) -> bool {
        self.has_custom_data(namespace, key)
    }
}

impl PersistentDataHolder for Player {
    fn set_custom_data(&self, namespace: &str, key: &str, value: &NbtTree) {
        self.as_entity().set_custom_data(namespace, key, value);
    }

    fn get_custom_data(&self, namespace: &str, key: &str) -> Option<NbtTree> {
        self.as_entity().get_custom_data(namespace, key)
    }

    fn remove_custom_data(&self, namespace: &str, key: &str) {
        self.as_entity().remove_custom_data(namespace, key);
    }

    fn has_custom_data(&self, namespace: &str, key: &str) -> bool {
        self.as_entity().has_custom_data(namespace, key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    struct MockHolder {
        data: RefCell<HashMap<(String, String), NbtTree>>,
    }

    impl MockHolder {
        fn new() -> Self {
            Self {
                data: RefCell::new(HashMap::new()),
            }
        }
    }

    impl PersistentDataHolder for MockHolder {
        fn set_custom_data(&self, namespace: &str, key: &str, value: &NbtTree) {
            self.data
                .borrow_mut()
                .insert((namespace.to_string(), key.to_string()), value.clone());
        }

        fn get_custom_data(&self, namespace: &str, key: &str) -> Option<NbtTree> {
            self.data
                .borrow()
                .get(&(namespace.to_string(), key.to_string()))
                .cloned()
        }

        fn remove_custom_data(&self, namespace: &str, key: &str) {
            self.data
                .borrow_mut()
                .remove(&(namespace.to_string(), key.to_string()));
        }

        fn has_custom_data(&self, namespace: &str, key: &str) -> bool {
            self.data
                .borrow()
                .contains_key(&(namespace.to_string(), key.to_string()))
        }
    }

    #[test]
    fn persistent_data_typed_methods() {
        let holder = MockHolder::new();

        // 字符串
        holder.set_string("my_mod", "greeting", "hello world");
        assert!(holder.has_custom_data("my_mod", "greeting"));
        assert_eq!(
            holder.get_string("my_mod", "greeting"),
            Some("hello world".to_string())
        );

        // 整数
        holder.set_int("my_mod", "score", 9001);
        assert_eq!(holder.get_int("my_mod", "score"), Some(9001));

        // 长整数
        holder.set_long("my_mod", "large_id", 123_456_789_012);
        assert_eq!(holder.get_long("my_mod", "large_id"), Some(123_456_789_012));

        // 布尔值
        holder.set_bool("my_mod", "is_admin", true);
        assert_eq!(holder.get_bool("my_mod", "is_admin"), Some(true));

        // 浮点与双精度浮点
        holder.set_float("my_mod", "multiplier", 1.5);
        assert_eq!(holder.get_float("my_mod", "multiplier"), Some(1.5));

        holder.set_double("my_mod", "precise", std::f64::consts::PI);
        assert_eq!(
            holder.get_double("my_mod", "precise"),
            Some(std::f64::consts::PI)
        );

        // 字节数组
        holder.set_byte_array("my_mod", "raw_bytes", vec![1, 2, 3, 4]);
        assert_eq!(
            holder.get_byte_array("my_mod", "raw_bytes"),
            Some(vec![1, 2, 3, 4])
        );

        // 移除
        holder.remove_custom_data("my_mod", "greeting");
        assert!(!holder.has_custom_data("my_mod", "greeting"));
        assert_eq!(holder.get_string("my_mod", "greeting"), None);
    }
}
