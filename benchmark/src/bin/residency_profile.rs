//! 轮次 50 基准：运行时驻留画像盘点（侦察轮）。
//!
//! 轮次 44-49 把实体构造、方块实体、区块存储逐面收至结构地板
//! 后，本基准用**真实生产类型**实测各大驻留面的每单元成本，
//! 产出世界规模投影表：认证既有地板、暴露任何意外分配、为后
//! 续轮次提供证据驱动的方向排序。
//!
//! 测量对象（全部直接构造生产类型）：
//! - `BlockPalette` 区段（同质 / 2-4 / 5-8 / 9-16 / 17-64 /
//!   65-300 态六带，轮 49 分布）；
//! - `BiomePalette` 区段（4³ 均质 / 双群系边界）；
//! - `LazyItemSlots<27>`（空箱 / 半满 / 全满，轮 46 形态）；
//! - `EntityEquipment`（空 / 四槽，轮 45 形态）；
//! - `ItemStack`（裸物品 / 带组件 patch）；
//! - `TextComponent`（text / translate 两种构造）。
//!
//! 产出：每实例存活字节与分配数 + 千区块 / 万实体 / 千箱投影。

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use papokin_data::BlockStateId;
use papokin_data::data_component_impl::EquipmentSlot;
use papokin_data::item::Item;
use papokin_data::item_stack::ItemStack;
use papokin_inventory::entity_equipment::EntityEquipment;
use papokin_inventory::inventory::LazyItemSlots;
use papokin_util::text::TextComponent;
use papokin_world::chunk::palette::{BiomePalette, BlockPalette};

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

struct CountingAlloc;

// SAFETY: 仅统计后原样转发系统分配器，不改变任何分配语义。
unsafe impl GlobalAlloc for CountingAlloc {
    // SAFETY: 转发给系统分配器；仅在成功时记账。
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: 转发给系统分配器；仅在成功时记账。
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            LIVE_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        ptr
    }

    // SAFETY: 转发系统分配器，ptr 来自本分配器的 alloc。
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        // SAFETY: 转发系统分配器。
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

const N: usize = 2000;

/// 构造 n 个实例并测量每实例存活字节与分配数。
fn measure<T>(n: usize, build: impl Fn() -> T) -> (f64, f64) {
    let b0 = LIVE_BYTES.load(Ordering::Relaxed);
    let a0 = ALLOCS.load(Ordering::Relaxed);
    let corpus: Vec<T> = (0..n).map(|_| build()).collect();
    let bytes = (LIVE_BYTES.load(Ordering::Relaxed) - b0) as f64 / n as f64;
    let allocs = (ALLOCS.load(Ordering::Relaxed) - a0) as f64 / n as f64;
    // 触碰语料防优化（内容影响后续读取）。
    std::hint::black_box(&corpus);
    (bytes, allocs)
}

const fn state_id(i: u16) -> BlockStateId {
    BlockStateId::new_or_air(i)
}

/// 按目标态数构建一个区段（逐格填充 + 尾部扰动，同轮 49 法）。
fn build_section(palette_size: usize, variant: usize) -> BlockPalette {
    let mut section = BlockPalette::default();
    let mut v = variant as u16;
    for i in 0..4096usize {
        let (y, z, x) = (i / 256, (i / 16) % 16, i % 16);
        let pick = if i < palette_size {
            i as u16
        } else {
            v = v.wrapping_mul(3).wrapping_add(7);
            v % palette_size as u16
        };
        section.set(x, y, z, state_id(pick));
    }
    section
}

fn build_biome_section(heterogeneous: bool, variant: usize) -> BiomePalette {
    let mut section = BiomePalette::default();
    if !heterogeneous {
        return section;
    }
    for i in 0..64usize {
        let (y, z, x) = (i / 16, (i / 4) % 4, i % 4);
        let v = ((i + variant) % 2) as u8;
        section.set(x, y, z, v);
    }
    section
}

fn build_chest(fill: usize, variant: usize) -> LazyItemSlots<27> {
    let chest = LazyItemSlots::new();
    for i in 0..fill {
        let stack = ItemStack::new(1, item_of(variant + i));
        let stack = std::cell::Cell::new(Some(stack));
        let mut fill = |s: &mut ItemStack| {
            stack.take().is_some_and(|v| {
                *s = v;
                true
            })
        };
        chest.with_stack_mut(i, &mut fill);
    }
    chest
}

const fn item_of(i: usize) -> &'static Item {
    // 从静态物品表取确定性条目，仅作占位物品源。
    match i % 3 {
        0 => &Item::STONE,
        1 => &Item::DIRT,
        _ => &Item::GRASS_BLOCK,
    }
}

fn item_of_stack(variant: usize) -> ItemStack {
    ItemStack::new((variant % 64 + 1) as u8, item_of(variant))
}

