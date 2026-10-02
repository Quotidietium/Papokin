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

    /// 用当前准入链（若有）包裹 `future`，使派生任务继承同一
    /// 因果链：链内的 Store 调用走重入路由，不再申请根准入
    /// （根信号量容量为 1，链持有期间重复申请必然自锁——例如
    /// 插件经宿主 API 动态加载新插件时，其初始化任务若申请
    /// 根准入，便会与正持有根并等待加载完成的调用链互相等待）。
    ///
    /// 上下文在调用时立即捕获，因此包裹后的 future 可安全送入
    /// 新任务；不在任何链中时是透明的直通包装。
    pub fn wrap_inherited<T>(
        self,
        future: impl Future<Output = T> + Send,
    ) -> impl Future<Output = T> + Send {
        let context = self.inherited_context();
        async move {
            if let Some(context) = context {
                scope(context, future).await
            } else {
                future.await
            }
        }
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
