mod state;

pub use state::GameTestState;

use std::sync::Arc;
use std::time::Instant;

use papokin_util::math::position::BlockPos;

use crate::block_based::BlockBasedTest;
use crate::error::{GameTestError, GameTestResult};
use crate::model::GameTestRotation;
use crate::structure::{
    GameTestPosition, GameTestStructureTemplate, TestBlockMode, TestStructureInstance,
    clear_success_entities, encase_structure, place_structure_with_controller_rotation,
    remove_barriers,
};
use crate::world::GameTestWorld;

enum RunningEvaluation {
    Continue,
    Passed,
    Failed(GameTestError),
}

pub struct GameTestSession {
    pub test: BlockBasedTest,
    pub state: GameTestState,
    pub placement: Option<TestStructureInstance>,
    world: Arc<dyn GameTestWorld>,
    template: Arc<GameTestStructureTemplate>,
    extra_rotation: GameTestRotation,
    effective_rotation: GameTestRotation,
    test_x: i32,
    test_y: Option<i32>,
    test_z: i32,
    chunks_loaded: bool,
    started_at: Option<Instant>,
}

impl GameTestSession {
    #[must_use]
    pub fn new(
        test: BlockBasedTest,
        world: Arc<dyn GameTestWorld>,
        template: Arc<GameTestStructureTemplate>,
        test_x: i32,
        test_z: i32,
    ) -> Self {
        Self::new_with_extra_rotation(
            test,
            world,
            template,
            test_x,
            test_z,
            GameTestRotation::None,
        )
    }

    #[must_use]
    pub fn new_with_extra_rotation(
        test: BlockBasedTest,
        world: Arc<dyn GameTestWorld>,
        template: Arc<GameTestStructureTemplate>,
        test_x: i32,
        test_z: i32,
        extra_rotation: GameTestRotation,
    ) -> Self {
        let effective_rotation = test.rotation().then(extra_rotation);
        Self {
            test,
            state: GameTestState::Queued,
            placement: None,
            world,
            template,
            extra_rotation,
            effective_rotation,
            test_x,
            test_y: None,
            test_z,
            chunks_loaded: false,
            started_at: None,
        }
    }

    /// 创建等价于原版 `GameTestInfo::copyReset()` 的内容。
    ///
    /// 重跑是一个新的执行对象，而不是把已完成的运行改回 Queued 状态。
    /// 保留控制器坐标与解析出的 Y，以便替换
    /// 原地准备就绪，而所有按次尝试的状态和放置句柄都是全新的。
    #[must_use]
    pub fn copy_reset(&self) -> Self {
        Self {
            test: self.test.clone(),
            state: GameTestState::Queued,
            placement: None,
            world: self.world.clone(),
            template: self.template.clone(),
            extra_rotation: self.extra_rotation,
            effective_rotation: self.effective_rotation,
            test_x: self.test_x,
            test_y: self.test_y,
            test_z: self.test_z,
            chunks_loaded: false,
            started_at: None,
        }
    }

    #[must_use]
    pub(crate) fn run_time_ms(&self) -> u128 {
        self.started_at
            .as_ref()
            .map_or(0, |started_at| started_at.elapsed().as_millis())
    }

    pub async fn tick(&mut self) {
        if self.state.is_finished() {
            return;
        }

        // 将当前状态移出，以便状态转换可以自由借用 `self`
        // 跨异步调用进行，而无需持有对 `self.state` 的借用。
        let state = std::mem::replace(&mut self.state, GameTestState::Queued);
        match state {
            GameTestState::Queued => self.tick_queued().await,
            GameTestState::SettingUp { elapsed_ticks } => self.tick_setup(elapsed_ticks).await,
            GameTestState::Running { elapsed_ticks } => self.tick_running(elapsed_ticks).await,
            finished @ (GameTestState::Passed { .. } | GameTestState::Failed { .. }) => {
                self.state = finished;
            }
        }
    }