fn main() {
    println!("轮次 50 基准：运行时驻留画像盘点（真实生产类型实测）");
    println!("每项 {N} 实例，报告每实例存活字节 / 分配数\n");

    // --- 区块存储面 ---
    let dist: [(usize, &str, f64); 6] = [
        (1, "同质（空气/岩层）", 0.55),
        (3, "2-4 态（洞穴贯通/水体）", 0.17),
        (7, "5-8 态", 0.08),
        (12, "9-16 态", 0.12),
        (40, "17-64 态", 0.06),
        (150, "65-300 态", 0.02),
    ];
    let mut section_bytes: f64 = 0.0;
    for (size, label, weight) in dist {
        let (bytes, allocs) = measure(N, || build_section(size, size * 31));
        println!("BlockPalette[{label:<2}{size:>3} 态]: {bytes:>7.1} B · {allocs:>5.2} alloc");
        section_bytes += bytes * weight;
    }
    let (bio_homo_b, bio_homo_a) = measure(N, || build_biome_section(false, 0));
    let (bio_hetero_b, bio_hetero_a) = measure(N, || build_biome_section(true, 1));
    println!(
        "BiomePalette[均质]: {bio_homo_b:>7.1} B · {bio_homo_a:>5.2} alloc；[边界]: {bio_hetero_b:.1} B · {bio_hetero_a:.2}"
    );
    let biome_avg = bio_homo_b * 0.9 + bio_hetero_b * 0.1;

    // --- 方块实体面（箱族） ---
    let (chest_empty_b, chest_empty_a) = measure(N, || build_chest(0, 0));
    let (chest_half_b, chest_half_a) = measure(N, || build_chest(13, 1));
    let (chest_full_b, chest_full_a) = measure(N, || build_chest(27, 2));
    println!(
        "LazyItemSlots<27>[空]: {chest_empty_b:>7.1} B · {chest_empty_a:.2}；[半满]: {chest_half_b:.1} · {chest_half_a:.2}；[全满]: {chest_full_b:.1} · {chest_full_a:.2}"
    );

    // --- 实体装备面 ---
    let (equip_empty_b, equip_empty_a) = measure(N, EntityEquipment::new);
    let (equip_four_b, equip_four_a) = measure(N, || {
        let mut eq = EntityEquipment::new();
        for i in 0..4 {
            eq.put(slot_of(i), item_of_stack(i));
        }
        eq
    });
    println!(
        "EntityEquipment[空]: {equip_empty_b:>7.1} B · {equip_empty_a:.2}；[四槽]: {equip_four_b:.1} · {equip_four_a:.2}"
    );

    // --- 物品面 ---
    let (item_plain_b, item_plain_a) = measure(N, || item_of_stack(3));
    println!("ItemStack[裸]: {item_plain_b:>10.1} B · {item_plain_a:>5.2} alloc");

    // --- 文本面 ---
    let (text_b, text_a) = measure(N, || TextComponent::text(String::from("实体名牌")));
    let (trans_b, trans_a) = measure(N, || {
        TextComponent::translate("entity.minecraft.zombie", vec![])
    });
    println!(
        "TextComponent[text]: {text_b:>7.1} B · {text_a:.2}；[translate]: {trans_b:.1} · {trans_a:.2}"
    );

    // --- 世界规模投影 ---
    let per_chunk = section_bytes * 24.0 + biome_avg * 24.0 + 296.0 * 3.0 + 64.0;
    println!("\n=== 投影（每实例实测推算） ===");
    println!(
        "每千区块：区块存储面 ≈ {:.1} MB",
        per_chunk * 1000.0 / 1024.0 / 1024.0
    );
    println!(
        "每万实体（装备 45% 四槽）：装备面 ≈ {:.1} MB",
        (equip_empty_b * 0.55 + equip_four_b * 0.45) * 10000.0 / 1024.0 / 1024.0
    );
    println!(
        "每千空箱（轮 46 后）：箱面 ≈ {:.1} MB",
        chest_empty_b * 1000.0 / 1024.0 / 1024.0
    );

    write_report(&Profile {
        section_bytes,
        biome_avg,
        chest_empty_b,
        chest_half_b,
        chest_full_b,
        equip_empty_b,
        equip_four_b,
        item_plain_b,
        text_b,
        trans_b,
        per_chunk,
    });
}

const fn slot_of(i: usize) -> &'static EquipmentSlot {
    // 头/胸/腿/脚四态（判别值规范实例 const，轮次 45 同源）。
    static SLOTS: [&EquipmentSlot; 4] = [
        &EquipmentSlot::HEAD,
        &EquipmentSlot::CHEST,
        &EquipmentSlot::LEGS,
        &EquipmentSlot::FEET,
    ];
    SLOTS[i]
}

struct Profile {
    section_bytes: f64,
    biome_avg: f64,
    chest_empty_b: f64,
    chest_half_b: f64,
    chest_full_b: f64,
    equip_empty_b: f64,
    equip_four_b: f64,
    item_plain_b: f64,
    text_b: f64,
    trans_b: f64,
    per_chunk: f64,
}

fn write_report(p: &Profile) {
    use std::io::Write as _;

    let report = serde_json::json!({
        "round": 50,
        "topic": "residency_profile",
        "kind": "recon",
        "samples": N,
        "per_instance_bytes": {
            "block_palette_weighted": p.section_bytes,
            "biome_palette_avg": p.biome_avg,
            "chest_empty": p.chest_empty_b,
            "chest_half": p.chest_half_b,
            "chest_full": p.chest_full_b,
            "equipment_empty": p.equip_empty_b,
            "equipment_four_slots": p.equip_four_b,
            "item_stack_plain": p.item_plain_b,
            "text_component_text": p.text_b,
            "text_component_translate": p.trans_b,
        },
        "projections": {
            "per_1000_chunks_mb": p.per_chunk * 1000.0 / 1024.0 / 1024.0,
            "per_10k_entities_equipment_mb": (p.equip_empty_b * 0.55 + p.equip_four_b * 0.45)
                * 10000.0 / 1024.0 / 1024.0,
            "per_1000_empty_chests_mb": p.chest_empty_b * 1000.0 / 1024.0 / 1024.0,
        },
        "outcome": "recon",
    });

    let path = std::path::Path::new("note/report/perf/round50-residency-profile.json");
    let Ok(mut file) = std::fs::File::create(path) else {
        eprintln!("写入报告失败");
        return;
    };
    if let Err(err) = file.write_all(
        serde_json::to_string_pretty(&report)
            .unwrap_or_default()
            .as_bytes(),
    ) {
        eprintln!("写入报告失败：{err}");
    }
}
