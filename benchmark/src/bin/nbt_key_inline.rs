//! 轮次 41 基准：NBT 复合标签键内联（`Box<str>` → 内联枚举键）。
//!
//! 对照形态（`NbtMap` 的键存储）：
//! - 堆盒键（轮次 40 终态，生产现行）：每个键一次独立堆分配
//!   （`Box<str>`，请求字节按精确长度计），条目 48B；
//! - 内联键（评估臂）：≤22 字节的键（`x`/`Pos`/
//!   `minecraft:display_name` 等 NBT 键的绝大多数）直接内联进
//!   24B 枚举，仅超长键落堆盒；条目 56B（+8B/键）。
//!
//! 负载模型与轮次 40 相同（15000 个复合标签，确定性语料）：
//! - 5000 实体存档形（~25 键，嵌套属性/装备/brain 子复合）；
//! - 5000 方块实体形（~12 键，含 8 槽物品列表）；
//! - 5000 物品 `custom_data` 形（嵌套 3 层，~30 键）。
//!
//! 观测：语料驻留（净分配字节）、构建/查找/序列化/克隆的
//! 分配次数与耗时；语义等价由「序列化字节 → 生产解析器回读 →
//! canonical 键序无关指纹全等」闸门证明（两臂输出共用同一
//! 解析器与指纹函数，中立即裁判）。
//!
//! **结论性负结果**：驻留闸门未过（请求字节口径 +6%）——条目
//! +8B/键吃掉了短键分配的节省；按 Win32 堆 16B 粒度建模的真实
//! 口径亦仅约 −3%。Rust 布局下 16B 键枚举结构性不可行（双 16B
//! 载荷无 niche、加 tag 必须 24B），该设计无内存净收益，生产不
//! 收编（回退至轮次 40 终态）。基准与数据留档防重复探查。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

// ---------------------------------------------------------------------------
// 分配计数器（次数 + 字节）
// ---------------------------------------------------------------------------

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);
static FREED_BYTES: AtomicUsize = AtomicUsize::new(0);

struct Counting;

// SAFETY: 仅统计后原样转发系统分配器，不改变任何分配语义
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: 转发给系统分配器。
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        FREED_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
        // SAFETY: 转发给系统分配器。
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// （分配次数， 净驻留字节 = 累计分配 - 累计释放）
fn counters() -> (usize, usize) {
    (
        ALLOCS.load(Ordering::Relaxed),
        ALLOC_BYTES.load(Ordering::Relaxed) - FREED_BYTES.load(Ordering::Relaxed),
    )
}

// ---------------------------------------------------------------------------
// 语料：确定性伪随机与三类复合标签形状
// ---------------------------------------------------------------------------

/// 简单确定性伪随机（xorshift64*），语料形状逐条可复现
struct Rng(u64);

impl Rng {
    const fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

const ENTITY_N: usize = 5000;
const BLOCK_ENTITY_N: usize = 5000;
const CUSTOM_DATA_N: usize = 5000;

/// 单件物品（复用形状：方块实体 Items 槽与实体装备槽）
struct ItemSpec {
    id: u32,
    count: i8,
    damage: i32,
    custom: bool,
}

const fn item_spec(rng: &mut Rng) -> ItemSpec {
    ItemSpec {
        id: (rng.next() % 1200) as u32,
        count: (rng.next() % 64) as i8,
        damage: (rng.next() % 500) as i32,
        custom: rng.next().is_multiple_of(4),
    }
}

/// 实体存档形复合标签的字段规格（新旧两臂共用，保证同构）
struct EntitySpec {
    air: i16,
    health: f32,
    fire: i16,
    hurt_time: i16,
    uuid: [i32; 4],
    pos: [f64; 3],
    motion: [f64; 3],
    rotation: [f32; 2],
    equipment: [Option<ItemSpec>; 4],
    brain_memories: usize,
    paper_keys: usize,
}

fn entity_spec(idx: usize) -> EntitySpec {
    let mut rng = Rng(idx as u64 + 1);
    EntitySpec {
        air: (rng.next() % 300) as i16,
        health: (rng.next() % 20) as f32 * 0.5,
        fire: (rng.next() % 8) as i16 - 2,
        hurt_time: (rng.next() % 10) as i16,
        uuid: [
            rng.next() as i32,
            rng.next() as i32,
            rng.next() as i32,
            rng.next() as i32,
        ],
        pos: [
            rng.next() as f64 / 65536.0,
            rng.next() as f64 / 65536.0,
            rng.next() as f64 / 65536.0,
        ],
        motion: [
            rng.next() as f64 / 1e9,
            rng.next() as f64 / 1e9,
            rng.next() as f64 / 1e9,
        ],
        rotation: [rng.next() as f32 / 1e6, rng.next() as f32 / 1e6],
        equipment: [
            rng.next().is_multiple_of(3).then(|| item_spec(&mut rng)),
            rng.next().is_multiple_of(3).then(|| item_spec(&mut rng)),
            rng.next().is_multiple_of(4).then(|| item_spec(&mut rng)),
            rng.next().is_multiple_of(2).then(|| item_spec(&mut rng)),
        ],
        brain_memories: (rng.next() % 12) as usize,
        paper_keys: (rng.next() % 6) as usize,
    }
}

/// 方块实体形规格
struct BlockEntitySpec {
    x: i32,
    y: i32,
    z: i32,
    items: Vec<ItemSpec>,
    has_name: bool,
    locked: bool,
}

fn block_entity_spec(idx: usize) -> BlockEntitySpec {
    let mut rng = Rng(idx as u64 + 600_001);
    let items = (0..8).map(|_| item_spec(&mut rng)).collect();
    BlockEntitySpec {
        x: (rng.next() % 1000) as i32,
        y: (rng.next() % 256) as i32 - 64,
        z: (rng.next() % 1000) as i32,
        items,
        has_name: rng.next().is_multiple_of(3),
        locked: rng.next().is_multiple_of(5),
    }
}

/// 物品 `custom_data` 形规格（嵌套 3 层）
struct CustomDataSpec {
    display_name: u32,
    lore_lines: usize,
    flags: usize,
}

const fn custom_data_spec(idx: usize) -> CustomDataSpec {
    let mut rng = Rng(idx as u64 + 1_200_001);
    CustomDataSpec {
        display_name: (rng.next() % 10_000) as u32,
        lore_lines: (rng.next() % 4) as usize,
        flags: (rng.next() % 6) as usize,
    }
}

// ---------------------------------------------------------------------------
// 旧臂（生产现行）：插入序 Vec + Box<str> 键
// ---------------------------------------------------------------------------

mod old {
    /// 轮次 40 终态的标签（Compound 装载 `Box<str>` 键复合标签）
    #[derive(Clone)]
    #[allow(dead_code)] // 完整复刻生产枚举面（含语料未覆盖的变体）
    pub enum OldTag {
        Byte(i8),
        Short(i16),
        Int(i32),
        Long(i64),
        Float(f32),
        Double(f64),
        ByteArray(Box<[i8]>),
        String(Box<str>),
        List(Vec<Self>),
        Compound(BoxCompound),
        IntArray(Vec<i32>),
        LongArray(Vec<i64>),
    }

