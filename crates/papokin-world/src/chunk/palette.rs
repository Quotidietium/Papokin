use std::hash::Hash;

use papokin_data::{
    BlockStateId,
    block_properties::{has_random_ticks, is_air, is_liquid},
    fluid::Fluid,
};
use papokin_util::encompassing_bits;
use tracing::warn;

use super::format::{ChunkSectionBiomes, ChunkSectionBlockStates};

/// 按 y,z,x 索引的三维数组
type AbstractCube<T, const DIM: usize> = [[[T; DIM]; DIM]; DIM];

#[inline]
#[must_use]
pub fn has_random_ticking_fluid(id: BlockStateId) -> bool {
    Fluid::from_state_id(id).is_some_and(|fluid| Fluid::same_fluid_type(fluid.id, Fluid::LAVA.id))
}

#[derive(Clone)]
pub struct HeterogeneousPaletteData<V: Hash + Eq + Copy, const DIM: usize> {
    storage: PaletteStorage<V, DIM>,
    palette: Vec<V>,
    counts: Vec<u16>,
}

#[derive(Clone)]
enum PaletteStorage<V, const DIM: usize> {
    Dense(Box<AbstractCube<V, DIM>>),
    Indexed(Box<AbstractCube<u8, DIM>>),
    /// 调色板不超过 16 态时的半字节索引存储：每个字节装两个
    /// 4 bit 索引，占用恰为 `Indexed` 的一半。缓冲区长度恒为
    /// `DIM * DIM * DIM / 2`。
    Nibble(Box<[u8]>),
}

/// 半字节索引读取：偶数线性索引取低半字节，奇数取高半字节。
/// 线性索引按 y,z,x 行主序，与 `AbstractCube` 展平顺序一致。
#[inline]
const fn nibble_get(buf: &[u8], linear: usize) -> u8 {
    let byte = buf[linear / 2];
    if linear.is_multiple_of(2) {
        byte & 0x0F
    } else {
        byte >> 4
    }
}

/// 半字节索引写入，`value` 必须不超过 15。
#[inline]
fn nibble_set(buf: &mut [u8], linear: usize, value: u8) {
    debug_assert!(value <= 0x0F);
    let byte = &mut buf[linear / 2];
    if linear.is_multiple_of(2) {
        *byte = (*byte & 0xF0) | value;
    } else {
        *byte = (*byte & 0x0F) | (value << 4);
    }
}

impl<V: Hash + Eq + Copy + Default, const DIM: usize> HeterogeneousPaletteData<V, DIM> {
    fn get(&self, x: usize, y: usize, z: usize) -> V {
        debug_assert!(x < DIM);
        debug_assert!(y < DIM);
        debug_assert!(z < DIM);

        match &self.storage {
            PaletteStorage::Dense(cube) => cube[y][z][x],
            PaletteStorage::Indexed(indices) => self.palette[indices[y][z][x] as usize],
            PaletteStorage::Nibble(buf) => {
                self.palette[nibble_get(buf, (y * DIM + z) * DIM + x) as usize]
            }
        }
    }

    ///返回 Original（原始值）
    fn set(&mut self, x: usize, y: usize, z: usize, value: V) -> V {
        debug_assert!(x < DIM);
        debug_assert!(y < DIM);
        debug_assert!(z < DIM);

        let original = self.get(x, y, z);
        if original == value {
            return original;
        }

        let original_index = self
            .palette
            .iter()
            .position(|v| v == &original)
            .unwrap_or(0);

        // 在调色板中查找或添加新值
        let new_index = if let Some(new_index) = self.palette.iter().position(|v| v == &value) {
            self.counts[new_index] += 1;
            new_index
        } else {
            self.palette.push(value);
            self.counts.push(1);
            self.palette.len() - 1
        };

        // 处理存储升级或更新
        match &mut self.storage {
            PaletteStorage::Dense(cube) => {
                cube[y][z][x] = value;
            }
            PaletteStorage::Indexed(indices) => {
                if new_index <= 255 {
                    indices[y][z][x] = new_index as u8;
                } else {
                    // 升级为 Dense
                    let mut cube = Box::new([[[V::default(); DIM]; DIM]; DIM]);
                    for (i, v) in cube
                        .as_flattened_mut()
                        .as_flattened_mut()
                        .iter_mut()
                        .enumerate()
                    {
                        let y = i / (DIM * DIM);
                        let z = (i / DIM) % DIM;
                        let x = i % DIM;
                        *v = self.palette[indices[y][z][x] as usize];
                    }
                    cube[y][z][x] = value;
                    self.storage = PaletteStorage::Dense(cube);
                }
            }
            PaletteStorage::Nibble(buf) => {
                if new_index <= 15 {
                    nibble_set(buf, (y * DIM + z) * DIM + x, new_index as u8);
                } else {
                    // 调色板超过 16 态：解包为 u8 索引存储。
                    // 半字节存储的调色板至多 16 态，单次新增一项后至多
                    // 17 态，必然落在 u8 索引区间内。
                    let mut indices = Box::new([[[0u8; DIM]; DIM]; DIM]);
                    for (i, v) in indices
                        .as_flattened_mut()
                        .as_flattened_mut()
                        .iter_mut()
                        .enumerate()
                    {
                        *v = nibble_get(buf, i);
                    }
                    indices[y][z][x] = new_index as u8;
                    self.storage = PaletteStorage::Indexed(indices);
                }
            }
        }

        self.counts[original_index] -= 1;

        if self.counts[original_index] == 0 {
            let last_index = self.palette.len() - 1;
            // 如果计数归零，则从调色板和计数 Vec 中移除。
            self.palette.swap_remove(original_index);
            self.counts.swap_remove(original_index);

            if self.palette.capacity() > 16 && self.palette.len() < self.palette.capacity() / 2 {
                self.palette.shrink_to_fit();
                self.counts.shrink_to_fit();
            }

            // 索引型存储需要按 swap_remove 改写被移动的索引；
            // Dense 存的是值本身，无需改写。
            match &mut self.storage {
                PaletteStorage::Dense(_) => {}
                PaletteStorage::Indexed(indices) => {
                    for row in indices.iter_mut() {
                        for col in row.iter_mut() {
                            for idx in col.iter_mut() {
                                if *idx as usize == last_index {
                                    *idx = original_index as u8;
                                }
                            }
                        }
                    }
                }
                PaletteStorage::Nibble(buf) => {
                    for linear in 0..DIM * DIM * DIM {
                        if nibble_get(buf, linear) as usize == last_index {
                            nibble_set(buf, linear, original_index as u8);
                        }
                    }
                }
            }
        }

        original
    }
}

