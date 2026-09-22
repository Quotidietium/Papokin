use crate::random::RandomImpl;
#[cfg(feature = "codegen")]
use proc_macro2::{Span, TokenStream};
#[cfg(feature = "codegen")]
use quote::{ToTokens, quote};
use serde::Deserialize;
#[cfg(feature = "codegen")]
use syn::LitFloat;

/// 表示生成浮点数值的各类数字提供器。
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type")]
pub enum NormalFloatProvider {
    /// 始终返回同一个常量值。
    #[serde(rename = "minecraft:constant")]
    Constant(ConstantFloatProvider),
    /// Returns uniformly distributed random values within a range [min, max).
    #[serde(rename = "minecraft:uniform")]
    Uniform(UniformFloatProvider),
    /// 返回来自正态（高斯）分布的值，并钳制到指定范围内。
    #[serde(rename = "minecraft:clamped_normal")]
    ClampedNormal(ClampedNormalFloatProvider),
    /// 返回来自梯形分布的值（平坦平台加线性斜坡）。
    #[serde(rename = "minecraft:trapezoid")]
    Trapezoid(TrapezoidFloatProvider),
}

#[cfg(feature = "codegen")]
impl ToTokens for NormalFloatProvider {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Self::Constant(constant) => {
                tokens.extend(quote! {
                    NormalFloatProvider::Constant(#constant)
                });
            }
            Self::Uniform(uniform) => {
                tokens.extend(quote! {
                    NormalFloatProvider::Uniform(#uniform)
                });
            }
            Self::ClampedNormal(clamped_normal) => {
                tokens.extend(quote! {
                    NormalFloatProvider::ClampedNormal(#clamped_normal)
                });
            }
            Self::Trapezoid(trapezoid) => {
                tokens.extend(quote! {
                    NormalFloatProvider::Trapezoid(#trapezoid)
                });
            }
        }
    }
}

/// 一个灵活的浮点数提供器，可以是常量值或复杂提供器。
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum FloatProvider {
    /// 一个分布可配置的复杂提供器。
    Object(NormalFloatProvider),
    /// 一个简单的常量浮点值。
    Constant(f32),
}

#[cfg(feature = "codegen")]
impl ToTokens for FloatProvider {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Self::Object(float_provider) => {
                tokens.extend(quote! {
                    FloatProvider::Object(#float_provider)
                });
            }
            Self::Constant(f) => tokens.extend(quote! {
                FloatProvider::Constant(#f)
            }),
        }
    }
}

impl FloatProvider {
    #[must_use]
    /// 此提供器可生成的最小可能值。
    ///
    /// # Returns
    /// 此提供器可生成的最小值（含）。
    pub const fn get_min(&self) -> f32 {
        match self {
            Self::Object(inv_provider) => match inv_provider {
                NormalFloatProvider::Constant(constant) => constant.get_min(),
                NormalFloatProvider::Uniform(uniform) => uniform.get_min(),
                NormalFloatProvider::ClampedNormal(clamped_normal) => clamped_normal.get_min(),
                NormalFloatProvider::Trapezoid(trapezoid) => trapezoid.get_min(),
            },
            Self::Constant(i) => *i,
        }
    }

    /// 使用所配置的分布生成随机浮点值。
    ///
    /// # Arguments
    /// - `random` – 要使用的随机数生成器。
    ///
    /// # Returns
    /// 根据提供者的分布策略生成的随机浮点值。
    pub fn get(&self, random: &mut impl RandomImpl) -> f32 {
        match self {
            Self::Object(inv_provider) => match inv_provider {
                NormalFloatProvider::Constant(constant) => constant.get(random),
                NormalFloatProvider::Uniform(uniform) => uniform.get(random),
                NormalFloatProvider::ClampedNormal(clamped_normal) => clamped_normal.get(random),
                NormalFloatProvider::Trapezoid(trapezoid) => trapezoid.get(random),
            },
            Self::Constant(i) => *i,
        }
    }

