use std::future::Future;

use crate::chain::{ReentryContext, RootAdmission, RootAdmissionGuard, scope};

mod sealed {
    pub trait StorePolicy {}
}

#[doc(hidden)]
pub trait StorePolicy: sealed::StorePolicy + Clone + Send + Sync + 'static {
    const NAME: &'static str;
}

/// 序列化同步的 guest 根，同时允许来自当前活跃
/// 因果链重新进入已在该链上的 store。
#[derive(Clone, Debug)]
pub struct LegacySyncReentry {
    root_admission: RootAdmission,
}

impl LegacySyncReentry {
    pub const NAME: &'static str = "LegacySyncReentry";

    /// 创建一个准入授权机构。克隆此策略并在所有……之间共享
    /// 可参与同一同步插件调用图的 Store。
    #[must_use]
    pub fn new() -> Self {
        Self {
            root_admission: RootAdmission::new(),
        }
    }

    #[must_use]
    pub(crate) fn inherited_context(&self) -> Option<ReentryContext> {
        ReentryContext::current().filter(|context| self.root_admission.admits(*context))
    }

    pub(crate) async fn acquire_root(&self) -> wasmtime::Result<RootAdmissionGuard> {
        self.root_admission.acquire_root().await
    }

    #[cfg(test)]
    #[must_use]
    pub(crate) fn root_context(&self) -> ReentryContext {
        self.root_admission.root_context()
    }

    /// 在相同的准入权限下运行 Store 引导工作，该权限也被
    /// 后续访客调用。
    pub async fn scope_bootstrap<T>(
        &self,
        future: impl Future<Output = wasmtime::Result<T>>,
    ) -> wasmtime::Result<T> {
        if let Some(context) = self.inherited_context() {
            tracing::trace!(
                wasm_plugin_admission_id = context.admission_id,
                wasm_plugin_chain_id = context.chain_id,
                wasm_plugin_reentry_depth = context.depth,
                "继承了旧版 Wasm 插件根准入"
            );
            return scope(context, future).await;
        }

        let admission = self.acquire_root().await?;
        let context = admission.context();
        let output = scope(context, future).await;
        drop(admission);
        output
    }
}

impl Default for LegacySyncReentry {
    fn default() -> Self {
        Self::new()
    }
}

impl sealed::StorePolicy for LegacySyncReentry {}

impl StorePolicy for LegacySyncReentry {
    const NAME: &'static str = Self::NAME;
}