/// 调色板容器是一个由注册表 ID 组成的立方体。它使用一种基于以下情况的自定义压缩方案
/// 该立方体中可能包含多个不同的注册表 id。
#[derive(Clone)]
pub enum PalettedContainer<V: Hash + Eq + Copy + Default, const DIM: usize> {
    Homogeneous(V),
    Heterogeneous(Box<HeterogeneousPaletteData<V, DIM>>),
}

impl<V: Hash + Eq + Copy + Default, const DIM: usize> PalettedContainer<V, DIM> {
    pub const SIZE: usize = DIM;
    pub const VOLUME: usize = DIM * DIM * DIM;

    fn from_cube(cube: Box<AbstractCube<V, DIM>>) -> Self {
        let mut palette: Vec<V> = Vec::new();
        let mut counts: Vec<u16> = Vec::new();

        // 迭代展平后的立方体以填充调色板和计数
        for val in cube.as_flattened().as_flattened() {
            if let Some(index) = palette.iter().position(|v| v == val) {
                // 值已存在，递增其计数
                counts[index] += 1;
            } else {
                // 新值，将其加入调色板并开始计数
                palette.push(*val);
                counts.push(1);
            }
        }

        if palette.len() == 1 {
            // 快速路径：立方体是同质的，因此只需存储一个值
            Self::Homogeneous(palette[0])
        } else {
            // 异构立方体，存储完整数据
            if palette.len() <= 16 && std::mem::size_of::<V>() > 1 {
                let mut buf = vec![0u8; Self::VOLUME / 2].into_boxed_slice();
                for (i, v) in cube.as_flattened().as_flattened().iter().enumerate() {
                    let idx = palette.iter().position(|p| p == v).unwrap_or(0);
                    nibble_set(&mut buf, i, idx as u8);
                }
                Self::Heterogeneous(Box::new(HeterogeneousPaletteData {
                    storage: PaletteStorage::Nibble(buf),
                    palette,
                    counts,
                }))
            } else if palette.len() <= 256 && std::mem::size_of::<V>() > 1 {
                let mut indices = Box::new([[[0u8; DIM]; DIM]; DIM]);
                for (i, v) in cube.as_flattened().as_flattened().iter().enumerate() {
                    let idx = palette.iter().position(|p| p == v).unwrap_or(0);
                    indices.as_flattened_mut().as_flattened_mut()[i] = idx as u8;
                }
                Self::Heterogeneous(Box::new(HeterogeneousPaletteData {
                    storage: PaletteStorage::Indexed(indices),
                    palette,
                    counts,
                }))
            } else {
                Self::Heterogeneous(Box::new(HeterogeneousPaletteData {
                    storage: PaletteStorage::Dense(cube),
                    palette,
                    counts,
                }))
            }
        }
    }

    /// 一次性构建完整调色板，免去逐次修改的簿记开销。
    pub(crate) fn from_fn(mut value_at: impl FnMut(usize, usize, usize) -> V) -> Self {
        let mut cube = Box::new([[[V::default(); DIM]; DIM]; DIM]);
        let mut indices = Box::new([[[0u8; DIM]; DIM]; DIM]);
        let mut palette = Vec::new();
        let mut counts = Vec::<u16>::new();

        for y in 0..DIM {
            for z in 0..DIM {
                for x in 0..DIM {
                    let value = value_at(x, y, z);
                    cube[y][z][x] = value;
                    let index =
                        if let Some(index) = palette.iter().position(|entry| *entry == value) {
                            counts[index] += 1;
                            index
                        } else {
                            palette.push(value);
                            counts.push(1);
                            palette.len() - 1
                        };
                    if let Ok(index) = u8::try_from(index) {
                        indices[y][z][x] = index;
                    }
                }
            }
        }

        if palette.len() == 1 {
            return Self::Homogeneous(palette[0]);
        }

        let storage = if palette.len() <= 16 && std::mem::size_of::<V>() > 1 {
            let mut buf = vec![0u8; Self::VOLUME / 2].into_boxed_slice();
            for (i, idx) in indices.as_flattened().as_flattened().iter().enumerate() {
                nibble_set(&mut buf, i, *idx);
            }
            PaletteStorage::Nibble(buf)
        } else if palette.len() <= 256 && std::mem::size_of::<V>() > 1 {
            PaletteStorage::Indexed(indices)
        } else {
            PaletteStorage::Dense(cube)
        };
        Self::Heterogeneous(Box::new(HeterogeneousPaletteData {
            storage,
            palette,
            counts,
        }))
    }