    /// 返回此提供者可生成的最大可能值。
    ///
    /// # Returns
    /// 可生成的最大值（含或不含，取决于提供器类型）。
    #[must_use]
    pub const fn get_max(&self) -> f32 {
        match self {
            Self::Object(inv_provider) => match inv_provider {
                NormalFloatProvider::Constant(constant) => constant.get_max(),
                NormalFloatProvider::Uniform(uniform) => uniform.get_max(),
                NormalFloatProvider::ClampedNormal(clamped_normal) => clamped_normal.get_max(),
                NormalFloatProvider::Trapezoid(trapezoid) => trapezoid.get_max(),
            },
            Self::Constant(i) => *i,
        }
    }
}

/// 一种始终返回同一常量值的浮点数提供器。
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct ConstantFloatProvider {
    /// 始终返回的常量值。
    value: f32,
}

#[cfg(feature = "codegen")]
impl ToTokens for ConstantFloatProvider {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let value = LitFloat::new(&self.value.to_string(), Span::call_site());
        tokens.extend(quote! {
            ConstantFloatProvider { value: #value }
        });
    }
}

impl ConstantFloatProvider {
    /// 使用给定值创建新的常量提供者。
    ///
    /// # Arguments
    /// - `value` – 要返回的常量值。
    ///
    /// # Returns
    /// 一个新的 `ConstantFloatProvider` 实例。
    #[must_use]
    pub const fn new(value: f32) -> Self {
        Self { value }
    }

    /// 返回最小值（与常量值相同）。
    ///
    /// # Returns
    /// 常量值。
    #[must_use]
    pub const fn get_min(&self) -> f32 {
        self.value
    }

    ///返回常量值，忽略随机数生成器。
    ///
    /// # Arguments
    /// - `_random` – 随机数生成器（未使用）。
    ///
    /// # Returns
    /// 常量值。
    pub const fn get(&self, _random: &mut impl RandomImpl) -> f32 {
        self.value
    }

    /// 返回最大值（与常量值相同）。
    ///
    /// # Returns
    /// 常量值。
    #[must_use]
    pub const fn get_max(&self) -> f32 {
        self.value
    }
}

/// A float provider that generates uniformly distributed random values.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct UniformFloatProvider {
    /// 可生成的最小值（含）。
    min_inclusive: f32,
    /// 可生成的最大值（不含）。
    max_exclusive: f32,
}

#[cfg(feature = "codegen")]
impl ToTokens for UniformFloatProvider {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let min_inclusive = LitFloat::new(&self.min_inclusive.to_string(), Span::call_site());
        let max_exclusive = LitFloat::new(&self.max_exclusive.to_string(), Span::call_site());
        tokens.extend(quote! {
            UniformFloatProvider { min_inclusive: #min_inclusive, max_exclusive: #max_exclusive }
        });
    }
}

impl UniformFloatProvider {
    /// 使用指定范围创建一个新的均匀分布提供器。
    ///
    /// # Arguments
    /// - `min_inclusive` – 最小值（含）。
    /// - `max_exclusive` – 最大值（不含）。
    ///
    /// # Returns
    /// 一个新的 `UniformFloatProvider` 实例。
    #[must_use]
    pub const fn new(min_inclusive: f32, max_exclusive: f32) -> Self {
        Self {
            min_inclusive,
            max_exclusive,
        }
    }

    /// 返回最小值（含）。
    ///
    /// # Returns
    /// 可生成的最小值（含）。
    #[must_use]
    pub const fn get_min(&self) -> f32 {
        self.min_inclusive
    }

    /// Generates a uniformly distributed random value in the configured range.
    ///
    /// # Arguments
    /// - `random` – 要使用的随机数生成器。
    ///
    /// # Returns
    /// 范围在 [`min_inclusive`, `max_exclusive`] 内的随机浮点数。
    pub fn get(&self, random: &mut impl RandomImpl) -> f32 {
        // NOTE: 使用 [min_inclusive, max_exclusive) 内的随机范围
        let range = self.max_exclusive - self.min_inclusive;
        random.next_f32().mul_add(range, self.min_inclusive)
    }

    /// 返回最大值（不含）。
    ///
    /// # Returns
    /// 可生成的最大值（不含）。
    #[must_use]
    pub const fn get_max(&self) -> f32 {
        self.max_exclusive
    }
}

/// 一种从正态（高斯）分布生成数值的浮点数提供器。
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct ClampedNormalFloatProvider {
    /// 正态分布的均值（中心）。
    mean: f32,
    /// 正态分布的标准差。
    deviation: f32,
    /// 最小钳制值（含）。
    min: f32,
    /// 最大钳制值（含）。
    max: f32,
}

#[cfg(feature = "codegen")]
impl ToTokens for ClampedNormalFloatProvider {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let mean = LitFloat::new(&self.mean.to_string(), Span::call_site());
        let deviation = LitFloat::new(&self.deviation.to_string(), Span::call_site());
        let min = LitFloat::new(&self.min.to_string(), Span::call_site());
        let max = LitFloat::new(&self.max.to_string(), Span::call_site());
        tokens.extend(quote! {
            ClampedNormalFloatProvider {
                mean: #mean,
                deviation: #deviation,
                min: #min,
                max: #max
            }
        });
    }
}