    /// 轮次 40 终态的复合标签：插入序 Vec + 逐键堆盒
    #[derive(Clone, Default)]
    pub struct BoxCompound {
        pub child_tags: Vec<(Box<str>, OldTag)>,
    }

    impl BoxCompound {
        #[must_use]
        pub const fn new() -> Self {
            Self {
                child_tags: Vec::new(),
            }
        }

        /// 复刻生产 put：新键总是分配键盒
        pub fn put(&mut self, name: &str, value: OldTag) {
            match self.child_tags.iter_mut().find(|(k, _)| k.as_ref() == name) {
                Some((_, slot)) => *slot = value,
                None => self.child_tags.push((name.into(), value)),
            }
        }

        pub fn put_list(&mut self, name: &str, value: Vec<OldTag>) {
            self.put(name, OldTag::List(value));
        }

        pub fn put_compound(&mut self, name: &str, value: Self) {
            self.put(name, OldTag::Compound(value));
        }

        pub fn put_short(&mut self, name: &str, v: i16) {
            self.put(name, OldTag::Short(v));
        }

        pub fn put_float(&mut self, name: &str, v: f32) {
            self.put(name, OldTag::Float(v));
        }

        pub fn put_bool(&mut self, name: &str, v: bool) {
            self.put(name, OldTag::Byte(i8::from(v)));
        }

        pub fn put_int(&mut self, name: &str, v: i32) {
            self.put(name, OldTag::Int(v));
        }

        #[must_use]
        pub fn get(&self, name: &str) -> Option<&OldTag> {
            self.child_tags
                .iter()
                .find(|(k, _)| k.as_ref() == name)
                .map(|(_, v)| v)
        }

        #[must_use]
        pub fn get_int(&self, name: &str) -> Option<i32> {
            self.get(name).and_then(|tag| match tag {
                OldTag::Int(v) => Some(*v),
                _ => None,
            })
        }

        #[must_use]
        pub fn get_float(&self, name: &str) -> Option<f32> {
            self.get(name).and_then(|tag| match tag {
                OldTag::Float(v) => Some(*v),
                _ => None,
            })
        }

        #[must_use]
        pub fn get_string(&self, name: &str) -> Option<&str> {
            self.get(name).and_then(|tag| match tag {
                OldTag::String(v) => Some(v.as_ref()),
                _ => None,
            })
        }
    }
}

use old::{BoxCompound, OldTag};

// ---------------------------------------------------------------------------
// 评估臂：键内联枚举（≤22B 内联，超长落堆）
// ---------------------------------------------------------------------------

mod new {
    /// 键内联形态（24B：`Inline{len,buf[22]}` 与 `Heap(Box<str>)`）
    #[derive(Clone)]
    pub enum KeyInline {
        /// ≤22 字节的键直接内联（无堆分配）
        Inline {
            /// 有效字节数
            len: u8,
            /// 键的 UTF-8 字节，仅前 `len` 字节有效
            buf: [u8; 22],
        },
        /// 超长键的堆盒形态
        Heap(Box<str>),
    }

    impl KeyInline {
        /// 以字节内容判定内联或堆形态构建键
        #[must_use]
        pub fn new(key: &str) -> Self {
            if key.len() <= 22 {
                let mut buf = [0u8; 22];
                buf[..key.len()].copy_from_slice(key.as_bytes());
                Self::Inline {
                    len: key.len() as u8,
                    buf,
                }
            } else {
                Self::Heap(key.into())
            }
        }

        /// 键的字符串视图
        #[must_use]
        pub fn as_str(&self) -> &str {
            match self {
                // SAFETY: 字节来源自有效 &str，此后仅整体复制
                Self::Inline { len, buf } => unsafe {
                    std::str::from_utf8_unchecked(&buf[..usize::from(*len)])
                },
                Self::Heap(s) => s,
            }
        }
    }

    /// 键内联形态的标签（Compound 装载 [`InlineCompound`]）。
    #[derive(Clone)]
    #[allow(dead_code)] // 完整复刻生产枚举面（含语料未覆盖的变体）
    pub enum NewTag {
        Byte(i8),
        Short(i16),
        Int(i32),
        Long(i64),
        Float(f32),
        Double(f64),
        ByteArray(Box<[i8]>),
        String(Box<str>),
        List(Vec<Self>),
        Compound(InlineCompound),
        IntArray(Vec<i32>),
        LongArray(Vec<i64>),
    }