    fn bits_per_entry(&self) -> u8 {
        match self {
            Self::Homogeneous(_) => 0,
            Self::Heterogeneous(data) => encompassing_bits(data.counts.len()),
        }
    }

    pub fn to_palette_and_packed_data(&self, bits_per_entry: u8) -> (Box<[V]>, Box<[i64]>) {
        match self {
            Self::Homogeneous(registry_id) => (Box::new([*registry_id]), Box::new([])),
            Self::Heterogeneous(data) => {
                debug_assert!(bits_per_entry >= encompassing_bits(data.counts.len()));
                debug_assert!(bits_per_entry <= 15);

                // 不要在这里使用 HashMap，因为它很慢
                let blocks_per_i64 = 64 / bits_per_entry;

                let packed_indices: Box<[i64]> = match &data.storage {
                    PaletteStorage::Dense(cube) => cube
                        .as_flattened()
                        .as_flattened()
                        .chunks(blocks_per_i64 as usize)
                        .map(|chunk| {
                            chunk.iter().enumerate().fold(0, |acc, (index, key)| {
                                let key_index =
                                    data.palette.iter().position(|&x| x == *key).unwrap_or(0);
                                debug_assert!((1 << bits_per_entry) > key_index);

                                let packed_offset_index =
                                    (key_index as u64) << (bits_per_entry as u64 * index as u64);
                                acc | packed_offset_index as i64
                            })
                        })
                        .collect(),
                    PaletteStorage::Indexed(indices) => indices
                        .as_flattened()
                        .as_flattened()
                        .chunks(blocks_per_i64 as usize)
                        .map(|chunk| {
                            chunk.iter().enumerate().fold(0, |acc, (index, key_index)| {
                                let key_index = *key_index as usize;
                                debug_assert!((1 << bits_per_entry) > key_index);

                                let packed_offset_index =
                                    (key_index as u64) << (bits_per_entry as u64 * index as u64);
                                acc | packed_offset_index as i64
                            })
                        })
                        .collect(),
                    PaletteStorage::Nibble(buf) => (0..Self::VOLUME
                        .div_ceil(blocks_per_i64 as usize))
                        .map(|word| {
                            let base = word * blocks_per_i64 as usize;
                            (0..blocks_per_i64 as usize).fold(0, |acc, index| {
                                let linear = base + index;
                                if linear >= Self::VOLUME {
                                    acc
                                } else {
                                    let key_index = nibble_get(buf, linear) as usize;
                                    debug_assert!((1 << bits_per_entry) > key_index);

                                    let packed_offset_index = (key_index as u64)
                                        << (bits_per_entry as u64 * index as u64);
                                    acc | packed_offset_index as i64
                                }
                            })
                        })
                        .collect(),
                };

                (data.palette.clone().into_boxed_slice(), packed_indices)
            }
        }
    }