impl ClampedNormalFloatProvider {
    /// 使用指定参数创建新的钳制正态分布提供者。
    ///
    /// # Arguments
    /// - `mean` – 正态分布的均值（中心）。
    /// - `deviation` – 正态分布的标准差。
    /// - `min` – 夹取的下限值（含）。
    /// - `max` – 夹取的上限值（含）。
    ///
    /// # Returns
    /// 一个新的 `ClampedNormalFloatProvider` 实例。
    #[must_use]
    pub const fn new(mean: f32, deviation: f32, min: f32, max: f32) -> Self {
        Self {
            mean,
            deviation,
            min,
            max,
        }
    }

    /// 返回最小钳制值（含）。
    ///
    /// # Returns
    /// 钳制后可生成的最小值。
    #[must_use]
    pub const fn get_min(&self) -> f32 {
        self.min
    }

    /// 从截断正态分布生成随机值。
    ///
    /// # Arguments
    /// - `random` – 要使用的随机数生成器。
    ///
    /// # Returns
    /// 来自正态分布的随机浮点数，被限制在 [min, max] 范围内。
    pub fn get(&self, random: &mut impl RandomImpl) -> f32 {
        // NOTE: 生成正态分布值
        let gaussian = random.next_gaussian() as f32;
        let value = gaussian.mul_add(self.deviation, self.mean);

        // NOTE: 钳制到 min/max 范围
        value.clamp(self.min, self.max)
    }

    /// 返回最大钳制值（含）。
    ///
    /// # Returns
    /// 钳制后可生成的最大值。
    #[must_use]
    pub const fn get_max(&self) -> f32 {
        self.max
    }
}

/// 一种从梯形分布生成数值的浮点数提供器。
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct TrapezoidFloatProvider {
    /// 可生成的最小值（含）。
    min: f32,
    /// 可生成的最大值（含）。
    max: f32,
    /// 平坦平台宽度占总范围的比例（0.0 到 1.0）。
    plateau: f32,
}

#[cfg(feature = "codegen")]
impl ToTokens for TrapezoidFloatProvider {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let min = LitFloat::new(&self.min.to_string(), Span::call_site());
        let max = LitFloat::new(&self.max.to_string(), Span::call_site());
        let plateau = LitFloat::new(&self.plateau.to_string(), Span::call_site());
        tokens.extend(quote! {
            TrapezoidFloatProvider {
                min: #min,
                max: #max,
                plateau: #plateau
            }
        });
    }
}

impl TrapezoidFloatProvider {
    /// 使用指定参数创建一个新的梯形提供器。
    ///
    /// # Arguments
    /// - `min` – 最小值（含）。
    /// - `max` – 最大值（含）。
    /// - `plateau` – 平顶部分的宽度占总范围的比例（0.0 到 1.0）。
    ///
    /// # Returns
    /// 一个新的 `TrapezoidFloatProvider` 实例。
    #[must_use]
    pub const fn new(min: f32, max: f32, plateau: f32) -> Self {
        Self { min, max, plateau }
    }

    /// 返回最小值（含）。
    ///
    /// # Returns
    /// 可生成的最小值。
    #[must_use]
    pub const fn get_min(&self) -> f32 {
        self.min
    }

    /// 从梯形分布生成随机值。
    ///
    /// # Arguments
    /// - `random` – 要使用的随机数生成器。
    ///
    /// # Returns
    /// 来自梯形分布的随机浮点数，范围在 [min, max] 内。
    pub fn get(&self, random: &mut impl RandomImpl) -> f32 {
        // NOTE: 梯形分布：中间平坦高原，两侧线性斜坡。
        let range = self.max - self.min;
        let plateau_range = range * self.plateau;
        let ramp_range = (range - plateau_range) * 0.5;

        let random_value = random.next_f32();

        if random_value < self.plateau.mul_add(-0.5, 0.5) {
            // NOTE: 左斜坡：偏向高原的二次分布
            let scaled = random_value / self.plateau.mul_add(-0.5, 0.5);
            let sqrt_scaled = scaled.sqrt();
            self.min + ramp_range * sqrt_scaled
        } else if random_value > self.plateau.mul_add(0.5, 0.5) {
            // NOTE: 右斜坡：偏向高原的二次分布
            let scaled =
                (random_value - self.plateau.mul_add(0.5, 0.5)) / self.plateau.mul_add(-0.5, 0.5);
            let sqrt_scaled = (1.0 - scaled).sqrt();
            self.max - ramp_range * sqrt_scaled
        } else {
            // NOTE: 高原：均匀分布
            let plateau_pos = (random_value - self.plateau.mul_add(-0.5, 0.5)) / self.plateau;
            self.min + ramp_range + plateau_pos * plateau_range
        }
    }

