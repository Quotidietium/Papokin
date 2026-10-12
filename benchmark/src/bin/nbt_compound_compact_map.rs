//! 轮次 40 基准：NBT 复合标签容器紧凑化（HashMap → 插入序 Vec）。
//!
//! 对照形态（`NbtCompound` 的承载容器）：
//! - 哈希映射（旧态）：`HashMap<Box<str>, NbtTag>`——每复合标签
//!   一张哈希表（表槽余量 + 控制字节）+ 逐键独立堆分配，
//!   `NbtTag` 因内联 48B 哈希映射而达 56B；
//! - 紧凑有序映射（新态）：插入序 `Vec<(Box<str>, NbtTag)>`——
//!   无表槽/控制字节开销，`NbtTag` 收缩到 32B，空复合内联 24B。
//!
//! 负载模型（15000 个复合标签，确定性语料）：
//! - 5000 实体存档形（~25 键，嵌套属性/装备/brain 子复合）；
//! - 5000 方块实体形（~12 键，含 8 槽物品列表）；
//! - 5000 物品 `custom_data` 形（嵌套 3 层，~30 键）。
//!
//! 观测：语料驻留（净分配字节）、构建/查找/序列化/克隆的
//! 分配次数与耗时；语义等价由「序列化字节 → 生产解析器回读 →
//! canonical 键序无关指纹全等」闸门证明（两臂输出共用同一
//! 解析器与指纹函数，中立即裁判）。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::io::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use papokin_nbt::compound::NbtCompound;
use papokin_nbt::tag::NbtTag;

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

// ---------------------------------------------------------------------------
// 旧臂：忠实复刻旧生产形态（HashMap 承载 + 56B 标签）
// ---------------------------------------------------------------------------

mod old {
    use std::collections::HashMap;

    /// 旧生产形态的标签（Compound 内联 HashMap，56 字节）
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
        Compound(OldCompound),
        IntArray(Vec<i32>),
        LongArray(Vec<i64>),
    }

    /// 旧生产形态的复合标签
    #[derive(Clone, Default)]
    pub struct OldCompound {
        pub child_tags: HashMap<Box<str>, OldTag>,
    }

    impl OldCompound {
        pub fn new() -> Self {
            Self {
                child_tags: HashMap::new(),
            }
        }

        /// 复刻旧 `NbtCompound::put`：总是先分配键盒再插入
        pub fn put(&mut self, name: &str, value: impl Into<OldTag>) {
            self.child_tags.insert(name.into(), value.into());
        }

        pub fn put_list(&mut self, name: &str, value: Vec<OldTag>) {
            self.put(name, OldTag::List(value));
        }

        pub fn put_compound(&mut self, name: &str, value: Self) {
            self.put(name, OldTag::Compound(value));
        }

        #[must_use]
        pub fn get(&self, name: &str) -> Option<&OldTag> {
            self.child_tags.get(name)
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
            match self.get(name) {
                Some(OldTag::String(v)) => Some(v),
                _ => None,
            }
        }
    }

    impl From<i8> for OldTag {
        fn from(v: i8) -> Self {
            Self::Byte(v)
        }
    }
    impl From<i16> for OldTag {
        fn from(v: i16) -> Self {
            Self::Short(v)
        }
    }
    impl From<i32> for OldTag {
        fn from(v: i32) -> Self {
            Self::Int(v)
        }
    }
    impl From<i64> for OldTag {
        fn from(v: i64) -> Self {
            Self::Long(v)
        }
    }
    impl From<f32> for OldTag {
        fn from(v: f32) -> Self {
            Self::Float(v)
        }
    }
    impl From<f64> for OldTag {
        fn from(v: f64) -> Self {
            Self::Double(v)
        }
    }
    impl From<String> for OldTag {
        fn from(v: String) -> Self {
            Self::String(v.into())
        }
    }
}

use old::{OldCompound, OldTag};