    #[must_use]
    pub fn from_palette_and_packed_data(
        palette: &[V],
        packed_data: &[i64],
        minimum_bits_per_entry: u8,
    ) -> Self {
        if palette.is_empty() {
            warn!("没有调色板数据！使用默认值...");
            return Self::Homogeneous(V::default());
        }

        if palette.len() == 1 {
            return Self::Homogeneous(palette[0]);
        }

        let bits_per_key = encompassing_bits(palette.len()).max(minimum_bits_per_entry);
        let index_mask = (1 << bits_per_key) - 1;
        let keys_per_i64 = 64 / bits_per_key;

        // 调色板足够小时索引存储的优化路径
        if palette.len() <= 256 && std::mem::size_of::<V>() > 1 {
            let mut indices = Box::new([[[0u8; DIM]; DIM]; DIM]);
            let mut counts = vec![0u16; palette.len()];
            let indices_flat = indices.as_flattened_mut().as_flattened_mut();

            let mut packed_data_iter = packed_data.iter();
            let mut current_packed_word = *packed_data_iter.next().unwrap_or(&0);

            for (i, index_out) in indices_flat.iter_mut().enumerate().take(Self::VOLUME) {
                let bit_index_in_word = i % keys_per_i64 as usize;
                if bit_index_in_word == 0 && i > 0 {
                    current_packed_word = *packed_data_iter.next().unwrap_or(&0);
                }

                let lookup_index = ((current_packed_word as u64)
                    >> (bit_index_in_word as u64 * bits_per_key as u64))
                    & index_mask;

                let idx = lookup_index as usize;
                if idx < palette.len() {
                    *index_out = idx as u8;
                    counts[idx] += 1;
                } else {
                    warn!("查找索引越界！使用默认值...");
                    // 值已经是 0，如果我们跟踪它，counts[0] 会被正确更新
                }
            }
            // 若 counts[0] 在越界情况下被跳过则加以修正（罕见）
            // 但实际上我们应当直接确保它是正确的。

            if palette.len() <= 16 {
                let mut buf = vec![0u8; Self::VOLUME / 2].into_boxed_slice();
                for (i, idx) in indices.as_flattened().as_flattened().iter().enumerate() {
                    nibble_set(&mut buf, i, *idx);
                }
                return Self::Heterogeneous(Box::new(HeterogeneousPaletteData {
                    storage: PaletteStorage::Nibble(buf),
                    palette: palette.to_vec(),
                    counts,
                }));
            }

            return Self::Heterogeneous(Box::new(HeterogeneousPaletteData {
                storage: PaletteStorage::Indexed(indices),
                palette: palette.to_vec(),
                counts,
            }));
        }

        let mut decompressed_values = Vec::with_capacity(Self::VOLUME);

        let mut packed_data_iter = packed_data.iter();
        let mut current_packed_word = *packed_data_iter.next().unwrap_or(&0);

        for i in 0..Self::VOLUME {
            let bit_index_in_word = i % keys_per_i64 as usize;

            if bit_index_in_word == 0 && i > 0 {
                current_packed_word = *packed_data_iter.next().unwrap_or(&0);
            }

            let lookup_index = (current_packed_word as u64
                >> (bit_index_in_word as u64 * bits_per_key as u64))
                & index_mask;

            let value = palette
                .get(lookup_index as usize)
                .copied()
                .unwrap_or_else(|| {
                    warn!("查找索引越界！使用默认值...");
                    V::default()
                });

            decompressed_values.push(value);
        }

        // 现在，利用所有解压后的值构建计数。
        let mut counts = vec![0; palette.len()];

        for &value in &decompressed_values {
            // 这是关键优化：在调色板 Vec 中查找索引
            // 并递增对应计数。
            if let Some(index) = palette.iter().position(|v| v == &value) {
                counts[index] += 1;
            } else {
                // 如果调色板是完整的，理想情况下不应出现这种情况。
                warn!("解压后的值不在调色板中！");
            }
        }

        let mut cube = Box::new([[[V::default(); DIM]; DIM]; DIM]);
        cube.as_flattened_mut()
            .as_flattened_mut()
            .copy_from_slice(&decompressed_values);

        Self::Heterogeneous(Box::new(HeterogeneousPaletteData {
            storage: PaletteStorage::Dense(cube),
            palette: palette.to_vec(),
            counts,
        }))
    }

    pub fn get(&self, x: usize, y: usize, z: usize) -> V {
        match self {
            Self::Homogeneous(value) => *value,
            Self::Heterogeneous(data) => data.get(x, y, z),
        }
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, value: V) -> V {
        debug_assert!(x < Self::SIZE);
        debug_assert!(y < Self::SIZE);
        debug_assert!(z < Self::SIZE);

        match self {
            Self::Homogeneous(original) => {
                let original = *original;
                if value != original {
                    let mut cube = Box::new([[[original; DIM]; DIM]; DIM]);
                    cube[y][z][x] = value;
                    *self = Self::from_cube(cube);
                }
                original
            }
            Self::Heterogeneous(data) => {
                let original = data.set(x, y, z, value);
                if data.counts.len() == 1 {
                    *self = Self::Homogeneous(data.palette[0]);
                }
                original
            }
        }
    }

    pub fn iter(&self) -> Box<dyn Iterator<Item = V> + '_> {
        match self {
            Self::Homogeneous(registry_id) => {
                Box::new(std::iter::repeat_n(*registry_id, Self::VOLUME))
            }
            Self::Heterogeneous(data) => match &data.storage {
                PaletteStorage::Dense(cube) => {
                    Box::new(cube.as_flattened().as_flattened().iter().copied())
                }
                PaletteStorage::Indexed(indices) => Box::new(
                    indices
                        .as_flattened()
                        .as_flattened()
                        .iter()
                        .map(|&idx| data.palette[idx as usize]),
                ),
                PaletteStorage::Nibble(buf) => {
                    Box::new((0..Self::VOLUME).map(|i| data.palette[nibble_get(buf, i) as usize]))
                }
            },
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::Homogeneous(value) => *value == V::default(),
            Self::Heterogeneous(_) => false,
        }
    }
}

impl<'a, V: Hash + Eq + Copy + Default, const DIM: usize> IntoIterator
    for &'a PalettedContainer<V, DIM>
{
    type Item = V;
    type IntoIter = Box<dyn Iterator<Item = Self::Item> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        PalettedContainer::iter(self)
    }
}

impl<V: Default + Hash + Eq + Copy, const DIM: usize> Default for PalettedContainer<V, DIM> {
    fn default() -> Self {
        Self::Homogeneous(V::default())
    }
}

