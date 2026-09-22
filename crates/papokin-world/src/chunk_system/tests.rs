use super::*;
use crate::chunk_system::dag::Node;
use crate::chunk_system::dag::NodeKey;
use slotmap::Key;
use std::collections::BinaryHeap;
use std::collections::HashMap;

#[test]
fn ensure_dependency_chain_builds_multistage_chain() {
    let mut graph = DAG::default();
    let mut queue = BinaryHeap::new();
    let last_level: ChunkLevel = HashMapType::default();
    let last_high_priority: Vec<ChunkPos> = Vec::new();

    let chunk_pos = ChunkPos::new(0, 0);

    // 在图中创建依赖该链的依赖节点
    let dependency_task = graph
        .nodes
        .insert(Node::new(ChunkPos::new(10, 10), StagedChunkEnum::Features));

    let mut holder = ChunkHolder {
        current_stage: StagedChunkEnum::None,
        ..Default::default()
    };

    // 构建一条直到 Surface 的链 (Empty -> ... -> Surface)
    GenerationSchedule::ensure_dependency_chain(
        &mut graph,
        &mut queue,
        &last_level,
        &last_high_priority,
        dependency_task,
        chunk_pos,
        &mut holder,
        StagedChunkEnum::Surface,
    );

    let start = (holder.current_stage as usize + 1).max(StagedChunkEnum::Empty as usize);
    let end = StagedChunkEnum::Surface as u8 as usize;

    // 确保任务已创建、存在于 DAG 中且正确链接
    for idx in start..=end {
        let key = holder.tasks[idx];
        assert!(!key.is_null(), "task {idx} was not created");

        let node = graph.nodes.get(key).expect("图缺少节点");

        if idx == start {
            assert_eq!(node.in_degree, 0, "Start task should have 0 in_degree");
        } else {
            assert_eq!(
                node.in_degree, 1,
                "Intermediate task {idx} should have in_degree of 1"
            );
        }
    }

    // 依赖节点的 in_degree 应当被递增
    let dep_node = graph.nodes.get(dependency_task).unwrap();
    assert_eq!(dep_node.in_degree, 1);

    // 入口任务本应已入队
    let queued = queue.pop().expect("队列应包含条目任务");
    assert_eq!(queued.node_key(), holder.tasks[start]);
}

#[test]
fn ensure_dependency_chain_resumes_partial_chain() {
    let mut graph = DAG::default();
    let mut queue = BinaryHeap::new();
    let last_level: ChunkLevel = HashMapType::default();
    let last_high_priority: Vec<ChunkPos> = Vec::new();

    let chunk_pos = ChunkPos::new(0, 0);
    let dependency_task = graph
        .nodes
        .insert(Node::new(ChunkPos::new(10, 10), StagedChunkEnum::Features));

    let mut holder = ChunkHolder {
        current_stage: StagedChunkEnum::Biomes,
        ..Default::default()
    };

    GenerationSchedule::ensure_dependency_chain(
        &mut graph,
        &mut queue,
        &last_level,
        &last_high_priority,
        dependency_task,
        chunk_pos,
        &mut holder,
        StagedChunkEnum::Surface,
    );

    // 动态计算 Biomes 之后的下一阶段，而非猜测
    let empty = StagedChunkEnum::Empty as usize;
    let start = (holder.current_stage as usize + 1).max(empty);

    let queued = queue.pop().expect("队列应包含条目任务");
    assert_eq!(
        queued.node_key(),
        holder.tasks[start],
        "Should resume directly from the next stage after Biomes"
    );

    let entry_node = graph.nodes.get(queued.node_key()).unwrap();
    assert_eq!(
        entry_node.in_degree, 0,
        "Resumed task should have 0 in_degree because previous stages are already done"
    );
}