    async fn tick_queued(&mut self) {
        let placement = place_structure_with_controller_rotation(
            self.world.as_ref(),
            &self.template,
            self.test.id(),
            self.effective_rotation,
            self.extra_rotation,
            GameTestPosition::new(self.test_x, self.test_y, self.test_z),
            self.test.definition().padding,
        )
        .await;

        match placement {
            Ok(placement) => {
                if let Err(error) = encase_structure(
                    self.world.as_ref(),
                    &placement,
                    self.test.definition().sky_access,
                )
                .await
                {
                    self.finish_failure(0, error, None).await;
                    return;
                }

                self.test_y = Some(placement.test_instance_pos().0.y);
                self.placement = Some(placement);
                // 原版的 StructureSpawner 调用 startExecution(1)，因此即使某个测试
                // 零准备刻的测试会等到下一个服务器刻才开始。
                self.state = GameTestState::SettingUp { elapsed_ticks: 0 };
            }
            Err(error) => self.finish_failure(0, error, None).await,
        }
    }

    async fn tick_setup(&mut self, elapsed_ticks: u32) {
        // GameTestInfo::tick 在所有相交区块……之前不会推进 tickCount
        // 放置的结构确实已加载并正在跑刻。该检查是一次性的
        // 每次尝试一次，与原版的 chunksLoaded 标志完全一致。
        if !self.chunks_loaded {
            let Some(placement) = &self.placement else {
                self.finish_failure(
                    0,
                    GameTestError::World(
                        "GameTest is ticking without a placed structure".to_string(),
                    ),
                    None,
                )
                .await;
                return;
            };
            let origin = placement.origin();
            let size = placement.size();
            let max = BlockPos::new(
                origin.0.x + size[0],
                origin.0.y + size[1],
                origin.0.z + size[2],
            );
            if !self.world.test_area_loaded_and_ticking(origin, &max).await {
                self.state = GameTestState::SettingUp { elapsed_ticks };
                return;
            }
            self.chunks_loaded = true;
        }

        let elapsed_ticks = elapsed_ticks.saturating_add(1);
        if elapsed_ticks <= self.test.setup_ticks() {
            self.state = GameTestState::SettingUp { elapsed_ticks };
            return;
        }

        match self.begin_running(0).await {
            Ok(()) if self.test.max_ticks() > 0 => self.evaluate_test_tick(0).await,
            Ok(()) => self.state = GameTestState::Running { elapsed_ticks: 0 },
            Err(error) => self.finish_failure(0, error, None).await,
        }
    }

    async fn tick_running(&mut self, elapsed_ticks: u32) {
        let tick = elapsed_ticks.saturating_add(1);
        if tick > self.test.max_ticks() {
            self.finish_failure(
                tick,
                GameTestError::Timeout {
                    max_ticks: self.test.max_ticks(),
                },
                None,
            )
            .await;
            return;
        }

        // BlockBasedTestInstance 为半开区间安装 onEachTick
        // [0, timeoutTicks)，因此 timeoutTicks 本身没有 ACCEPT/FAIL/LOG 检查。
        if tick == self.test.max_ticks() {
            self.state = GameTestState::Running {
                elapsed_ticks: tick,
            };
            return;
        }

        self.evaluate_test_tick(tick).await;
    }

    async fn evaluate_test_tick(&mut self, tick: u32) {
        match self.evaluate_running(tick).await {
            Ok(RunningEvaluation::Passed) => self.handle_attempt_pass(tick).await,
            Ok(RunningEvaluation::Failed(error)) | Err(error) => {
                let marker = assertion_marker(&error);
                self.finish_failure(tick, error, marker).await;
            }
            Ok(RunningEvaluation::Continue) => {
                self.state = GameTestState::Running {
                    elapsed_ticks: tick,
                };
            }
        }
    }

    async fn begin_running(&mut self, tick: u32) -> GameTestResult<()> {
        // 原版在调用之前立即启动 GameTestInfo 的秒表
        // 测试体，因此结构放置与准备刻不计入运行时间。
        self.started_at.get_or_insert_with(Instant::now);

        let start_blocks = self.test_block_positions(TestBlockMode::Start);
        if start_blocks.is_empty() {
            return Err(GameTestError::Assertion {
                tick,
                position: None,
                message: "missing START test block".to_string(),
            });
        }
        if start_blocks.len() != 1 {
            return Err(GameTestError::Assertion {
                tick,
                position: None,
                message: format!(
                    "expected exactly one START test block, found {}",
                    start_blocks.len()
                ),
            });
        }

        if let Some(placement) = &self.placement {
            // GameTestInfo.startTest 紧接在前将控制器标记为 RUNNING
            // 调用 BlockBasedTestInstance.run，这将触发 START。
            self.world
                .set_test_instance_running(placement.test_instance_pos())
                .await?;
        }
        self.world.trigger_test_block(&start_blocks[0]).await
    }