impl BiomePalette {
    #[must_use]
    pub fn convert_network(&self) -> NetworkSerialization<u8> {
        match self {
            Self::Homogeneous(registry_id) => NetworkSerialization {
                bits_per_entry: 0,
                palette: NetworkPalette::Single(*registry_id),
                packed_data: Box::new([]),
            },
            Self::Heterogeneous(data) => {
                let raw_bits_per_entry = encompassing_bits(data.counts.len());
                if raw_bits_per_entry > BIOME_NETWORK_MAX_MAP_BITS {
                    let bits_per_entry = BIOME_NETWORK_MAX_BITS;
                    let values_per_i64 = 64 / bits_per_entry;
                    let mut packed_data =
                        Vec::with_capacity(Self::VOLUME.div_ceil(values_per_i64 as usize));
                    let mut current_idx = 0;
                    while current_idx < Self::VOLUME {
                        let mut acc = 0u64;
                        for i in 0..values_per_i64 as usize {
                            if current_idx + i < Self::VOLUME {
                                let y = (current_idx + i) / (Self::SIZE * Self::SIZE);
                                let z = ((current_idx + i) / Self::SIZE) % Self::SIZE;
                                let x = (current_idx + i) % Self::SIZE;
                                let value = data.get(x, y, z);
                                debug_assert!((1 << bits_per_entry) > value);
                                acc |= (value as u64) << (bits_per_entry as u64 * i as u64);
                            }
                        }
                        packed_data.push(acc as i64);
                        current_idx += values_per_i64 as usize;
                    }

                    NetworkSerialization {
                        bits_per_entry,
                        palette: NetworkPalette::Direct,
                        packed_data: packed_data.into_boxed_slice(),
                    }
                } else {
                    let bits_per_entry = raw_bits_per_entry.max(BIOME_NETWORK_MIN_MAP_BITS);
                    let (palette, packed) = self.to_palette_and_packed_data(bits_per_entry);

                    NetworkSerialization {
                        bits_per_entry,
                        palette: NetworkPalette::Indirect(palette),
                        packed_data: packed,
                    }
                }
            }
        }
    }

    #[must_use]
    pub fn from_disk_nbt(nbt: ChunkSectionBiomes) -> Self {
        let palette = nbt.palette;

        Self::from_palette_and_packed_data(
            &palette,
            nbt.data.as_ref().unwrap_or(&Box::default()),
            BIOME_DISK_MIN_BITS,
        )
    }

    #[must_use]
    pub fn to_disk_nbt(&self) -> ChunkSectionBiomes {
        #[expect(clippy::unnecessary_min_or_max)]
        let bits_per_entry = self.bits_per_entry().max(BIOME_DISK_MIN_BITS);
        let (palette, packed_data) = self.to_palette_and_packed_data(bits_per_entry);
        ChunkSectionBiomes {
            data: if packed_data.is_empty() {
                None
            } else {
                Some(packed_data)
            },
            palette,
        }
    }
}

impl BlockPalette {
    #[must_use]
    pub fn convert_network(&self) -> NetworkSerialization<u16> {
        match self {
            Self::Homogeneous(registry_id) => NetworkSerialization {
                bits_per_entry: 0,
                palette: NetworkPalette::Single(registry_id.as_u16()),
                packed_data: Box::new([]),
            },
            Self::Heterogeneous(data) => {
                let raw_bits_per_entry = encompassing_bits(data.counts.len());
                if raw_bits_per_entry > BLOCK_NETWORK_MAX_MAP_BITS {
                    let bits_per_entry = BLOCK_NETWORK_MAX_BITS;
                    let values_per_i64 = 64 / bits_per_entry;
                    let mut packed_data =
                        Vec::with_capacity(Self::VOLUME.div_ceil(values_per_i64 as usize));
                    let mut current_idx = 0;
                    while current_idx < Self::VOLUME {
                        let mut acc = 0u64;
                        for i in 0..values_per_i64 as usize {
                            if current_idx + i < Self::VOLUME {
                                let y = (current_idx + i) / (Self::SIZE * Self::SIZE);
                                let z = ((current_idx + i) / Self::SIZE) % Self::SIZE;
                                let x = (current_idx + i) % Self::SIZE;
                                let value = data.get(x, y, z).as_u16();
                                debug_assert!((1u32 << bits_per_entry) > u32::from(value));
                                acc |= (value as u64) << (bits_per_entry as u64 * i as u64);
                            }
                        }
                        packed_data.push(acc as i64);
                        current_idx += values_per_i64 as usize;
                    }

                    NetworkSerialization {
                        bits_per_entry,
                        palette: NetworkPalette::Direct,
                        packed_data: packed_data.into_boxed_slice(),
                    }
                } else {
                    let bits_per_entry = raw_bits_per_entry.max(BLOCK_NETWORK_MIN_MAP_BITS);
                    let (palette, packed) = self.to_palette_and_packed_data(bits_per_entry);

                    NetworkSerialization {
                        bits_per_entry,
                        palette: NetworkPalette::Indirect(
                            palette.iter().map(|v| v.as_u16()).collect(),
                        ),
                        packed_data: packed,
                    }
                }
            }
        }
    }

    /// 检查整个区块是否只由空气填充
    #[must_use]
    pub fn has_only_air(&self) -> bool {
        match self {
            Self::Homogeneous(id) => is_air(*id),
            Self::Heterogeneous(data) => data.palette.iter().all(|&id| is_air(id)),
        }
    }

