use super::channel::LevelChange;
use super::chunk_holder::ChunkHolder;
use super::chunk_state::{Chunk, StagedChunkEnum};
use super::dag::{DAG, EdgeKey, Node, NodeKey};
use super::generation_cache::{Cache, SurfaceBiomeNeighborhood};
use super::worker_logic::{RecvChunk, io_read_work, io_write_work};
use super::{
    ChunkLevel, ChunkListener, ChunkLoading, ChunkPos, HashMapType, HashSetType, IOLock,
    LevelChannel,
};
use crate::chunk::io::Dirtiable;
use crate::level::{Level, LoadedChunkChange, SyncChunk};
use dashmap::DashMap;
use papokin_config::lighting::LightingEngineConfig;
use papokin_util::math::vector2::Vector2;
use slotmap::Key;
use std::cmp::{Ordering, max};
use std::collections::{BinaryHeap, HashMap};
use std::mem::swap;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tracing::{debug, error, info, trace, warn};

pub(crate) struct TaskHeapNode(i8, NodeKey);
impl PartialEq for TaskHeapNode {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl TaskHeapNode {
    #[cfg(test)]
    pub(crate) const fn node_key(&self) -> NodeKey {
        self.1
    }
}
impl Eq for TaskHeapNode {}
impl PartialOrd for TaskHeapNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for TaskHeapNode {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0).reverse()
    }
}

pub struct GenerationSchedule {
    queue: BinaryHeap<TaskHeapNode>,
    graph: DAG,

    last_level: ChunkLevel,
    last_high_priority: Vec<ChunkPos>,
    send_level: Arc<LevelChannel>,

    public_chunk_map: Arc<DashMap<Vector2<i32>, SyncChunk>>,
    loaded_chunk_changes: Arc<crossbeam::queue::SegQueue<LoadedChunkChange>>,
    chunk_map: HashMap<ChunkPos, ChunkHolder>,
    unload_chunks: HashSetType<ChunkPos>,

    /// 已满足图就绪条件（`in_degree` == 0）但尚不能运行的任务，
    /// 其所需的一个或多个相邻区块尚未送达。
    /// 在此暂停等待，当区块数据到达时由 `check_waiting_tasks()` 重新排队。
    waiting_for_chunks: HashSetType<NodeKey>,

    io_lock: IOLock,
    running_task_count: u16,
    max_in_flight: u16,
    queue_dirty: bool,
    recv_chunk: crossbeam::channel::Receiver<(ChunkPos, RecvChunk)>,
    io_read: tokio::sync::mpsc::Sender<Vec<ChunkPos>>,
    io_write: tokio::sync::mpsc::Sender<Vec<(ChunkPos, Chunk)>>,
    send_chunk: crossbeam::channel::Sender<(ChunkPos, RecvChunk)>,
    listener: Arc<ChunkListener>,
    lighting_config: LightingEngineConfig,
    last_unload: std::time::Instant,
    generation_pool: Arc<rayon::ThreadPool>,
}

impl GenerationSchedule {
    fn publish_chunk(&self, pos: ChunkPos, chunk: SyncChunk) -> Option<SyncChunk> {
        let previous = self.public_chunk_map.insert(pos, chunk);
        if previous.is_none() {
            self.loaded_chunk_changes
                .push(LoadedChunkChange::Loaded(pos));
        }
        previous
    }

    fn unpublish_chunk(&self, pos: ChunkPos) -> Option<SyncChunk> {
        let removed = self.public_chunk_map.remove(&pos).map(|(_, chunk)| chunk);
        if removed.is_some() {
            self.loaded_chunk_changes
                .push(LoadedChunkChange::Unloaded(pos));
        }
        removed
    }

    pub fn create(
        io_read_thread_count: usize,
        level: Arc<Level>,
        level_channel: Arc<LevelChannel>,
        listener: Arc<ChunkListener>,
        thread_tracker: &mut Vec<thread::JoinHandle<()>>,
    ) {
        let (send_chunk, recv_chunk) = crossbeam::channel::unbounded();

        let (send_read_io, recv_read_io) = tokio::sync::mpsc::channel(io_read_thread_count + 5);
        let recv_read_io = Arc::new(tokio::sync::Mutex::new(recv_read_io));

        let (send_write_io, recv_write_io) = tokio::sync::mpsc::channel(500);

        let io_lock = Arc::new((
            Mutex::new(HashMapType::default()),
            tokio::sync::Notify::new(),
        ));

        for _ in 0..io_read_thread_count {
            level.chunk_system_tasks.spawn(io_read_work(
                recv_read_io.clone(),
                send_chunk.clone(),
                level.clone(),
                io_lock.clone(),
            ));
        }

        level.chunk_system_tasks.spawn(io_write_work(
            recv_write_io,
            level.clone(),
            io_lock.clone(),
        ));

        let cpus = thread::available_parallelism().map_or(1, std::num::NonZero::get);
        let gen_threads = (cpus / 2).clamp(2, 16);
        let generation_pool = Arc::new(
            rayon::ThreadPoolBuilder::new()
                .num_threads(gen_threads)
                .thread_name(|i| format!("ChunkGen-{i}"))
                .build()
                .expect("构建区块生成线程池失败"),
        );
        let max_in_flight = (gen_threads * 2) as u16;

        let level_sched = level;
        let lighting_config = level_sched.lighting_config;
        let handle = thread::Builder::new()
            .name("Schedule".to_string())
            .spawn(move || {
                let scheduler = Self {
                    queue: BinaryHeap::new(),
                    graph: DAG::default(),
                    last_level: ChunkLevel::default(),
                    last_high_priority: Vec::new(),
                    send_level: level_channel,
                    public_chunk_map: level_sched.loaded_chunks.clone(),
                    loaded_chunk_changes: level_sched.loaded_chunk_changes.clone(),
                    unload_chunks: HashSetType::default(),
                    waiting_for_chunks: HashSetType::default(),
                    io_lock,
                    running_task_count: 0,
                    max_in_flight,
                    queue_dirty: false,
                    recv_chunk,
                    io_read: send_read_io,
                    io_write: send_write_io,
                    send_chunk,
                    listener,
                    chunk_map: HashMap::default(),
                    lighting_config,
                    last_unload: std::time::Instant::now(),
                    generation_pool,
                };
                scheduler.work(&level_sched);
            })
            .expect("启动调度器线程失败");

        thread_tracker.push(handle);
    }

    fn apply_lighting_override(&self, chunk: &SyncChunk) {
        match self.lighting_config {
            LightingEngineConfig::Full => {
                let mut engine = chunk
                    .light_engine
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                for section in &mut engine.block_light {
                    section.fill(15);
                }
                for section in &mut engine.sky_light {
                    section.fill(15);
                }
                chunk.dirty.store(true, Relaxed);
            }
            LightingEngineConfig::Dark => {
                let mut engine = chunk
                    .light_engine
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                for section in &mut engine.block_light {
                    section.fill(0);
                }
                for section in &mut engine.sky_light {
                    section.fill(0);
                }
                chunk.dirty.store(true, Relaxed);
            }
            LightingEngineConfig::Default => {}
        }
    }