    async fn evaluate_running(&self, tick: u32) -> GameTestResult<RunningEvaluation> {
        let accept_blocks = self.test_block_positions(TestBlockMode::Accept);
        if accept_blocks.is_empty() {
            return Ok(RunningEvaluation::Failed(GameTestError::Assertion {
                tick,
                position: None,
                message: "missing ACCEPT test block".to_string(),
            }));
        }

        // 原版先检查 ACCEPT 再检查 FAIL；若两者同刻触发，ACCEPT 优先。
        for position in &accept_blocks {
            if self.world.test_block_triggered(position).await? {
                return Ok(RunningEvaluation::Passed);
            }
        }

        for position in self.test_block_positions(TestBlockMode::Fail) {
            if self.world.test_block_triggered(&position).await? {
                let message = self.world.test_block_message(&position).await?;
                return Ok(RunningEvaluation::Failed(GameTestError::Assertion {
                    tick,
                    position: Some(position),
                    message,
                }));
            }
        }

        for position in self.test_block_positions(TestBlockMode::Log) {
            if self.world.test_block_triggered(&position).await? {
                self.world.trigger_test_block(&position).await?;
                self.world.reset_test_block(&position).await?;
            }
        }

        Ok(RunningEvaluation::Continue)
    }

    async fn handle_attempt_pass(&mut self, tick: u32) {
        if let Some(placement) = &self.placement {
            // GameTestInfo::succeed 在监听器之前移除非玩家实体
            // 报告成功或安排一次 copyReset 重跑。
            if let Err(error) = clear_success_entities(self.world.as_ref(), placement).await {
                self.state = GameTestState::Failed { tick, error };
                return;
            }

            if let Err(error) = self
                .world
                .set_test_instance_success(placement.test_instance_pos())
                .await
            {
                self.state = GameTestState::Failed { tick, error };
                return;
            }

            // GameTestRunner 的批处理监听器会移除测试实例的屏障外壳
            // 在每次通过的执行时触发，包括将要重跑的执行。
            if let Err(error) = remove_barriers(
                self.world.as_ref(),
                placement,
                self.test.definition().sky_access,
            )
            .await
            {
                self.state = GameTestState::Failed { tick, error };
                return;
            }
        }
        self.state = GameTestState::Passed { tick };
    }

    async fn finish_failure(
        &mut self,
        tick: u32,
        error: GameTestError,
        marker: Option<(BlockPos, String)>,
    ) {
        if let Some(placement) = &self.placement {
            let message = error.to_string();
            if let Err(controller_error) = self
                .world
                .set_test_instance_failure(placement.test_instance_pos(), &message, marker)
                .await
            {
                self.state = GameTestState::Failed {
                    tick,
                    error: controller_error,
                };
                return;
            }
        }
        self.state = GameTestState::Failed { tick, error };
    }

    fn test_block_positions(&self, mode: TestBlockMode) -> Vec<BlockPos> {
        let Some(placement) = &self.placement else {
            return Vec::new();
        };

        self.template
            .blocks()
            .iter()
            .filter(|block| block.test_mode == Some(mode))
            .map(|block| {
                placement.transform(&BlockPos::new(
                    block.position[0],
                    block.position[1],
                    block.position[2],
                ))
            })
            .collect()
    }
}

fn assertion_marker(error: &GameTestError) -> Option<(BlockPos, String)> {
    match error {
        GameTestError::Assertion {
            position: Some(position),
            message,
            ..
        } => Some((*position, message.clone())),
        _ => None,
    }
}

#[derive(Default)]
pub struct TestRunner {
    active: Vec<GameTestSession>,
}

impl TestRunner {
    #[must_use]
    pub const fn new() -> Self {
        Self { active: Vec::new() }
    }

    pub fn enqueue(&mut self, run: GameTestSession) {
        self.active.push(run);
    }

    pub async fn tick(&mut self) {
        for run in &mut self.active {
            run.tick().await;
        }
    }

    #[must_use]
    pub fn active(&self) -> &[GameTestSession] {
        &self.active
    }

    pub fn active_mut(&mut self) -> &mut [GameTestSession] {
        &mut self.active
    }
}
