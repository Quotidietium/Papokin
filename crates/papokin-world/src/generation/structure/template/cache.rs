//! 为内嵌结构模板提供缓存。
//!
//! 此模块为结构模板提供惰性加载缓存，这些模板
//! 在编译期通过 `include_bytes!` 嵌入二进制文件中。

use std::sync::Arc;

use dashmap::DashMap;

use super::{StructureTemplate, structure_template::TemplateError};

/// 原版的隐式命名空间。
const DEFAULT_NAMESPACE: &str = "minecraft";

/// 将资源 ID 规范化为完全限定的 `namespace:path` 形式。
///
/// 裸的 `foo` 会变为 `minecraft:foo`，与原版的解析规则一致。
fn canonicalize(name: &str) -> String {
    if name.contains(':') {
        name.to_owned()
    } else {
        format!("{DEFAULT_NAMESPACE}:{name}")
    }
}

/// 用于已加载结构模板的缓存。
///
/// 模板在首次访问时惰性加载并存储以供复用。
/// 键为完全限定的资源 ID，因此 `foo` 与 `minecraft:foo`
/// 共享单一条目。
/// 该缓存是线程安全的，可从多个线程访问。
pub struct TemplateCache {
    cache: DashMap<String, Arc<StructureTemplate>>,
}

impl Default for TemplateCache {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateCache {
    /// 创建新的空模板缓存。
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: DashMap::new(),
        }
    }

    /// 按 `name` 获取模板，若未缓存则从内嵌资源中加载。
    ///
    /// `name` 可为裸形式（`foo`）或带命名空间（`minecraft:foo`、`papokin:foo`）。
    ///
    /// 返回以 `Arc` 包装的已加载模板；若该模板
    /// 不存在或加载失败。
    pub fn get(&self, name: &str) -> Option<Arc<StructureTemplate>> {
        match self.get_or_error(name) {
            Ok(template) => Some(template),
            Err(TemplateError::MissingField("template file not found")) => None,
            Err(e) => {
                tracing::error!("加载模板 '{}' 失败：{}", name, e);
                None
            }
        }
    }

    /// 按名称获取模板，加载失败时返回错误。
    ///
    /// # Errors
    ///
    ///若模板不存在或解析失败，则返回错误。
    pub fn get_or_error(&self, name: &str) -> Result<Arc<StructureTemplate>, TemplateError> {
        let key = canonicalize(name);

        // 先检查缓存
        if let Some(template) = self.cache.get(&key) {
            return Ok(Arc::clone(&template));
        }

        // 尝试加载模板
        let bytes = Self::load_template_bytes(&key)
            .ok_or(TemplateError::MissingField("template file not found"))?;

        let template = StructureTemplate::from_nbt_bytes(bytes)?;
        let arc = Arc::new(template);
        self.cache.insert(key, Arc::clone(&arc));
        Ok(arc)
    }

    /// 将模板列表预加载到缓存中。
    ///
    /// 这在服务器启动期间很有用，可以避免加载延迟
    /// 在游戏过程中。
    pub fn preload(&self, names: &[&str]) {
        for name in names {
            if let Err(e) = self.get_or_error(name) {
                tracing::warn!("预加载模板 '{}' 失败：{}", name, e);
            }
        }
    }

    /// 在运行时从原始 gzip 压缩的 NBT 字节注册模板（原版
    /// `.nbt` 结构格式）。
    ///
    /// `name` 可为裸形式（`foo`）或带命名空间（`papokin:foo`）；
    /// 以与 [`Self::get`] 相同的方式规范化。以某个名称注册
    /// 已存在的模板——无论是内嵌的还是先前注册的——都会替换它，
    /// 因为查找会先查缓存，再查内嵌资源。
    ///
    /// # Errors
    ///
    ///若 `nbt_bytes` 解压缩失败或无法解析为复合标签，则返回错误
    /// 结构模板。出错时缓存保持不变。
    pub fn register_template(
        &self,
        name: &str,
        nbt_bytes: &[u8],
    ) -> Result<Arc<StructureTemplate>, TemplateError> {
        let key = canonicalize(name);
        let template = StructureTemplate::from_nbt_bytes(nbt_bytes)?;
        let arc = Arc::new(template);
        self.cache.insert(key, Arc::clone(&arc));
        Ok(arc)
    }

    /// 返回名为 `name` 的模板能否被解析，无论是从
    /// 缓存（已加载或运行时注册的），或来自内嵌资源。
    ///
    /// 与 [`Self::get`] 不同，此方法从不解析或缓存内嵌模板。
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        let key = canonicalize(name);
        self.cache.contains_key(&key) || Self::load_template_bytes(&key).is_some()
    }

    /// 返回当前缓存中所有模板的名称。
    ///
    /// 这同时涵盖运行时注册的模板和内嵌的模板
    /// 已加载的模板；尚未被访问的内嵌模板
    /// 而应改用 [`all_template_names`] 列出的内容。
    #[must_use]
    pub fn cached_names(&self) -> Vec<String> {
        self.cache.iter().map(|entry| entry.key().clone()).collect()
    }

    /// 返回已缓存模板的数量。
    #[must_use]
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// 返回缓存是否为空。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// 清除所有缓存的模板。
    pub fn clear(&self) {
        self.cache.clear();
    }

    /// 从内嵌资源加载原始模板字节。
    fn load_template_bytes(path: &str) -> Option<&'static [u8]> {
        get_template_bytes(path)
    }
}

