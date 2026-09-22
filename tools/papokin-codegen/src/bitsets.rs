use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Ident;

/// 一个已编译的位集，随时可嵌入生成的代码中。
pub struct Bitset {
    /// 生成的模块，包含位集常量和查找函数。
    pub items: TokenStream,
    /// 封装位集的私有模块的标识符。
    pub mod_ident: Ident,
    /// 从模块导出的 `contains` 函数的标识符。
    pub contains_ident: Ident,
}

/// 为一组 `u16` ID 构建紧凑的 `u64` 字位集，并返回生成的代码。
///
/// # Arguments
/// - `name` – 用于派生所有生成标识符名称的基础名称。
/// - `ids` – 要包含在位集中的数字 ID 切片。
///
/// # Returns
/// 一个 [`Bitset`]，包含生成的模块 `TokenStream` 和辅助标识符。
pub fn gen_u16_bitset(name: &str, ids: &[u16]) -> Bitset {
    let max_id = ids.iter().copied().max().unwrap_or(0);
    //let min_id = ids.iter().copied().min().unwrap_or(0);

    let words = ((max_id as usize) + 64) / 64;

    let mut bitset = vec![0u64; words];
    for &id in ids {
        let index = (id as usize) >> 6;
        let bit = u32::from(id) & 63;
        bitset[index] |= 1u64 << bit;
    }
    let name_uppercase = name.to_uppercase();

    let mod_ident = Ident::new(
        &format!("__{}_bitset", name.to_lowercase()),
        Span::call_site(),
    );
    let max_ident = Ident::new(&format!("{name_uppercase}_MAX_ID"), Span::call_site());
    let words_ident = Ident::new(&format!("{name_uppercase}_WORDS"), Span::call_site());
    let bitset_ident = Ident::new(&format!("{name_uppercase}_BITSET"), Span::call_site());
    let contains_ident = Ident::new(
        &format!("{}_contains", name.to_lowercase()),
        Span::call_site(),
    );

    let items = quote! {
        mod #mod_ident {
            pub const #max_ident: u16 = #max_id;
            pub const #words_ident: usize = #words;
            pub static #bitset_ident: [u64; #words_ident] = [ #(#bitset),* ];

            #[inline(always)]
            pub(super) const fn #contains_ident(id: u16) -> bool {
                if id > #max_ident {
                    return false;
                }
                let index: usize = (id as usize) >> 6;
                let bit: u32 = (id as u32) & 63;

                ((#bitset_ident[index] >> bit) & 1) != 0
            }
        }

    };
    Bitset {
        items,
        mod_ident,
        contains_ident,
    }
}
