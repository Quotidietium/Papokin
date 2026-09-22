use crate::random::RandomImpl;
#[cfg(feature = "codegen")]
use proc_macro2::{Span, TokenStream};
#[cfg(feature = "codegen")]
use quote::{ToTokens, quote};
use serde::Deserialize;
#[cfg(feature = "codegen")]
use syn::LitInt;

/// 表示生成整数值的各类数字提供器。
#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum NormalIntProvider {
    /// 始终返回同一个常量值。
    #[serde(rename = "minecraft:constant")]
    Constant(ConstantIntProvider),
    /// Returns uniformly distributed random values within an inclusive range [min, max].
    #[serde(rename = "minecraft:uniform")]
    Uniform(UniformIntProvider),
    /// 返回偏向范围较低端的值（三角分布）。
    #[serde(rename = "minecraft:biased_to_bottom")]
    BiasedToBottom(BiasedToBottomIntProvider),
    /// 返回强烈偏向范围较低端的值。
    #[serde(rename = "minecraft:very_biased_to_bottom")]
    VeryBiasedToBottom(VeryBiasedToBottomIntProvider),
    /// 包装另一个提供器，并将其输出限制在指定范围内。
    #[serde(rename = "minecraft:clamped")]
    Clamped(ClampedIntProvider),
    #[serde(rename = "minecraft:trapezoid")]
    Trapezoid(TrapezoidIntProvider),
    /// 返回来自正态（高斯）分布的值，并钳制到指定的闭区间内。
    #[serde(rename = "minecraft:clamped_normal")]
    ClampedNormal(ClampedNormalIntProvider),
    /// 返回来自加权列表的值，其中各条目具有不同的概率。
    #[serde(rename = "minecraft:weighted_list")]
    WeightedList(WeightedListIntProvider),
}