    #[must_use]
    pub fn random_ticking_counts(&self) -> (u16, u16) {
        match self {
            Self::Homogeneous(registry_id) => {
                let block_count = if has_random_ticks(*registry_id) {
                    Self::VOLUME as u16
                } else {
                    0
                };
                let fluid_count = if has_random_ticking_fluid(*registry_id) {
                    Self::VOLUME as u16
                } else {
                    0
                };
                (block_count, fluid_count)
            }
            Self::Heterogeneous(data) => data.palette.iter().zip(data.counts.iter()).fold(
                (0, 0),
                |(block_count, fluid_count), (registry_id, count)| {
                    let block_count = if has_random_ticks(*registry_id) {
                        block_count.saturating_add(*count)
                    } else {
                        block_count
                    };
                    let fluid_count = if has_random_ticking_fluid(*registry_id) {
                        fluid_count.saturating_add(*count)
                    } else {
                        fluid_count
                    };
                    (block_count, fluid_count)
                },
            ),
        }
    }

    #[must_use]
    pub fn non_air_block_count(&self) -> u16 {
        match self {
            Self::Homogeneous(registry_id) => {
                if is_air(*registry_id) {
                    0
                } else {
                    Self::VOLUME as u16
                }
            }
            Self::Heterogeneous(data) => data
                .palette
                .iter()
                .zip(data.counts.iter())
                .filter_map(|(registry_id, count)| (!is_air(*registry_id)).then_some(*count))
                .sum(),
        }
    }

    #[must_use]
    pub fn liquid_block_count(&self) -> u16 {
        match self {
            Self::Homogeneous(registry_id) => {
                if is_liquid(*registry_id) {
                    Self::VOLUME as u16
                } else {
                    0
                }
            }
            Self::Heterogeneous(data) => data
                .palette
                .iter()
                .zip(data.counts.iter())
                .filter_map(|(registry_id, count)| is_liquid(*registry_id).then_some(*count))
                .sum(),
        }
    }

    #[must_use]
    pub fn from_disk_nbt(nbt: ChunkSectionBlockStates) -> Self {
        let palette = nbt.palette;

        Self::from_palette_and_packed_data(
            &palette,
            nbt.data.as_ref().unwrap_or(&Box::default()),
            BLOCK_DISK_MIN_BITS,
        )
    }

    #[must_use]
    pub fn to_disk_nbt(&self) -> ChunkSectionBlockStates {
        let bits_per_entry = self.bits_per_entry().max(BLOCK_DISK_MIN_BITS);
        let (palette, packed_data) = self.to_palette_and_packed_data(bits_per_entry);
        ChunkSectionBlockStates {
            data: if packed_data.is_empty() {
                None
            } else {
                Some(packed_data)
            },
            palette,
        }
    }
}

/// 表示 Minecraft 按位打包的区块段中使用的各种数据编码类型。
///
/// Minecraft 使用“调色板（Palette）”系统来压缩区块数据。它不会发送完整的，
/// 每个方块的 15 位 `BlockState` ID，它发送的是更小的索引（例如 4 位），
/// 指向这些调色板中的某个值。
pub enum NetworkPalette<V> {
    /// **单值调色板（每条目位数：0）**
    ///
    /// 当整个区块段（16x16x16）只包含一种方块或生物群系时使用。
    /// 在网络缓冲区中，此调色板之后没有数据数组。
    Single(V),
    /// **间接调色板（每条目位数：方块 1-8，生物群系 1-3）**
    ///
    /// 该区块段中所有去重值的列表。数据数组中包含的是索引，
    /// 指向此列表的。
    Indirect(Box<[V]>),
    /// **直接调色板（每条目位数：方块 15+，生物群系 6+）**
    ///
    /// 当区块段对小型调色板而言过于复杂时使用。数据数组
    /// 直接包含全局注册表 ID。不发送任何调色板列表。
    Direct,
}

pub struct NetworkSerialization<V> {
    pub bits_per_entry: u8,
    pub palette: NetworkPalette<V>,
    pub packed_data: Box<[i64]>,
}

// 根据 wiki，调色板在磁盘与网络上的序列化方式不同。磁盘
// 只要条目数大于 1，序列化时总是使用调色板。网络序列化会将 id 打包
// 直接存储，而非在每条目比特数超过某一阈值时使用调色板

// TODO: 自行测试；我们真的需要区别处理网络与磁盘序列化吗？
pub type BlockPalette = PalettedContainer<BlockStateId, 16>;
const BLOCK_DISK_MIN_BITS: u8 = 4;
const BLOCK_NETWORK_MIN_MAP_BITS: u8 = 4;
const BLOCK_NETWORK_MAX_MAP_BITS: u8 = 8;
pub(crate) const BLOCK_NETWORK_MAX_BITS: u8 = 16;

pub type BiomePalette = PalettedContainer<u8, 4>;
const BIOME_DISK_MIN_BITS: u8 = 0;
const BIOME_NETWORK_MIN_MAP_BITS: u8 = 1;
const BIOME_NETWORK_MAX_MAP_BITS: u8 = 3;
pub(crate) const BIOME_NETWORK_MAX_BITS: u8 = 7;

#[cfg(test)]
mod tests {
    use super::{BlockPalette, NetworkPalette, PaletteStorage, PalettedContainer};
    use papokin_data::{Block, BlockStateId};

    fn network_palette_values(palette: NetworkPalette<u16>) -> Option<Box<[u16]>> {
        match palette {
            NetworkPalette::Single(value) => Some(Box::new([value])),
            NetworkPalette::Indirect(values) => Some(values),
            NetworkPalette::Direct => None,
        }
    }