#[test]
fn ensure_dependency_chain_does_nothing_if_already_met() {
    let mut graph = DAG::default();
    let mut queue = BinaryHeap::new();
    let last_level: ChunkLevel = HashMapType::default();
    let last_high_priority: Vec<ChunkPos> = Vec::new();

    let chunk_pos = ChunkPos::new(0, 0);
    let dependency_task = graph
        .nodes
        .insert(Node::new(ChunkPos::new(10, 10), StagedChunkEnum::Features));

    let mut holder = ChunkHolder {
        current_stage: StagedChunkEnum::Full,
        ..Default::default()
    };

    GenerationSchedule::ensure_dependency_chain(
        &mut graph,
        &mut queue,
        &last_level,
        &last_high_priority,
        dependency_task,
        chunk_pos,
        &mut holder,
        StagedChunkEnum::Surface, // 请求低于当前所处的阶段
    );

    // 确保该函数提前返回，未创建任何任务或入队任何内容
    for task in &holder.tasks {
        assert!(
            task.is_null(),
            "No tasks should be created if the stage requirement is already met"
        );
    }
    assert!(queue.is_empty(), "Nothing should be queued");
}

#[test]
fn ensure_dependency_chain_respects_occupied_lock() {
    let mut graph = DAG::default();
    let mut queue = BinaryHeap::new();
    let last_level: ChunkLevel = HashMapType::default();
    let last_high_priority: Vec<ChunkPos> = Vec::new();

    let chunk_pos = ChunkPos::new(0, 0);
    let dependency_task = graph
        .nodes
        .insert(Node::new(ChunkPos::new(10, 10), StagedChunkEnum::Features));

    // 创建“占据”节点，模拟另一线程/进程正在处理该区块
    let occupy_node = graph.nodes.insert(Node::new(
        ChunkPos::new(i32::MAX, i32::MAX),
        StagedChunkEnum::None,
    ));

    let mut holder = ChunkHolder {
        current_stage: StagedChunkEnum::None,
        occupied: occupy_node,
        ..Default::default()
    };

    GenerationSchedule::ensure_dependency_chain(
        &mut graph,
        &mut queue,
        &last_level,
        &last_high_priority,
        dependency_task,
        chunk_pos,
        &mut holder,
        StagedChunkEnum::Surface,
    );

    let start = StagedChunkEnum::Empty as usize;
    let entry_task = holder.tasks[start];

    let entry_node = graph.nodes.get(entry_task).unwrap();
    // 第一个任务应当依赖 occupy 节点完成！
    assert_eq!(
        entry_node.in_degree, 1,
        "Entry task should be blocked by the occupy node"
    );

    // 因此，队列必须为空。在 occupy 节点被移除之前，它不应触发。
    assert!(
        queue.is_empty(),
        "Task should not be queued because it is blocked by occupied status"
    );
}

#[test]
fn ensure_dependency_chain_early_return_skips_edge() {
    let mut graph = DAG::default();
    let mut queue = BinaryHeap::new();
    let last_level = HashMapType::default();
    let last_high_priority = Vec::new();

    let dependency_task = graph
        .nodes
        .insert(Node::new(ChunkPos::new(1, 1), StagedChunkEnum::Surface));

    // 创建已处于所需阶段的 holder（Features > Surface）
    let mut holder = ChunkHolder {
        current_stage: StagedChunkEnum::Features,
        target_stage: StagedChunkEnum::Features,
        ..Default::default()
    };

    // 要求 'Empty'，但持有者已处于 'Features'
    GenerationSchedule::ensure_dependency_chain(
        &mut graph,
        &mut queue,
        &last_level,
        &last_high_priority,
        dependency_task,
        ChunkPos::new(0, 0),
        &mut holder,
        StagedChunkEnum::Empty,
    );

    let dep_node = graph.nodes.get(dependency_task).unwrap();

    // 由于提前返回，dependency_task 的 in_degree 不应增加
    assert_eq!(
        dep_node.in_degree, 0,
        "Dependency task should not be blocked if the neighbor is already past the required stage"
    );
}

