//! Template caching for embedded structure templates.
//!
//! This module provides a lazy-loading cache for structure templates that are
//! embedded in the binary at compile time using `include_bytes!`.

use std::sync::Arc;

use dashmap::DashMap;

use super::{StructureTemplate, structure_template::TemplateError};

/// Vanilla's implicit namespace.
const DEFAULT_NAMESPACE: &str = "minecraft";

/// Canonicalizes a resource id to fully-qualified `namespace:path` form.
///
/// A bare `foo` becomes `minecraft:foo`, matching vanilla resolution.
fn canonicalize(name: &str) -> String {
    if name.contains(':') {
        name.to_owned()
    } else {
        format!("{DEFAULT_NAMESPACE}:{name}")
    }
}

/// A cache for loaded structure templates.
///
/// Templates are loaded lazily on first access and stored for reuse.
/// Keys are fully-qualified resource ids, so `foo` and `minecraft:foo`
/// share a single entry.
/// The cache is thread-safe and can be accessed from multiple threads.
pub struct TemplateCache {
    cache: DashMap<String, Arc<StructureTemplate>>,
}

impl Default for TemplateCache {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateCache {
    /// Creates a new empty template cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: DashMap::new(),
        }
    }

    /// Gets a template by `name`, loading it from embedded resources if not cached.
    ///
    /// `name` may be bare (`foo`) or namespaced (`minecraft:foo`, `pumpkin:foo`).
    ///
    /// Returns the loaded template wrapped in an `Arc`, or `None` if the template
    /// doesn't exist or failed to load.
    pub fn get(&self, name: &str) -> Option<Arc<StructureTemplate>> {
        match self.get_or_error(name) {
            Ok(template) => Some(template),
            Err(TemplateError::MissingField("template file not found")) => None,
            Err(e) => {
                tracing::error!("Failed to load template '{}': {}", name, e);
                None
            }
        }
    }

    /// Gets a template by name, returning an error if loading fails.
    ///
    /// # Errors
    ///
    /// Returns an error if the template doesn't exist or fails to parse.
    pub fn get_or_error(&self, name: &str) -> Result<Arc<StructureTemplate>, TemplateError> {
        let key = canonicalize(name);

        // Check cache first
        if let Some(template) = self.cache.get(&key) {
            return Ok(Arc::clone(&template));
        }

        // Try to load the template
        let bytes = Self::load_template_bytes(&key)
            .ok_or(TemplateError::MissingField("template file not found"))?;

        let template = StructureTemplate::from_nbt_bytes(bytes)?;
        let arc = Arc::new(template);
        self.cache.insert(key, Arc::clone(&arc));
        Ok(arc)
    }

    /// Preloads a list of templates into the cache.
    ///
    /// This can be useful during server startup to avoid loading delays
    /// during gameplay.
    pub fn preload(&self, names: &[&str]) {
        for name in names {
            if let Err(e) = self.get_or_error(name) {
                tracing::warn!("Failed to preload template '{}': {}", name, e);
            }
        }
    }

    /// Registers a template at runtime from raw gzipped NBT bytes (the vanilla
    /// `.nbt` structure format).
    ///
    /// `name` may be bare (`foo`) or namespaced (`pumpkin:foo`); it is
    /// canonicalized the same way as [`Self::get`]. Registering under a name
    /// that already exists — embedded or previously registered — replaces it,
    /// because lookups consult the cache before the embedded resources.
    ///
    /// # Errors
    ///
    /// Returns an error if `nbt_bytes` fails to decompress or parse as a
    /// structure template. On error the cache is left unchanged.
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

    /// Returns whether a template with `name` can be resolved, either from the
    /// cache (loaded or runtime-registered) or from the embedded resources.
    ///
    /// Unlike [`Self::get`] this never parses or caches an embedded template.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        let key = canonicalize(name);
        self.cache.contains_key(&key) || Self::load_template_bytes(&key).is_some()
    }

    /// Returns the names of all templates currently held in the cache.
    ///
    /// This covers both runtime-registered templates and embedded templates
    /// that have already been loaded; embedded templates not yet accessed are
    /// listed by [`all_template_names`] instead.
    #[must_use]
    pub fn cached_names(&self) -> Vec<String> {
        self.cache.iter().map(|entry| entry.key().clone()).collect()
    }

    /// Returns the number of cached templates.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Returns whether the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Clears all cached templates.
    pub fn clear(&self) {
        self.cache.clear();
    }

    /// Loads raw template bytes from embedded resources.
    fn load_template_bytes(path: &str) -> Option<&'static [u8]> {
        get_template_bytes(path)
    }
}

include!(concat!(env!("OUT_DIR"), "/template_embeddings.rs"));

/// Global template cache instance.
///
/// This provides a singleton cache that can be used throughout the codebase
/// without needing to pass around a cache reference.
static GLOBAL_CACHE: std::sync::LazyLock<TemplateCache> =
    std::sync::LazyLock::new(TemplateCache::new);

/// Gets the global template cache.
#[must_use]
pub fn global_cache() -> &'static TemplateCache {
    &GLOBAL_CACHE
}

/// Gets a template by `name` from the global cache.
///
/// Returns the loaded template wrapped in an `Arc`, or `None` if not found.
#[must_use]
pub fn get_template(name: &str) -> Option<Arc<StructureTemplate>> {
    global_cache().get(name)
}

/// Registers a template at runtime on the global cache from raw gzipped NBT
/// bytes (the vanilla `.nbt` structure format).
///
/// This is the same cache the `/place template` command resolves names
/// against, so a template registered here is immediately placeable by both
/// the command and the plugin API. `name` may be bare (`foo`) or namespaced
/// (`my_plugin:foo`); registering under an existing name replaces it.
///
/// # Errors
///
/// Returns an error if `nbt_bytes` fails to decompress or parse as a
/// structure template.
pub fn register_template(
    name: &str,
    nbt_bytes: &[u8],
) -> Result<Arc<StructureTemplate>, TemplateError> {
    global_cache().register_template(name, nbt_bytes)
}