    fn storage_is_nibble(palette: &BlockPalette) -> bool {
        match palette {
            PalettedContainer::Homogeneous(_) => false,
            PalettedContainer::Heterogeneous(data) => {
                matches!(data.storage, PaletteStorage::Nibble(_))
            }
        }
    }

    fn storage_is_indexed(palette: &BlockPalette) -> bool {
        match palette {
            PalettedContainer::Homogeneous(_) => false,
            PalettedContainer::Heterogeneous(data) => {
                matches!(data.storage, PaletteStorage::Indexed(_))
            }
        }
    }

    fn storage_is_dense(palette: &BlockPalette) -> bool {
        match palette {
            PalettedContainer::Homogeneous(_) => false,
            PalettedContainer::Heterogeneous(data) => {
                matches!(data.storage, PaletteStorage::Dense(_))
            }
        }
    }

    fn assert_matches_model(palette: &BlockPalette, model: &[[[BlockStateId; 16]; 16]; 16]) {
        for (y, row) in model.iter().enumerate() {
            for (z, col) in row.iter().enumerate() {
                for (x, expected) in col.iter().enumerate() {
                    assert_eq!(
                        palette.get(x, y, z),
                        *expected,
                        "坐标 ({x}, {y}, {z}) 取值与模型不一致"
                    );
                }
            }
        }
    }

    fn assert_bulk_matches_mutations(value_at: impl Fn(usize, usize, usize) -> BlockStateId) {
        let mut mutated = BlockPalette::default();
        for y in 0..BlockPalette::SIZE {
            for z in 0..BlockPalette::SIZE {
                for x in 0..BlockPalette::SIZE {
                    mutated.set(x, y, z, value_at(x, y, z));
                }
            }
        }
        let bulk = BlockPalette::from_fn(value_at);

        assert_eq!(
            mutated.iter().collect::<Vec<_>>(),
            bulk.iter().collect::<Vec<_>>()
        );
        assert_eq!(
            mutated.random_ticking_counts(),
            bulk.random_ticking_counts()
        );
        assert_eq!(mutated.non_air_block_count(), bulk.non_air_block_count());
        assert_eq!(mutated.liquid_block_count(), bulk.liquid_block_count());

        let mutated_network = mutated.convert_network();
        let bulk_network = bulk.convert_network();
        assert_eq!(mutated_network.bits_per_entry, bulk_network.bits_per_entry);
        assert_eq!(mutated_network.packed_data, bulk_network.packed_data);
        assert_eq!(
            network_palette_values(mutated_network.palette),
            network_palette_values(bulk_network.palette)
        );
    }

    #[test]
    fn bulk_palette_matches_individual_mutations() {
        let states = [
            Block::AIR.default_state.id,
            Block::STONE.default_state.id,
            Block::WATER.default_state.id,
            Block::LAVA.default_state.id,
        ];
        assert_bulk_matches_mutations(|x, y, z| states[(x + y + z) % states.len()]);
    }

    #[test]
    fn bulk_palette_handles_homogeneous_sections() {
        assert_bulk_matches_mutations(|_, _, _| Block::STONE.default_state.id);
    }

    #[test]
    fn bulk_palette_handles_direct_network_palettes() {
        assert_bulk_matches_mutations(|x, y, z| {
            BlockStateId::new_or_air(((y * 256 + z * 16 + x) % 300) as u16)
        });
    }

    #[test]
    fn small_palette_uses_nibble_storage_on_all_build_paths() {
        let states = [
            Block::AIR.default_state.id,
            Block::STONE.default_state.id,
            Block::WATER.default_state.id,
        ];

        // 逐次变异路径（Homogeneous 升级）
        let mut mutated = BlockPalette::default();
        for (i, state) in states.iter().enumerate() {
            mutated.set(i, 0, 0, *state);
        }
        assert!(storage_is_nibble(&mutated));

        // 批量构建路径
        let bulk = BlockPalette::from_fn(|x, _, _| states[x % states.len()]);
        assert!(storage_is_nibble(&bulk));

        // 磁盘反序列化路径（3 态调色板 + 4 bit 打包数据）
        let mut packed = vec![0i64; 256];
        for (i, word) in packed.iter_mut().enumerate() {
            for j in 0..16usize {
                let cell = i * 16 + j;
                *word |= (((cell % states.len()) as u64) << (4 * j)) as i64;
            }
        }
        let disk = BlockPalette::from_palette_and_packed_data(&states, &packed, 4);
        assert!(storage_is_nibble(&disk));
        assert_eq!(disk.get(5, 0, 0), states[5 % states.len()]);
    }

    #[test]
    fn nibble_upgrades_to_indexed_at_seventeen_states() {
        let states: Vec<BlockStateId> = (1u16..=17).map(BlockStateId::new_or_air).collect();
        assert!(
            states.iter().all(|s| *s != BlockStateId::default()),
            "测试前提：状态池不含默认（空气）状态"
        );
        let mut palette = BlockPalette::default();
        let mut model = [[[BlockStateId::default(); 16]; 16]; 16];

        // 默认状态 + 15 个相异状态 = 16 态，恰为半字节存储上限
        for (i, state) in states.iter().take(15).enumerate() {
            palette.set(i, 0, 0, *state);
            model[0][0][i] = *state;
        }
        assert!(storage_is_nibble(&palette));

        palette.set(0, 1, 0, states[15]);
        model[1][0][0] = states[15];
        assert!(storage_is_indexed(&palette));
        assert_matches_model(&palette, &model);
    }