    /// 返回最大值（含）。
    ///
    /// # Returns
    /// 可生成的最大值。
    #[must_use]
    pub const fn get_max(&self) -> f32 {
        self.max
    }
}

/// 浮点提供器实现的测试。
#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::{RandomGenerator, get_seed};

    #[test]
    fn constant_float_provider() {
        let mut random = RandomGenerator::Xoroshiro(
            crate::random::xoroshiro128::Xoroshiro::from_seed(get_seed()),
        );
        let provider = ConstantFloatProvider::new(5.5);

        assert_eq!(provider.get_min(), 5.5);
        assert_eq!(provider.get_max(), 5.5);
        assert_eq!(provider.get(&mut random), 5.5);
        assert_eq!(provider.get(&mut random), 5.5); // 应始终返回相同的值
    }

    #[test]
    fn uniform_float_provider() {
        let mut random = RandomGenerator::Xoroshiro(
            crate::random::xoroshiro128::Xoroshiro::from_seed(get_seed()),
        );
        let provider = UniformFloatProvider::new(1.0, 5.0);

        assert_eq!(provider.get_min(), 1.0);
        assert_eq!(provider.get_max(), 5.0);

        // 测试值在范围内
        for _ in 0..100 {
            let value = provider.get(&mut random);
            assert!(
                (1.0..5.0).contains(&value),
                "Value {value} is outside range [1.0, 5.0)"
            );
        }
    }

    #[test]
    fn clamped_normal_float_provider() {
        let mut random = RandomGenerator::Xoroshiro(
            crate::random::xoroshiro128::Xoroshiro::from_seed(get_seed()),
        );
        let provider = ClampedNormalFloatProvider::new(3.0, 1.0, 1.0, 5.0);

        assert_eq!(provider.get_min(), 1.0);
        assert_eq!(provider.get_max(), 5.0);

        // 测试值在范围内
        for _ in 0..100 {
            let value = provider.get(&mut random);
            assert!(
                (1.0..=5.0).contains(&value),
                "Value {value} is outside range [1.0, 5.0]"
            );
        }
    }

    #[test]
    fn trapezoid_float_provider() {
        let mut random = RandomGenerator::Xoroshiro(
            crate::random::xoroshiro128::Xoroshiro::from_seed(get_seed()),
        );
        let provider = TrapezoidFloatProvider::new(0.0, 10.0, 0.5);

        assert_eq!(provider.get_min(), 0.0);
        assert_eq!(provider.get_max(), 10.0);

        // 测试值在范围内
        for _ in 0..100 {
            let value = provider.get(&mut random);
            assert!(
                (0.0..=10.0).contains(&value),
                "Value {value} is outside range [0.0, 10.0]"
            );
        }
    }

    #[test]
    fn float_provider_enum_constant() {
        let mut random = RandomGenerator::Xoroshiro(
            crate::random::xoroshiro128::Xoroshiro::from_seed(get_seed()),
        );
        let provider = FloatProvider::Constant(7.5);

        assert_eq!(provider.get_min(), 7.5);
        assert_eq!(provider.get_max(), 7.5);
        assert_eq!(provider.get(&mut random), 7.5);
    }

    #[test]
    fn float_provider_enum_object() {
        let mut random = RandomGenerator::Xoroshiro(
            crate::random::xoroshiro128::Xoroshiro::from_seed(get_seed()),
        );
        let uniform = UniformFloatProvider::new(2.0, 8.0);
        let provider = FloatProvider::Object(NormalFloatProvider::Uniform(uniform));

        assert_eq!(provider.get_min(), 2.0);
        assert_eq!(provider.get_max(), 8.0);

        let value = provider.get(&mut random);
        assert!(
            (2.0..8.0).contains(&value),
            "Value {value} is outside range [2.0, 8.0)"
        );
    }
}