/// Returns whether a template with `name` can be resolved from the global
/// cache, covering runtime-registered, already-loaded, and embedded templates.
#[must_use]
pub fn has_template(name: &str) -> bool {
    global_cache().contains(name)
}

/// Lists all template names known to the global cache: every embedded
/// template plus every runtime-registered one, deduplicated and sorted.
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

/// Returns the raw JSON for a template pool, or `None` if not found.
#[must_use]
pub fn template_pool_json(pool_id: &str) -> Option<&'static str> {
    get_template_pool_json(&canonicalize(pool_id))
}

/// Returns the raw JSON for a processor list, or `None` if not found.
#[must_use]
pub fn processor_list_json(id: &str) -> Option<&'static str> {
    get_processor_list_json(&canonicalize(id))
}

/// Returns the element template ids for a pool, or `None` if not found.
///
/// Element ids are fully qualified and can be passed directly to [`get_template`].
#[must_use]
pub fn pool_elements(pool_id: &str) -> Option<&'static [&'static str]> {
    get_pool_elements(&canonicalize(pool_id))
}

/// Returns a list of all available template names that can be loaded.
///
/// These are derived from the embedded structure files at compile time.
/// Names are fully qualified (e.g. `minecraft:village/plains/houses/...`).
/// Useful for tab-completion in commands.
#[must_use]
#[allow(clippy::used_underscore_items)]
pub const fn all_template_names() -> &'static [&'static str] {
    _generated_all_template_names()
}

/// Returns a list of all available structure names for `/place structure` tab-completion.
#[must_use]
pub const fn all_structure_names() -> &'static [&'static str] {
    pumpkin_data::structures::StructureKeys::all_names()
}

/// Returns a list of all available pool names for `/place jigsaw` tab-completion.
#[must_use]
#[allow(clippy::used_underscore_items)]
pub const fn all_pool_names() -> &'static [&'static str] {
    _generated_all_pool_names()
}

/// Returns raw NBT bytes for an embedded structure template.
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
    use pumpkin_nbt::compound::NbtCompound;
    use pumpkin_nbt::nbt_compress::write_gzip_compound_tag_to_bytes;
    use pumpkin_nbt::tag::NbtTag;

    use super::*;

    /// Builds the smallest valid structure template: a single `minecraft:stone`
    /// block at the origin, serialized as gzipped NBT (the `.nbt` file format).
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

        write_gzip_compound_tag_to_bytes(root).expect("failed to serialize template")
    }

    #[test]
    fn register_template_makes_it_resolvable() {
        let cache = TemplateCache::new();
        let bytes = minimal_template_bytes();

        assert!(!cache.contains("pumpkin_test:mono_block"));
        let template = cache
            .register_template("pumpkin_test:mono_block", &bytes)
            .expect("registration must succeed");
        assert_eq!(template.size.x, 1);
        assert_eq!(template.size.y, 1);
        assert_eq!(template.size.z, 1);

        // Resolvable through both the query and the loading paths.
        assert!(cache.contains("pumpkin_test:mono_block"));
        let loaded = cache
            .get_or_error("pumpkin_test:mono_block")
            .expect("registered template must load");
        assert!(Arc::ptr_eq(&template, &loaded));
        assert!(
            cache
                .cached_names()
                .contains(&"pumpkin_test:mono_block".to_string())
        );
    }

    #[test]
    fn register_template_canonicalizes_bare_names() {
        let cache = TemplateCache::new();
        let bytes = minimal_template_bytes();

        cache
            .register_template("pumpkin_test_bare", &bytes)
            .expect("registration must succeed");
        // A bare name lands in the vanilla namespace, matching `get` semantics.
        assert!(cache.contains("minecraft:pumpkin_test_bare"));
        assert!(cache.get("pumpkin_test_bare").is_some());
    }

    #[test]
    fn register_template_replaces_existing_entry() {
        let cache = TemplateCache::new();
        let bytes = minimal_template_bytes();

        let first = cache
            .register_template("pumpkin_test:replace_me", &bytes)
            .expect("first registration must succeed");
        let second = cache
            .register_template("pumpkin_test:replace_me", &bytes)
            .expect("second registration must succeed");
        assert!(!Arc::ptr_eq(&first, &second));
        let loaded = cache
            .get_or_error("pumpkin_test:replace_me")
            .expect("template must load");
        assert!(Arc::ptr_eq(&second, &loaded));
    }

    #[test]
    fn register_template_rejects_invalid_bytes() {
        let cache = TemplateCache::new();
        let result = cache.register_template("pumpkin_test:broken", b"not nbt at all");
        assert!(result.is_err());
        assert!(!cache.contains("pumpkin_test:broken"));
    }

    #[test]
    fn global_cache_register_and_list() {
        let bytes = minimal_template_bytes();
        register_template("pumpkin_test:global_mono_block", &bytes)
            .expect("global registration must succeed");

        assert!(has_template("pumpkin_test:global_mono_block"));
        assert!(get_template("pumpkin_test:global_mono_block").is_some());

        let names = list_template_names();
        assert!(names.contains(&"pumpkin_test:global_mono_block".to_string()));
        // Embedded templates are listed too, next to runtime-registered ones.
        assert!(names.iter().any(|name| name.starts_with("minecraft:")));
        // No duplicates after merging registered names with embedded ones.
        let mut deduped = names.clone();
        deduped.dedup();
        assert_eq!(names, deduped);
    }
}