    #[test]
    fn indexed_stays_until_256_then_dense_at_257_states() {
        let states: Vec<BlockStateId> = (1u16..=256).map(BlockStateId::new_or_air).collect();
        let mut palette = BlockPalette::default();
        let mut model = [[[BlockStateId::default(); 16]; 16]; 16];

        // 默认状态 + 255 个相异状态 = 256 态，恰为 u8 索引存储上限
        for (i, state) in states.iter().take(255).enumerate() {
            let (x, y, z) = (i % 16, i / 256, (i / 16) % 16);
            palette.set(x, y, z, *state);
            model[y][z][x] = *state;
        }
        assert!(storage_is_indexed(&palette));

        let state = states[255];
        palette.set(0, 1, 0, state);
        model[1][0][0] = state;
        assert!(storage_is_dense(&palette));
        assert_matches_model(&palette, &model);
    }

    #[test]
    fn nibble_swap_remove_rewrites_indices() {
        let states: Vec<BlockStateId> = (1u16..=5).map(BlockStateId::new_or_air).collect();
        let mut palette = BlockPalette::default();
        let mut model = [[[BlockStateId::default(); 16]; 16]; 16];

        for i in 0..64usize {
            let (x, z) = (i % 16, (i / 16) % 16);
            let state = states[i % states.len()];
            palette.set(x, 0, z, state);
            model[0][z][x] = state;
        }
        // 把 states[2] 的全部单元改写为 states[3]：states[2] 计数归零，
        // 触发 swap_remove 与索引改写
        for i in (2..64usize).step_by(5) {
            let (x, z) = (i % 16, (i / 16) % 16);
            palette.set(x, 0, z, states[3]);
            model[0][z][x] = states[3];
        }

        assert!(storage_is_nibble(&palette));
        assert_matches_model(&palette, &model);
    }

    #[test]
    fn nibble_demotes_to_homogeneous_when_single_state_remains() {
        let a = BlockStateId::new_or_air(7);
        let b = BlockStateId::new_or_air(9);
        let mut palette = BlockPalette::default();

        palette.set(3, 3, 3, a);
        palette.set(4, 4, 4, b);
        assert!(storage_is_nibble(&palette));

        palette.set(3, 3, 3, BlockStateId::default());
        palette.set(4, 4, 4, BlockStateId::default());
        assert!(matches!(
            palette,
            PalettedContainer::Homogeneous(value) if value == BlockStateId::default()
        ));
    }

    #[test]
    fn randomized_mutations_match_model_across_storage_tiers() {
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = move || {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            state.wrapping_mul(0x2545_F491_4F6C_DD1D)
        };
        // 24 态池：变异过程中跨越 Nibble（≤16）→ Indexed（17+）边界
        let pool: Vec<BlockStateId> = (1u16..=24).map(BlockStateId::new_or_air).collect();
        let mut palette = BlockPalette::default();
        let mut model = [[[BlockStateId::default(); 16]; 16]; 16];

        for op in 0..4096usize {
            let (x, y, z) = (
                (next() % 16) as usize,
                (next() % 16) as usize,
                (next() % 16) as usize,
            );
            let value = pool[(next() % pool.len() as u64) as usize];
            palette.set(x, y, z, value);
            model[y][z][x] = value;

            if op % 512 == 0 {
                for _ in 0..64 {
                    let (sx, sy, sz) = (
                        (next() % 16) as usize,
                        (next() % 16) as usize,
                        (next() % 16) as usize,
                    );
                    assert_eq!(palette.get(sx, sy, sz), model[sy][sz][sx]);
                }
            }
        }

        assert_matches_model(&palette, &model);

        // 与同一模型的批量构建比对：取值序列与位宽必须一致
        // （随机变异与行主序批量构建的调色板内序不同，打包字节
        // 不作逐位比对——行主序流的字节一致性由 bulk_palette_* 覆盖）
        let bulk = BlockPalette::from_fn(|x, y, z| model[y][z][x]);
        assert_eq!(
            palette.iter().collect::<Vec<_>>(),
            bulk.iter().collect::<Vec<_>>()
        );
        assert_eq!(
            palette.convert_network().bits_per_entry,
            bulk.convert_network().bits_per_entry
        );
    }

    #[test]
    fn bulk_palette_matches_nibble_tier_mutations() {
        // 空气（默认态）纳入池中：内容里始终存在默认态单元，
        // 其调色板条目不会因计数归零而被 swap_remove 重排，
        // 变异与批量构建的调色板内序一致，序列化字节可逐位比对。
        let mut states = vec![Block::AIR.default_state.id];
        states.extend((1u16..=11).map(BlockStateId::new_or_air));
        assert_bulk_matches_mutations(|x, y, z| states[(x + y * 3 + z * 7) % states.len()]);
    }

    #[test]
    fn bulk_palette_matches_indexed_tier_mutations() {
        let mut states = vec![Block::AIR.default_state.id];
        states.extend((1u16..=19).map(BlockStateId::new_or_air));
        assert_bulk_matches_mutations(|x, y, z| states[(x * 5 + y + z * 11) % states.len()]);
    }
}