fn old_item(spec: &ItemSpec) -> OldCompound {
    let mut item = OldCompound::new();
    item.put("id", format!("minecraft:item_{}", spec.id));
    item.put("count", OldTag::Byte(spec.count));
    item.put_int_if("damage", spec.damage);
    if spec.custom {
        let mut custom = OldCompound::new();
        custom.put_int_if("timestamp", spec.damage.wrapping_mul(3));
        custom.put_string_key("owner", spec.id);
        item.put_compound("components", custom);
    }
    item
}

/// 与新臂共用形状的辅助（名字避开与 put 语义混淆，直接内联）
trait OldPutExt {
    fn put_int_if(&mut self, name: &str, v: i32);
    fn put_string_key(&mut self, name: &str, v: u32);
}

impl OldPutExt for OldCompound {
    fn put_int_if(&mut self, name: &str, v: i32) {
        self.put(name, OldTag::Int(v));
    }
    fn put_string_key(&mut self, name: &str, v: u32) {
        self.put(name, OldTag::String(format!("player_{v}").into()));
    }
}

fn new_item(spec: &ItemSpec) -> NbtCompound {
    let mut item = NbtCompound::new();
    item.put(
        "id",
        NbtTag::String(format!("minecraft:item_{}", spec.id).into()),
    );
    item.put("count", NbtTag::Byte(spec.count));
    item.put("damage", NbtTag::Int(spec.damage));
    if spec.custom {
        let mut custom = NbtCompound::new();
        custom.put("timestamp", NbtTag::Int(spec.damage.wrapping_mul(3)));
        custom.put(
            "owner",
            NbtTag::String(format!("player_{}", spec.id).into()),
        );
        item.put_compound("components", custom);
    }
    item
}

trait NewPutExt {
    fn put_int_if(&mut self, name: &str, v: i32);
    fn put_string_key(&mut self, name: &str, v: u32);
}

impl NewPutExt for NbtCompound {
    fn put_int_if(&mut self, name: &str, v: i32) {
        self.put(name, NbtTag::Int(v));
    }
    fn put_string_key(&mut self, name: &str, v: u32) {
        self.put(name, NbtTag::String(format!("player_{v}").into()));
    }
}

/// 实体存档形（旧臂）
fn build_old_entity(spec: &EntitySpec) -> OldCompound {
    let mut root = OldCompound::new();
    root.put_string_key("id", spec.uuid[0] as u32 % 100);
    root.put("Air", spec.air);
    root.put("Health", spec.health);
    root.put("Fire", spec.fire);
    root.put("HurtTime", spec.hurt_time);
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
    root.put("FallDistance", OldTag::Float(spec.rotation[0] / 8.0));
    root.put(
        "Invulnerable",
        OldTag::Byte(i8::from(spec.paper_keys.is_multiple_of(2))),
    );
    root.put("PortalCooldown", OldTag::Int(spec.brain_memories as i32));

    // 属性子复合（原版属性键）
    let mut attributes = OldCompound::new();
    attributes.put("minecraft:generic.max_health", OldTag::Double(20.0));
    attributes.put("minecraft:generic.movement_speed", OldTag::Double(0.25));
    if spec.paper_keys.is_multiple_of(3) {
        attributes.put("minecraft:generic.attack_damage", OldTag::Double(3.0));
    }
    root.put_compound("attributes", attributes);

    // 装备子复合（4 槽，复用物品形状）
    let mut armor = OldCompound::new();
    for (i, slot) in spec.equipment.iter().enumerate() {
        if let Some(item) = slot {
            armor.put_compound(&format!("slot_{i}"), old_item(item));
        }
    }
    root.put_compound("ArmorItems", armor);

    // brain 子复合（memories 数量可变）
    let mut brain = OldCompound::new();
    let mut memories = OldCompound::new();
    for m in 0..spec.brain_memories {
        memories.put_int_if(&format!("memory_{m}"), m as i32 * 7);
    }
    brain.put_compound("memories", memories);
    root.put_compound("Brain", brain);

    // Paper 保留键（Bukkit 世界常见）
    for p in 0..spec.paper_keys {
        root.put_int_if(&format!("Paper.SpigotData_{p}"), p as i32);
    }
    root
}