    fn calc_priority(
        last_level: &ChunkLevel,
        last_high_priority: &[ChunkPos],
        pos: ChunkPos,
        stage: StagedChunkEnum,
    ) -> i8 {
        let base_level = *last_level.get(&pos).unwrap_or(&ChunkLoading::MAX_LEVEL);
        if base_level == ChunkLoading::MAX_LEVEL {
            return 127;
        }
        if last_high_priority.is_empty() {
            return base_level + (stage as i8);
        }
        let mut min_dst = i32::MAX;
        for i in last_high_priority {
            let dst = max((i.x - pos.x).abs(), (i.y - pos.y).abs());
            min_dst = min_dst.min(dst);
            if dst <= StagedChunkEnum::FULL_RADIUS
                && stage <= StagedChunkEnum::FULL_DEPENDENCIES[dst as usize]
            {
                return base_level + (stage as i8) - 100 + (dst as i8);
            }
        }
        base_level + (stage as i8) + (min_dst.min(60) as i8)
    }

    fn sort_queue(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        let mut tasks: Vec<_> = self.queue.drain().collect();
        for i in &mut tasks {
            if let Some(node) = self.graph.nodes.get(i.1) {
                i.0 = Self::calc_priority(
                    &self.last_level,
                    &self.last_high_priority,
                    node.pos,
                    node.stage,
                );
            }
        }
        self.queue = BinaryHeap::from(tasks);
    }

    /// TODO: 将来某个时候会移除
    pub(crate) fn restore_ready_tasks(
        graph: &mut DAG,
        queue: &mut BinaryHeap<TaskHeapNode>,
        chunk_map: &HashMap<ChunkPos, ChunkHolder>,
        last_level: &ChunkLevel,
        last_high_priority: &[ChunkPos],
        _waiting_for_chunks: &HashSetType<NodeKey>,
    ) -> usize {
        debug_assert!(queue.is_empty());

        let mut to_drop = Vec::new();
        let mut ready = Vec::new();

        for (key, node) in &graph.nodes {
            if node.stage == StagedChunkEnum::None || node.in_flight {
                continue;
            }

            let Some(holder) = chunk_map.get(&node.pos) else {
                to_drop.push(key);
                continue;
            };

            let effective_target = holder.target_stage.max(holder.dependency_stage);
            if holder.current_stage >= node.stage
                || holder.tasks[node.stage as usize] != key
                || node.stage > effective_target
            {
                to_drop.push(key);
                continue;
            }

            if node.in_degree == 0 && !node.in_queue {
                ready.push((key, node.pos, node.stage));
            }
        }

        for key in to_drop {
            let Some(old) = graph.nodes.remove(key) else {
                continue;
            };
            let mut edge = old.edge;
            while !edge.is_null() {
                let Some(cur) = graph.edges.remove(edge) else {
                    break;
                };
                if let Some(target_node) = graph.nodes.get_mut(cur.to)
                    && target_node.in_degree > 0
                {
                    target_node.in_degree -= 1;
                    if target_node.in_degree == 0 && !target_node.in_queue && !target_node.in_flight
                    {
                        target_node.in_queue = true;
                        queue.push(TaskHeapNode(
                            Self::calc_priority(
                                last_level,
                                last_high_priority,
                                target_node.pos,
                                target_node.stage,
                            ),
                            cur.to,
                        ));
                    }
                }
                edge = cur.next;
            }
        }

        for (key, pos, stage) in ready {
            if let Some(node) = graph.nodes.get_mut(key)
                && node.in_degree == 0
                && !node.in_queue
            {
                node.in_queue = true;
                queue.push(TaskHeapNode(
                    Self::calc_priority(last_level, last_high_priority, pos, stage),
                    key,
                ));
            }
        }

        queue.len()
    }