    /// 键内联形态的复合标签
    #[derive(Clone, Default)]
    pub struct InlineCompound {
        pub child_tags: Vec<(KeyInline, NewTag)>,
    }

    impl InlineCompound {
        #[must_use]
        pub const fn new() -> Self {
            Self {
                child_tags: Vec::new(),
            }
        }

        /// 键内联 put：新键零分配（≤22B）
        pub fn put(&mut self, name: &str, value: NewTag) {
            match self.child_tags.iter_mut().find(|(k, _)| k.as_str() == name) {
                Some((_, slot)) => *slot = value,
                None => self.child_tags.push((KeyInline::new(name), value)),
            }
        }

        pub fn put_list(&mut self, name: &str, value: Vec<NewTag>) {
            self.put(name, NewTag::List(value));
        }

        pub fn put_compound(&mut self, name: &str, value: Self) {
            self.put(name, NewTag::Compound(value));
        }

        pub fn put_short(&mut self, name: &str, v: i16) {
            self.put(name, NewTag::Short(v));
        }

        pub fn put_float(&mut self, name: &str, v: f32) {
            self.put(name, NewTag::Float(v));
        }

        pub fn put_bool(&mut self, name: &str, v: bool) {
            self.put(name, NewTag::Byte(i8::from(v)));
        }

        pub fn put_int(&mut self, name: &str, v: i32) {
            self.put(name, NewTag::Int(v));
        }

        #[must_use]
        pub fn get(&self, name: &str) -> Option<&NewTag> {
            self.child_tags
                .iter()
                .find(|(k, _)| k.as_str() == name)
                .map(|(_, v)| v)
        }

        #[must_use]
        pub fn get_int(&self, name: &str) -> Option<i32> {
            self.get(name).and_then(|tag| match tag {
                NewTag::Int(v) => Some(*v),
                _ => None,
            })
        }

        #[must_use]
        pub fn get_float(&self, name: &str) -> Option<f32> {
            self.get(name).and_then(|tag| match tag {
                NewTag::Float(v) => Some(*v),
                _ => None,
            })
        }