#[cfg(feature = "codegen")]
impl ToTokens for NormalIntProvider {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Self::Constant(constant) => {
                tokens.extend(quote! {
                    NormalIntProvider::Constant(#constant)
                });
            }
            Self::Uniform(uniform) => {
                tokens.extend(quote! {
                    NormalIntProvider::Uniform(#uniform)
                });
            }
            Self::BiasedToBottom(biased) => {
                tokens.extend(quote! {
                    NormalIntProvider::BiasedToBottom(#biased)
                });
            }
            Self::VeryBiasedToBottom(very_biased) => {
                tokens.extend(quote! {
                    NormalIntProvider::VeryBiasedToBottom(#very_biased)
                });
            }
            Self::Clamped(clamped) => {
                tokens.extend(quote! {
                    NormalIntProvider::Clamped(#clamped)
                });
            }
            Self::ClampedNormal(clamped_normal) => {
                tokens.extend(quote! {
                    NormalIntProvider::ClampedNormal(#clamped_normal)
                });
            }
            Self::Trapezoid(trapezoid) => {
                tokens.extend(quote! {
                    NormalIntProvider::Trapezoid(#trapezoid)
                });
            }
            Self::WeightedList(weighted_list) => {
                tokens.extend(quote! {
                    NormalIntProvider::WeightedList(#weighted_list)
                });
            }
        }
    }
}

/// 一个灵活的整数提供器，可以是常量值或复杂提供器。
#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum IntProvider {
    /// 一个分布可配置的复杂提供器。
    Object(NormalIntProvider),
    /// 一个简单的常量整数值。
    Constant(i32),
}

#[cfg(feature = "codegen")]
impl ToTokens for IntProvider {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Self::Object(int_provider) => {
                tokens.extend(quote! {
                    IntProvider::Object(#int_provider)
                });
            }
            Self::Constant(i) => tokens.extend(quote! {
                IntProvider::Constant(#i)
            }),
        }
    }
}

impl IntProvider {
    /// 返回此提供者可生成的最小可能值。
    ///
    /// # Returns
    /// 此提供器可生成的最小值（含）。
    #[must_use]
    pub fn get_min(&self) -> i32 {
        match self {
            Self::Object(int_provider) => match int_provider {
                NormalIntProvider::Constant(constant) => constant.get_min(),
                NormalIntProvider::Uniform(uniform) => uniform.get_min(),
                NormalIntProvider::BiasedToBottom(biased) => biased.get_min(),
                NormalIntProvider::VeryBiasedToBottom(very_biased) => very_biased.get_min(),
                NormalIntProvider::Clamped(clamped) => clamped.get_min(),
                NormalIntProvider::Trapezoid(trapezoid) => trapezoid.get_min(),
                NormalIntProvider::ClampedNormal(clamped_normal) => clamped_normal.get_min(),
                NormalIntProvider::WeightedList(weighted_list) => weighted_list.get_min(),
            },
            Self::Constant(i) => *i,
        }
    }

    /// 使用所配置的分布生成随机整数值。
    ///
    /// # Arguments
    /// - `random` – 要使用的随机数生成器。
    ///
    /// # Returns
    /// 根据提供者的分布策略生成的随机整数值。
    pub fn get(&self, random: &mut impl RandomImpl) -> i32 {
        match self {
            Self::Object(int_provider) => match int_provider {
                NormalIntProvider::Constant(constant) => constant.get(random),
                NormalIntProvider::Uniform(uniform) => uniform.get(random),
                NormalIntProvider::BiasedToBottom(biased) => biased.get(random),
                NormalIntProvider::VeryBiasedToBottom(very_biased) => very_biased.get(random),
                NormalIntProvider::Clamped(clamped) => clamped.get(random),
                NormalIntProvider::ClampedNormal(clamped_normal) => clamped_normal.get(random),
                NormalIntProvider::WeightedList(weighted_list) => weighted_list.get(random),
                NormalIntProvider::Trapezoid(trapezoid_int_provider) => {
                    trapezoid_int_provider.get(random)
                }
            },
            Self::Constant(i) => *i,
        }
    }

    /// 返回此提供者可生成的最大可能值。
    ///
    /// # Returns
    /// 此提供器可生成的最大值（含）。
    #[must_use]
    pub fn get_max(&self) -> i32 {
        match self {
            Self::Object(int_provider) => match int_provider {
                NormalIntProvider::Constant(constant) => constant.get_max(),
                NormalIntProvider::Uniform(uniform) => uniform.get_max(),
                NormalIntProvider::BiasedToBottom(biased) => biased.get_max(),
                NormalIntProvider::VeryBiasedToBottom(very_biased) => very_biased.get_max(),
                NormalIntProvider::Clamped(clamped) => clamped.get_max(),
                NormalIntProvider::ClampedNormal(clamped_normal) => clamped_normal.get_max(),
                NormalIntProvider::WeightedList(weighted_list) => weighted_list.get_max(),
                NormalIntProvider::Trapezoid(trapezoid_int_provider) => {
                    trapezoid_int_provider.get_max()
                }
            },
            Self::Constant(i) => *i,
        }
    }
}

/// 始终返回同一常量值的整数提供器。
#[derive(Deserialize, Clone, Debug)]
pub struct ConstantIntProvider {
    /// 始终返回的常量值。
    pub value: i32,
}

#[cfg(feature = "codegen")]
impl ToTokens for ConstantIntProvider {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let value = LitInt::new(&self.value.to_string(), Span::call_site());
        tokens.extend(quote! {
            ConstantIntProvider { value: #value }
        });
    }
}

impl ConstantIntProvider {
    /// 使用给定值创建新的常量提供者。
    ///
    /// # Arguments
    /// - `value` – 要返回的常量值。
    ///
    /// # Returns
    /// 一个新的 `ConstantIntProvider` 实例。
    #[must_use]
    pub const fn new(value: i32) -> Self {
        Self { value }
    }

    /// 返回最小值（与常量值相同）。
    ///
    /// # Returns
    /// 常量值。
    #[must_use]
    pub const fn get_min(&self) -> i32 {
        self.value
    }

    ///返回常量值，忽略随机数生成器。
    ///
    /// # Arguments
    /// - `_random` – 随机数生成器（未使用）。
    ///
    /// # Returns
    /// 常量值。
    pub const fn get(&self, _random: &mut impl RandomImpl) -> i32 {
        self.value
    }

    /// 返回最大值（与常量值相同）。
    ///
    /// # Returns
    /// 常量值。
    #[must_use]
    pub const fn get_max(&self) -> i32 {
        self.value
    }
}

/// 生成偏向范围较低一端数值的整数提供器。
#[derive(Deserialize, Clone, Debug)]
pub struct BiasedToBottomIntProvider {
    /// 可生成的最小值（含）。
    pub min_inclusive: i32,
    /// 可生成的最大值（含）。
    pub max_inclusive: i32,
}

#[cfg(feature = "codegen")]
impl ToTokens for BiasedToBottomIntProvider {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let min_inclusive = LitInt::new(&self.min_inclusive.to_string(), Span::call_site());
        let max_inclusive = LitInt::new(&self.max_inclusive.to_string(), Span::call_site());
        tokens.extend(quote! {
            BiasedToBottomIntProvider { min_inclusive: #min_inclusive, max_inclusive: #max_inclusive }
        });
    }
}

impl BiasedToBottomIntProvider {
    /// 使用指定范围创建新的偏向底部提供者。
    ///
    /// # Arguments
    /// - `min_inclusive` – 最小值（含）。
    /// - `max_inclusive` – 最大值（含）。
    ///
    /// # Returns
    /// 一个新的 `BiasedToBottomIntProvider` 实例。
    #[must_use]
    pub const fn new(min_inclusive: i32, max_inclusive: i32) -> Self {
        Self {
            min_inclusive,
            max_inclusive,
        }
    }

    /// 返回最小值（含）。
    ///
    /// # Returns
    /// 可生成的最小值（含）。
    #[must_use]
    pub const fn get_min(&self) -> i32 {
        self.min_inclusive
    }

    /// 生成偏向范围较低一端的随机值。
    ///
    /// # Arguments
    /// - `random` – 要使用的随机数生成器。
    ///
    /// # Returns
    /// 范围在 [`min_inclusive`, `max_inclusive`] 内的随机整数，较小的值出现的概率更高。
    pub fn get(&self, random: &mut impl RandomImpl) -> i32 {
        if self.min_inclusive >= self.max_inclusive {
            return self.min_inclusive;
        }
        let range = self.max_inclusive - self.min_inclusive + 1;
        let bound = random.next_bounded_i32(range) + 1;
        self.min_inclusive + random.next_bounded_i32(bound)
    }

    /// 返回最大值（含）。
    ///
    /// # Returns
    /// 可生成的最大值（含）。
    #[must_use]
    pub const fn get_max(&self) -> i32 {
        self.max_inclusive
    }
}

/// 生成强烈偏向范围较低一端数值的整数提供器。
#[derive(Deserialize, Clone, Debug)]
pub struct VeryBiasedToBottomIntProvider {
    /// 可生成的最小值（含）。
    pub min_inclusive: i32,
    /// 可生成的最大值（含）。
    pub max_inclusive: i32,
    /// 额外随机边界步数（默认为 1）。
    #[serde(default = "default_very_biased_inner")]
    pub inner: i32,
}

const fn default_very_biased_inner() -> i32 {
    1
}

#[cfg(feature = "codegen")]
impl ToTokens for VeryBiasedToBottomIntProvider {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let min_inclusive = LitInt::new(&self.min_inclusive.to_string(), Span::call_site());
        let max_inclusive = LitInt::new(&self.max_inclusive.to_string(), Span::call_site());
        let inner = LitInt::new(&self.inner.to_string(), Span::call_site());
        tokens.extend(quote! {
            VeryBiasedToBottomIntProvider {
                min_inclusive: #min_inclusive,
                max_inclusive: #max_inclusive,
                inner: #inner,
            }
        });
    }
}

impl VeryBiasedToBottomIntProvider {
    #[must_use]
    pub const fn new(min_inclusive: i32, max_inclusive: i32, inner: i32) -> Self {
        Self {
            min_inclusive,
            max_inclusive,
            inner,
        }
    }

    #[must_use]
    pub const fn get_min(&self) -> i32 {
        self.min_inclusive
    }

    pub fn get(&self, random: &mut impl RandomImpl) -> i32 {
        if self.min_inclusive >= self.max_inclusive {
            return self.min_inclusive;
        }
        let range = self.max_inclusive - self.min_inclusive + 1;
        let mut bound = range;
        for _ in 0..=self.inner {
            bound = random.next_bounded_i32(bound) + 1;
        }
        self.min_inclusive + bound - 1
    }

    #[must_use]
    pub const fn get_max(&self) -> i32 {
        self.max_inclusive
    }
}

/// 包装另一个提供器并对其输出进行钳制的整数提供器。
#[derive(Deserialize, Clone, Debug)]
pub struct ClampedIntProvider {
    /// 从中获取值的来源提供者。
    pub source: Box<IntProvider>,
    /// 钳制目标的最小值（含）。
    pub min_inclusive: i32,
    /// 钳制目标的最大值（含）。
    pub max_inclusive: i32,
}

#[cfg(feature = "codegen")]
impl ToTokens for ClampedIntProvider {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let source = &self.source;
        let min_inclusive = LitInt::new(&self.min_inclusive.to_string(), Span::call_site());
        let max_inclusive = LitInt::new(&self.max_inclusive.to_string(), Span::call_site());
        tokens.extend(quote! {
            ClampedIntProvider {
                source: Box::new(#source),
                min_inclusive: #min_inclusive,
                max_inclusive: #max_inclusive
            }
        });
    }
}

impl ClampedIntProvider {
    /// 使用指定的来源和范围创建新的钳制提供者。
    ///
    /// # Arguments
    /// - `source` – 用于取值的源提供者。
    /// - `min_inclusive` – 夹取目标的最小值（含）。
    /// - `max_inclusive` – 钳制目标的最大值（含）。
    ///
    /// # Returns
    /// 一个新的 `ClampedIntProvider` 实例。
    #[must_use]
    pub fn new(source: IntProvider, min_inclusive: i32, max_inclusive: i32) -> Self {
        Self {
            source: Box::new(source),
            min_inclusive,
            max_inclusive,
        }
    }

    /// 返回钳制后的最小值。
    ///
    /// # Returns
    /// 源最小值与钳制最小值中的较大者。
    #[must_use]
    pub fn get_min(&self) -> i32 {
        self.min_inclusive.max(self.source.get_min())
    }

    /// 从源生成随机值并将其钳制到配置的范围内。
    ///
    /// # Arguments
    /// - `random` – 要使用的随机数生成器。
    ///
    /// # Returns
    /// 来自源提供者的随机整数，被限制在 [`min_inclusive`, `max_inclusive`] 范围内。
    pub fn get(&self, random: &mut impl RandomImpl) -> i32 {
        self.source
            .get(random)
            .clamp(self.min_inclusive, self.max_inclusive)
    }

    /// 返回钳制后的最大值。
    ///
    /// # Returns
    /// 来源最大值与钳制最大值中的较小者。
    #[must_use]
    pub fn get_max(&self) -> i32 {
        self.max_inclusive.min(self.source.get_max())
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct TrapezoidIntProvider {
    /// 钳制目标的最小值（含）。
    pub min_inclusive: i32,
    /// 钳制目标的最大值（含）。
    pub max_inclusive: i32,
    pub plateau: i32,
}

#[cfg(feature = "codegen")]
impl ToTokens for TrapezoidIntProvider {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let min_inclusive = LitInt::new(&self.min_inclusive.to_string(), Span::call_site());
        let max_inclusive = LitInt::new(&self.max_inclusive.to_string(), Span::call_site());
        let plateau = LitInt::new(&self.plateau.to_string(), Span::call_site());
        tokens.extend(quote! {
            TrapezoidIntProvider {
                min_inclusive: #min_inclusive,
                max_inclusive: #max_inclusive,
                plateau: #plateau
            }
        });
    }
}

impl TrapezoidIntProvider {
    /// 使用指定的来源和范围创建新的钳制提供者。
    ///
    /// # Arguments
    /// - `min_inclusive` – 夹取目标的最小值（含）。
    /// - `max_inclusive` – 钳制目标的最大值（含）。
    ///
    /// # Returns
    /// 一个新的 `ClampedIntProvider` 实例。
    #[must_use]
    pub const fn new(min_inclusive: i32, max_inclusive: i32, plateau: i32) -> Self {
        Self {
            min_inclusive,
            max_inclusive,
            plateau,
        }
    }

    /// 返回钳制后的最小值。
    ///
    /// # Returns
    /// 源最小值与钳制最小值中的较大者。
    #[must_use]
    pub const fn get_min(&self) -> i32 {
        self.min_inclusive
    }

    /// 从源生成随机值并将其钳制到配置的范围内。
    ///
    /// # Arguments
    /// - `random` – 要使用的随机数生成器。
    ///
    /// # Returns
    /// 来自源提供者的随机整数，被限制在 [`min_inclusive`, `max_inclusive`] 范围内。
    pub fn get(&self, random: &mut impl RandomImpl) -> i32 {
        if self.min_inclusive >= self.max_inclusive {
            return self.min_inclusive;
        }
        let range = self.max_inclusive - self.min_inclusive;
        if self.plateau >= range {
            return self.min_inclusive + random.next_bounded_i32(range + 1);
        }
        let plateau_start = (range - self.plateau) / 2;
        let plateau_end = range - plateau_start;
        self.min_inclusive
            + random.next_bounded_i32(plateau_end + 1)
            + random.next_bounded_i32(plateau_start + 1)
    }

    /// 返回钳制后的最大值。
    ///
    /// # Returns
    /// 来源最大值与钳制最大值中的较小者。
    #[must_use]
    pub const fn get_max(&self) -> i32 {
        self.max_inclusive
    }
}

/// 从截断正态分布生成数值的整数提供器。
#[derive(Deserialize, Clone, Debug)]
pub struct ClampedNormalIntProvider {
    /// 正态分布的均值（中心）。
    pub mean: f32,
    /// 正态分布的标准差。
    pub deviation: f32,
    /// 最小钳制值（含）。
    pub min_inclusive: i32,
    /// 最大钳制值（含）。
    pub max_inclusive: i32,
}

#[cfg(feature = "codegen")]
impl ToTokens for ClampedNormalIntProvider {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let mean = syn::LitFloat::new(&self.mean.to_string(), Span::call_site());
        let deviation = syn::LitFloat::new(&self.deviation.to_string(), Span::call_site());
        let min_inclusive = LitInt::new(&self.min_inclusive.to_string(), Span::call_site());
        let max_inclusive = LitInt::new(&self.max_inclusive.to_string(), Span::call_site());
        tokens.extend(quote! {
            ClampedNormalIntProvider {
                mean: #mean,
                deviation: #deviation,
                min_inclusive: #min_inclusive,
                max_inclusive: #max_inclusive
            }
        });
    }
}

impl ClampedNormalIntProvider {
    /// 使用指定参数创建新的钳制正态分布提供者。
    ///
    /// # Arguments
    /// - `mean` – 正态分布的均值（中心）。
    /// - `deviation` – 正态分布的标准差。
    /// - `min_inclusive` – 夹取的下限值（含）。
    /// - `max_inclusive` – 钳制的最大包含值。
    ///
    /// # Returns
    /// 一个新的 `ClampedNormalIntProvider` 实例。
    #[must_use]
    pub const fn new(mean: f32, deviation: f32, min_inclusive: i32, max_inclusive: i32) -> Self {
        Self {
            mean,
            deviation,
            min_inclusive,
            max_inclusive,
        }
    }

    /// 返回最小钳制值（含）。
    ///
    /// # Returns
    /// 钳制后可生成的最小值。
    #[must_use]
    pub const fn get_min(&self) -> i32 {
        self.min_inclusive
    }

    /// 从截断正态分布生成随机值。
    ///
    /// # Arguments
    /// - `random` – 要使用的随机数生成器。
    ///
    /// # Returns
    /// 来自正态分布的随机整数，经四舍五入并限制在 [`min_inclusive`, `max_inclusive`] 范围内。
    pub fn get(&self, random: &mut impl RandomImpl) -> i32 {
        // NOTE: 生成正态分布值并钳制到范围内
        let gaussian = random.next_gaussian() as f32;
        let value = gaussian.mul_add(self.deviation, self.mean).round() as i32;
        value.clamp(self.min_inclusive, self.max_inclusive)
    }

    /// 返回最大钳制值（含）。
    ///
    /// # Returns
    /// 钳制后可生成的最大值。
    #[must_use]
    pub const fn get_max(&self) -> i32 {
        self.max_inclusive
    }
}

/// 加权列表提供者中的一个加权条目。
#[derive(Deserialize, Clone, Debug)]
pub struct WeightedEntry {
    /// 生成此条目值的整数提供器。
    pub data: IntProvider,
    /// 此条目被选中的权重（概率）。
    pub weight: i32,
}

#[cfg(feature = "codegen")]
impl ToTokens for WeightedEntry {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let data = &self.data;
        let weight = LitInt::new(&self.weight.to_string(), Span::call_site());
        tokens.extend(quote! {
            WeightedEntry { data: #data, weight: #weight }
        });
    }
}

/// 从带权列表中选择数值的整数提供器。
#[derive(Deserialize, Clone, Debug)]
pub struct WeightedListIntProvider {
    /// 可供选择的加权条目列表。
    pub distribution: Vec<WeightedEntry>,
}

#[cfg(feature = "codegen")]
impl ToTokens for WeightedListIntProvider {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let distribution = &self.distribution;
        tokens.extend(quote! {
            WeightedListIntProvider { distribution: vec![#(#distribution),*] }
        });
    }
}

impl WeightedListIntProvider {
    /// 使用给定的分布创建一个新的加权列表提供器。
    ///
    /// # Arguments
    /// - `distribution` – 带权重的条目列表。
    ///
    /// # Returns
    /// 一个新的 `WeightedListIntProvider` 实例。
    #[must_use]
    pub const fn new(distribution: Vec<WeightedEntry>) -> Self {
        Self { distribution }
    }

    /// 返回所有条目中可能的最小值。
    ///
    /// # Returns
    /// 所有条目中最小的最小值；若列表为空则为 0。
    #[must_use]
    pub fn get_min(&self) -> i32 {
        self.distribution
            .iter()
            .map(|entry| entry.data.get_min())
            .min()
            .unwrap_or(0)
    }

    /// 根据权重随机选择一个条目，并从中生成一个值。
    ///
    /// # Arguments
    /// - `random` – 要使用的随机数生成器。
    ///
    /// # Returns
    /// 来自所选加权条目的随机整数，若列表为空则为 0。
    pub fn get(&self, random: &mut impl RandomImpl) -> i32 {
        if self.distribution.is_empty() {
            return 0;
        }

        // 计算总权重。
        let total_weight: i32 = self.distribution.iter().map(|entry| entry.weight).sum();

        if total_weight == 0 {
            return 0;
        }

        // 随机选择一个权重。
        let chosen_weight = random.next_bounded_i32(total_weight);
        let mut current_weight = 0;

        // 找到与所选权重对应的条目
        for entry in &self.distribution {
            current_weight += entry.weight;
            if chosen_weight < current_weight {
                return entry.data.get(random);
            }
        }

        // 回退到最后一项
        self.distribution
            .last()
            .map_or(0, |entry| entry.data.get(random))
    }

    /// 返回所有条目中可能的最大值。
    ///
    /// # Returns
    /// 所有条目中的最大值，列表为空时为 0。
    #[must_use]
    pub fn get_max(&self) -> i32 {
        self.distribution
            .iter()
            .map(|entry| entry.data.get_max())
            .max()
            .unwrap_or(0)
    }
}

/// An integer provider that generates uniformly distributed random values.
#[derive(Deserialize, Clone, Debug)]
pub struct UniformIntProvider {
    /// 可生成的最小值（含）。
    pub min_inclusive: i32,
    /// 可生成的最大值（含）。
    pub max_inclusive: i32,
}

#[cfg(feature = "codegen")]
impl ToTokens for UniformIntProvider {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let min_inclusive = LitInt::new(&self.min_inclusive.to_string(), Span::call_site());
        let max_inclusive = LitInt::new(&self.max_inclusive.to_string(), Span::call_site());

        tokens.extend(quote! {
            UniformIntProvider { min_inclusive: #min_inclusive, max_inclusive: #max_inclusive }
        });
    }
}

impl UniformIntProvider {
    /// 使用指定的闭区间范围创建一个新的均匀分布提供器。
    ///
    /// # Arguments
    /// - `min_inclusive` – 最小值（含）。
    /// - `max_inclusive` – 最大值（含）。
    ///
    /// # Returns
    /// 一个新的 `UniformIntProvider` 实例。
    #[must_use]
    pub const fn new(min_inclusive: i32, max_inclusive: i32) -> Self {
        Self {
            min_inclusive,
            max_inclusive,
        }
    }

    /// 返回最小值（含）。
    ///
    /// # Returns
    /// 可生成的最小值（含）。
    #[must_use]
    pub const fn get_min(&self) -> i32 {
        self.min_inclusive
    }

    /// Generates a uniformly distributed random value in the configured range.
    ///
    /// # Arguments
    /// - `random` – 要使用的随机数生成器。
    ///
    /// # Returns
    /// 闭区间 [`min_inclusive`, `max_inclusive`] 内的随机整数。
    pub fn get(&self, random: &mut impl RandomImpl) -> i32 {
        random.next_inbetween_i32(self.min_inclusive, self.max_inclusive)
    }

    /// 返回最大值（含）。
    ///
    /// # Returns
    /// 可生成的最大值（含）。
    #[must_use]
    pub const fn get_max(&self) -> i32 {
        self.max_inclusive
    }
}

/// 整数提供器实现的测试。
#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::{RandomGenerator, get_seed};

    #[test]
    fn constant_int_provider() {
        let mut random = RandomGenerator::Xoroshiro(
            crate::random::xoroshiro128::Xoroshiro::from_seed(get_seed()),
        );
        let provider = ConstantIntProvider::new(42);

        assert_eq!(provider.get_min(), 42);
        assert_eq!(provider.get_max(), 42);
        assert_eq!(provider.get(&mut random), 42);
        assert_eq!(provider.get(&mut random), 42); // 应始终返回相同的值
    }

    #[test]
    fn uniform_int_provider() {
        let mut random = RandomGenerator::Xoroshiro(
            crate::random::xoroshiro128::Xoroshiro::from_seed(get_seed()),
        );
        let provider = UniformIntProvider::new(1, 10);

        assert_eq!(provider.get_min(), 1);
        assert_eq!(provider.get_max(), 10);

        // 测试值在范围内
        for _ in 0..100 {
            let value = provider.get(&mut random);
            assert!(
                (1..=10).contains(&value),
                "Value {value} is outside range [1, 10]"
            );
        }
    }

    #[test]
    fn biased_to_bottom_int_provider() {
        let mut random = RandomGenerator::Xoroshiro(
            crate::random::xoroshiro128::Xoroshiro::from_seed(get_seed()),
        );
        let provider = BiasedToBottomIntProvider::new(1, 20);

        assert_eq!(provider.get_min(), 1);
        assert_eq!(provider.get_max(), 20);

        // 测试值在范围内（偏向较小值）
        for _ in 0..100 {
            let value = provider.get(&mut random);
            assert!(
                (1..=20).contains(&value),
                "Value {value} is outside range [1, 20]"
            );
        }
    }

    #[test]
    fn clamped_normal_int_provider() {
        let mut random = RandomGenerator::Xoroshiro(
            crate::random::xoroshiro128::Xoroshiro::from_seed(get_seed()),
        );
        let provider = ClampedNormalIntProvider::new(5.0, 2.0, 1, 10);

        assert_eq!(provider.get_min(), 1);
        assert_eq!(provider.get_max(), 10);

        // 测试值在范围内
        for _ in 0..100 {
            let value = provider.get(&mut random);
            assert!(
                (1..=10).contains(&value),
                "Value {value} is outside range [1, 10]"
            );
        }
    }

    #[test]
    fn clamped_int_provider() {
        let mut random = RandomGenerator::Xoroshiro(
            crate::random::xoroshiro128::Xoroshiro::from_seed(get_seed()),
        );
        let source =
            IntProvider::Object(NormalIntProvider::Uniform(UniformIntProvider::new(1, 100)));
        let provider = ClampedIntProvider::new(source, 5, 15);

        assert_eq!(provider.get_min(), 5);
        assert_eq!(provider.get_max(), 15);

        // 测试值处于被钳制的范围内。
        for _ in 0..100 {
            let value = provider.get(&mut random);
            assert!(
                (5..=15).contains(&value),
                "Value {value} is outside clamped range [5, 15]"
            );
        }
    }

    #[test]
    fn weighted_list_int_provider() {
        let mut random = RandomGenerator::Xoroshiro(
            crate::random::xoroshiro128::Xoroshiro::from_seed(get_seed()),
        );

        let entries = vec![
            WeightedEntry {
                data: IntProvider::Constant(1),
                weight: 10,
            },
            WeightedEntry {
                data: IntProvider::Constant(2),
                weight: 20,
            },
            WeightedEntry {
                data: IntProvider::Constant(3),
                weight: 5,
            },
        ];

        let provider = WeightedListIntProvider::new(entries);

        assert_eq!(provider.get_min(), 1);
        assert_eq!(provider.get_max(), 3);

        // 测试值来自加权列表
        for _ in 0..100 {
            let value = provider.get(&mut random);
            assert!(
                (1..=3).contains(&value),
                "Value {value} is not from the weighted list"
            );
        }
    }

    #[test]
    fn int_provider_enum_constant() {
        let mut random = RandomGenerator::Xoroshiro(
            crate::random::xoroshiro128::Xoroshiro::from_seed(get_seed()),
        );
        let provider = IntProvider::Constant(25);

        assert_eq!(provider.get_min(), 25);
        assert_eq!(provider.get_max(), 25);
        assert_eq!(provider.get(&mut random), 25);
    }

    #[test]
    fn int_provider_enum_object() {
        let mut random = RandomGenerator::Xoroshiro(
            crate::random::xoroshiro128::Xoroshiro::from_seed(get_seed()),
        );
        let uniform = UniformIntProvider::new(5, 15);
        let provider = IntProvider::Object(NormalIntProvider::Uniform(uniform));

        assert_eq!(provider.get_min(), 5);
        assert_eq!(provider.get_max(), 15);

        let value = provider.get(&mut random);
        assert!(
            (5..=15).contains(&value),
            "Value {value} is outside range [5, 15]"
        );
    }
}
