mod chain;
mod executor;
mod lifecycle;
mod policy;
mod spawn;

pub use executor::{
    LegacyGuestScope, LegacyStore, StoreDataMut, StoreExecutor, StoreFuture, StoreHandle,
};
pub use lifecycle::{DriverError, DriverJoin, DriverState};
pub use policy::{LegacySyncReentry, StorePolicy};
pub use spawn::{RuntimeSpawner, SpawnError, SpawnFuture};

#[cfg(test)]
pub(crate) use chain::MAX_SYNC_REENTRY_DEPTH;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::{
        ops::Deref,
        sync::{
            Arc, Mutex, OnceLock,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
    };

    use tokio::{
        sync::Notify,
        time::{Duration, timeout},
    };
    use wasm_encoder::{
        CodeSection, ComponentBuilder, ComponentExportKind, ComponentTypeRef, ComponentValType,
        ConstExpr, EntityType, ExportKind, ExportSection, Function, FunctionSection, GlobalSection,
        GlobalType, ImportSection, Instruction, Module, ModuleArg, PrimitiveValType, TypeBounds,
        TypeSection, ValType,
    };
    use wasmtime::{
        Config, Engine, Store,
        component::{Component, Linker, Resource, ResourceType, TypedFunc},
    };

    use super::{
        DriverError, DriverState, LegacyStore, LegacySyncReentry, MAX_SYNC_REENTRY_DEPTH,
        RuntimeSpawner, SpawnError, SpawnFuture,
    };

    struct TestHostState;

    struct BoundaryResource;

    struct BoundaryState {
        resource_drops: Arc<AtomicUsize>,
        store_dropped: Arc<AtomicBool>,
    }

    impl Drop for BoundaryState {
        fn drop(&mut self) {
            self.store_dropped.store(true, Ordering::Release);
        }
    }

    struct TestSpawner {
        runtime: tokio::runtime::Handle,
    }

    impl RuntimeSpawner for TestSpawner {
        fn spawn(&self, task: SpawnFuture) -> Result<(), SpawnError> {
            drop(self.runtime.spawn(task));
            Ok(())
        }

        fn spawn_blocking(
            &self,
            task: Box<dyn FnOnce() + Send + 'static>,
        ) -> Result<(), SpawnError> {
            drop(self.runtime.spawn_blocking(task));
            Ok(())
        }
    }

    struct DroppingSpawner;

    impl RuntimeSpawner for DroppingSpawner {
        fn spawn(&self, task: SpawnFuture) -> Result<(), SpawnError> {
            drop(task);
            Ok(())
        }

        fn spawn_blocking(
            &self,
            task: Box<dyn FnOnce() + Send + 'static>,
        ) -> Result<(), SpawnError> {
            drop(task);
            Ok(())
        }
    }

    struct AbortRecordingSpawner {
        runtime: tokio::runtime::Handle,
        driver_abort: Arc<Mutex<Option<tokio::task::AbortHandle>>>,
    }

    impl RuntimeSpawner for AbortRecordingSpawner {
        fn spawn(&self, task: SpawnFuture) -> Result<(), SpawnError> {
            let task = self.runtime.spawn(task);
            let abort = task.abort_handle();
            drop(task);
            self.driver_abort.lock().expect("驱动中止锁").replace(abort);
            Ok(())
        }

        fn spawn_blocking(
            &self,
            task: Box<dyn FnOnce() + Send + 'static>,
        ) -> Result<(), SpawnError> {
            drop(self.runtime.spawn_blocking(task));
            Ok(())
        }
    }

    #[derive(Clone)]
    struct ConcurrentStore(Arc<LegacyStore<TestHostState>>);

    impl ConcurrentStore {
        async fn new(store: Store<TestHostState>) -> Self {
            Self::with_policy(store, LegacySyncReentry::new()).await
        }

        async fn with_policy(store: Store<TestHostState>, policy: LegacySyncReentry) -> Self {
            let spawner = Arc::new(TestSpawner {
                runtime: tokio::runtime::Handle::current(),
            });
            let executor = LegacyStore::start(store, policy, spawner)
                .await
                .expect("启动测试 Store 驱动");
            Self(Arc::new(executor))
        }
    }

    impl Deref for ConcurrentStore {
        type Target = LegacyStore<TestHostState>;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    struct BackgroundTask {
        started: Arc<Notify>,
        release: Arc<Notify>,
    }

    impl wasmtime::component::AccessorTask<TestHostState> for BackgroundTask {
        async fn run(
            self,
            _accessor: &wasmtime::component::Accessor<TestHostState>,
        ) -> wasmtime::Result<()> {
            self.started.notify_one();
            self.release.notified().await;
            Ok(())
        }
    }

    fn test_store() -> Store<TestHostState> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.wasm_component_model_async(true);
        config.concurrency_support(true);
        let engine = Engine::new(&config).expect("测试引擎");
        Store::new(&engine, TestHostState)
    }

    async fn legacy_store(
        store: Store<TestHostState>,
        policy: &LegacySyncReentry,
    ) -> ConcurrentStore {
        ConcurrentStore::with_policy(store, policy.clone()).await
    }

    fn assert_synchronous_reentry_depth_is_bounded_and_recovers() {
        let root = LegacySyncReentry::new().root_context();
        let mut context = root;
        for _ in 0..MAX_SYNC_REENTRY_DEPTH {
            context = context.child().expect("创建子重入上下文");
        }
        assert_eq!(context.depth, MAX_SYNC_REENTRY_DEPTH);

        let error = context.child().expect_err("超出限制一层应当被拒绝");
        assert!(error.to_string().contains("maximum depth"));
        assert!(root.child().is_ok());
    }

    #[allow(clippy::too_many_lines)]
    async fn root_admission_lifecycle_scenario() {
        let policy = LegacySyncReentry::new();
        let holding_store = legacy_store(test_store(), &policy).await;
        let first_store = legacy_store(test_store(), &policy).await;
        let second_store = legacy_store(test_store(), &policy).await;
        let third_store = legacy_store(test_store(), &policy).await;
        let holding_started = Arc::new(Notify::new());
        let release_holding = Arc::new(Notify::new());
        let holding_completed = Arc::new(Notify::new());
        let third_attempted = Arc::new(Notify::new());
        let third_started = Arc::new(Notify::new());
        let order = Arc::new(Mutex::new(Vec::new()));

        let holding_driver = holding_store.clone();
        let started = Arc::clone(&holding_started);
        let release = Arc::clone(&release_holding);
        let completed = Arc::clone(&holding_completed);
        let holding = tokio::spawn(async move {
            holding_driver
                .call_guest(move |_| {
                    Box::pin(async move {
                        started.notify_one();
                        release.notified().await;
                        completed.notify_one();
                        Ok(())
                    })
                })
                .await
        });
        holding_started.notified().await;
        holding.abort();
        assert!(
            holding
                .await
                .expect_err("持有根的等待者应当被取消")
                .is_cancelled()
        );

        let first_order = Arc::clone(&order);
        let first = first_store.call_guest(move |_| {
            Box::pin(async move {
                first_order.lock().expect("顺序锁").push(1);
                Ok(1)
            })
        });
        tokio::pin!(first);
        assert!(
            timeout(Duration::from_millis(10), &mut first)
                .await
                .is_err()
        );

        let second_order = Arc::clone(&order);
        let second = second_store.call_guest(move |_| {
            Box::pin(async move {
                second_order.lock().expect("顺序锁").push(2);
                Ok(2)
            })
        });
        tokio::pin!(second);
        assert!(
            timeout(Duration::from_millis(10), &mut second)
                .await
                .is_err()
        );

        let runtime = tokio::runtime::Handle::current();
        let third_driver = third_store.clone();
        let attempted = Arc::clone(&third_attempted);
        let entered = Arc::clone(&third_started);
        let third_order = Arc::clone(&order);
        let third = tokio::task::spawn_blocking(move || {
            attempted.notify_one();
            runtime.block_on(third_driver.call_guest(move |_| {
                Box::pin(async move {
                    entered.notify_one();
                    third_order.lock().expect("顺序锁").push(3);
                    Ok(3)
                })
            }))
        });
        third_attempted.notified().await;
        assert!(
            timeout(Duration::from_millis(100), third_started.notified())
                .await
                .is_err(),
            "来自另一个执行域的根请求在已接受的任务完成前进入了"
        );

        release_holding.notify_one();
        holding_completed.notified().await;
        assert_eq!(first.await.expect("第一个根"), 1);
        assert_eq!(second.await.expect("第二个根"), 2);
        assert_eq!(third.await.expect("第三个根任务").expect("第三个根结果"), 3);
        assert_eq!(*order.lock().expect("顺序锁"), [1, 2, 3]);

        holding_store
            .shutdown(|_| Box::pin(async move { Ok(()) }))
            .await
            .expect("关停持有的存储");
        first_store
            .shutdown(|_| Box::pin(async move { Ok(()) }))
            .await
            .expect("关停第一个存储");
        second_store
            .shutdown(|_| Box::pin(async move { Ok(()) }))
            .await
            .expect("关停第二个存储");
        third_store
            .shutdown(|_| Box::pin(async move { Ok(()) }))
            .await
            .expect("关停第三个存储");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn legacy_root_admission_is_fifo_and_cancellation_safe_across_domains() {
        timeout(Duration::from_secs(10), root_admission_lifecycle_scenario())
            .await
            .expect("根准入生命周期场景超时");
    }

    fn sync_reentry_component() -> Vec<u8> {
        let mut module = Module::new();
        let mut types = TypeSection::new();
        types.ty().function([], []);
        module.section(&types);

        let mut imports = ImportSection::new();
        imports.import("", "host", EntityType::Function(0));
        module.section(&imports);

        let mut functions = FunctionSection::new();
        functions.function(0);
        module.section(&functions);

        let mut exports = ExportSection::new();
        exports.export("run", ExportKind::Func, 1);
        module.section(&exports);

        let mut body = Function::new([]);
        body.instruction(&Instruction::Call(0));
        body.instruction(&Instruction::End);
        let mut code = CodeSection::new();
        code.function(&body);
        module.section(&code);

        let mut component = ComponentBuilder::default();
        let (function_type, mut function) = component.type_function(Some("run-type"));
        function
            .params([] as [(&str, PrimitiveValType); 0])
            .result(None);
        let imported = component.import("host", ComponentTypeRef::Func(function_type));
        let lowered = component.lower_func(Some("host-lowered"), imported, []);
        let module = component.core_module(Some("guest"), &module);
        let host_instance = component
            .core_instantiate_exports(Some("host-instance"), [("host", ExportKind::Func, lowered)]);
        let guest_instance = component.core_instantiate(
            Some("guest-instance"),
            module,
            [("", ModuleArg::Instance(host_instance))],
        );
        let run_core =
            component.core_alias_export(Some("run-core"), guest_instance, "run", ExportKind::Func);
        let run = component.lift_func(Some("run"), run_core, function_type, []);
        component.export("run", ComponentExportKind::Func, run, None);
        component.finish()
    }

    #[allow(clippy::too_many_lines)]
    fn owned_resource_component() -> Vec<u8> {
        let mut module = Module::new();
        let mut types = TypeSection::new();
        types.ty().function([ValType::I32], []);
        types.ty().function([ValType::I32], [ValType::I32]);
        types.ty().function([], []);
        module.section(&types);

        let mut imports = ImportSection::new();
        imports.import("", "resource-drop", EntityType::Function(0));
        module.section(&imports);

        let mut functions = FunctionSection::new();
        functions.function(0);
        functions.function(1);
        functions.function(0);
        functions.function(0);
        functions.function(2);
        module.section(&functions);

        let mut globals = GlobalSection::new();
        globals.global(
            GlobalType {
                val_type: ValType::I32,
                mutable: true,
                shared: false,
            },
            &ConstExpr::i32_const(0),
        );
        module.section(&globals);

        let mut exports = ExportSection::new();
        exports.export("drop", ExportKind::Func, 1);
        exports.export("return", ExportKind::Func, 2);
        exports.export("trap", ExportKind::Func, 3);
        exports.export("retain", ExportKind::Func, 4);
        exports.export("drop-retained", ExportKind::Func, 5);
        module.section(&exports);

        let mut drop_body = Function::new([]);
        drop_body.instruction(&Instruction::LocalGet(0));
        drop_body.instruction(&Instruction::Call(0));
        drop_body.instruction(&Instruction::End);
        let mut return_body = Function::new([]);
        return_body.instruction(&Instruction::LocalGet(0));
        return_body.instruction(&Instruction::End);
        let mut trap_body = Function::new([]);
        trap_body.instruction(&Instruction::Unreachable);
        trap_body.instruction(&Instruction::End);
        let mut retain_body = Function::new([]);
        retain_body.instruction(&Instruction::LocalGet(0));
        retain_body.instruction(&Instruction::GlobalSet(0));
        retain_body.instruction(&Instruction::End);
        let mut drop_retained_body = Function::new([]);
        drop_retained_body.instruction(&Instruction::GlobalGet(0));
        drop_retained_body.instruction(&Instruction::Call(0));
        drop_retained_body.instruction(&Instruction::End);
        let mut code = CodeSection::new();
        code.function(&drop_body);
        code.function(&return_body);
        code.function(&trap_body);
        code.function(&retain_body);
        code.function(&drop_retained_body);
        module.section(&code);

        let mut component = ComponentBuilder::default();
        let resource =
            component.import("resource", ComponentTypeRef::Type(TypeBounds::SubResource));
        let resource_drop = component.resource_drop(resource);
        let (owned_resource, owned) = component.type_defined(Some("owned-resource"));
        owned.own(resource);

        let (drop_type, mut drop_signature) = component.type_function(Some("drop-type"));
        drop_signature
            .params([("resource", ComponentValType::Type(owned_resource))])
            .result(None);
        let (return_type, mut return_signature) = component.type_function(Some("return-type"));
        return_signature
            .params([("resource", ComponentValType::Type(owned_resource))])
            .result(Some(ComponentValType::Type(owned_resource)));
        let (drop_retained_type, mut drop_retained_signature) =
            component.type_function(Some("drop-retained-type"));
        drop_retained_signature
            .params([] as [(&str, PrimitiveValType); 0])
            .result(None);

        let module = component.core_module(Some("guest"), &module);
        let intrinsics = component.core_instantiate_exports(
            Some("intrinsics"),
            [("resource-drop", ExportKind::Func, resource_drop)],
        );
        let instance = component.core_instantiate(
            Some("guest-instance"),
            module,
            [("", ModuleArg::Instance(intrinsics))],
        );
        let drop_core =
            component.core_alias_export(Some("drop-core"), instance, "drop", ExportKind::Func);
        let return_core =
            component.core_alias_export(Some("return-core"), instance, "return", ExportKind::Func);
        let trap_core =
            component.core_alias_export(Some("trap-core"), instance, "trap", ExportKind::Func);
        let retain_core =
            component.core_alias_export(Some("retain-core"), instance, "retain", ExportKind::Func);
        let drop_retained_core = component.core_alias_export(
            Some("drop-retained-core"),
            instance,
            "drop-retained",
            ExportKind::Func,
        );
        let drop_resource = component.lift_func(Some("drop"), drop_core, drop_type, []);
        let return_resource = component.lift_func(Some("return"), return_core, return_type, []);
        let trap = component.lift_func(Some("trap"), trap_core, drop_type, []);
        let retain = component.lift_func(Some("retain"), retain_core, drop_type, []);
        let drop_retained = component.lift_func(
            Some("drop-retained"),
            drop_retained_core,
            drop_retained_type,
            [],
        );
        component.export("drop", ComponentExportKind::Func, drop_resource, None);
        component.export("return", ComponentExportKind::Func, return_resource, None);
        component.export("trap", ComponentExportKind::Func, trap, None);
        component.export("retain", ComponentExportKind::Func, retain, None);
        component.export(
            "drop-retained",
            ComponentExportKind::Func,
            drop_retained,
            None,
        );
        component.finish()
    }

    async fn shutdown_drains_accepted_calls_and_rejects_new_work() {
        let store = ConcurrentStore::new(test_store()).await;
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());

        let call_store = store.clone();
        let call_started = Arc::clone(&started);
        let call_release = Arc::clone(&release);
        let active_call = tokio::spawn(async move {
            call_store
                .call(move |_| {
                    Box::pin(async move {
                        call_started.notify_one();
                        call_release.notified().await;
                        Ok(7u8)
                    })
                })
                .await
        });
        started.notified().await;

        let shutdown_store = store.clone();
        let shutdown = tokio::spawn(async move {
            shutdown_store
                .shutdown(|_| Box::pin(async move { Ok("unloaded") }))
                .await
        });
        while matches!(store.state(), DriverState::Accepting) {
            tokio::task::yield_now().await;
        }

        let rejected = store
            .call(|_| Box::pin(async move { Ok(()) }))
            .await
            .expect_err("关停期间新任务应当被拒绝");
        assert!(rejected.to_string().contains("shutting down"));
        assert!(!shutdown.is_finished());

        release.notify_one();
        assert_eq!(active_call.await.expect("活动调用任务").unwrap(), 7);
        assert_eq!(shutdown.await.expect("关停任务").unwrap(), "unloaded");
        assert_eq!(store.state(), DriverState::Stopped);
    }

    async fn shutdown_waits_for_joined_store_background_tasks() {
        let store = ConcurrentStore::new(test_store()).await;
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let task_started = Arc::clone(&started);
        let task_release = Arc::clone(&release);

        let call_store = store.clone();
        let active_call = tokio::spawn(async move {
            call_store
                .call(move |accessor| {
                    let spawned = accessor.spawn(BackgroundTask {
                        started: task_started,
                        release: task_release,
                    });
                    Box::pin(async move {
                        spawned?.await;
                        Ok(())
                    })
                })
                .await
        });
        started.notified().await;

        let shutdown_store = store.clone();
        let shutdown = tokio::spawn(async move {
            shutdown_store
                .shutdown(|_| Box::pin(async move { Ok(()) }))
                .await
        });
        while matches!(store.state(), DriverState::Accepting) {
            tokio::task::yield_now().await;
        }
        assert!(!shutdown.is_finished());

        release.notify_one();
        active_call
            .await
            .expect("活动调用任务")
            .expect("活动调用结果");
        shutdown.await.expect("关停任务").expect("关停结果");
    }

    async fn explicit_discard_drains_accepted_work() {
        let spawner = Arc::new(TestSpawner {
            runtime: tokio::runtime::Handle::current(),
        });
        let executor = LegacyStore::start(test_store(), LegacySyncReentry::new(), spawner)
            .await
            .expect("启动测试 Store 驱动");
        let handle = executor.handle();
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());

        let call_handle = handle.clone();
        let call_started = Arc::clone(&started);
        let call_release = Arc::clone(&release);
        let accepted = tokio::spawn(async move {
            call_handle
                .call(move |_| {
                    Box::pin(async move {
                        call_started.notify_one();
                        call_release.notified().await;
                        Ok(11u8)
                    })
                })
                .await
        });
        started.notified().await;

        executor.discard();
        let rejected = handle
            .call(|_| Box::pin(async move { Ok(()) }))
            .await
            .expect_err("丢弃应当关闭准入");
        assert!(rejected.to_string().contains("shutting down"));

        release.notify_one();
        assert_eq!(accepted.await.expect("已接受的调用任务").unwrap(), 11);

        let mut join = handle.driver_join();
        join.wait().await.expect("显式丢弃应当干净地停止驱动");
        assert_eq!(handle.state(), DriverState::Stopped);
    }

    async fn owner_drop_drains_accepted_work() {
        let spawner = Arc::new(TestSpawner {
            runtime: tokio::runtime::Handle::current(),
        });
        let executor = LegacyStore::start(test_store(), LegacySyncReentry::new(), spawner)
            .await
            .expect("启动测试 Store 驱动");
        let handle = executor.handle();
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());

        let call_handle = handle.clone();
        let call_started = Arc::clone(&started);
        let call_release = Arc::clone(&release);
        let accepted = tokio::spawn(async move {
            call_handle
                .call(move |_| {
                    Box::pin(async move {
                        call_started.notify_one();
                        call_release.notified().await;
                        Ok(11u8)
                    })
                })
                .await
        });
        started.notified().await;

        drop(executor);
        let rejected = handle
            .call(|_| Box::pin(async move { Ok(()) }))
            .await
            .expect_err("owner 被 drop 时应当关闭准入");
        assert!(rejected.to_string().contains("shutting down"));

        release.notify_one();
        assert_eq!(accepted.await.expect("已接受的调用任务").unwrap(), 11);

        let mut join = handle.driver_join();
        let error = join
            .wait()
            .await
            .expect_err("owner 被 drop 时应当显式使驱动失败");
        assert!(
            error.to_string().contains("control was dropped"),
            "意外的驱动错误：{error}"
        );

        let executor = LegacyStore::start(
            test_store(),
            LegacySyncReentry::new(),
            Arc::new(TestSpawner {
                runtime: tokio::runtime::Handle::current(),
            }),
        )
        .await
        .expect("启动测试 Store 驱动");
        let mut join = executor.driver_join();
        drop(executor);
        let error = join
            .wait()
            .await
            .expect_err("drop 唯一的 owner 应当显式使驱动失败");
        assert!(
            error.to_string().contains("control was dropped"),
            "意外的驱动错误：{error}"
        );
    }

    /// 针对经由以下内容的普通同步 WIT 递归的验收测试
    /// 调用作用域的重入泵。
    async fn sync_lifted_same_instance_reentry_completes() {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.wasm_component_model_async(true);
        config.concurrency_support(true);
        let engine = Engine::new(&config).expect("测试引擎");
        let component = Component::new(&engine, sync_reentry_component()).expect("测试组件");

        let run_slot = Arc::new(OnceLock::<TypedFunc<(), ()>>::new());
        let driver_slot = Arc::new(OnceLock::<ConcurrentStore>::new());
        let host_calls = Arc::new(AtomicUsize::new(0));

        let mut linker = Linker::<TestHostState>::new(&engine);
        let run_for_host = Arc::clone(&run_slot);
        let driver_for_host = Arc::clone(&driver_slot);
        let calls_for_host = Arc::clone(&host_calls);
        linker
            .root()
            .func_wrap_async("host", move |mut store, ()| {
                let run = *run_for_host.get().expect("运行已初始化");
                let driver = driver_for_host.get().expect("存储驱动已初始化").clone();
                let host_calls = Arc::clone(&calls_for_host);
                Box::new(async move {
                    if host_calls.fetch_add(1, Ordering::SeqCst) == 0 {
                        let nested_driver = driver.clone();
                        driver
                            .pump_reentry(
                                &mut store,
                                nested_driver.call_guest(move |mut context| {
                                    assert!(
                                        context.is_reentrant(),
                                        "同实例回调必须使用当前活动的 store 帧"
                                    );
                                    Box::pin(async move { context.call(run, ()).await })
                                }),
                            )
                            .await??;
                    }
                    Ok(())
                })
            })
            .expect("链接宿主函数");

        let mut store = Store::new(&engine, TestHostState);
        let instance = linker
            .instantiate_async(&mut store, &component)
            .await
            .expect("实例化测试组件");
        let run = instance
            .get_typed_func::<(), ()>(&mut store, "run")
            .expect("运行导出");
        assert!(run_slot.set(run).is_ok());

        let driver = ConcurrentStore::new(store).await;
        assert!(driver_slot.set(driver.clone()).is_ok());

        timeout(
            Duration::from_secs(2),
            driver.call_guest(move |mut context| {
                assert!(!context.is_reentrant(), "顶层调用必须使用并发 store 路径");
                Box::pin(async move { context.call(run, ()).await })
            }),
        )
        .await
        .expect("同步提升的 guest 重入超时")
        .expect("同步提升的 guest 重入失败");

        assert_eq!(host_calls.load(Ordering::SeqCst), 2);
        driver
            .shutdown(|_| Box::pin(async move { Ok(()) }))
            .await
            .expect("关停测试存储");
    }

    /// 对应从某个上下文中调用 `fire_blocking` 的同步游戏方法
    /// 阻塞工作线程，同时宿主导入让活动 store 保持可泵送。
    async fn blocking_host_operation_propagates_the_reentry_chain() {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.wasm_component_model_async(true);
        config.concurrency_support(true);
        let engine = Engine::new(&config).expect("测试引擎");
        let component = Component::new(&engine, sync_reentry_component()).expect("测试组件");

        let run_slot = Arc::new(OnceLock::<TypedFunc<(), ()>>::new());
        let driver_slot = Arc::new(OnceLock::<ConcurrentStore>::new());
        let host_calls = Arc::new(AtomicUsize::new(0));

        let mut linker = Linker::<TestHostState>::new(&engine);
        let run_for_host = Arc::clone(&run_slot);
        let driver_for_host = Arc::clone(&driver_slot);
        let calls_for_host = Arc::clone(&host_calls);
        linker
            .root()
            .func_wrap_async("host", move |mut store, ()| {
                let run = *run_for_host.get().expect("运行已初始化");
                let driver = driver_for_host.get().expect("存储驱动已初始化").clone();
                let host_calls = Arc::clone(&calls_for_host);
                Box::new(async move {
                    if host_calls.fetch_add(1, Ordering::SeqCst) == 0 {
                        let runtime = tokio::runtime::Handle::current();
                        let nested_driver = driver.clone();
                        driver
                            .pump_blocking(&mut store, move || {
                                tokio::task::block_in_place(|| {
                                    runtime.block_on(nested_driver.call_guest(
                                        move |mut context| {
                                            assert!(
                                                context.is_reentrant(),
                                                "阻塞回调必须使用当前活动的 store 帧"
                                            );
                                            Box::pin(async move { context.call(run, ()).await })
                                        },
                                    ))
                                })
                            })
                            .await??;
                    }
                    Ok(())
                })
            })
            .expect("链接宿主函数");

        let mut store = Store::new(&engine, TestHostState);
        let instance = linker
            .instantiate_async(&mut store, &component)
            .await
            .expect("实例化测试组件");
        let run = instance
            .get_typed_func::<(), ()>(&mut store, "run")
            .expect("运行导出");
        assert!(run_slot.set(run).is_ok());

        let driver = ConcurrentStore::new(store).await;
        assert!(driver_slot.set(driver.clone()).is_ok());

        timeout(
            Duration::from_secs(2),
            driver.call_guest(move |mut context| {
                Box::pin(async move { context.call(run, ()).await })
            }),
        )
        .await
        .expect("阻塞式同步提升的 guest 重入超时")
        .expect("阻塞式同步提升的 guest 重入失败");

        assert_eq!(host_calls.load(Ordering::SeqCst), 2);
        driver
            .shutdown(|_| Box::pin(async move { Ok(()) }))
            .await
            .expect("关停测试存储");
    }

    async fn unrelated_guest_call_waits_for_the_active_chain() {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.wasm_component_model_async(true);
        config.concurrency_support(true);
        let engine = Engine::new(&config).expect("测试引擎");
        let component = Component::new(&engine, sync_reentry_component()).expect("测试组件");

        let driver_slot = Arc::new(OnceLock::<ConcurrentStore>::new());
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let unrelated_entered = Arc::new(Notify::new());
        let host_calls = Arc::new(AtomicUsize::new(0));

        let mut linker = Linker::<TestHostState>::new(&engine);
        let driver_for_host = Arc::clone(&driver_slot);
        let entered_for_host = Arc::clone(&entered);
        let release_for_host = Arc::clone(&release);
        let unrelated_for_host = Arc::clone(&unrelated_entered);
        let calls_for_host = Arc::clone(&host_calls);
        linker
            .root()
            .func_wrap_async("host", move |mut store, ()| {
                let driver = driver_for_host.get().expect("存储驱动已初始化").clone();
                let entered = Arc::clone(&entered_for_host);
                let release = Arc::clone(&release_for_host);
                let unrelated_entered = Arc::clone(&unrelated_for_host);
                let host_calls = Arc::clone(&calls_for_host);
                Box::new(async move {
                    if host_calls.fetch_add(1, Ordering::SeqCst) == 0 {
                        entered.notify_one();
                        driver.pump_reentry(&mut store, release.notified()).await?;
                    } else {
                        unrelated_entered.notify_one();
                    }
                    Ok(())
                })
            })
            .expect("链接宿主函数");

        let mut store = Store::new(&engine, TestHostState);
        let instance = linker
            .instantiate_async(&mut store, &component)
            .await
            .expect("实例化测试组件");
        let run = instance
            .get_typed_func::<(), ()>(&mut store, "run")
            .expect("运行导出");
        let driver = ConcurrentStore::new(store).await;
        assert!(driver_slot.set(driver.clone()).is_ok());

        let outer_driver = driver.clone();
        let outer = tokio::spawn(async move {
            outer_driver
                .call_guest(move |mut context| Box::pin(async move { context.call(run, ()).await }))
                .await
        });
        entered.notified().await;

        let unrelated_driver = driver.clone();
        let unrelated = tokio::spawn(async move {
            unrelated_driver
                .call_guest(move |mut context| Box::pin(async move { context.call(run, ()).await }))
                .await
        });

        assert!(
            timeout(Duration::from_millis(100), unrelated_entered.notified())
                .await
                .is_err(),
            "一个无关的根调用被路由进了活动的重入作用域"
        );

        release.notify_one();
        outer.await.expect("外部任务").expect("外部客户调用");
        unrelated.await.expect("无关任务").expect("无关的访客调用");
        assert_eq!(host_calls.load(Ordering::SeqCst), 2);

        driver
            .shutdown(|_| Box::pin(async move { Ok(()) }))
            .await
            .expect("关停测试存储");
    }

    #[allow(clippy::too_many_lines)]
    async fn shutdown_reentry_scenario() {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.wasm_component_model_async(true);
        config.concurrency_support(true);
        let engine = Engine::new(&config).expect("测试引擎");
        let component = Component::new(&engine, sync_reentry_component()).expect("测试组件");

        let run_slot = Arc::new(OnceLock::<TypedFunc<(), ()>>::new());
        let driver_slot = Arc::new(OnceLock::<ConcurrentStore>::new());
        let entered = Arc::new(Notify::new());
        let start_callback = Arc::new(Notify::new());
        let callback_completed = Arc::new(Notify::new());
        let host_calls = Arc::new(AtomicUsize::new(0));
        let waiting_root_ran = Arc::new(AtomicBool::new(false));

        let mut linker = Linker::<TestHostState>::new(&engine);
        let run_for_host = Arc::clone(&run_slot);
        let driver_for_host = Arc::clone(&driver_slot);
        let entered_for_host = Arc::clone(&entered);
        let start_for_host = Arc::clone(&start_callback);
        let completed_for_host = Arc::clone(&callback_completed);
        let calls_for_host = Arc::clone(&host_calls);
        linker
            .root()
            .func_wrap_async("host", move |mut store, ()| {
                let run = *run_for_host.get().expect("运行已初始化");
                let driver = driver_for_host.get().expect("存储驱动已初始化").clone();
                let entered = Arc::clone(&entered_for_host);
                let start_callback = Arc::clone(&start_for_host);
                let callback_completed = Arc::clone(&completed_for_host);
                let host_calls = Arc::clone(&calls_for_host);
                Box::new(async move {
                    if host_calls.fetch_add(1, Ordering::SeqCst) == 0 {
                        entered.notify_one();
                        let callback_driver = driver.clone();
                        let outbound = async move {
                            start_callback.notified().await;
                            callback_driver
                                .call_guest(move |mut context| {
                                    Box::pin(async move { context.call(run, ()).await })
                                })
                                .await
                        };
                        driver.pump_reentry(&mut store, outbound).await??;
                    } else {
                        callback_completed.notify_one();
                    }
                    Ok(())
                })
            })
            .expect("链接宿主函数");

        let mut store = Store::new(&engine, TestHostState);
        let instance = linker
            .instantiate_async(&mut store, &component)
            .await
            .expect("实例化测试组件");
        let run = instance
            .get_typed_func::<(), ()>(&mut store, "run")
            .expect("运行导出");
        assert!(run_slot.set(run).is_ok());
        let driver = ConcurrentStore::new(store).await;
        assert!(driver_slot.set(driver.clone()).is_ok());

        let outer_driver = driver.clone();
        let outer = tokio::spawn(async move {
            outer_driver
                .call_guest(move |mut context| Box::pin(async move { context.call(run, ()).await }))
                .await
        });
        entered.notified().await;

        let shutdown_driver = driver.clone();
        let shutdown = shutdown_driver.shutdown(|_| Box::pin(async move { Ok(()) }));
        tokio::pin!(shutdown);
        assert!(
            timeout(Duration::from_millis(10), &mut shutdown)
                .await
                .is_err()
        );
        assert_eq!(driver.state(), DriverState::Accepting);

        let root_ran = Arc::clone(&waiting_root_ran);
        let waiting_root = driver.call_guest(move |_| {
            Box::pin(async move {
                root_ran.store(true, Ordering::Release);
                Ok(())
            })
        });
        tokio::pin!(waiting_root);
        assert!(
            timeout(Duration::from_millis(10), &mut waiting_root)
                .await
                .is_err()
        );

        start_callback.notify_one();
        timeout(Duration::from_secs(2), callback_completed.notified())
            .await
            .expect("回调在关停期间被拒绝");
        outer.await.expect("外部任务").expect("外部客户调用");
        shutdown.await.expect("关停存储");
        let error = waiting_root
            .await
            .expect_err("排在关停之后的根请求应当被拒绝");
        assert!(error.to_string().contains("shutting down"), "{error}");
        assert!(!waiting_root_ran.load(Ordering::Acquire));
        assert_eq!(host_calls.load(Ordering::SeqCst), 2);
    }

    #[allow(clippy::too_many_lines)]
    async fn opposing_plugin_roots_scenario() {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.wasm_component_model_async(true);
        config.concurrency_support(true);
        let engine = Engine::new(&config).expect("测试引擎");
        let component = Component::new(&engine, sync_reentry_component()).expect("测试组件");

        let a_run_slot = Arc::new(OnceLock::<TypedFunc<(), ()>>::new());
        let b_run_slot = Arc::new(OnceLock::<TypedFunc<(), ()>>::new());
        let a_driver_slot = Arc::new(OnceLock::<ConcurrentStore>::new());
        let b_driver_slot = Arc::new(OnceLock::<ConcurrentStore>::new());
        let a_calls = Arc::new(AtomicUsize::new(0));
        let b_calls = Arc::new(AtomicUsize::new(0));
        let trace = Arc::new(Mutex::new(Vec::new()));
        let first_root_entered = Arc::new(Notify::new());
        let release_first_root = Arc::new(Notify::new());
        let second_root_started = Arc::new(Notify::new());

        let mut a_linker = Linker::<TestHostState>::new(&engine);
        let a_driver_for_a = Arc::clone(&a_driver_slot);
        let b_driver_for_a = Arc::clone(&b_driver_slot);
        let b_run_for_a = Arc::clone(&b_run_slot);
        let a_calls_for_host = Arc::clone(&a_calls);
        let trace_for_a = Arc::clone(&trace);
        let entered_for_a = Arc::clone(&first_root_entered);
        let release_for_a = Arc::clone(&release_first_root);
        a_linker
            .root()
            .func_wrap_async("host", move |mut store, ()| {
                let a_driver = a_driver_for_a.get().expect("A 存储驱动已初始化").clone();
                let b_driver = b_driver_for_a.get().expect("B 存储驱动已初始化").clone();
                let b_run = *b_run_for_a.get().expect("B 运行已初始化");
                let calls = Arc::clone(&a_calls_for_host);
                let trace = Arc::clone(&trace_for_a);
                let entered = Arc::clone(&entered_for_a);
                let release = Arc::clone(&release_for_a);
                Box::new(async move {
                    match calls.fetch_add(1, Ordering::SeqCst) {
                        0 => {
                            trace.lock().expect("追踪锁").push("a:first-outer");
                            entered.notify_one();
                            a_driver
                                .pump_reentry(&mut store, release.notified())
                                .await?;
                            let outbound = b_driver.call_guest(move |mut context| {
                                assert!(
                                    !context.is_reentrant(),
                                    "首次调用 B 必须使用其并发 store 路径"
                                );
                                Box::pin(async move { context.call(b_run, ()).await })
                            });
                            a_driver.pump_reentry(&mut store, outbound).await??;
                        }
                        1 => trace.lock().expect("追踪锁").push("a:first-inner"),
                        2 => {
                            trace.lock().expect("追踪锁").push("a:second-middle");
                            let outbound = b_driver.call_guest(move |mut context| {
                                assert!(
                                    context.is_reentrant(),
                                    "A 到 B 的回调必须使用 B 的活动 store 帧"
                                );
                                Box::pin(async move { context.call(b_run, ()).await })
                            });
                            a_driver.pump_reentry(&mut store, outbound).await??;
                        }
                        _ => return Err(wasmtime::Error::msg("unexpected extra call into A")),
                    }
                    Ok(())
                })
            })
            .expect("链接 A 宿主函数");

        let mut b_linker = Linker::<TestHostState>::new(&engine);
        let a_driver_for_b = Arc::clone(&a_driver_slot);
        let b_driver_for_b = Arc::clone(&b_driver_slot);
        let a_run_for_b = Arc::clone(&a_run_slot);
        let b_calls_for_host = Arc::clone(&b_calls);
        let trace_for_b = Arc::clone(&trace);
        b_linker
            .root()
            .func_wrap_async("host", move |mut store, ()| {
                let a_driver = a_driver_for_b.get().expect("A 存储驱动已初始化").clone();
                let b_driver = b_driver_for_b.get().expect("B 存储驱动已初始化").clone();
                let a_run = *a_run_for_b.get().expect("A 运行已初始化");
                let calls = Arc::clone(&b_calls_for_host);
                let trace = Arc::clone(&trace_for_b);
                Box::new(async move {
                    match calls.fetch_add(1, Ordering::SeqCst) {
                        0 => {
                            trace.lock().expect("追踪锁").push("b:first-middle");
                            let outbound = a_driver.call_guest(move |mut context| {
                                assert!(
                                    context.is_reentrant(),
                                    "B 到 A 的回调必须使用 A 的活动 store 帧"
                                );
                                Box::pin(async move { context.call(a_run, ()).await })
                            });
                            b_driver.pump_reentry(&mut store, outbound).await??;
                        }
                        1 => {
                            trace.lock().expect("追踪锁").push("b:second-outer");
                            let outbound = a_driver.call_guest(move |mut context| {
                                assert!(
                                    !context.is_reentrant(),
                                    "第二个根首次调用 A 必须使用其并发 store 路径"
                                );
                                Box::pin(async move { context.call(a_run, ()).await })
                            });
                            b_driver.pump_reentry(&mut store, outbound).await??;
                        }
                        2 => trace.lock().expect("追踪锁").push("b:second-inner"),
                        _ => return Err(wasmtime::Error::msg("unexpected extra call into B")),
                    }
                    Ok(())
                })
            })
            .expect("链接 B 宿主函数");

        let mut a_store = Store::new(&engine, TestHostState);
        let a_instance = a_linker
            .instantiate_async(&mut a_store, &component)
            .await
            .expect("实例化 A");
        let a_run = a_instance
            .get_typed_func::<(), ()>(&mut a_store, "run")
            .expect("获取 A 运行导出");
        assert!(a_run_slot.set(a_run).is_ok());

        let mut b_store = Store::new(&engine, TestHostState);
        let b_instance = b_linker
            .instantiate_async(&mut b_store, &component)
            .await
            .expect("实例化 B");
        let b_run = b_instance
            .get_typed_func::<(), ()>(&mut b_store, "run")
            .expect("获取 B 运行导出");
        assert!(b_run_slot.set(b_run).is_ok());

        let policy = LegacySyncReentry::new();
        let a_driver = legacy_store(a_store, &policy).await;
        let b_driver = legacy_store(b_store, &policy).await;
        assert!(a_driver_slot.set(a_driver.clone()).is_ok());
        assert!(b_driver_slot.set(b_driver.clone()).is_ok());

        let first_driver = a_driver.clone();
        let first_root = tokio::spawn(async move {
            first_driver
                .call_guest(move |mut context| {
                    assert!(
                        !context.is_reentrant(),
                        "顶层调用 A 必须使用其并发 store 路径"
                    );
                    Box::pin(async move { context.call(a_run, ()).await })
                })
                .await
        });
        first_root_entered.notified().await;

        let started = Arc::clone(&second_root_started);
        let second_root = b_driver.call_guest(move |mut context| {
            Box::pin(async move {
                started.notify_one();
                assert!(
                    !context.is_reentrant(),
                    "顶层调用 B 必须使用其并发 store 路径"
                );
                context.call(b_run, ()).await
            })
        });
        tokio::pin!(second_root);
        assert!(
            timeout(Duration::from_millis(10), &mut second_root)
                .await
                .is_err()
        );
        assert!(
            timeout(Duration::from_millis(100), second_root_started.notified())
                .await
                .is_err(),
            "第一个根仍持有准入时，对向的根进入了 B"
        );

        release_first_root.notify_one();
        timeout(Duration::from_secs(2), async {
            first_root
                .await
                .expect("第一个根任务")
                .expect("第一个根结果");
            second_root.await.expect("第二个根结果");
        })
        .await
        .expect("对向的同步根请求超时");

        assert_eq!(a_calls.load(Ordering::SeqCst), 3);
        assert_eq!(b_calls.load(Ordering::SeqCst), 3);
        assert_eq!(
            *trace.lock().expect("追踪锁"),
            [
                "a:first-outer",
                "b:first-middle",
                "a:first-inner",
                "b:second-outer",
                "a:second-middle",
                "b:second-inner"
            ]
        );
        a_driver
            .shutdown(|_| Box::pin(async move { Ok(()) }))
            .await
            .expect("关停 A 存储");
        b_driver
            .shutdown(|_| Box::pin(async move { Ok(()) }))
            .await
            .expect("关停 B 存储");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn legacy_reentry_routes_complete() {
        assert_synchronous_reentry_depth_is_bounded_and_recovers();
        timeout(Duration::from_secs(10), async {
            sync_lifted_same_instance_reentry_completes().await;
            blocking_host_operation_propagates_the_reentry_chain().await;
            unrelated_guest_call_waits_for_the_active_chain().await;
            opposing_plugin_roots_scenario().await;
        })
        .await
        .expect("旧版重入路由场景超时");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shutdown_drains_accepted_work() {
        timeout(Duration::from_secs(10), async {
            shutdown_drains_accepted_calls_and_rejects_new_work().await;
            shutdown_waits_for_joined_store_background_tasks().await;
            explicit_discard_drains_accepted_work().await;
            owner_drop_drains_accepted_work().await;
            shutdown_reentry_scenario().await;
        })
        .await
        .expect("Store 关停场景超时");
    }

    async fn startup_task_loss_is_reported() {
        let started = LegacyStore::start(
            test_store(),
            LegacySyncReentry::new(),
            Arc::new(DroppingSpawner),
        )
        .await;
        let Err(error) = started else {
            panic!("已丢弃的驱动任务不应启动");
        };
        assert!(error.to_string().contains("ended without reporting"));
    }

    async fn active_driver_task_loss_is_reported() {
        let driver_abort = Arc::new(Mutex::new(None));
        let spawner = Arc::new(AbortRecordingSpawner {
            runtime: tokio::runtime::Handle::current(),
            driver_abort: Arc::clone(&driver_abort),
        });
        let executor = LegacyStore::start(test_store(), LegacySyncReentry::new(), spawner)
            .await
            .expect("启动测试 Store 驱动");
        let handle = executor.handle();
        let started = Arc::new(Notify::new());
        let never_release = Arc::new(Notify::new());

        let call_handle = handle.clone();
        let call_started = Arc::clone(&started);
        let call_release = Arc::clone(&never_release);
        let active = tokio::spawn(async move {
            call_handle
                .call(move |_| {
                    Box::pin(async move {
                        call_started.notify_one();
                        call_release.notified().await;
                        Ok(())
                    })
                })
                .await
        });
        started.notified().await;

        driver_abort
            .lock()
            .expect("驱动中止锁")
            .take()
            .expect("驱动中止句柄")
            .abort();
        let call_error = active
            .await
            .expect("活动调用任务")
            .expect_err("驱动取消应当使进行中的调用失败");
        assert!(call_error.to_string().contains("ended without reporting"));

        let retained = match executor.state() {
            DriverState::Failed(error) => error,
            state => panic!("预期驱动处于失败状态，实际得到：{state:?}"),
        };
        assert!(retained.to_string().contains("ended without reporting"));
        let mut join = handle.driver_join();
        assert_eq!(join.wait().await.expect_err("驱动 join 应当失败"), retained);
    }

    async fn driver_panic_is_reported() {
        let store = ConcurrentStore::new(test_store()).await;
        let handle = store.handle();
        let call_error = timeout(
            Duration::from_secs(2),
            handle.call_guest::<(), _>(|_| panic!("故意制造的 Store 驱动 panic")),
        )
        .await
        .expect("panic 的 Store 驱动调用超时")
        .expect_err("panic 的 Store 驱动调用应当失败");
        assert!(call_error.to_string().contains("ended without reporting"));

        let retained = match store.state() {
            DriverState::Failed(error) => error,
            state => panic!("预期驱动处于失败状态，实际得到：{state:?}"),
        };
        let mut join = store.driver_join();
        assert_eq!(join.wait().await.expect_err("驱动 join 应当失败"), retained);
        let later_error = handle
            .call(|_| Box::pin(async move { Ok(()) }))
            .await
            .expect_err("后续调用应当保留该驱动错误");
        assert!(later_error.to_string().contains("ended without reporting"));
    }

    #[allow(clippy::too_many_lines)]
    async fn caught_reentrant_guest_trap_stops_driver_and_queued_work() {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.wasm_component_model_async(true);
        config.concurrency_support(true);
        let engine = Engine::new(&config).expect("测试引擎");
        let component = Component::new(&engine, sync_reentry_component()).expect("测试组件");
        let run_slot = Arc::new(OnceLock::<TypedFunc<(), ()>>::new());
        let driver_slot = Arc::new(OnceLock::<ConcurrentStore>::new());
        let host_calls = Arc::new(AtomicUsize::new(0));
        let queued_call_ran = Arc::new(AtomicBool::new(false));

        let mut linker = Linker::<TestHostState>::new(&engine);
        let run_for_host = Arc::clone(&run_slot);
        let driver_for_host = Arc::clone(&driver_slot);
        let calls_for_host = Arc::clone(&host_calls);
        let queued_for_host = Arc::clone(&queued_call_ran);
        linker
            .root()
            .func_wrap_async("host", move |mut store, ()| {
                let run = *run_for_host.get().expect("运行已初始化");
                let driver = driver_for_host.get().expect("存储驱动已初始化").clone();
                let host_calls = Arc::clone(&calls_for_host);
                let queued_call_ran = Arc::clone(&queued_for_host);
                Box::new(async move {
                    if host_calls.fetch_add(1, Ordering::SeqCst) != 0 {
                        return Err(wasmtime::Error::msg("intentional reentrant guest failure"));
                    }

                    let first_driver = driver.clone();
                    let second_driver = driver.clone();
                    let queued = async move {
                        tokio::join!(
                            biased;
                            first_driver.call_guest(move |mut guest| {
                                Box::pin(async move { guest.call(run, ()).await })
                            }),
                            second_driver.call_guest(move |_| {
                                queued_call_ran.store(true, Ordering::Release);
                                Box::pin(async move { Ok(()) })
                            }),
                        )
                    };
                    assert!(driver.pump_reentry(&mut store, queued).await.is_err());
                    Ok(())
                })
            })
            .expect("链接宿主函数");

        let mut store = Store::new(&engine, TestHostState);
        let instance = linker
            .instantiate_async(&mut store, &component)
            .await
            .expect("实例化测试组件");
        let run = instance
            .get_typed_func::<(), ()>(&mut store, "run")
            .expect("运行导出");
        assert!(run_slot.set(run).is_ok());
        let driver = ConcurrentStore::new(store).await;
        assert!(driver_slot.set(driver.clone()).is_ok());

        let error = timeout(
            Duration::from_secs(2),
            driver.call_guest(move |mut guest| Box::pin(async move { guest.call(run, ()).await })),
        )
        .await
        .expect("重入 guest 陷阱超时")
        .expect_err("重入 guest 调用应当失败");
        assert!(error.to_string().contains("wasm backtrace"), "{error}");

        let mut join = driver.driver_join();
        let retained = join.wait().await.expect_err("驱动应当失败");
        assert!(retained.to_string().contains("wasm backtrace"));
        assert!(!queued_call_ran.load(Ordering::Acquire));
        assert_eq!(host_calls.load(Ordering::SeqCst), 2);
    }

    #[allow(clippy::too_many_lines)]
    async fn owned_resource_lifecycle_retires_store_on_trap() {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.wasm_component_model_async(true);
        config.concurrency_support(true);
        let engine = Engine::new(&config).expect("测试引擎");
        let component = Component::new(&engine, owned_resource_component()).expect("测试组件");
        let resource_drops = Arc::new(AtomicUsize::new(0));
        let store_dropped = Arc::new(AtomicBool::new(false));
        let mut linker = Linker::<BoundaryState>::new(&engine);
        linker
            .root()
            .resource(
                "resource",
                ResourceType::host::<BoundaryResource>(),
                |store, rep| {
                    assert!(matches!(rep, 11 | 22 | 33 | 44));
                    store.data().resource_drops.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                },
            )
            .expect("链接宿主资源");
        let mut store = Store::new(
            &engine,
            BoundaryState {
                resource_drops: Arc::clone(&resource_drops),
                store_dropped: Arc::clone(&store_dropped),
            },
        );
        let instance = linker
            .instantiate_async(&mut store, &component)
            .await
            .expect("实例化测试组件");
        let drop_resource = instance
            .get_typed_func::<(Resource<BoundaryResource>,), ()>(&mut store, "drop")
            .expect("drop 导出");
        let return_resource = instance
            .get_typed_func::<(Resource<BoundaryResource>,), (Resource<BoundaryResource>,)>(
                &mut store, "return",
            )
            .expect("return 导出");
        let trap = instance
            .get_typed_func::<(Resource<BoundaryResource>,), ()>(&mut store, "trap")
            .expect("陷阱导出");
        let retain = instance
            .get_typed_func::<(Resource<BoundaryResource>,), ()>(&mut store, "retain")
            .expect("retain 导出");
        let drop_retained = instance
            .get_typed_func::<(), ()>(&mut store, "drop-retained")
            .expect("drop-retained 导出");
        let driver = LegacyStore::start(
            store,
            LegacySyncReentry::new(),
            Arc::new(TestSpawner {
                runtime: tokio::runtime::Handle::current(),
            }),
        )
        .await
        .expect("启动测试 Store 驱动");
        let handle = driver.handle();

        let setup_error = driver
            .call_guest(|_| {
                Box::pin(async move { Err::<(), _>(wasmtime::Error::msg("setup failed")) })
            })
            .await
            .expect_err("调用前 setup 应当失败");
        assert!(setup_error.to_string().contains("setup failed"));
        assert_eq!(driver.state(), DriverState::Accepting);
        driver
            .call_guest(|_| Box::pin(async move { Ok(()) }))
            .await
            .expect("调用前 setup 失败应当不影响 store 可用");

        driver
            .call_guest(move |mut context| {
                Box::pin(async move { context.call(drop_resource, (Resource::new_own(11),)).await })
            })
            .await
            .expect("guest 应当能 drop 拥有的资源");
        assert_eq!(resource_drops.load(Ordering::SeqCst), 1);

        let (returned,) = driver
            .call_guest(move |mut context| {
                Box::pin(async move {
                    context
                        .call(return_resource, (Resource::new_own(22),))
                        .await
                })
            })
            .await
            .expect("guest 应当能返回拥有的资源");
        assert_eq!(returned.rep(), 22);
        assert!(returned.owned());
        assert_eq!(resource_drops.load(Ordering::SeqCst), 1);

        driver
            .call_guest(move |mut context| {
                Box::pin(async move { context.call(drop_resource, (returned,)).await })
            })
            .await
            .expect("返回的所有权应当可以 drop");
        assert_eq!(resource_drops.load(Ordering::SeqCst), 2);

        driver
            .call_guest(move |mut context| {
                Box::pin(async move { context.call(retain, (Resource::new_own(44),)).await })
            })
            .await
            .expect("guest 应当能跨调用保留所有权");
        assert_eq!(resource_drops.load(Ordering::SeqCst), 2);
        driver
            .call_guest(move |mut context| {
                Box::pin(async move { context.call(drop_retained, ()).await })
            })
            .await
            .expect("guest 应当能稍后 drop 保留的所有权");
        assert_eq!(resource_drops.load(Ordering::SeqCst), 3);

        let call_error = driver
            .call_guest(move |mut context| {
                Box::pin(async move { context.call(trap, (Resource::new_own(33),)).await })
            })
            .await
            .expect_err("guest 调用应当失败");
        assert!(
            call_error.to_string().contains("wasm backtrace"),
            "{call_error}"
        );

        let later_call_ran = Arc::new(AtomicBool::new(false));
        let ran = Arc::clone(&later_call_ran);
        let immediate_error = handle
            .call_guest(move |_| {
                ran.store(true, Ordering::Release);
                Box::pin(async move { Ok(()) })
            })
            .await
            .expect_err("guest 陷阱之后的调用应当被拒绝");
        assert!(
            immediate_error.to_string().contains("guest call")
                || immediate_error.to_string().contains("wasm backtrace"),
            "{immediate_error}"
        );
        assert!(!later_call_ran.load(Ordering::Acquire));

        let mut join = driver.driver_join();
        let retained = join.wait().await.expect_err("驱动应当失败");
        assert!(
            retained.to_string().contains("wasm backtrace"),
            "{retained}"
        );
        // 陷入（trap）不会为 guest 持有的句柄运行析构函数。它
        // 当失败的 Store 被丢弃时，其后端宿主数据会被释放。
        assert!(store_dropped.load(Ordering::Acquire));
        assert_eq!(resource_drops.load(Ordering::SeqCst), 3);
        assert_eq!(driver.state(), DriverState::Failed(Arc::clone(&retained)));

        let later_error = handle
            .call(|_| Box::pin(async move { Ok(()) }))
            .await
            .expect_err("后续调用应当保留该 guest 错误");
        assert!(
            later_error.to_string().contains("wasm backtrace"),
            "{later_error}"
        );
    }

    fn shutdown_does_not_hide_terminal_failure() {
        let error = super::executor::reconcile_shutdown_result(
            Ok(Ok("finalized")),
            Err(Arc::new(DriverError::new("late driver failure"))),
        )
        .expect_err("驱动的终止性失败应当覆盖 finalizer 的成功");
        assert!(error.to_string().contains("driver failed"), "{error}");
        assert!(error.to_string().contains("late driver failure"), "{error}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn terminal_failure_is_retained() {
        startup_task_loss_is_reported().await;
        active_driver_task_loss_is_reported().await;
        driver_panic_is_reported().await;
        caught_reentrant_guest_trap_stops_driver_and_queued_work().await;
        owned_resource_lifecycle_retires_store_on_trap().await;
        shutdown_does_not_hide_terminal_failure();

        let store = ConcurrentStore::new(test_store()).await;
        let handle = store.handle();
        let shutdown_error = store
            .shutdown(|_| {
                Box::pin(async move { Err::<(), _>(wasmtime::Error::msg("finalizer failed")) })
            })
            .await
            .expect_err("finalizer 失败应当使关停失败");
        assert!(shutdown_error.to_string().contains("finalizer failed"));

        let retained = match store.state() {
            DriverState::Failed(error) => error,
            state => panic!("预期驱动处于失败状态，实际得到：{state:?}"),
        };
        assert!(retained.to_string().contains("finalizer failed"));

        let mut join = store.driver_join();
        let joined = join.wait().await.expect_err("驱动 join 应当失败");
        assert_eq!(joined, retained);

        let call_error = handle
            .call(|_| Box::pin(async move { Ok(()) }))
            .await
            .expect_err("之后的调用应当保留该驱动错误");
        assert!(call_error.to_string().contains("finalizer failed"));
    }
}