#[test]
fn completed_proto_stage_drops_all_satisfied_tasks() {
    let mut graph = DAG::default();
    let mut holder = ChunkHolder {
        current_stage: StagedChunkEnum::None,
        target_stage: StagedChunkEnum::StructureStart,
        ..Default::default()
    };

    for stage in StagedChunkEnum::Empty as usize..=StagedChunkEnum::StructureStart as usize {
        holder.tasks[stage] = graph.nodes.insert(Node::new(
            ChunkPos::new(0, 0),
            StagedChunkEnum::from(stage as u8),
        ));
    }

    for stage in StagedChunkEnum::Empty as usize..StagedChunkEnum::StructureStart as usize {
        graph.add_edge(holder.tasks[stage], holder.tasks[stage + 1]);
    }

    let returned_stage = StagedChunkEnum::Biomes as usize;
    for task_idx in
        (holder.current_stage as usize + 1)..=(returned_stage).min(holder.tasks.len() - 1)
    {
        if !holder.tasks[task_idx].is_null() {
            if let Some(old) = graph.nodes.remove(holder.tasks[task_idx]) {
                let mut edge = old.edge;
                while !edge.is_null() {
                    let cur = graph.edges.remove(edge).unwrap();
                    if let Some(node) = graph.nodes.get_mut(cur.to) {
                        node.in_degree -= 1;
                    }
                    edge = cur.next;
                }
            }
            holder.tasks[task_idx] = NodeKey::null();
        }
    }

    assert!(holder.tasks[StagedChunkEnum::Empty as usize].is_null());
    assert!(holder.tasks[StagedChunkEnum::Biomes as usize].is_null());
    assert!(!holder.tasks[StagedChunkEnum::StructureStart as usize].is_null());
    assert!(
        graph
            .nodes
            .get(holder.tasks[StagedChunkEnum::StructureStart as usize])
            .is_some()
    );
}

#[test]
fn cancellation_path_decrements_in_degree() {
    let mut graph = DAG::default();

    // 创建一个等待中的任务（in_degree = 1）
    let waiting_task_key = graph
        .nodes
        .insert(Node::new(ChunkPos::new(0, 0), StagedChunkEnum::Surface));
    let waiting_node = graph.nodes.get_mut(waiting_task_key).unwrap();
    waiting_node.in_degree = 1;

    // 创建该任务正在等待的占据节点
    let occupy_key = graph.nodes.insert(Node::new(
        ChunkPos::new(i32::MAX, i32::MAX),
        StagedChunkEnum::None,
    ));
    graph.add_edge(occupy_key, waiting_task_key);

    // 搭建区块映射以模拟进行中的任务
    let mut chunk_map = HashMap::new();
    let mut holder = ChunkHolder {
        current_stage: StagedChunkEnum::Empty,
        target_stage: StagedChunkEnum::Surface,
        occupied: occupy_key,
        ..Default::default()
    };
    holder.tasks[StagedChunkEnum::Surface as usize] = waiting_task_key;
    chunk_map.insert(ChunkPos::new(0, 0), holder);

    // 模拟 `work()` 中的取消逻辑
    let mut nodes_to_drop = Vec::new();
    for holder in chunk_map.values_mut() {
        for task in &mut holder.tasks {
            if !task.is_null() {
                nodes_to_drop.push(*task);
                *task = NodeKey::null();
            }
        }
        if !holder.occupied.is_null() {
            nodes_to_drop.push(holder.occupied);
            holder.occupied = NodeKey::null();
        }
    }

    // 使用正确的图边遍历来丢弃节点
    for node_key in nodes_to_drop {
        // 在测试内模拟 self.drop_node 逻辑
        if let Some(old) = graph.nodes.remove(node_key) {
            let mut edge = old.edge;
            while !edge.is_null() {
                let cur = graph.edges.remove(edge).unwrap();
                if let Some(node) = graph.nodes.get_mut(cur.to) {
                    node.in_degree -= 1;
                }
                edge = cur.next;
            }
        }
    }

    // 断言等待的任务已被正确清理
    assert!(
        graph.nodes.get(waiting_task_key).is_none(),
        "The waiting task should have been dropped during cancellation"
    );
}

