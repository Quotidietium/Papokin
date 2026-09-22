use proc_macro2::TokenStream;
use quote::quote;

const MAX_VIEW_DISTANCE: u8 = 32;
const MAX_CHEBYSHEV_RADIUS: u8 = 48;

pub fn build() -> TokenStream {
    let entries = (0..=MAX_VIEW_DISTANCE).map(|dist| {
        if dist < 2 {
            return quote! { &[] };
        }
        let mut positions = vec![];
        let d = i64::from(dist);

        for z in -(d + 2)..=(d + 2) {
            for x in -(d + 2)..=(d + 2) {
                let rel_x = (x.abs() - 2).max(0);
                let rel_z = (z.abs() - 2).max(0);
                if rel_x * rel_x + rel_z * rel_z < d * d {
                    positions.push((x as i8, z as i8));
                }
            }
        }

        positions.sort_by_key(|&(x, z)| i32::from(x).pow(2) + i32::from(z).pow(2));

        let array_elems = positions.into_iter().map(|(x, z)| quote!((#x, #z)));

        quote! {
            &[ #(#array_elems),* ]
        }
    });

    let array_len = MAX_VIEW_DISTANCE as usize + 1;

    // 生成切比雪夫/方形同心环偏移
    let mut all_chebyshev_offsets = vec![];
    let mut ring_ranges = vec![];
    let mut square_ranges = vec![];

    let mut current_idx = 0usize;
    for r in 0..=MAX_CHEBYSHEV_RADIUS {
        let r_i = r as i8;
        let start = current_idx;
        if r == 0 {
            all_chebyshev_offsets.push((0i8, 0i8));
            current_idx += 1;
        } else {
            // 顶部行和底部行
            for x in -r_i..=r_i {
                all_chebyshev_offsets.push((x, -r_i));
                all_chebyshev_offsets.push((x, r_i));
                current_idx += 2;
            }
            // 左右列（不含上面已添加的角落）
            for z in (-r_i + 1)..=(r_i - 1) {
                all_chebyshev_offsets.push((-r_i, z));
                all_chebyshev_offsets.push((r_i, z));
                current_idx += 2;
            }
        }
        let end = current_idx;
        ring_ranges.push((start, end));
        square_ranges.push((0usize, end));
    }

    let chebyshev_total = all_chebyshev_offsets.len();
    let chebyshev_elems = all_chebyshev_offsets
        .into_iter()
        .map(|(x, z)| quote!((#x, #z)));

    let ring_bounds = ring_ranges.iter().map(|(s, e)| quote!((#s, #e)));
    let square_bounds = square_ranges.iter().map(|(_, e)| quote!(#e));
    let chebyshev_len = MAX_CHEBYSHEV_RADIUS as usize + 1;

    quote! {
        /// 支持的最大视距
        pub const MAX_VIEW_DISTANCE: u8 = #MAX_VIEW_DISTANCE;

        /// 按视距计算相对区块偏移量的静态预计算查找表（0..=32）。
        pub static CHUNK_VIEW_LUT: [&[(i8, i8)]; #array_len] = [ #(#entries),* ];

        /// 支持的最大切比雪夫/方形半径（用于票证等级和模拟距离）
        pub const MAX_CHEBYSHEV_RADIUS: u8 = #MAX_CHEBYSHEV_RADIUS;

        /// 最高至 `MAX_CHEBYSHEV_RADIUS` 的所有同心切比雪夫偏移的扁平底层存储。
        pub static CHEBYSHEV_OFFSETS: [(i8, i8); #chebyshev_total] = [ #(#chebyshev_elems),* ];

        /// `CHEBYSHEV_OFFSETS` 中每个切比雪夫半径 (0..=48) 的 (start, end) 索引边界。
        pub static CHEBYSHEV_RING_BOUNDS: [(usize, usize); #chebyshev_len] = [ #(#ring_bounds),* ];

        /// `CHEBYSHEV_OFFSETS` 中覆盖切比雪夫半径（0..=48）内所有区块的结束索引。
        pub static CHEBYSHEV_SQUARE_BOUNDS: [usize; #chebyshev_len] = [ #(#square_bounds),* ];

        ///返回预计算的切片，包含恰好位于切比雪夫半径 `radius`（0..=48）处的区块偏移。
        #[inline]
        #[must_use]
        pub fn get_chebyshev_ring(radius: u8) -> &'static [(i8, i8)] {
            let r = (radius as usize).min(MAX_CHEBYSHEV_RADIUS as usize);
            let (start, end) = CHEBYSHEV_RING_BOUNDS[r];
            &CHEBYSHEV_OFFSETS[start..end]
        }

        ///返回预计算的切片，包含切比雪夫半径 `radius`（0..=48）内的所有区块偏移，并按同心顺序排序。
        #[inline]
        #[must_use]
        pub fn get_chebyshev_square(radius: u8) -> &'static [(i8, i8)] {
            let r = (radius as usize).min(MAX_CHEBYSHEV_RADIUS as usize);
            let end = CHEBYSHEV_SQUARE_BOUNDS[r];
            &CHEBYSHEV_OFFSETS[..end]
        }
    }
}
