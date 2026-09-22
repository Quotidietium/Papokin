use std::{
    any::Any,
    sync::{Arc, LazyLock},
};

use libloading::Library;

use crate::plugin::{
    PLUGIN_API_VERSION,
    loader::{PluginLoadFuture, PluginUnloadFuture},
};

use super::{LoaderError, Path, Plugin, PluginLoader, PluginMetadata};

pub struct NativePluginLoader;

impl PluginLoader for NativePluginLoader {
    fn load<'a>(&'a self, path: &'a Path) -> PluginLoadFuture<'a> {
        Box::pin(async {
            let path = path.to_owned();

            // SAFETY: 从服务器管理员配置的路径加载动态库。
            let library = unsafe { Library::new(&path) }
                .map_err(|e| LoaderError::LibraryLoad(e.to_string()))?;

            // 确保此插件构建时使用了兼容的 Pumpkin 插件 API 版本
            // SAFETY: `PUMPKIN_API_VERSION` 是由 `#[plugin_impl]` 创建的导出常量符号 `u32`。
            let plugin_api_version = unsafe {
                match library.get::<*const u32>(b"PUMPKIN_API_VERSION") {
                    Ok(symbol) => **symbol,
                    Err(_) => return Err(LoaderError::ApiVersionMissing),
                }
            };

            if plugin_api_version != PLUGIN_API_VERSION {
                return Err(LoaderError::ApiVersionMismatch {
                    plugin_version: plugin_api_version,
                    server_version: PLUGIN_API_VERSION,
                });
            }

            // 2. 提取元数据（METADATA）
            // `#[plugin_impl]` 将其导出为 `LazyLock`，因为 `PluginMetadata`
            // 拥有自己的字符串，无法在常量中构建。
            // SAFETY: `METADATA` 是由 `#[plugin_impl]` 创建的导出符号 `LazyLock<PluginMetadata>`。
            let metadata = unsafe {
                let metadata = library
                    .get::<*const LazyLock<PluginMetadata>>(b"METADATA")
                    .map_err(|_| LoaderError::MetadataMissing)?;
                (**metadata).clone()
            };

            // 3. 提取插件工厂（plugin）
            // SAFETY: `plugin` 是由 `#[plugin_impl]` 创建的签名 `fn() -> Box<dyn Plugin>` 的导出构造函数符号。
            let plugin_factory = unsafe {
                library
                    .get::<fn() -> Box<dyn Plugin>>(b"plugin")
                    .map_err(|_| LoaderError::EntrypointMissing)?
            };

            Ok((
                Arc::from(plugin_factory()),
                metadata,
                Box::new(library) as Box<dyn Any + Send + Sync>,
            ))
        })
    }

    fn can_load(&self, path: &Path) -> bool {
        let ext = path.extension().unwrap_or_default();

        if cfg!(target_os = "windows") {
            ext.eq_ignore_ascii_case("dll")
        } else if cfg!(target_os = "macos") {
            ext.eq_ignore_ascii_case("dylib")
        } else {
            ext.eq_ignore_ascii_case("so")
        }
    }

    fn unload(&self, data: Box<dyn Any + Send + Sync>) -> PluginUnloadFuture<'_> {
        Box::pin(async {
            data.downcast::<Library>()
                .map_or(Err(LoaderError::InvalidLoaderData), |library| {
                    drop(library);
                    Ok(())
                })
        })
    }

    /// Windows 特有的问题：Windows 会锁定 DLL，因此我们必须表明它们无法被卸载。
    fn can_unload(&self) -> bool {
        !cfg!(target_os = "windows")
    }
}