/// 实体存档形（新臂，同构）
fn build_new_entity(spec: &EntitySpec) -> NbtCompound {
    let mut root = NbtCompound::new();
    root.put_string_key("id", spec.uuid[0] as u32 % 100);
    root.put_short("Air", spec.air);
    root.put_float("Health", spec.health);
    root.put_short("Fire", spec.fire);
    root.put_short("HurtTime", spec.hurt_time);
    root.put("UUID", NbtTag::IntArray(spec.uuid.to_vec()));
    root.put_list("Pos", spec.pos.iter().map(|v| NbtTag::Double(*v)).collect());
    root.put_list(
        "Motion",
        spec.motion.iter().map(|v| NbtTag::Double(*v)).collect(),
    );
    root.put_list(
        "Rotation",
        spec.rotation.iter().map(|v| NbtTag::Float(*v)).collect(),
    );
    root.put_float("FallDistance", spec.rotation[0] / 8.0);
    root.put_bool("Invulnerable", spec.paper_keys.is_multiple_of(2));
    root.put_int("PortalCooldown", spec.brain_memories as i32);

    let mut attributes = NbtCompound::new();
    attributes.put("minecraft:generic.max_health", NbtTag::Double(20.0));
    attributes.put("minecraft:generic.movement_speed", NbtTag::Double(0.25));
    if spec.paper_keys.is_multiple_of(3) {
        attributes.put("minecraft:generic.attack_damage", NbtTag::Double(3.0));
    }
    root.put_compound("attributes", attributes);

    let mut armor = NbtCompound::new();
    for (i, slot) in spec.equipment.iter().enumerate() {
        if let Some(item) = slot {
            armor.put_compound(&format!("slot_{i}"), new_item(item));
        }
    }
    root.put_compound("ArmorItems", armor);

    let mut brain = NbtCompound::new();
    let mut memories = NbtCompound::new();
    for m in 0..spec.brain_memories {
        memories.put_int_if(&format!("memory_{m}"), m as i32 * 7);
    }
    brain.put_compound("memories", memories);
    root.put_compound("Brain", brain);

    for p in 0..spec.paper_keys {
        root.put_int_if(&format!("Paper.SpigotData_{p}"), p as i32);
    }
    root
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

fn build_old_block_entity(spec: &BlockEntitySpec) -> OldCompound {
    let mut root = OldCompound::new();
    root.put_int_if("x", spec.x);
    root.put_int_if("y", spec.y);
    root.put_int_if("z", spec.z);
    root.put_string_key("id", (spec.x + spec.z) as u32 % 60);
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

fn build_new_block_entity(spec: &BlockEntitySpec) -> NbtCompound {
    let mut root = NbtCompound::new();
    root.put_int_if("x", spec.x);
    root.put_int_if("y", spec.y);
    root.put_int_if("z", spec.z);
    root.put_string_key("id", (spec.x + spec.z) as u32 % 60);
    root.put_list(
        "Items",
        spec.items
            .iter()
            .map(|i| NbtTag::Compound(new_item(i)))
            .collect(),
    );
    if spec.has_name {
        let serial = (spec.x * 31 + spec.z) & 0xFF;
        root.put(
            "CustomName",
            NbtTag::String(format!("容器 #{serial}").into()),
        );
    }
    if spec.locked {
        root.put("Lock", NbtTag::String("key".into()));
    }
    root
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

fn build_old_custom_data(spec: &CustomDataSpec) -> OldCompound {
    let mut root = OldCompound::new();
    let mut display = OldCompound::new();
    display.put(
        "Name",
        OldTag::String(format!("物品 {name}", name = spec.display_name).into()),
    );
    if spec.lore_lines > 0 {
        display.put_list(
            "Lore",
            (0..spec.lore_lines)
                .map(|i| OldTag::String(format!("第 {i} 行").into()))
                .collect(),
        );
    }
    let mut components = OldCompound::new();
    components.put_compound("minecraft:display_name", display);
    let mut custom = OldCompound::new();
    custom.put_int_if("serial", spec.display_name as i32);
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

fn build_new_custom_data(spec: &CustomDataSpec) -> NbtCompound {
    let mut root = NbtCompound::new();
    let mut display = NbtCompound::new();
    display.put(
        "Name",
        NbtTag::String(format!("物品 {}", spec.display_name).into()),
    );
    if spec.lore_lines > 0 {
        display.put_list(
            "Lore",
            (0..spec.lore_lines)
                .map(|i| NbtTag::String(format!("第 {i} 行").into()))
                .collect(),
        );
    }
    let mut components = NbtCompound::new();
    components.put_compound("minecraft:display_name", display);
    let mut custom = NbtCompound::new();
    custom.put_int_if("serial", spec.display_name as i32);
    for f in 0..spec.flags {
        custom.put(
            &format!("flag_{f}"),
            NbtTag::Byte(i8::from(f.is_multiple_of(2))),
        );
    }
    components.put_compound("minecraft:custom_data", custom);
    root.put_compound("components", components);
    root
}

// ---------------------------------------------------------------------------
// 旧臂序列化：忠实复刻 NBT 二进制格式（键序任意，同哈希映射）
// ---------------------------------------------------------------------------

fn old_write_payload(buf: &mut Vec<u8>, tag: &OldTag) {
    match tag {
        OldTag::Byte(v) => buf.extend_from_slice(&v.to_be_bytes()),
        OldTag::Short(v) => buf.extend_from_slice(&v.to_be_bytes()),
        OldTag::Int(v) => buf.extend_from_slice(&v.to_be_bytes()),
        OldTag::Long(v) => buf.extend_from_slice(&v.to_be_bytes()),
        OldTag::Float(v) => buf.extend_from_slice(&v.to_be_bytes()),
        OldTag::Double(v) => buf.extend_from_slice(&v.to_be_bytes()),
        OldTag::ByteArray(v) => {
            buf.extend_from_slice(&(v.len() as i32).to_be_bytes());
            for &b in v {
                buf.extend_from_slice(&b.to_be_bytes());
            }
        }
        OldTag::String(s) => {
            buf.extend_from_slice(&(s.len() as u16).to_be_bytes());
            buf.extend_from_slice(s.as_bytes());
        }
        OldTag::List(list) => {
            let elem_id = list.first().map_or(0, old_type_id);
            buf.push(elem_id);
            buf.extend_from_slice(&(list.len() as i32).to_be_bytes());
            for item in list {
                old_write_payload(buf, item);
            }
        }
        OldTag::Compound(compound) => {
            for (name, tag) in &compound.child_tags {
                buf.push(old_type_id(tag));
                buf.extend_from_slice(&(name.len() as u16).to_be_bytes());
                buf.extend_from_slice(name.as_bytes());
                old_write_payload(buf, tag);
            }
            buf.push(0);
        }
        OldTag::IntArray(v) => {
            buf.extend_from_slice(&(v.len() as i32).to_be_bytes());
            for i in v {
                buf.extend_from_slice(&i.to_be_bytes());
            }
        }
        OldTag::LongArray(v) => {
            buf.extend_from_slice(&(v.len() as i32).to_be_bytes());
            for l in v {
                buf.extend_from_slice(&l.to_be_bytes());
            }
        }
    }
}

const fn old_type_id(tag: &OldTag) -> u8 {
    match tag {
        OldTag::Byte(_) => 1,
        OldTag::Short(_) => 2,
        OldTag::Int(_) => 3,
        OldTag::Long(_) => 4,
        OldTag::Float(_) => 5,
        OldTag::Double(_) => 6,
        OldTag::ByteArray(_) => 7,
        OldTag::String(_) => 8,
        OldTag::List(_) => 9,
        OldTag::Compound(_) => 10,
        OldTag::IntArray(_) => 11,
        OldTag::LongArray(_) => 12,
    }
}

/// 旧臂序列化输出（未命名网络形态，键序为哈希序）
fn old_serialize(root: &OldCompound) -> Vec<u8> {
    let mut buf = Vec::new();
    // 与生产 write_unnamed 对齐：根复合标签的类型 ID 打头
    buf.push(10);
    old_write_payload(&mut buf, &OldTag::Compound(root.clone()));
    buf
}

// ---------------------------------------------------------------------------
// canonical 指纹：键序无关、列表序敏感的递归折叠（生产类型上）
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

fn tag_fp(tag: &NbtTag) -> u64 {
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
fn compound_fp(compound: &NbtCompound) -> u64 {
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
    let compound = NbtCompound::from(nbt);
    compound_fp(&compound)
}

// ---------------------------------------------------------------------------
// 观测过程
// ---------------------------------------------------------------------------

/// 查找电池：既有键 + 缺失键混合取值，折叠命中值指纹
fn lookup_old(corpus: &[OldCompound]) -> (u64, usize) {
    let mut fp = 0x00C0_FFEEu64;
    let mut hits = 0usize;
    for (i, compound) in corpus.iter().enumerate() {
        match i % 3 {
            0 => {
                if compound.get_int("Air").is_some() {
                    hits += 1;
                }
                if let Some(name) = compound.get_string("CustomName") {
                    fp = fp_mix(fp, str_fp(name));
                } else {
                    fp = fp_mix(fp, 0xABCD);
                }
            }
            1 => {
                if compound.get_int("x").is_some() {
                    hits += 1;
                }
                fp = fp_mix(fp, compound.get("missing").is_some() as u64);
            }
            _ => {
                if compound.get("serial").is_some() {
                    hits += 1;
                }
                fp = fp_mix(
                    fp,
                    compound.get_float("Health").map_or(0xFEED, f32::to_bits) as u64,
                );
            }
        }
    }
    (fp, hits)
}

fn lookup_new(corpus: &[NbtCompound]) -> (u64, usize) {
    let mut fp = 0x00C0_FFEEu64;
    let mut hits = 0usize;
    for (i, compound) in corpus.iter().enumerate() {
        match i % 3 {
            0 => {
                if compound.get_int("Air").is_some() {
                    hits += 1;
                }
                if let Some(name) = compound.get_string("CustomName") {
                    fp = fp_mix(fp, str_fp(name));
                } else {
                    fp = fp_mix(fp, 0xABCD);
                }
            }
            1 => {
                if compound.get_int("x").is_some() {
                    hits += 1;
                }
                fp = fp_mix(fp, compound.get("missing").is_some() as u64);
            }
            _ => {
                if compound.get("serial").is_some() {
                    hits += 1;
                }
                fp = fp_mix(
                    fp,
                    compound.get_float("Health").map_or(0xFEED, f32::to_bits) as u64,
                );
            }
        }
    }
    (fp, hits)
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
fn build_old_corpus() -> Vec<OldCompound> {
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

/// 构建新臂语料（与旧臂同构）
fn build_new_corpus() -> Vec<NbtCompound> {
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

/// 旧臂的查找/序列化/克隆三段测量（构建段计量由 `run_arm` 完成后传入）
fn old_segments(corpus: &[OldCompound], mut r: ArmResult) -> ArmResult {
    let (before, _) = counters();
    let start = Instant::now();
    let (fp, hits) = lookup_old(corpus);
    r.lookup_time_us = start.elapsed().as_secs_f64() * 1e6;
    let (after, _) = counters();
    r.lookup_fp = fp;
    r.lookup_hits = hits;
    r.lookup_allocs = after - before;

    let (before, _) = counters();
    let start = Instant::now();
    let mut ser_fp = 0u64;
    for compound in corpus {
        let bytes = old_serialize(compound);
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
        sink += cloned.child_tags.len();
    }
    std::hint::black_box(sink);
    r.clone_time_ms = start.elapsed().as_secs_f64() * 1e3;
    let (after, _) = counters();
    r.clone_allocs = after - before;
    r
}

/// 新臂的查找/序列化/克隆三段测量（与旧臂逐段同构）
fn new_segments(corpus: &[NbtCompound], mut r: ArmResult) -> ArmResult {
    let (before, _) = counters();
    let start = Instant::now();
    let (fp, hits) = lookup_new(corpus);
    r.lookup_time_us = start.elapsed().as_secs_f64() * 1e6;
    let (after, _) = counters();
    r.lookup_fp = fp;
    r.lookup_hits = hits;
    r.lookup_allocs = after - before;

    let (before, _) = counters();
    let start = Instant::now();
    let mut ser_fp = 0u64;
    for compound in corpus {
        let bytes = papokin_nbt::Nbt::from(compound.clone()).write_unnamed();
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
        sink += cloned.child_tags.len();
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
    segments: fn(&[T], ArmResult) -> ArmResult,
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
    segments(&corpus, r)
}

/// 汇总闸门判定与细节描述
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
            "驻留字节下降",
            new.resident_bytes < old.resident_bytes,
            format!(
                "{} → {} B（{:.2}×，每复合标签均值 {:.1} → {:.1} B）",
                old.resident_bytes,
                new.resident_bytes,
                old.resident_bytes as f64 / new.resident_bytes as f64,
                old.resident_bytes as f64 / (ENTITY_N + BLOCK_ENTITY_N + CUSTOM_DATA_N) as f64,
                new.resident_bytes as f64 / (ENTITY_N + BLOCK_ENTITY_N + CUSTOM_DATA_N) as f64,
            ),
        ),
        (
            "构建分配次数不增",
            new.build_allocs <= old.build_allocs,
            format!("{} → {} 次", old.build_allocs, new.build_allocs),
        ),
    ]
}

/// 汇总两臂数值与闸门为 JSON 报告
fn build_report(
    old: &ArmResult,
    new: &ArmResult,
    gates: &[(&str, bool, String)],
) -> serde_json::Value {
    serde_json::json!({
        "round": 40,
        "topic": "nbt_compound_compact_map",
        "corpus": {
            "entities": ENTITY_N,
            "block_entities": BLOCK_ENTITY_N,
            "custom_data": CUSTOM_DATA_N,
        },
        "sizes": {
            "tag_old_bytes": std::mem::size_of::<OldTag>(),
            "tag_new_bytes": std::mem::size_of::<NbtTag>(),
            "compound_old_inline_bytes": std::mem::size_of::<OldCompound>(),
            "compound_new_inline_bytes": std::mem::size_of::<NbtCompound>(),
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

fn main() {
    println!("轮次 40 基准：NBT 复合标签容器紧凑化（HashMap → 插入序 Vec）");
    println!(
        "标签尺寸：旧臂 {} B · 新臂 {} B · 复合标签内联：旧 {} B · 新 {} B",
        std::mem::size_of::<OldTag>(),
        std::mem::size_of::<NbtTag>(),
        std::mem::size_of::<OldCompound>(),
        std::mem::size_of::<NbtCompound>(),
    );

    let old = run_arm(build_old_corpus, old_segments, counters());
    // 旧臂语料已随 run_arm 作用域结束释放，新臂计量不受其驻留影响
    let new = run_arm(build_new_corpus, new_segments, counters());

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

    let report = build_report(&old, &new, &gates);

    let path = "note/report/perf/round40-nbt-compound-compact-map.json";
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

    let all_passed = gates.iter().all(|(_, ok, _)| *ok);
    println!(
        "\n闸门判定：{}",
        if all_passed {
            "全部通过"
        } else {
            "存在未过闸门"
        }
    );
    if !all_passed {
        std::process::exit(1);
    }
}