        #[must_use]
        pub fn get_string(&self, name: &str) -> Option<&str> {
            self.get(name).and_then(|tag| match tag {
                NewTag::String(v) => Some(v.as_ref()),
                _ => None,
            })
        }
    }
}

use new::{InlineCompound, KeyInline, NewTag};

// ---------------------------------------------------------------------------
// 两臂构造器（同一规格，仅类型与键形态不同）
// ---------------------------------------------------------------------------

fn old_item(spec: &ItemSpec) -> BoxCompound {
    let mut item = BoxCompound::new();
    item.put(
        "id",
        OldTag::String(format!("minecraft:item_{}", spec.id).into()),
    );
    item.put("count", OldTag::Byte(spec.count));
    item.put("damage", OldTag::Int(spec.damage));
    if spec.custom {
        let mut custom = BoxCompound::new();
        custom.put("timestamp", OldTag::Int(spec.damage.wrapping_mul(3)));
        custom.put(
            "owner",
            OldTag::String(format!("player_{}", spec.id).into()),
        );
        item.put_compound("components", custom);
    }
    item
}

fn new_item(spec: &ItemSpec) -> InlineCompound {
    let mut item = InlineCompound::new();
    item.put(
        "id",
        NewTag::String(format!("minecraft:item_{}", spec.id).into()),
    );
    item.put("count", NewTag::Byte(spec.count));
    item.put("damage", NewTag::Int(spec.damage));
    if spec.custom {
        let mut custom = InlineCompound::new();
        custom.put("timestamp", NewTag::Int(spec.damage.wrapping_mul(3)));
        custom.put(
            "owner",
            NewTag::String(format!("player_{}", spec.id).into()),
        );
        item.put_compound("components", custom);
    }
    item
}

fn build_old_entity(spec: &EntitySpec) -> BoxCompound {
    let mut root = BoxCompound::new();
    root.put(
        "id",
        OldTag::String(format!("player_{}", spec.uuid[0] as u32 % 100).into()),
    );
    root.put_short("Air", spec.air);
    root.put_float("Health", spec.health);
    root.put_short("Fire", spec.fire);
    root.put_short("HurtTime", spec.hurt_time);
    root.put("UUID", OldTag::IntArray(spec.uuid.to_vec()));
    root.put_list("Pos", spec.pos.iter().map(|v| OldTag::Double(*v)).collect());
    root.put_list(
        "Motion",
        spec.motion.iter().map(|v| OldTag::Double(*v)).collect(),
    );
    root.put_list(
        "Rotation",
        spec.rotation.iter().map(|v| OldTag::Float(*v)).collect(),
    );
    root.put_float("FallDistance", spec.rotation[0] / 8.0);
    root.put_bool("Invulnerable", spec.paper_keys.is_multiple_of(2));
    root.put_int("PortalCooldown", spec.brain_memories as i32);

    let mut attributes = BoxCompound::new();
    attributes.put("minecraft:generic.max_health", OldTag::Double(20.0));
    attributes.put("minecraft:generic.movement_speed", OldTag::Double(0.25));
    if spec.paper_keys.is_multiple_of(3) {
        attributes.put("minecraft:generic.attack_damage", OldTag::Double(3.0));
    }
    root.put_compound("attributes", attributes);

    let mut armor = BoxCompound::new();
    for (i, slot) in spec.equipment.iter().enumerate() {
        if let Some(item) = slot {
            armor.put_compound(&format!("slot_{i}"), old_item(item));
        }
    }
    root.put_compound("ArmorItems", armor);

    let mut brain = BoxCompound::new();
    let mut memories = BoxCompound::new();
    for m in 0..spec.brain_memories {
        memories.put(&format!("memory_{m}"), OldTag::Int(m as i32 * 7));
    }
    brain.put_compound("memories", memories);
    root.put_compound("Brain", brain);

    for p in 0..spec.paper_keys {
        root.put(&format!("Paper.SpigotData_{p}"), OldTag::Int(p as i32));
    }
    root
}

fn build_new_entity(spec: &EntitySpec) -> InlineCompound {
    let mut root = InlineCompound::new();
    root.put(
        "id",
        NewTag::String(format!("player_{}", spec.uuid[0] as u32 % 100).into()),
    );
    root.put_short("Air", spec.air);
    root.put_float("Health", spec.health);
    root.put_short("Fire", spec.fire);
    root.put_short("HurtTime", spec.hurt_time);
    root.put("UUID", NewTag::IntArray(spec.uuid.to_vec()));
    root.put_list("Pos", spec.pos.iter().map(|v| NewTag::Double(*v)).collect());
    root.put_list(
        "Motion",
        spec.motion.iter().map(|v| NewTag::Double(*v)).collect(),
    );
    root.put_list(
        "Rotation",
        spec.rotation.iter().map(|v| NewTag::Float(*v)).collect(),
    );
    root.put_float("FallDistance", spec.rotation[0] / 8.0);
    root.put_bool("Invulnerable", spec.paper_keys.is_multiple_of(2));
    root.put_int("PortalCooldown", spec.brain_memories as i32);

    let mut attributes = InlineCompound::new();
    attributes.put("minecraft:generic.max_health", NewTag::Double(20.0));
    attributes.put("minecraft:generic.movement_speed", NewTag::Double(0.25));
    if spec.paper_keys.is_multiple_of(3) {
        attributes.put("minecraft:generic.attack_damage", NewTag::Double(3.0));
    }
    root.put_compound("attributes", attributes);

    let mut armor = InlineCompound::new();
    for (i, slot) in spec.equipment.iter().enumerate() {
        if let Some(item) = slot {
            armor.put_compound(&format!("slot_{i}"), new_item(item));
        }
    }
    root.put_compound("ArmorItems", armor);

    let mut brain = InlineCompound::new();
    let mut memories = InlineCompound::new();
    for m in 0..spec.brain_memories {
        memories.put(&format!("memory_{m}"), NewTag::Int(m as i32 * 7));
    }
    brain.put_compound("memories", memories);
    root.put_compound("Brain", brain);

    for p in 0..spec.paper_keys {
        root.put(&format!("Paper.SpigotData_{p}"), NewTag::Int(p as i32));
    }
    root
}

fn build_old_block_entity(spec: &BlockEntitySpec) -> BoxCompound {
    let mut root = BoxCompound::new();
    root.put_int("x", spec.x);
    root.put_int("y", spec.y);
    root.put_int("z", spec.z);
    root.put(
        "id",
        OldTag::String(format!("container_{}", (spec.x + spec.z) as u32 % 60).into()),
    );
    root.put_list(
        "Items",
        spec.items
            .iter()
            .map(|i| OldTag::Compound(old_item(i)))
            .collect(),
    );
    if spec.has_name {
        let serial = (spec.x * 31 + spec.z) & 0xFF;
        root.put(
            "CustomName",
            OldTag::String(format!("容器 #{serial}").into()),
        );
    }
    if spec.locked {
        root.put("Lock", OldTag::String("key".into()));
    }
    root
}

fn build_new_block_entity(spec: &BlockEntitySpec) -> InlineCompound {
    let mut root = InlineCompound::new();
    root.put_int("x", spec.x);
    root.put_int("y", spec.y);
    root.put_int("z", spec.z);
    root.put(
        "id",
        NewTag::String(format!("container_{}", (spec.x + spec.z) as u32 % 60).into()),
    );
    root.put_list(
        "Items",
        spec.items
            .iter()
            .map(|i| NewTag::Compound(new_item(i)))
            .collect(),
    );
    if spec.has_name {
        let serial = (spec.x * 31 + spec.z) & 0xFF;
        root.put(
            "CustomName",
            NewTag::String(format!("容器 #{serial}").into()),
        );
    }
    if spec.locked {
        root.put("Lock", NewTag::String("key".into()));
    }
    root
}

fn build_old_custom_data(spec: &CustomDataSpec) -> BoxCompound {
    let mut root = BoxCompound::new();
    let mut display = BoxCompound::new();
    display.put(
        "Name",
        OldTag::String(format!("物品 {}", spec.display_name).into()),
    );
    if spec.lore_lines > 0 {
        display.put_list(
            "Lore",
            (0..spec.lore_lines)
                .map(|i| OldTag::String(format!("第 {i} 行").into()))
                .collect(),
        );
    }
    let mut components = BoxCompound::new();
    components.put_compound("minecraft:display_name", display);
    let mut custom = BoxCompound::new();
    custom.put_int("serial", spec.display_name as i32);
    for f in 0..spec.flags {
        custom.put(
            &format!("flag_{f}"),
            OldTag::Byte(i8::from(f.is_multiple_of(2))),
        );
    }
    components.put_compound("minecraft:custom_data", custom);
    root.put_compound("components", components);
    root
}

fn build_new_custom_data(spec: &CustomDataSpec) -> InlineCompound {
    let mut root = InlineCompound::new();
    let mut display = InlineCompound::new();
    display.put(
        "Name",
        NewTag::String(format!("物品 {}", spec.display_name).into()),
    );
    if spec.lore_lines > 0 {
        display.put_list(
            "Lore",
            (0..spec.lore_lines)
                .map(|i| NewTag::String(format!("第 {i} 行").into()))
                .collect(),
        );
    }
    let mut components = InlineCompound::new();
    components.put_compound("minecraft:display_name", display);
    let mut custom = InlineCompound::new();
    custom.put_int("serial", spec.display_name as i32);
    for f in 0..spec.flags {
        custom.put(
            &format!("flag_{f}"),
            NewTag::Byte(i8::from(f.is_multiple_of(2))),
        );
    }
    components.put_compound("minecraft:custom_data", custom);
    root.put_compound("components", components);
    root
}

// ---------------------------------------------------------------------------
// 两臂序列化：忠实复刻 NBT 二进制格式（插入序直写）
// ---------------------------------------------------------------------------

macro_rules! define_serializer {
    ($type_id:ident, $write_payload:ident, $serialize:ident, $tag:ident, $compound:ident, $lift:ident, $docs:literal) => {
        #[doc = $docs]
        const fn $type_id(tag: &$tag) -> u8 {
            match tag {
                $tag::Byte(_) => 1,
                $tag::Short(_) => 2,
                $tag::Int(_) => 3,
                $tag::Long(_) => 4,
                $tag::Float(_) => 5,
                $tag::Double(_) => 6,
                $tag::ByteArray(_) => 7,
                $tag::String(_) => 8,
                $tag::List(_) => 9,
                $tag::Compound(_) => 10,
                $tag::IntArray(_) => 11,
                $tag::LongArray(_) => 12,
            }
        }

        #[doc = $docs]
        fn $write_payload(buf: &mut Vec<u8>, tag: &$tag) {
            match tag {
                $tag::Byte(v) => buf.extend_from_slice(&v.to_be_bytes()),
                $tag::Short(v) => buf.extend_from_slice(&v.to_be_bytes()),
                $tag::Int(v) => buf.extend_from_slice(&v.to_be_bytes()),
                $tag::Long(v) => buf.extend_from_slice(&v.to_be_bytes()),
                $tag::Float(v) => buf.extend_from_slice(&v.to_be_bytes()),
                $tag::Double(v) => buf.extend_from_slice(&v.to_be_bytes()),
                $tag::ByteArray(v) => {
                    buf.extend_from_slice(&(v.len() as i32).to_be_bytes());
                    for &b in v {
                        buf.extend_from_slice(&b.to_be_bytes());
                    }
                }
                $tag::String(s) => {
                    buf.extend_from_slice(&(s.len() as u16).to_be_bytes());
                    buf.extend_from_slice(s.as_bytes());
                }
                $tag::List(list) => {
                    let elem_id = list.first().map_or(0, $type_id);
                    buf.push(elem_id);
                    buf.extend_from_slice(&(list.len() as i32).to_be_bytes());
                    for item in list {
                        $write_payload(buf, item);
                    }
                }
                $tag::Compound(compound) => {
                    for (name, tag) in &compound.child_tags {
                        buf.push($type_id(tag));
                        let key = name_key(name);
                        buf.extend_from_slice(&(key.len() as u16).to_be_bytes());
                        buf.extend_from_slice(key.as_bytes());
                        $write_payload(buf, tag);
                    }
                    buf.push(0);
                }
                $tag::IntArray(v) => {
                    buf.extend_from_slice(&(v.len() as i32).to_be_bytes());
                    for i in v {
                        buf.extend_from_slice(&i.to_be_bytes());
                    }
                }
                $tag::LongArray(v) => {
                    buf.extend_from_slice(&(v.len() as i32).to_be_bytes());
                    for l in v {
                        buf.extend_from_slice(&l.to_be_bytes());
                    }
                }
            }
        }

        #[doc = $docs]
        fn $serialize(root: &$compound) -> Vec<u8> {
            let mut buf = Vec::new();
            // 与生产 write_unnamed 对齐：根复合标签的类型 ID 打头
            buf.push(10);
            let root_tag = $lift(root);
            $write_payload(&mut buf, &root_tag);
            buf
        }
    };
}

/// 两臂键的统一 `&str` 视图（序列化宏共用）
trait KeyView {
    fn key_str(&self) -> &str;
}

impl KeyView for Box<str> {
    fn key_str(&self) -> &str {
        self
    }
}

impl KeyView for KeyInline {
    fn key_str(&self) -> &str {
        self.as_str()
    }
}

/// 宏内部的键视图转发
fn name_key(name: &impl KeyView) -> &str {
    name.key_str()
}

/// 宏内部的复合标签→标签提升（旧臂）
fn root_to_tag_old(root: &BoxCompound) -> OldTag {
    OldTag::Compound(root.clone())
}

/// 宏内部的复合标签→标签提升（评估臂）
fn root_to_tag_new(root: &InlineCompound) -> NewTag {
    NewTag::Compound(root.clone())
}

define_serializer!(
    old_type_id,
    old_write_payload,
    old_serialize_arm,
    OldTag,
    BoxCompound,
    root_to_tag_old,
    "旧臂（轮次 40 终态）序列化件。"
);
define_serializer!(
    new_type_id,
    new_write_payload,
    new_serialize_arm,
    NewTag,
    InlineCompound,
    root_to_tag_new,
    "评估臂（键内联）序列化件。"
);

// ---------------------------------------------------------------------------
// canonical 指纹：生产解析器回读（两臂共用中立即裁判）
// ---------------------------------------------------------------------------

const fn fp_mix(h: u64, v: u64) -> u64 {
    let mut x = h ^ v;
    x = x.wrapping_mul(0x0000_0100_0000_01B3);
    x.rotate_left(17)
}

fn str_fp(s: &str) -> u64 {
    let mut fp = 0xCBF2_9CE4_8422_2325u64;
    for (i, b) in s.bytes().enumerate() {
        fp = fp_mix(fp, u64::from(b) ^ ((i as u64) << 8));
    }
    fp
}

fn tag_fp(tag: &papokin_nbt::tag::NbtTag) -> u64 {
    use papokin_nbt::tag::NbtTag;
    match tag {
        NbtTag::End => 0,
        NbtTag::Byte(v) => fp_mix(1, *v as u64),
        NbtTag::Short(v) => fp_mix(2, *v as u64),
        NbtTag::Int(v) => fp_mix(3, *v as u64),
        NbtTag::Long(v) => fp_mix(4, u64::try_from(*v).unwrap_or(u64::MAX)),
        NbtTag::Float(v) => fp_mix(5, u64::from(v.to_bits())),
        NbtTag::Double(v) => fp_mix(6, v.to_bits()),
        NbtTag::ByteArray(arr) => arr_fp(7, arr.iter().map(|v| *v as u64)),
        NbtTag::String(s) => fp_mix(8, str_fp(s)),
        NbtTag::List(list) => {
            let mut fp = 9u64;
            for (i, item) in list.iter().enumerate() {
                fp = fp_mix(fp, tag_fp(item) ^ (i as u64));
            }
            fp
        }
        NbtTag::Compound(compound) => fp_mix(10, compound_fp(compound)),
        NbtTag::IntArray(arr) => arr_fp(11, arr.iter().map(|v| *v as u64)),
        NbtTag::LongArray(arr) => arr_fp(12, arr.iter().map(|v| *v as u64)),
    }
}

fn arr_fp(domain: u64, iter: impl Iterator<Item = u64>) -> u64 {
    let mut fp = domain;
    for (i, v) in iter.enumerate() {
        fp = fp_mix(fp, v ^ (i as u64));
    }
    fp
}

/// 复合标签 canonical 指纹：子键顺序无关（XOR 折叠）
fn compound_fp(compound: &papokin_nbt::compound::NbtCompound) -> u64 {
    let mut fp = 0u64;
    for (key, tag) in &compound.child_tags {
        fp ^= fp_mix(str_fp(key), tag_fp(tag));
    }
    fp
}

/// 字节 → 生产解析器 → canonical 指纹（两臂共用中立即裁判）
fn bytes_to_fp(bytes: &[u8]) -> u64 {
    let mut reader = papokin_nbt::deserializer::NbtReadHelperJava::new(std::io::Cursor::new(bytes));
    let nbt = papokin_nbt::Nbt::read_unnamed(&mut reader).unwrap_or_else(|err| {
        eprintln!("解析失败：{err}");
        std::process::exit(1);
    });
    compound_fp(&papokin_nbt::compound::NbtCompound::from(nbt))
}

// ---------------------------------------------------------------------------
// 观测过程
// ---------------------------------------------------------------------------

/// 查找电池：既有键 + 缺失键混合取值，折叠命中值指纹（泛化两臂）
fn lookup<C: CompoundLookup>(corpus: &[C]) -> (u64, usize) {
    let mut fp = 0x00C0_FFEEu64;
    let mut hits = 0usize;
    for (i, compound) in corpus.iter().enumerate() {
        match i % 3 {
            0 => {
                if compound.get_int_opt("Air").is_some() {
                    hits += 1;
                }
                if let Some(name) = compound.get_string_opt("CustomName") {
                    fp = fp_mix(fp, str_fp(&name));
                } else {
                    fp = fp_mix(fp, 0xABCD);
                }
            }
            1 => {
                if compound.get_int_opt("x").is_some() {
                    hits += 1;
                }
                fp = fp_mix(fp, compound.has_key("missing") as u64);
            }
            _ => {
                if compound.has_key("serial") {
                    hits += 1;
                }
                fp = fp_mix(
                    fp,
                    compound
                        .get_float_opt("Health")
                        .map_or(0xFEED, f32::to_bits) as u64,
                );
            }
        }
    }
    (fp, hits)
}

/// 查找电池对两臂复合标签的操作面
trait CompoundLookup {
    fn get_int_opt(&self, name: &str) -> Option<i32>;
    fn get_float_opt(&self, name: &str) -> Option<f32>;
    fn get_string_opt(&self, name: &str) -> Option<String>;
    fn has_key(&self, name: &str) -> bool;
    fn entry_len(&self) -> usize;
}

impl CompoundLookup for BoxCompound {
    fn get_int_opt(&self, name: &str) -> Option<i32> {
        self.get_int(name)
    }
    fn get_float_opt(&self, name: &str) -> Option<f32> {
        self.get_float(name)
    }
    fn get_string_opt(&self, name: &str) -> Option<String> {
        self.get_string(name).map(str::to_string)
    }
    fn has_key(&self, name: &str) -> bool {
        self.get(name).is_some()
    }
    fn entry_len(&self) -> usize {
        self.child_tags.len()
    }
}

impl CompoundLookup for InlineCompound {
    fn get_int_opt(&self, name: &str) -> Option<i32> {
        self.get_int(name)
    }
    fn get_float_opt(&self, name: &str) -> Option<f32> {
        self.get_float(name)
    }
    fn get_string_opt(&self, name: &str) -> Option<String> {
        self.get_string(name).map(str::to_string)
    }
    fn has_key(&self, name: &str) -> bool {
        self.get(name).is_some()
    }
    fn entry_len(&self) -> usize {
        self.child_tags.len()
    }
}

#[derive(Default)]
struct ArmResult {
    resident_bytes: usize,
    build_allocs: usize,
    build_time_ms: f64,
    lookup_fp: u64,
    lookup_hits: usize,
    lookup_allocs: usize,
    lookup_time_us: f64,
    serialize_fp: u64,
    serialize_allocs: usize,
    serialize_time_ms: f64,
    clone_allocs: usize,
    clone_time_ms: f64,
}

/// 构建旧臂语料（实体/方块实体/`custom_data` 三形混合）
fn build_old_corpus() -> Vec<BoxCompound> {
    let mut corpus = Vec::with_capacity(ENTITY_N + BLOCK_ENTITY_N + CUSTOM_DATA_N);
    for i in 0..ENTITY_N {
        corpus.push(build_old_entity(&entity_spec(i)));
    }
    for i in 0..BLOCK_ENTITY_N {
        corpus.push(build_old_block_entity(&block_entity_spec(i)));
    }
    for i in 0..CUSTOM_DATA_N {
        corpus.push(build_old_custom_data(&custom_data_spec(i)));
    }
    corpus
}

/// 构建评估臂语料（与旧臂同构）
fn build_new_corpus() -> Vec<InlineCompound> {
    let mut corpus = Vec::with_capacity(ENTITY_N + BLOCK_ENTITY_N + CUSTOM_DATA_N);
    for i in 0..ENTITY_N {
        corpus.push(build_new_entity(&entity_spec(i)));
    }
    for i in 0..BLOCK_ENTITY_N {
        corpus.push(build_new_block_entity(&block_entity_spec(i)));
    }
    for i in 0..CUSTOM_DATA_N {
        corpus.push(build_new_custom_data(&custom_data_spec(i)));
    }
    corpus
}

/// 两臂共用的查找/序列化/克隆段测量（泛化，逐段同构）
fn segments<C: CompoundLookup + Clone>(
    corpus: &[C],
    serialize: fn(&C) -> Vec<u8>,
    mut r: ArmResult,
) -> ArmResult {
    let (before, _) = counters();
    let start = Instant::now();
    let (fp, hits) = lookup(corpus);
    r.lookup_time_us = start.elapsed().as_secs_f64() * 1e6;
    let (after, _) = counters();
    r.lookup_fp = fp;
    r.lookup_hits = hits;
    r.lookup_allocs = after - before;

    let (before, _) = counters();
    let start = Instant::now();
    let mut ser_fp = 0u64;
    for compound in corpus {
        let bytes = serialize(compound);
        ser_fp = fp_mix(ser_fp, bytes_to_fp(&bytes));
    }
    r.serialize_time_ms = start.elapsed().as_secs_f64() * 1e3;
    let (after, _) = counters();
    r.serialize_fp = ser_fp;
    r.serialize_allocs = after - before;

    let (before, _) = counters();
    let start = Instant::now();
    let mut sink = 0usize;
    for compound in corpus.iter().step_by(7) {
        let cloned = compound.clone();
        sink += cloned.entry_len();
    }
    std::hint::black_box(sink);
    r.clone_time_ms = start.elapsed().as_secs_f64() * 1e3;
    let (after, _) = counters();
    r.clone_allocs = after - before;
    r
}

/// 单臂全流程：构建计量 + 分段测量，语料随函数返回一并释放
fn run_arm<T>(
    build: fn() -> Vec<T>,
    segments_fn: fn(&[T], ArmResult) -> ArmResult,
    base: (usize, usize),
) -> ArmResult {
    let (before_allocs, base_resident) = base;
    let start = Instant::now();
    let corpus = build();
    let build_time = start.elapsed();
    let (after_allocs, now_resident) = counters();
    let r = ArmResult {
        resident_bytes: now_resident.saturating_sub(base_resident),
        build_allocs: after_allocs - before_allocs,
        build_time_ms: build_time.as_secs_f64() * 1e3,
        ..ArmResult::default()
    };
    segments_fn(&corpus, r)
}

/// 递归累计键总数与内联键数
fn walk_keys(compound: &InlineCompound, total: &mut usize, inlined: &mut usize) {
    for (key, tag) in &compound.child_tags {
        *total += 1;
        if matches!(key, KeyInline::Inline { .. }) {
            *inlined += 1;
        }
        if let NewTag::Compound(inner) = tag {
            walk_keys(inner, total, inlined);
        }
        if let NewTag::List(list) = tag {
            for item in list {
                if let NewTag::Compound(inner) = item {
                    walk_keys(inner, total, inlined);
                }
            }
        }
    }
}

/// 统计语料中内联键占比（形状说明，不参与闸门）
fn inline_ratio(corpus: &[InlineCompound]) -> (usize, usize) {
    let mut total = 0usize;
    let mut inlined = 0usize;
    for compound in corpus.iter().step_by(11) {
        walk_keys(compound, &mut total, &mut inlined);
    }
    (inlined, total)
}

/// 汇总闸门判定与细节描述（负结果档案：驻留闸门预期不通过）
fn build_gates(old: &ArmResult, new: &ArmResult) -> Vec<(&'static str, bool, String)> {
    vec![
        (
            "构建内容 canonical 指纹全等",
            old.serialize_fp == new.serialize_fp,
            format!(
                "旧 {:#018x} vs 新 {:#018x}（经序列化→生产解析器回读）",
                old.serialize_fp, new.serialize_fp
            ),
        ),
        (
            "查找电池命中与值指纹全等",
            old.lookup_fp == new.lookup_fp && old.lookup_hits == new.lookup_hits,
            format!(
                "指纹 {:#018x} vs {:#018x}，命中 {} vs {}",
                old.lookup_fp, new.lookup_fp, old.lookup_hits, new.lookup_hits
            ),
        ),
        (
            "驻留字节下降（预期不通过，负结果依据）",
            new.resident_bytes < old.resident_bytes,
            format!(
                "{} → {} B（×{:.3}，条目 +8B/键 vs 逐键分配消除）",
                old.resident_bytes,
                new.resident_bytes,
                old.resident_bytes as f64 / new.resident_bytes as f64,
            ),
        ),
        (
            "构建分配次数下降",
            new.build_allocs < old.build_allocs,
            format!(
                "{} → {} 次（−{}）",
                old.build_allocs,
                new.build_allocs,
                old.build_allocs - new.build_allocs
            ),
        ),
    ]
}

/// 单臂数值的 JSON 形态
fn arm_json(r: &ArmResult) -> serde_json::Value {
    serde_json::json!({
        "resident_bytes": r.resident_bytes,
        "build_allocs": r.build_allocs,
        "build_time_ms": r.build_time_ms,
        "lookup_time_us": r.lookup_time_us,
        "lookup_allocs": r.lookup_allocs,
        "serialize_time_ms": r.serialize_time_ms,
        "serialize_allocs": r.serialize_allocs,
        "clone_time_ms": r.clone_time_ms,
        "clone_allocs": r.clone_allocs,
        "content_fingerprint": format!("{:#018x}", r.serialize_fp),
        "lookup_fingerprint": format!("{:#018x}", r.lookup_fp),
    })
}

/// 汇总两臂数值与闸门为 JSON 报告
fn build_report(
    old: &ArmResult,
    new: &ArmResult,
    gates: &[(&str, bool, String)],
    inline: (usize, usize),
) -> serde_json::Value {
    serde_json::json!({
        "round": 41,
        "topic": "nbt_key_inline",
        "outcome": "negative",
        "corpus": {
            "entities": ENTITY_N,
            "block_entities": BLOCK_ENTITY_N,
            "custom_data": CUSTOM_DATA_N,
            "inline_keys": inline.0,
            "total_keys_sampled": inline.1,
        },
        "sizes": {
            "key_old_bytes": 16,
            "key_new_bytes": std::mem::size_of::<KeyInline>(),
            "entry_old_bytes": 48,
            "entry_new_bytes": std::mem::size_of::<(KeyInline, NewTag)>(),
        },
        "old": arm_json(old),
        "new": arm_json(new),
        "gates": gates
            .iter()
            .map(|(name, ok, detail)| {
                serde_json::json!({
                    "name": name,
                    "passed": ok,
                    "detail": detail,
                })
            })
            .collect::<Vec<_>>(),
    })
}

fn main() {
    println!("轮次 41 基准（负结果档案）：NBT 复合标签键内联（Box<str> → 内联枚举）");
    println!(
        "键尺寸：旧臂 16 B（堆盒指针）· 新臂 {} B（内联枚举）· 条目：旧 48 B · 新 {} B",
        std::mem::size_of::<KeyInline>(),
        std::mem::size_of::<(KeyInline, NewTag)>(),
    );

    let old = run_arm(
        build_old_corpus,
        |c, r| segments(c, old_serialize_arm, r),
        counters(),
    );
    // 旧臂语料已随 run_arm 作用域结束释放，新臂计量不受其驻留影响
    let new = run_arm(
        build_new_corpus,
        |c, r| segments(c, new_serialize_arm, r),
        counters(),
    );

    // 抽样语料重建以统计内联占比（计量已完成，不污染闸门）
    let sample: Vec<InlineCompound> = (0..ENTITY_N)
        .step_by(11)
        .map(|i| build_new_entity(&entity_spec(i)))
        .collect();
    let inline = inline_ratio(&sample);
    println!(
        "内联键占比（实体形抽样）：{}/{}（{:.1}%）",
        inline.0,
        inline.1,
        inline.0 as f64 / inline.1 as f64 * 100.0
    );

    let gates = build_gates(&old, &new);

    println!("\n=== 闸门 ===");
    for (name, ok, detail) in &gates {
        println!("{} {name}：{detail}", if *ok { "√" } else { "×" });
    }

    println!("\n=== 数值 ===");
    println!(
        "驻留字节：{} → {}（×{:.3}）",
        old.resident_bytes,
        new.resident_bytes,
        old.resident_bytes as f64 / new.resident_bytes as f64
    );
    println!(
        "构建：{:.2} → {:.2} ms（×{:.2}），分配 {} → {} 次",
        old.build_time_ms,
        new.build_time_ms,
        old.build_time_ms / new.build_time_ms,
        old.build_allocs,
        new.build_allocs
    );
    println!(
        "查找：{:.0} → {:.0} µs（×{:.2}），分配 {} → {} 次",
        old.lookup_time_us,
        new.lookup_time_us,
        old.lookup_time_us / new.lookup_time_us,
        old.lookup_allocs,
        new.lookup_allocs
    );
    println!(
        "序列化+回读：{:.2} → {:.2} ms（×{:.2}），分配 {} → {} 次",
        old.serialize_time_ms,
        new.serialize_time_ms,
        old.serialize_time_ms / new.serialize_time_ms,
        old.serialize_allocs,
        new.serialize_allocs
    );
    println!(
        "深克隆：{:.2} → {:.2} ms（×{:.2}），分配 {} → {} 次",
        old.clone_time_ms,
        new.clone_time_ms,
        old.clone_time_ms / new.clone_time_ms,
        old.clone_allocs,
        new.clone_allocs
    );

    let report = build_report(&old, &new, &gates, inline);

    let path = "note/report/perf/round41-nbt-key-inline.json";
    match std::fs::File::create(path) {
        Ok(mut file) => {
            if let Err(err) = file.write_all(
                serde_json::to_string_pretty(&report)
                    .unwrap_or_else(|_| "{}".into())
                    .as_bytes(),
            ) {
                eprintln!("写入报告失败：{err}");
            }
        }
        Err(err) => eprintln!("创建报告失败：{err}"),
    }

    // 负结果档案：指纹闸门必须过（证明两臂同构），驻留闸门预期不过
    let semantics_ok = old.serialize_fp == new.serialize_fp
        && old.lookup_fp == new.lookup_fp
        && old.lookup_hits == new.lookup_hits;
    println!(
        "\n闸门判定：语义等价{}；驻留{}（负结果成立，生产不收编）",
        if semantics_ok { "通过" } else { "失败" },
        if new.resident_bytes < old.resident_bytes {
            "下降"
        } else {
            "上升"
        }
    );
    if !semantics_ok {
        std::process::exit(1);
    }
}
