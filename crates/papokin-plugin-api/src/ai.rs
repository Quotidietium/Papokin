use crate::wit::papokin::plugin::context::Server;
use crate::wit::papokin::plugin::world::Entity;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// 表示生物的自定义实体 AI 目标。
///
/// goal 回调使用共享引用以便重入。可变的 goal 状态必须使用
/// 线程安全的内部可变性。
#[allow(unused_variables)]
pub trait AiGoal: Send + Sync {
    ///若该目标应开始执行，则返回 `true`。
    fn can_start(&self, server: Server, entity: Entity) -> bool {
        false
    }
    ///若该目标应在后续刻中继续执行，则返回 `true`。
    fn should_continue(&self, server: Server, entity: Entity) -> bool {
        false
    }
    /// 目标启动时执行。
    fn start(&self, server: Server, entity: Entity) {}
    /// 目标激活期间每个服务器刻执行。
    fn tick(&self, server: Server, entity: Entity) {}
    /// 目标停止执行时执行。
    fn stop(&self, server: Server, entity: Entity) {}
}

pub(crate) static AI_GOAL_HANDLERS: Mutex<LazyAiGoalHandlers> = Mutex::new(LazyAiGoalHandlers {
    handlers: BTreeMap::new(),
    next_id: 0,
});

pub(crate) struct LazyAiGoalHandlers {
    pub handlers: BTreeMap<u32, Arc<dyn AiGoal>>,
    pub next_id: u32,
}

impl LazyAiGoalHandlers {
    #[must_use]
    pub fn register(&mut self, goal: Box<dyn AiGoal>) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.insert(id, Arc::from(goal));
        id
    }

    #[must_use]
    pub fn get(&self, id: u32) -> Option<Arc<dyn AiGoal>> {
        self.handlers.get(&id).map(Arc::clone)
    }
}

/// 向服务器运行时注册自定义 mob AI goal 的管理器。
pub struct AiGoalManager;

impl AiGoalManager {
    /// 注册一个自定义 AI 目标，并返回其唯一的目标 ID。
    ///
    /// 随后即可使用以下方法将该目标附加到生物上
    /// `mob.add_custom_ai_goal(priority, goal_id)`。
    pub fn register<G: AiGoal + 'static>(goal: G) -> u32 {
        AI_GOAL_HANDLERS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .register(Box::new(goal))
    }
}