#[test]
fn dag_drop_edge_chain_removes_all_edges() {
    let mut graph = DAG::default();
    let node1 = graph
        .nodes
        .insert(Node::new(ChunkPos::new(0, 0), StagedChunkEnum::Empty));
    let node2 = graph
        .nodes
        .insert(Node::new(ChunkPos::new(1, 0), StagedChunkEnum::Empty));

    let edge1 = graph.edges.insert(crate::chunk_system::dag::Edge::new(
        node1,
        crate::chunk_system::dag::EdgeKey::null(),
    ));
    let edge2 = graph
        .edges
        .insert(crate::chunk_system::dag::Edge::new(node2, edge1));

    assert_eq!(graph.edges.len(), 2);
    graph.drop_edge_chain(edge2);
    assert_eq!(graph.edges.len(), 0);
}

#[test]
fn dag_prune_edge_chain_removes_dead_target_edges() {
    let mut graph = DAG::default();
    let node_alive = graph
        .nodes
        .insert(Node::new(ChunkPos::new(0, 0), StagedChunkEnum::Empty));
    let node_dead = graph
        .nodes
        .insert(Node::new(ChunkPos::new(1, 0), StagedChunkEnum::Empty));

    let edge_alive = graph.edges.insert(crate::chunk_system::dag::Edge::new(
        node_alive,
        crate::chunk_system::dag::EdgeKey::null(),
    ));
    let edge_dead = graph
        .edges
        .insert(crate::chunk_system::dag::Edge::new(node_dead, edge_alive));

    // 从 graph.nodes 中移除 node_dead 以模拟已完成/已丢弃的任务
    graph.nodes.remove(node_dead);

    let mut head = edge_dead;
    let has_valid_task = graph.prune_edge_chain(&mut head);

    assert!(has_valid_task);
    assert_eq!(head, edge_alive);
    assert_eq!(graph.edges.len(), 1);
    assert!(graph.edges.contains_key(edge_alive));
    assert!(!graph.edges.contains_key(edge_dead));

    // 现在把 node_alive 也一并移除
    graph.nodes.remove(node_alive);
    let has_valid_task = graph.prune_edge_chain(&mut head);
    assert!(!has_valid_task);
    assert!(head.is_null());
    assert_eq!(graph.edges.len(), 0);
}

#[test]
fn cancelled_out_of_range_task_clears_holder_task_slot() {
    let mut graph = DAG::default();
    let mut chunk_map = HashMap::new();
    let pos = ChunkPos::new(0, 0);

    let stage = StagedChunkEnum::Biomes;
    let node_key = graph.nodes.insert(Node::new(pos, stage));

    let mut holder = ChunkHolder {
        current_stage: StagedChunkEnum::Empty,
        target_stage: StagedChunkEnum::None, // 区块已被降级/移出范围
        dependency_stage: StagedChunkEnum::None,
        ..Default::default()
    };
    holder.tasks[stage as usize] = node_key;
    chunk_map.insert(pos, holder);

    // 模拟当 node.stage > effective_target 时的任务处理循环取消
    let node = graph.nodes.get(node_key).unwrap().clone();
    let effective_target = chunk_map.get(&node.pos).map_or(StagedChunkEnum::None, |h| {
        h.target_stage.max(h.dependency_stage)
    });

    assert!(node.stage > effective_target);
    if let Some(holder) = chunk_map.get_mut(&node.pos) {
        let task_slot = &mut holder.tasks[node.stage as usize];
        if *task_slot == node_key {
            *task_slot = NodeKey::null();
        }
    }
    graph.fast_drop_node(node_key);

    let holder = chunk_map.get(&pos).unwrap();
    assert!(
        holder.tasks[stage as usize].is_null(),
        "Task slot must be null after cancellation"
    );
    assert!(
        holder.tasks.iter().all(Key::is_null),
        "All task slots must be null when idle"
    );
}