include!(concat!(env!("OUT_DIR"), "/template_embeddings.rs"));

/// 全局模板缓存实例。
///
/// 这提供了一个可在整个代码库中使用的单例缓存
/// 而无需到处传递缓存引用。
static GLOBAL_CACHE: std::sync::LazyLock<TemplateCache> =
    std::sync::LazyLock::new(TemplateCache::new);

/// 获取全局模板缓存。
#[must_use]
pub fn global_cache() -> &'static TemplateCache {
    &GLOBAL_CACHE
}

/// 按 `name` 从全局缓存中获取模板。
///
/// 返回以 `Arc` 包装的已加载模板；未找到时返回 `None`。
#[must_use]
pub fn get_template(name: &str) -> Option<Arc<StructureTemplate>> {
    global_cache().get(name)
}

/// 在运行时从原始 gzip 压缩的 NBT 在全局缓存中注册模板
/// 字节（原版 `.nbt` 结构格式）。
///
/// 这与 `/place template` 命令解析名称所使用的是同一个缓存
/// 检查依据，因此在此注册的模板立即可供双方放置
/// 命令与插件 API。`name` 可以是裸名（`foo`）或带命名空间
/// (`my_plugin:foo`)；以已有名称注册会替换原条目。
///
/// # Errors
///
///若 `nbt_bytes` 解压缩失败或无法解析为复合标签，则返回错误
/// 结构模板。
pub fn register_template(
    name: &str,
    nbt_bytes: &[u8],
) -> Result<Arc<StructureTemplate>, TemplateError> {
    global_cache().register_template(name, nbt_bytes)
}

/// 返回名为 `name` 的模板能否从全局
/// 缓存，涵盖运行时注册、已加载以及内嵌的模板。
#[must_use]
pub fn has_template(name: &str) -> bool {
    global_cache().contains(name)
}

/// 列出全局缓存已知的所有模板名称：每个内嵌的，
/// 模板，加上每个运行时注册的模板，去重并排序。
#[must_use]
pub fn list_template_names() -> Vec<String> {
    let mut names: Vec<String> = all_template_names()
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    for cached in global_cache().cached_names() {
        if !names.contains(&cached) {
            names.push(cached);
        }
    }
    names.sort_unstable();
    names
}

/// 返回模板池的原始 JSON；未找到时返回 `None`。
#[must_use]
pub fn template_pool_json(pool_id: &str) -> Option<&'static str> {
    get_template_pool_json(&canonicalize(pool_id))
}

/// 返回处理器列表的原始 JSON；未找到时返回 `None`。
#[must_use]
pub fn processor_list_json(id: &str) -> Option<&'static str> {
    get_processor_list_json(&canonicalize(id))
}

/// 返回某个池的元素模板 ID；未找到时返回 `None`。
///
/// 元素 ID 是完全限定的，可以直接传给 [`get_template`]。
#[must_use]
pub fn pool_elements(pool_id: &str) -> Option<&'static [&'static str]> {
    get_pool_elements(&canonicalize(pool_id))
}

///返回所有可加载模板名称的列表。
///
/// 这些是在编译期从内嵌的结构文件派生的。
/// 名称是完全限定的（例如 `minecraft:village/plains/houses/...`）。
/// 对命令的 Tab 补全很有用。
#[must_use]
#[allow(clippy::used_underscore_items)]
pub const fn all_template_names() -> &'static [&'static str] {
    _generated_all_template_names()
}

///返回所有可用结构名称的列表，用于 `/place structure` 的 Tab 补全。
#[must_use]
pub const fn all_structure_names() -> &'static [&'static str] {
    papokin_data::structures::StructureKeys::all_names()
}