    /// 确保 `req_stage` 的依赖链存在于 `holder` 上（对于位于
    /// `chunk_pos`），并将其接为依赖 `dependency_task`。
    ///
    /// 将 `holder.dependency_stage`（而非 `target_stage`）提升到至少 `req_stage`，以便
    /// 作为生成依赖拉入的邻近区块在此之前不会被丢弃
    /// 其依赖已满足。`target_stage` 保持不动，以便关卡变更
    /// 簿记不变式（`old_stage == holder.target_stage`）绝不会被破坏。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn ensure_dependency_chain(
        graph: &mut DAG,
        queue: &mut BinaryHeap<TaskHeapNode>,
        last_level: &ChunkLevel,
        last_high_priority: &[ChunkPos],
        dependency_task: NodeKey,
        chunk_pos: ChunkPos,
        holder: &mut ChunkHolder,
        req_stage: StagedChunkEnum,
    ) -> [bool; StagedChunkEnum::COUNT] {
        // 插入 occupied_by 边头
        holder.occupied_by = graph.edges.insert(crate::chunk_system::dag::Edge::new(
            dependency_task,
            holder.occupied_by,
        ));

        if !holder.occupied.is_null() {
            graph.add_edge(holder.occupied, dependency_task);
        }

        // 提升 dependency_stage，以便调度该区块的 IO/生成任务，并
        // 即使 target_stage 为 None（在玩家视距之外）也保持存活。
        // 我们刻意不碰 target_stage —— 该字段归 resort_work 所有
        // 且必须与等级变更簿记一致，否则 debug_assert 会触发。
        if holder.dependency_stage < req_stage {
            holder.dependency_stage = req_stage;
        }

        // 有效目标是玩家想要的与依赖所需两者中的最大值
        let effective_target = holder.target_stage.max(holder.dependency_stage);
        let mut newly_created = [false; StagedChunkEnum::COUNT];

        // 创建从 current_stage+1 到 effective_target 之间所有缺失的任务。
        // 即使 current_stage >= req_stage 我们也这样做，因为 dependency_stage 可能
        // 需要 req_stage 之外尚未创建的任务。
        if holder.current_stage < effective_target {
            let empty = StagedChunkEnum::Empty as usize;
            let start = (holder.current_stage as usize + 1).max(empty);
            let end = effective_target as u8 as usize;

            for (i, flag) in newly_created[start..=end].iter_mut().enumerate() {
                let stage_i = start + i;
                if holder.tasks[stage_i].is_null() {
                    let new_node = graph
                        .nodes
                        .insert(Node::new(chunk_pos, StagedChunkEnum::from(stage_i as u8)));
                    holder.tasks[stage_i] = new_node;
                    *flag = true;
                    if !holder.occupied.is_null() {
                        graph.add_edge(holder.occupied, new_node);
                    }
                }
            }

            for stage_i in start..=end {
                if !newly_created[stage_i] {
                    continue;
                }
                let cur = holder.tasks[stage_i];

                if stage_i > empty {
                    let prev = holder.tasks[stage_i - 1];
                    if !prev.is_null() {
                        graph.add_edge(prev, cur);
                    }
                }
                if stage_i < end {
                    let next = holder.tasks[stage_i + 1];
                    if !next.is_null() && !newly_created[stage_i + 1] {
                        graph.add_edge(cur, next);
                    }
                }
            }

            // 将入口任务（最低的未阻塞阶段）入队
            let entry_task = holder.tasks[start];
            if !entry_task.is_null()
                && let Some(n) = graph.nodes.get_mut(entry_task)
                && n.in_degree == 0
                && !n.in_queue
            {
                n.in_queue = true;
                queue.push(TaskHeapNode(
                    Self::calc_priority(
                        last_level,
                        last_high_priority,
                        chunk_pos,
                        StagedChunkEnum::from(start as u8),
                    ),
                    entry_task,
                ));
            }
        }

        // 如果 req_stage 已满足，dependency_task 无需等待 —
        // 它只是被 `occupied` 阻塞（上文已处理），而该阶段本身已完成。
        // 不要在此处添加边：tasks[req_stage] 为 null（已完成并丢弃）。
        if holder.current_stage >= req_stage {
            return newly_created;
        }

        // 把 req_stage 任务连接到 dependency_task，使 dependency_task 在此之前无法运行
        // 该区块达到 req_stage。此处 tasks[req_stage] 保证非空：
        // effective_target >= req_stage（我们刚将 dependency_stage 设为 req_stage）且
        // current_stage < req_stage，因此该任务是在上方的循环中创建的（或者
        // 已存在）。
        let req_end = req_stage as u8 as usize;
        let ano_task = holder.tasks[req_end];
        debug_assert!(
            !ano_task.is_null(),
            "holder.tasks[req_stage] must not be null before adding edge"
        );
        graph.add_edge(ano_task, dependency_task);

        newly_created
    }

    /// 检查停留在 `waiting_for_chunks` 状态的任务现在是否已具备其全部相邻
    /// 区块数据可用，若可用则重新入队。
    /// 必须在每次调用 `receive_chunk` 之后调用。
    fn check_waiting_tasks(&mut self) {
        if self.waiting_for_chunks.is_empty() {
            return;
        }

        let mut now_ready: Vec<NodeKey> = Vec::new();
        let mut stale: Vec<NodeKey> = Vec::new();

        self.waiting_for_chunks.retain(|&node_key| {
            let Some(node) = self.graph.nodes.get(node_key) else {
                return false; // 节点已被丢弃，静默放弃
            };

            let Some(holder) = self.chunk_map.get(&node.pos) else {
                stale.push(node_key);
                return false;
            };

            let effective_target = holder.target_stage.max(holder.dependency_stage);
            if holder.current_stage >= node.stage
                || holder.tasks[node.stage as usize] != node_key
                || node.stage > effective_target
            {
                stale.push(node_key);
                return false;
            }

            let read_radius = node.stage.get_read_radius();
            let pos = node.pos;
            let all_ready = (-read_radius..=read_radius).all(|dx| {
                (-read_radius..=read_radius).all(|dy| {
                    self.chunk_map
                        .get(&pos.add_raw(dx, dy))
                        .is_some_and(|h| h.chunk.is_some())
                })
            });
            if all_ready {
                if node.in_degree == 0 {
                    now_ready.push(node_key);
                    false
                } else {
                    true
                }
            } else {
                true
            }
        });

        for node_key in stale {
            self.drop_node(node_key);
        }

        for node_key in now_ready {
            if let Some(n) = self.graph.nodes.get_mut(node_key)
                && n.in_degree == 0
                && !n.in_queue
            {
                n.in_queue = true;
                let priority =
                    Self::calc_priority(&self.last_level, &self.last_high_priority, n.pos, n.stage);
                self.queue.push(TaskHeapNode(priority, node_key));
            }
        }
    }

    #[expect(clippy::too_many_lines)]
    fn resort_work(&mut self, new_data: (Option<LevelChange>, Option<Vec<ChunkPos>>)) -> bool {
        if new_data.0.is_none() && new_data.1.is_none() {
            return false;
        }
        if let Some(high_priority) = new_data.1 {
            self.last_high_priority = high_priority;
            self.queue_dirty = true;
        }
        let Some(new_level) = new_data.0 else {
            return true;
        };
        let mut worklist: std::collections::VecDeque<(ChunkPos, StagedChunkEnum, NodeKey)> =
            std::collections::VecDeque::new();

        for (pos, (old_stage, new_stage)) in new_level.0 {
            debug_assert_ne!(old_stage, new_stage);
            debug_assert_eq!(
                new_stage,
                StagedChunkEnum::level_to_stage(
                    *new_level.1.get(&pos).unwrap_or(&ChunkLoading::MAX_LEVEL)
                )
            );
            let mut holder = self.chunk_map.remove(&pos).unwrap_or_default();
            debug_assert_eq!(holder.target_stage, old_stage);
            holder.target_stage = new_stage;

            // 有效目标是我们实际需要调度任务达到的目标
            let effective_old = old_stage.max(holder.dependency_stage);
            let effective_new = new_stage.max(holder.dependency_stage);

            if effective_old > effective_new {
                for i in (effective_new.max(holder.current_stage) as usize + 1)
                    ..=(effective_old as usize)
                {
                    let task = &mut holder.tasks[i];
                    if !task.is_null() {
                        let is_in_flight = self.graph.nodes.get(*task).is_some_and(|n| n.in_flight);
                        if !is_in_flight {
                            self.waiting_for_chunks.remove(task);
                            self.drop_node(*task);
                            *task = NodeKey::null();
                        }
                    }
                }
                if new_stage == StagedChunkEnum::None {
                    if holder.dependency_stage != StagedChunkEnum::None {
                        let has_valid_task = self.graph.prune_edge_chain(&mut holder.occupied_by);
                        if !has_valid_task {
                            holder.dependency_stage = StagedChunkEnum::None;
                        }
                    }
                    if holder.dependency_stage == StagedChunkEnum::None {
                        self.unload_chunks.insert(pos);
                    }
                }
            } else {
                if old_stage == StagedChunkEnum::None {
                    self.unload_chunks.remove(&pos);
                    if holder.current_stage == StagedChunkEnum::Full && !holder.public {
                        holder.public = true;
                        match holder.chunk.as_ref().expect("区块应存在") {
                            Chunk::Level(chunk) => {
                                self.apply_lighting_override(chunk);
                                self.publish_chunk(pos, chunk.clone());
                                self.listener.process_new_chunk(pos, chunk);
                            }
                            Chunk::Proto(_) => panic!(),
                        }
                    }
                }
                for i in (effective_old.max(holder.current_stage) as u8 + 1)..=(effective_new as u8)
                {
                    let task = &mut holder.tasks[i as usize];
                    if task.is_null() {
                        *task = self.graph.nodes.insert(Node::new(pos, i.into()));
                        if !holder.occupied.is_null() {
                            self.graph.add_edge(holder.occupied, *task);
                        }
                    }
                    let task = *task;
                    if i > 1 {
                        let stage = StagedChunkEnum::from(i);
                        let dependency = stage.get_direct_dependencies();
                        let radius = stage.get_direct_radius();
                        self.unload_chunks.remove(&pos);
                        let req_stage = dependency[0];
                        let newly_created = Self::ensure_dependency_chain(
                            &mut self.graph,
                            &mut self.queue,
                            &self.last_level,
                            &self.last_high_priority,
                            task,
                            pos,
                            &mut holder,
                            req_stage,
                        );
                        for (stage_i, &created) in newly_created.iter().enumerate() {
                            if created && stage_i > 1 {
                                let stage = StagedChunkEnum::from(stage_i as u8);
                                let dependency = stage.get_direct_dependencies();
                                let radius = stage.get_direct_radius();
                                let cur_task = holder.tasks[stage_i];
                                for r in 1..=(radius as u8) {
                                    let neighbor_req = dependency[r as usize];
                                    for &(ndx, ndz) in
                                        papokin_data::chunk_view_lut::get_chebyshev_ring(r)
                                    {
                                        let neighbor_pos = pos.add_raw(ndx as i32, ndz as i32);
                                        worklist.push_back((neighbor_pos, neighbor_req, cur_task));
                                    }
                                }
                            }
                        }

                        for r in 1..=(radius as u8) {
                            let req_stage = dependency[r as usize];
                            for &(dx, dz) in papokin_data::chunk_view_lut::get_chebyshev_ring(r) {
                                let new_pos = pos.add_raw(dx as i32, dz as i32);
                                self.unload_chunks.remove(&new_pos);
                                worklist.push_back((new_pos, req_stage, task));
                            }
                        }
                    }
                    let node = self.graph.nodes.get_mut(task).expect("节点应存在");
                    if node.in_degree == 0 && !node.in_queue {
                        node.in_queue = true;
                        self.queue.push(TaskHeapNode(0, task));
                    }
                }
            }
            self.chunk_map.insert(pos, holder);
        }

        while let Some((pos, req_stage, dep_task)) = worklist.pop_front() {
            self.unload_chunks.remove(&pos);
            let mut holder = self.chunk_map.remove(&pos).unwrap_or_default();
            let newly_created = Self::ensure_dependency_chain(
                &mut self.graph,
                &mut self.queue,
                &new_level.1,
                &self.last_high_priority,
                dep_task,
                pos,
                &mut holder,
                req_stage,
            );
            for (stage_i, &created) in newly_created.iter().enumerate() {
                if created && stage_i > 1 {
                    let stage = StagedChunkEnum::from(stage_i as u8);
                    let dependency = stage.get_direct_dependencies();
                    let radius = stage.get_direct_radius();
                    let cur_task = holder.tasks[stage_i];
                    for r in 1..=(radius as u8) {
                        let neighbor_req = dependency[r as usize];
                        for &(dx, dz) in papokin_data::chunk_view_lut::get_chebyshev_ring(r) {
                            let neighbor_pos = pos.add_raw(dx as i32, dz as i32);
                            worklist.push_back((neighbor_pos, neighbor_req, cur_task));
                        }
                    }
                }
            }
            self.chunk_map.insert(pos, holder);
        }

        self.last_level = new_level.1;
        self.queue_dirty = true;
        true
    }

    fn recompute_dependency_stages(&mut self) {
        let mut required: HashMapType<ChunkPos, StagedChunkEnum> = HashMapType::default();
        let mut worklist: Vec<(ChunkPos, StagedChunkEnum)> = self
            .chunk_map
            .iter()
            .filter(|(_, holder)| holder.target_stage != StagedChunkEnum::None)
            .map(|(pos, holder)| (*pos, holder.target_stage))
            .collect();

        while let Some((pos, req)) = worklist.pop() {
            let entry = required.entry(pos).or_insert(StagedChunkEnum::None);
            if *entry >= req {
                continue;
            }
            let start = *entry as u8 + 1;
            *entry = req;

            // 只扩展尚未计入的阶段。
            for i in start..=(req as u8) {
                let stage = StagedChunkEnum::from(i);
                let radius = stage.get_direct_radius();
                if radius == 0 {
                    continue;
                }
                let dependencies = stage.get_direct_dependencies();
                for r in 1..=(radius as u8) {
                    let req = dependencies[r as usize];
                    for &(dx, dz) in papokin_data::chunk_view_lut::get_chebyshev_ring(r) {
                        let neighbor = pos.add_raw(dx as i32, dz as i32);
                        worklist.push((neighbor, req));
                    }
                }
            }
        }

        let mut nodes_to_drop = Vec::new();
        let mut newly_unused = Vec::new();
        for (pos, holder) in &mut self.chunk_map {
            let new_dependency = required.get(pos).copied().unwrap_or(StagedChunkEnum::None);
            if new_dependency >= holder.dependency_stage {
                continue;
            }
            holder.dependency_stage = new_dependency;

            let effective_target = holder.target_stage.max(new_dependency);
            for i in (effective_target as usize + 1)..StagedChunkEnum::COUNT {
                let task = holder.tasks[i];
                if !task.is_null() {
                    nodes_to_drop.push((*pos, i, task));
                }
            }
            if effective_target == StagedChunkEnum::None {
                newly_unused.push(*pos);
            }
        }
        self.unload_chunks.extend(newly_unused);

        for (pos, index, task) in nodes_to_drop {
            if self
                .graph
                .nodes
                .get(task)
                .is_some_and(|node| node.in_flight)
            {
                continue;
            }
            self.waiting_for_chunks.remove(&task);
            self.drop_node(task);
            if let Some(holder) = self.chunk_map.get_mut(&pos) {
                holder.tasks[index] = NodeKey::null();
            }
        }

        self.purge_dropped_queue_entries();
    }

    /// 丢弃节点已被取消的堆条目。它们在弹出时会被跳过，
    /// 但饱和的队列永远不会被排空，因此若缺少这一步，堆会保留所有
    /// 玩家飞过的每个区块的已取消任务，而 `sort_queue` 会
    /// 每次等级变化都会变慢。
    fn purge_dropped_queue_entries(&mut self) {
        if self.queue.is_empty() {
            return;
        }
        let graph = &self.graph;
        let tasks: Vec<_> = self
            .queue
            .drain()
            .filter(|task| graph.nodes.contains_key(task.1))
            .collect();
        self.queue = BinaryHeap::from(tasks);
    }

    fn garbage_collect_dependencies(&mut self) {
        self.recompute_dependency_stages();

        // 垃圾回收孤立的依赖项和空的持有项
        let mut stranded = Vec::new();
        let mut empty_holders = Vec::new();

        for (pos, holder) in &self.chunk_map {
            if holder.target_stage == StagedChunkEnum::None {
                if holder.dependency_stage != StagedChunkEnum::None {
                    stranded.push(*pos);
                } else if holder.current_stage == StagedChunkEnum::None
                    && holder.chunk.is_none()
                    && holder.occupied.is_null()
                    && holder.tasks.iter().all(Key::is_null)
                    && !holder.public
                {
                    empty_holders.push(*pos);
                }
            }
        }

        for pos in stranded {
            let holder = self.chunk_map.get_mut(&pos).expect("持有者应存在");
            if !holder.occupied.is_null() && self.graph.nodes.contains_key(holder.occupied) {
                continue;
            }

            let has_valid_task = self.graph.prune_edge_chain(&mut holder.occupied_by);
            if !has_valid_task {
                holder.dependency_stage = StagedChunkEnum::None;
                self.unload_chunks.insert(pos);
            }
        }

        for pos in empty_holders {
            if let Some(mut holder) = self.chunk_map.remove(&pos) {
                self.graph.drop_edge_chain(holder.occupied_by);
                holder.occupied_by = EdgeKey::null();
            }
        }
    }

    fn process_unload_queue(&mut self) {
        if self.unload_chunks.is_empty() {
            return;
        }

        let mut unload_chunks = HashSetType::default();
        swap(&mut unload_chunks, &mut self.unload_chunks);
        let mut chunks = Vec::with_capacity(unload_chunks.len());
        for pos in unload_chunks {
            let Some(mut holder) = self.chunk_map.remove(&pos) else {
                continue;
            };
            if holder.target_stage != StagedChunkEnum::None
                || holder.dependency_stage != StagedChunkEnum::None
            {
                self.chunk_map.insert(pos, holder);
                continue;
            }
            if !holder.occupied.is_null() {
                self.chunk_map.insert(pos, holder);
                self.unload_chunks.insert(pos);
                continue;
            }

            for task in holder.tasks {
                if !task.is_null() {
                    let is_in_flight = self.graph.nodes.get(task).is_some_and(|n| n.in_flight);
                    if !is_in_flight {
                        self.waiting_for_chunks.remove(&task);
                        self.drop_node(task);
                    }
                }
            }

            self.graph.drop_edge_chain(holder.occupied_by);
            holder.occupied_by = EdgeKey::null();

            if holder.public {
                self.unpublish_chunk(pos);
                holder.public = false;
            }

            if let Some(tmp) = holder.chunk {
                match tmp {
                    Chunk::Level(chunk) => {
                        // 若区块已脏则保存到磁盘
                        if chunk.is_dirty() {
                            chunks.push((pos, Chunk::Level(chunk)));
                        }
                    }
                    Chunk::Proto(proto) => {
                        if !matches!(proto.stage, StagedChunkEnum::Empty | StagedChunkEnum::None) {
                            chunks.push((pos, Chunk::Proto(proto)));
                        }
                    }
                }
            }
        }
        if chunks.is_empty() {
            return;
        }
        let mut data = self
            .io_lock
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (pos, _chunk) in &chunks {
            *data.entry(*pos).or_insert(0) += 1;
        }
        drop(data);
        if let Err(e) = self.io_write.blocking_send(chunks) {
            error!(
                "保存时向 IO 写入线程发送区块失败（线程可能已关停）: {:?}",
                e
            );
        }
    }

    fn save_all_chunk(&mut self, save_proto_chunk: bool) {
        let mut chunks = Vec::with_capacity(self.chunk_map.len());

        for (pos, holder) in &mut self.chunk_map {
            if let Some(chunk) = &holder.chunk {
                let should_save = match chunk {
                    Chunk::Level(sync_chunk) => sync_chunk.is_dirty(),
                    Chunk::Proto(proto) => {
                        save_proto_chunk
                            && !matches!(
                                proto.stage,
                                crate::chunk_system::chunk_state::StagedChunkEnum::Empty
                                    | crate::chunk_system::chunk_state::StagedChunkEnum::None
                            )
                    }
                };

                if should_save {
                    let chunk_to_save = match chunk {
                        Chunk::Level(sync_chunk) => Chunk::Level(sync_chunk.clone()),
                        Chunk::Proto(_) => holder.chunk.take().expect("Proto 区块应存在"),
                    };
                    chunks.push((*pos, chunk_to_save));
                }
            }
        }

        if chunks.is_empty() {
            return;
        }

        info!(
            "正在保存 {} 个区块（收集自 {} 个持有者）...",
            chunks.len(),
            self.chunk_map.len()
        );

        let mut data = self
            .io_lock
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (pos, _) in &chunks {
            *data.entry(*pos).or_insert(0) += 1;
        }
        drop(data);

        if let Err(e) = self.io_write.blocking_send(chunks) {
            error!("向 IO 写入线程发送区块失败: {:?}", e);
        }
    }

    fn drop_node(&mut self, node: NodeKey) {
        let Some(old) = self.graph.nodes.remove(node) else {
            return;
        };
        let mut edge = old.edge;
        while !edge.is_null() {
            let cur = self.graph.edges.remove(edge).expect("边应存在");
            if let Some(node) = self.graph.nodes.get_mut(cur.to) {
                debug_assert!(node.in_degree >= 1);
                node.in_degree -= 1;
                if node.in_degree == 0 && !node.in_queue {
                    self.queue.push(TaskHeapNode(
                        Self::calc_priority(
                            &self.last_level,
                            &self.last_high_priority,
                            node.pos,
                            node.stage,
                        ),
                        cur.to,
                    ));
                    node.in_queue = true;
                }
            }
            edge = cur.next;
        }
    }

    fn drop_satisfied_tasks(&mut self, holder: &mut ChunkHolder, stage: StagedChunkEnum) {
        // 相邻的生成缓存可能已将此持有者推进越过
        // 返回任务的阶段。丢弃返回数据能满足的所有已调度任务，
        // 包括该任务遗留的过期在途节点。
        for task_idx in (StagedChunkEnum::None as usize + 1)..=(stage as usize) {
            if !holder.tasks[task_idx].is_null() {
                self.waiting_for_chunks.remove(&holder.tasks[task_idx]);
                self.drop_node(holder.tasks[task_idx]);
                holder.tasks[task_idx] = NodeKey::null();
            }
        }
    }

    #[expect(clippy::too_many_lines)]
    fn receive_chunk(&mut self, pos: ChunkPos, data: RecvChunk) {
        match data {
            RecvChunk::IO(chunk) => {
                let mut holder = self.chunk_map.remove(&pos).expect("持有者应存在");
                if holder.chunk.is_some() {
                    warn!("receive_chunk(IO): {:?} 处的持有者已有区块；将其替换", pos);
                }
                debug_assert_eq!(holder.current_stage, StagedChunkEnum::None);

                let stage = StagedChunkEnum::from(chunk.get_stage_id());
                self.drop_satisfied_tasks(&mut holder, stage);
                holder.current_stage = stage;
                debug_assert!(self.graph.nodes.contains_key(holder.occupied));
                self.drop_node(holder.occupied);
                holder.occupied = NodeKey::null();

                match &chunk {
                    Chunk::Level(data) => {
                        self.apply_lighting_override(data);
                        let result = self.publish_chunk(pos, data.clone());
                        if result.is_some() {
                            warn!("receive_chunk(IO): 正在替换 {:?} 处已存在的公开区块", pos);
                        }
                        holder.public = true;
                        trace!("通知玩家：区块 {:?} 已从磁盘加载（Full 状态）", pos);
                        self.listener.process_new_chunk(pos, data);
                    }
                    Chunk::Proto(_) => {
                        if holder.public {
                            debug!("区块 {:?} 已降级为 Proto 以重新计算光照，标记为非公开", pos);
                            self.unpublish_chunk(pos);
                            holder.public = false;
                        }
                    }
                }
                holder.chunk = Some(chunk);
                self.chunk_map.insert(pos, holder);

                // 新区块到达 — 解除所有等待中生成任务的阻塞
                self.check_waiting_tasks();
            }
            RecvChunk::Generation(data) => {
                let mut dx = 0;
                let mut dy = 0;
                for chunk in data.chunks {
                    let new_pos = ChunkPos::new(data.x + dx, data.z + dy);
                    match chunk {
                        Chunk::Level(chunk) => {
                            let mut holder = self.chunk_map.remove(&new_pos).expect("持有者应存在");
                            let stage = StagedChunkEnum::Full;
                            if new_pos == pos {
                                if holder.current_stage != StagedChunkEnum::Spawn {
                                    warn!(
                                        "receive_chunk(Level): 持有者阶段 {:?}（位置 {:?}），预期 {:?}；正在对齐",
                                        holder.current_stage,
                                        new_pos,
                                        StagedChunkEnum::Spawn
                                    );
                                    holder.current_stage = StagedChunkEnum::Spawn;
                                }
                                self.drop_satisfied_tasks(&mut holder, stage);
                                if self.graph.nodes.contains_key(holder.occupied) {
                                    self.drop_node(holder.occupied);
                                }
                                holder.current_stage = stage;

                                let was_public = holder.public;
                                self.apply_lighting_override(&chunk);
                                let public_chunk = chunk.clone();
                                if was_public {
                                    self.publish_chunk(new_pos, public_chunk);
                                    info!(
                                        "通知玩家：{:?} 处的区块已重新生成（此前已是公开状态）",
                                        new_pos
                                    );
                                    self.listener.process_new_chunk(new_pos, &chunk);
                                    holder.chunk = Some(Chunk::Level(chunk));
                                } else {
                                    holder.chunk = Some(Chunk::Level(chunk));
                                    let result = self.publish_chunk(new_pos, public_chunk);
                                    holder.public = true;
                                    if result.is_some() {
                                        warn!(
                                            "public_chunk_map.insert 对 {new_pos:?} 返回了已存在的区块"
                                        );
                                    }
                                    if let Some(pc) = self.public_chunk_map.get(&new_pos) {
                                        trace!("通知玩家：{:?} 处有新区块（生成完成）", new_pos);
                                        self.listener.process_new_chunk(new_pos, &pc);
                                    } else {
                                        error!(
                                            "严重：插入后立即从 public_chunk_map 获取区块 {:?} 失败！",
                                            new_pos
                                        );
                                    }
                                }
                            } else {
                                self.drop_satisfied_tasks(&mut holder, stage);
                                holder.current_stage = stage;
                                holder.chunk = Some(Chunk::Level(chunk));
                            }

                            if !holder.occupied.is_null()
                                && self.graph.nodes.contains_key(holder.occupied)
                            {
                                self.drop_node(holder.occupied);
                            }
                            holder.occupied = NodeKey::null();

                            self.chunk_map.insert(new_pos, holder);
                        }
                        Chunk::Proto(chunk) => {
                            let mut holder = self.chunk_map.remove(&new_pos).expect("持有者应存在");

                            let stage = StagedChunkEnum::from(chunk.stage_id());
                            self.drop_satisfied_tasks(&mut holder, stage);

                            if new_pos == pos {
                                debug_assert_ne!(holder.current_stage, StagedChunkEnum::None);
                                if self.graph.nodes.contains_key(holder.occupied) {
                                    self.drop_node(holder.occupied);
                                }
                                holder.current_stage = stage;
                            } else {
                                if holder.current_stage < stage {
                                    holder.current_stage = stage;
                                }
                                if !holder.occupied.is_null()
                                    && self.graph.nodes.contains_key(holder.occupied)
                                {
                                    self.drop_node(holder.occupied);
                                }
                            }

                            holder.occupied = NodeKey::null();
                            holder.chunk = Some(Chunk::Proto(chunk));
                            self.chunk_map.insert(new_pos, holder);
                        }
                    }
                    dy += 1;
                    if dy == data.size {
                        dy = 0;
                        dx += 1;
                    }
                }

                // 相邻区块已归还给持有者 — 解除等待任务的阻塞
                self.check_waiting_tasks();
            }
            RecvChunk::GenerationFailure {
                pos: fail_pos,
                stage,
                error,
            } => {
                error!(
                    "收到区块 {:?} 在阶段 {:?} 的生成失败通知: {}",
                    fail_pos, stage, error
                );

                if let Some(mut holder) = self.chunk_map.remove(&pos) {
                    let target_stage = holder.target_stage;

                    if !holder.occupied.is_null() {
                        if self.graph.nodes.contains_key(holder.occupied) {
                            self.drop_node(holder.occupied);
                        }
                        holder.occupied = NodeKey::null();
                    }

                    for i in 0..holder.tasks.len() {
                        if !holder.tasks[i].is_null() {
                            self.waiting_for_chunks.remove(&holder.tasks[i]);
                            self.drop_node(holder.tasks[i]);
                            holder.tasks[i] = NodeKey::null();
                        }
                    }

                    holder.current_stage = StagedChunkEnum::None;
                    holder.dependency_stage = StagedChunkEnum::None;
                    holder.chunk = None;

                    for i in (StagedChunkEnum::None as usize + 1)..=(target_stage as usize) {
                        let stage_enum = StagedChunkEnum::from(i as u8);
                        let task_node = Node::new(pos, stage_enum);
                        holder.tasks[i] = self.graph.nodes.insert(task_node);

                        if i > (StagedChunkEnum::None as usize + 1) {
                            self.graph.add_edge(holder.tasks[i - 1], holder.tasks[i]);
                        }
                    }

                    if target_stage > StagedChunkEnum::None {
                        let first_task = holder.tasks[StagedChunkEnum::None as usize + 1];
                        if let Some(node) = self.graph.nodes.get_mut(first_task) {
                            node.in_queue = true;
                        }
                        self.queue.push(TaskHeapNode(
                            Self::calc_priority(
                                &self.last_level,
                                &self.last_high_priority,
                                pos,
                                StagedChunkEnum::from(1),
                            ) - 50,
                            first_task,
                        ));
                    }

                    self.chunk_map.insert(pos, holder);

                    warn!(
                        "区块 {:?} 已重置为 None 并重新排队等待生成（目标: {:?}）",
                        pos, target_stage
                    );
                } else {
                    error!("找不到失败区块 {:?} 对应的持有者", pos);
                }
            }
        }
        self.running_task_count -= 1;
    }

    #[expect(clippy::too_many_lines)]
    fn work(mut self, level: &Arc<Level>) {
        debug!(
            "调度线程启动 id: {:?}，名称: {}",
            thread::current().id(),
            thread::current().name().unwrap_or("未知")
        );
        loop {
            if level.should_unload.swap(false, Relaxed) {
                self.garbage_collect_dependencies();
                self.process_unload_queue();
            }
            if level.should_save.swap(false, Relaxed) {
                self.save_all_chunk(false);
            }
            if level.shut_down_chunk_system.load(Relaxed) {
                info!("关停前正在保存区块...");
                self.garbage_collect_dependencies();
                self.process_unload_queue();
                self.save_all_chunk(true);
                break;
            }

            // 1. 获取最新世界状态（玩家移动等）
            if self.resort_work(self.send_level.get()) {
                self.garbage_collect_dependencies();
            }

            // 定期处理卸载队列（每 1 秒一次），将写操作批量合并
            // 并在玩家走回该区块时充当短暂的内存缓存。
            // 即使队列为空，这也必须运行：`garbage_collect_dependencies`
            // 正是把过期的依赖持有者放进队列的罪魁祸首。
            if self.last_unload.elapsed() >= std::time::Duration::from_secs(1) {
                self.garbage_collect_dependencies();
                self.process_unload_queue();
                self.last_unload = std::time::Instant::now();
            }

            // 2. 处理来自工作线程的所有待处理区块结果
            while let Ok((pos, data)) = self.recv_chunk.try_recv() {
                self.receive_chunk(pos, data);
            }

            // 3. 若世界状态变化或有新任务加入则重新排序
            if self.queue_dirty {
                self.sort_queue();
                self.queue_dirty = false;
            }

            // 4. 处理队列中就绪的任务（最多 max_in_flight 个）
            let mut io_batch = Vec::with_capacity(16);
            'out2: while let Some(task) = self.queue.pop() {
                if level.shut_down_chunk_system.load(Relaxed) {
                    self.queue.push(task);
                    info!("任务处理过程中检测到关停，正在保存区块...");
                    self.save_all_chunk(true);
                    break 'out2;
                }

                if self.running_task_count >= self.max_in_flight {
                    self.queue.push(task);
                    break 'out2;
                }

                // 快速检查高优先级结果或世界变化，以避免停滞
                while let Ok((pos, data)) = self.recv_chunk.try_recv() {
                    self.receive_chunk(pos, data);
                    if self.resort_work(self.send_level.get()) {
                        // 如果世界状态改变，必须在继续之前重新排序
                        self.garbage_collect_dependencies();
                        self.queue.push(task);
                        self.queue_dirty = true;
                        break 'out2;
                    }
                }

                if let Some(node) = self.graph.nodes.get_mut(task.1) {
                    node.in_queue = false;
                    if node.in_degree != 0 {
                        continue;
                    }
                    node.in_flight = true;
                    let node = node.clone();

                    // 区块可作为相邻任务写缓存的一部分被推进。
                    // 在这种情况下，其排队的节点可能存活下来，即使返回的
                    // ProtoChunk 已达到该阶段。分发过期的节点
                    // 会把同一阶段执行两次，从而破坏 ProtoChunk 的阶段不变式。
                    let actual_stage = self
                        .chunk_map
                        .get(&node.pos)
                        .and_then(|holder| holder.chunk.as_ref())
                        .map(Chunk::get_stage_id);
                    if actual_stage.is_some_and(|stage| stage >= node.stage as u8) {
                        if let Some(holder) = self.chunk_map.get_mut(&node.pos) {
                            holder.current_stage = holder
                                .current_stage
                                .max(StagedChunkEnum::from(actual_stage.expect("上文已检查")));
                            let task_slot = &mut holder.tasks[node.stage as usize];
                            if *task_slot == task.1 {
                                *task_slot = NodeKey::null();
                            }
                        }
                        self.waiting_for_chunks.remove(&task.1);
                        self.drop_node(task.1);
                        continue;
                    }

                    // 若区块超出范围或不再被任何目标/依赖需要，则取消/丢弃任务
                    let effective_target = self
                        .chunk_map
                        .get(&node.pos)
                        .map_or(StagedChunkEnum::None, |h| {
                            h.target_stage.max(h.dependency_stage)
                        });

                    if node.stage > effective_target {
                        if let Some(holder) = self.chunk_map.get_mut(&node.pos) {
                            let task_slot = &mut holder.tasks[node.stage as usize];
                            if *task_slot == task.1 {
                                *task_slot = NodeKey::null();
                            }
                        }
                        self.waiting_for_chunks.remove(&task.1);
                        self.drop_node(task.1);
                        continue;
                    }

                    if node.stage == StagedChunkEnum::Empty {
                        self.running_task_count += 1;
                        let holder = self.chunk_map.get_mut(&node.pos).expect("持有者应存在");
                        debug_assert!(holder.occupied.is_null());
                        debug_assert_eq!(holder.current_stage, StagedChunkEnum::None);
                        let occupy = self.graph.nodes.insert(Node::new(
                            ChunkPos::new(i32::MAX, i32::MAX),
                            StagedChunkEnum::None,
                        ));
                        let effective_target = holder.target_stage.max(holder.dependency_stage);
                        for i in (holder.current_stage as usize + 1)..=(effective_target as usize) {
                            self.graph.add_edge(occupy, holder.tasks[i]);
                        }
                        holder.occupied = occupy;

                        io_batch.push(node.pos);
                        if io_batch.len() >= 16
                            && self
                                .io_read
                                .blocking_send(std::mem::take(&mut io_batch))
                                .is_err()
                        {
                            info!("IO 读取线程已关闭，正在保存剩余区块...");
                            self.save_all_chunk(true);
                            break 'out2;
                        }
                    } else {
                        // 在开始生成之前发送所有待处理的 IO 批次
                        if !io_batch.is_empty()
                            && self
                                .io_read
                                .blocking_send(std::mem::take(&mut io_batch))
                                .is_err()
                        {
                            info!("IO 读取线程已关闭，正在保存剩余区块...");
                            self.save_all_chunk(true);
                            break 'out2;
                        }

                        let write_radius = node.stage.get_write_radius();
                        let read_radius = node.stage.get_read_radius();

                        // 预校验读取区域内的每个区块都有其数据
                        // 在对邻居做快照或交换写入区之前。
                        //
                        // 依赖图能保证前驱*任务*已完成，但
                        // 在生成
                        // 线程上完成的任务与其区块数据被放回持有器之间存在短暂窗口。任何
                        // 阶段，若其写入区域与当前正在运行的任务重叠，将会
                        // 在该窗口中看到 chunk==None。我们在此停驻并让
                        // 待全部数据到达后由 check_waiting_tasks() 重新入队。
                        {
                            let all_ready = (-read_radius..=read_radius).all(|dx| {
                                (-read_radius..=read_radius).all(|dy| {
                                    self.chunk_map
                                        .get(&node.pos.add_raw(dx, dy))
                                        .is_some_and(|h| h.chunk.is_some())
                                })
                            });

                            if !all_ready {
                                if let Some(n) = self.graph.nodes.get_mut(task.1) {
                                    n.in_queue = false;
                                    n.in_flight = false;
                                }
                                self.waiting_for_chunks.insert(task.1);
                                // 消除 TOCTOU 窗口：我们正在等待的区块可能
                                // 已在此前发生的 recv_chunk 排空中到达
                                // 在同一轮循环迭代中、本任务被挂起之前。
                                // 如果这样，check_waiting_tasks() 会立即将其重新入队
                                // 这样它就不会在 running_task_count==0 时被搁置。
                                self.check_waiting_tasks();
                                continue;
                            }
                        }

                        let mut cache = Cache::new(
                            node.pos.x - write_radius,
                            node.pos.y - write_radius,
                            write_radius << 1 | 1,
                        );

                        if node.stage == StagedChunkEnum::Surface {
                            let mut neighborhood =
                                SurfaceBiomeNeighborhood::new(node.pos.x, node.pos.y);
                            for dx in -1..=1 {
                                for dz in -1..=1 {
                                    let holder = self
                                        .chunk_map
                                        .get(&node.pos.add_raw(dx, dz))
                                        .expect("Surface 生物群系依赖的持有者应存在");
                                    let chunk =
                                        holder.chunk.as_ref().expect("Surface 生物群系依赖应可用");
                                    assert!(
                                        neighborhood.push_chunk(chunk),
                                        "surface biome dependency has an incomplete palette"
                                    );
                                }
                            }
                            cache.set_surface_biomes(neighborhood);
                        }

                        let occupy = self.graph.nodes.insert(Node::new(
                            ChunkPos::new(i32::MAX, i32::MAX),
                            StagedChunkEnum::None,
                        ));

                        for dx in -write_radius..=write_radius {
                            for dy in -write_radius..=write_radius {
                                let new_pos = node.pos.add_raw(dx, dy);
                                let holder =
                                    self.chunk_map.get_mut(&new_pos).expect("持有者应存在");
                                let mut tmp = None;
                                swap(&mut tmp, &mut holder.chunk);
                                let Some(tmp) = tmp else {
                                    panic!(
                                        "处理 {:?} 的 {:?} 阶段生成任务时，位置 {:?} 缺少区块",
                                        node.pos, node.stage, new_pos
                                    )
                                };
                                match tmp {
                                    Chunk::Level(chunk) => {
                                        cache.chunks.push(Chunk::Level(chunk));
                                    }
                                    Chunk::Proto(chunk) => {
                                        cache.chunks.push(Chunk::Proto(chunk));
                                    }
                                }

                                debug_assert!(holder.occupied.is_null());

                                let mut cur_edge = holder.occupied_by;
                                let mut prev_edge = EdgeKey::null();
                                let mut change_head = None;
                                while !cur_edge.is_null() {
                                    let edge = self.graph.edges.get(cur_edge).expect("边应存在");
                                    if self.graph.nodes.contains_key(edge.to) {
                                        prev_edge = cur_edge;
                                        cur_edge = edge.next;
                                        self.graph.add_edge(occupy, edge.to);
                                    } else {
                                        let next = edge.next;
                                        self.graph.edges.remove(cur_edge);
                                        cur_edge = next;
                                        if prev_edge.is_null() {
                                            change_head = Some(next);
                                        } else {
                                            self.graph
                                                .edges
                                                .get_mut(prev_edge)
                                                .expect("边应存在")
                                                .next = next;
                                        }
                                    }
                                }
                                if let Some(next) = change_head {
                                    holder.occupied_by = next;
                                }

                                holder.occupied = occupy;
                            }
                        }

                        self.running_task_count += 1;
                        let pos = node.pos;
                        let stage = node.stage;
                        let send_chunk = self.send_chunk.clone();
                        let level = level.clone();

                        self.generation_pool.spawn(move || {
                            let result = crate::chunk_system::worker_logic::run_generation(
                                pos, cache, stage, &level,
                            );
                            let _ = send_chunk.send((pos, result));
                        });
                    }
                }
            }

            // 刷新所有剩余的 IO 批次
            if !io_batch.is_empty()
                && self
                    .io_read
                    .blocking_send(std::mem::take(&mut io_batch))
                    .is_err()
            {
                info!("IO 读取线程已关闭，正在保存剩余区块...");
                self.save_all_chunk(true);
            }

            // 5. 等待工作或结果
            if self.queue.is_empty() {
                if self.running_task_count > 0 {
                    match self.recv_chunk.recv_timeout(Duration::from_millis(5)) {
                        Ok((pos, data)) => {
                            self.receive_chunk(pos, data);
                            if self.resort_work(self.send_level.get()) {
                                self.garbage_collect_dependencies();
                            }
                        }
                        Err(crossbeam::channel::RecvTimeoutError::Timeout) => {
                            // 定期检查 LevelChannel 是否有新请求
                            if self.resort_work(self.send_level.get()) {
                                self.garbage_collect_dependencies();
                            }
                        }
                        Err(crossbeam::channel::RecvTimeoutError::Disconnected) => break,
                    }
                } else {
                    // 没有正在执行的任务，检查是否有已解除阻塞的等待任务或滞留的就绪任务
                    self.check_waiting_tasks();
                    let restored = Self::restore_ready_tasks(
                        &mut self.graph,
                        &mut self.queue,
                        &self.chunk_map,
                        &self.last_level,
                        &self.last_high_priority,
                        &self.waiting_for_chunks,
                    );
                    if restored > 0 || !self.queue.is_empty() {
                        debug!("已将 {restored} 个滞留的就绪区块任务恢复到生成队列");
                        if self.queue_dirty {
                            self.sort_queue();
                            self.queue_dirty = false;
                        }
                        continue;
                    }
                    debug_assert!(self.debug_check());
                    debug_assert_eq!(self.running_task_count, 0);
                    if self.resort_work(self.send_level.wait_and_get(level)) {
                        self.garbage_collect_dependencies();
                    }
                }
                if self.queue_dirty {
                    self.sort_queue();
                    self.queue_dirty = false;
                }
            } else if self.running_task_count >= self.max_in_flight {
                // 队列中有任务，但我们已达到最大在途容量。
                // 等待一个执行中的 worker 完成，而不是忙等自旋。
                match self.recv_chunk.recv_timeout(Duration::from_millis(5)) {
                    Ok((pos, data)) => {
                        self.receive_chunk(pos, data);
                        if self.resort_work(self.send_level.get()) {
                            self.garbage_collect_dependencies();
                        }
                    }
                    Err(crossbeam::channel::RecvTimeoutError::Timeout) => {
                        if self.resort_work(self.send_level.get()) {
                            self.garbage_collect_dependencies();
                        }
                    }
                    Err(crossbeam::channel::RecvTimeoutError::Disconnected) => break,
                }
                if self.queue_dirty {
                    self.sort_queue();
                    self.queue_dirty = false;
                }
            }
        }
        info!(
            "调度器：正在等待 {} 个生成任务完成",
            self.running_task_count
        );
        let mut wait_iterations = 0;
        let max_wait_iterations = 100; // 最长等待 5 秒
        while self.running_task_count > 0 && wait_iterations < max_wait_iterations {
            if let Ok((pos, data)) = self.recv_chunk.try_recv() {
                self.receive_chunk(pos, data);
                wait_iterations = 0;
            } else {
                wait_iterations += 1;
                if wait_iterations % 20 == 0 {
                    warn!(
                        "仍在等待 {} 个任务完成（已等待 {}ms）",
                        self.running_task_count,
                        wait_iterations * 50
                    );
                }
                thread::sleep(Duration::from_millis(50));
            }
        }

        if self.running_task_count > 0 {
            warn!("正在取消 {} 个进行中的生成任务", self.running_task_count);
            let mut nodes_to_drop = Vec::new();

            for holder in self.chunk_map.values_mut() {
                for task in &mut holder.tasks {
                    if !task.is_null() {
                        self.waiting_for_chunks.remove(task);
                        nodes_to_drop.push(*task);
                        *task = NodeKey::null();
                    }
                }

                if !holder.occupied.is_null()
                    && let Some(node) = self.graph.nodes.get(holder.occupied)
                    && node.pos.x == i32::MAX
                    && node.pos.y == i32::MAX
                {
                    nodes_to_drop.push(holder.occupied);
                    holder.occupied = NodeKey::null();
                }

                self.graph.drop_edge_chain(holder.occupied_by);
                holder.occupied_by = EdgeKey::null();
            }

            for node_key in nodes_to_drop {
                self.drop_node(node_key);
            }

            self.running_task_count = 0;
        }

        drop(self.io_write);

        let unreleased_count = self.graph.nodes.len();
        if unreleased_count > 0 {
            warn!("正在清理未完成任务遗留的 {} 个未释放节点", unreleased_count);
        }
        self.graph.edges.clear();
    }

    fn debug_check(&self) -> bool {
        if !self.graph.nodes.is_empty() {
            for (key, value) in &self.graph.nodes {
                error!("未释放节点 {key:?}: {value:?}");
            }
            panic!("节点数量错误");
        }
        for (pos, holder) in &self.chunk_map {
            for i in &holder.tasks {
                debug_assert!(i.is_null());
            }
            debug_assert_eq!(
                holder.target_stage,
                StagedChunkEnum::level_to_stage(
                    *self.last_level.get(pos).unwrap_or(&ChunkLoading::MAX_LEVEL)
                )
            );
            let effective = holder.target_stage.max(holder.dependency_stage);
            debug_assert!(holder.current_stage >= effective);
            debug_assert!(holder.occupied.is_null());
            if holder.current_stage != StagedChunkEnum::None {
                debug_assert_eq!(
                    holder.chunk.as_ref().expect("区块应存在").get_stage_id(),
                    holder.current_stage as u8
                );
            }
        }
        true
    }
}