///返回所有可用结构池名称的列表，用于 `/place jigsaw` 的 Tab 补全。
#[must_use]
#[allow(clippy::used_underscore_items)]
pub const fn all_pool_names() -> &'static [&'static str] {
    _generated_all_pool_names()
}

///返回内嵌结构模板的原始 NBT 字节。
#[must_use]
pub fn template_bytes(name: &str) -> Option<&'static [u8]> {
    get_template_bytes(&canonicalize(name))
}

#[must_use]
#[allow(clippy::used_underscore_items)]
pub const fn all_embedded_datapack_names() -> &'static [&'static str] {
    _generated_all_embedded_datapack_names()
}

#[cfg(test)]
mod tests {
    use papokin_nbt::compound::NbtCompound;
    use papokin_nbt::nbt_compress::write_gzip_compound_tag_to_bytes;
    use papokin_nbt::tag::NbtTag;

    use super::*;

    /// 构建最小的有效结构模板：单个 `minecraft:stone`
    /// 原点处的方块，序列化为 gzip 压缩的 NBT（`.nbt` 文件格式）。
    fn minimal_template_bytes() -> Vec<u8> {
        let mut root = NbtCompound::new();
        root.put_list("size", vec![NbtTag::Int(1), NbtTag::Int(1), NbtTag::Int(1)]);

        let mut palette_entry = NbtCompound::new();
        palette_entry.put_string("Name", "minecraft:stone".to_string());
        root.put_list("palette", vec![palette_entry.into()]);

        let mut block = NbtCompound::new();
        block.put_list("pos", vec![NbtTag::Int(0), NbtTag::Int(0), NbtTag::Int(0)]);
        block.put_int("state", 0);
        root.put_list("blocks", vec![block.into()]);

        write_gzip_compound_tag_to_bytes(root).expect("序列化模板失败")
    }

    #[test]
    fn register_template_makes_it_resolvable() {
        let cache = TemplateCache::new();
        let bytes = minimal_template_bytes();

        assert!(!cache.contains("papokin_test:mono_block"));
        let template = cache
            .register_template("papokin_test:mono_block", &bytes)
            .expect("注册必须成功");
        assert_eq!(template.size.x, 1);
        assert_eq!(template.size.y, 1);
        assert_eq!(template.size.z, 1);

        // 可通过查询和加载两条路径解析。
        assert!(cache.contains("papokin_test:mono_block"));
        let loaded = cache
            .get_or_error("papokin_test:mono_block")
            .expect("已注册的模板必须能加载");
        assert!(Arc::ptr_eq(&template, &loaded));
        assert!(
            cache
                .cached_names()
                .contains(&"papokin_test:mono_block".to_string())
        );
    }

    #[test]
    fn register_template_canonicalizes_bare_names() {
        let cache = TemplateCache::new();
        let bytes = minimal_template_bytes();

        cache
            .register_template("papokin_test_bare", &bytes)
            .expect("注册必须成功");
        // 裸名称落入原版命名空间，与 `get` 语义一致。
        assert!(cache.contains("minecraft:papokin_test_bare"));
        assert!(cache.get("papokin_test_bare").is_some());
    }

    #[test]
    fn register_template_replaces_existing_entry() {
        let cache = TemplateCache::new();
        let bytes = minimal_template_bytes();

        let first = cache
            .register_template("papokin_test:replace_me", &bytes)
            .expect("首次注册必须成功");
        let second = cache
            .register_template("papokin_test:replace_me", &bytes)
            .expect("第二次注册必须成功");
        assert!(!Arc::ptr_eq(&first, &second));
        let loaded = cache
            .get_or_error("papokin_test:replace_me")
            .expect("模板必须能加载");
        assert!(Arc::ptr_eq(&second, &loaded));
    }

    #[test]
    fn register_template_rejects_invalid_bytes() {
        let cache = TemplateCache::new();
        let result = cache.register_template("papokin_test:broken", b"not nbt at all");
        assert!(result.is_err());
        assert!(!cache.contains("papokin_test:broken"));
    }

    #[test]
    fn global_cache_register_and_list() {
        let bytes = minimal_template_bytes();
        register_template("papokin_test:global_mono_block", &bytes).expect("全局注册必须成功");

        assert!(has_template("papokin_test:global_mono_block"));
        assert!(get_template("papokin_test:global_mono_block").is_some());

        let names = list_template_names();
        assert!(names.contains(&"papokin_test:global_mono_block".to_string()));
        // 嵌入式模板也会与运行时注册的模板一并列出
        assert!(names.iter().any(|name| name.starts_with("minecraft:")));
        // 将注册名与内嵌名合并后无重复。
        let mut deduped = names.clone();
        deduped.dedup();
        assert_eq!(names, deduped);
    }
}
