use std::sync::LazyLock;

use serde::Deserialize;

use crate::{noise::simplex::OctaveSimplexNoiseSampler, random::legacy_rand::LegacyRand};

/// 用于生物群系温度变化的全局噪声采样器。
pub static TEMPERATURE_NOISE: LazyLock<OctaveSimplexNoiseSampler> = LazyLock::new(|| {
    let mut rand = LegacyRand::from_seed(1234);
    OctaveSimplexNoiseSampler::new(&mut rand, &[0])
});

/// 用于冻洋生物群系调整的全局噪声采样器。
pub static FROZEN_OCEAN_NOISE: LazyLock<OctaveSimplexNoiseSampler> = LazyLock::new(|| {
    let mut rand = LegacyRand::from_seed(3456);
    OctaveSimplexNoiseSampler::new(&mut rand, &[-2, -1, 0])
});

/// 用于基于植被温度调整的全局噪声采样器。
pub static FOLIAGE_NOISE: LazyLock<OctaveSimplexNoiseSampler> = LazyLock::new(|| {
    let mut rand = LegacyRand::from_seed(2345);
    OctaveSimplexNoiseSampler::new(&mut rand, &[0])
});

/// 调整生物群系基础温度的修改器。
#[derive(Clone, Deserialize, Copy, Hash, PartialEq, Eq, Debug)]
#[serde(rename_all = "UPPERCASE")]
pub enum TemperatureModifier {
    /// 不修改温度。
    None,
    /// 冰冻生物群系调整，通常用于冰冻海洋和积雪区域。
    Frozen,
}

impl TemperatureModifier {
    /// 对给定的位置和基础温度应用温度修改器。
    ///
    /// # Parameters
    /// - `x`：世界中的 X 坐标。
    /// - `z`：世界中的 Z 坐标。
    /// - `temperature`：生物群系的基础温度。
    ///
    /// # Returns
    /// - 以 `f32` 表示的修改后的温度。
    pub fn convert_temperature(&self, x: f64, z: f64, temperature: f32) -> f32 {
        match self {
            Self::None => temperature,
            Self::Frozen => {
                let frozen_ocean_sample =
                    FROZEN_OCEAN_NOISE.sample(x * 0.05, z * 0.05, false) * 7.0;
                let foliage_sample = FOLIAGE_NOISE.sample(x * 0.2, z * 0.2, false);

                let threshold = frozen_ocean_sample + foliage_sample;
                if threshold < 0.3 {
                    let foliage_sample = FOLIAGE_NOISE.sample(x * 0.09, z * 0.09, false);
                    if foliage_sample < 0.8 {
                        return 0.2f32;
                    }
                }

                temperature
            }
        }
    }
}

/// 表示生物群系的天气信息，包括温度和降水。
#[derive(Clone, Debug)]
pub struct Weather {
    has_precipitation: bool,
    /// 生物群系的基础温度。
    temperature: f32,
    /// 影响基础温度的修改器。
    temperature_modifier: TemperatureModifier,
    /// 该生物群系的降雨或降雪强度。
    #[expect(dead_code)]
    downfall: f32,
}

impl Weather {
    /// 创建新的 `Weather` 实例。
    ///
    /// # Parameters
    /// - `has_precipitation`：该生物群系是否有降水。
    /// - `temperature`：生物群系的基础温度。
    /// - `temperature_modifier`：影响温度的修饰器。
    /// - `downfall`：降雨或降雪量。
    #[must_use]
    pub const fn new(
        has_precipitation: bool,
        temperature: f32,
        temperature_modifier: TemperatureModifier,
        downfall: f32,
    ) -> Self {
        Self {
            has_precipitation,
            temperature,
            temperature_modifier,
            downfall,
        }
    }

    /// 返回生物群系的原始基础温度，不含任何高度或修正调整。
    #[must_use]
    pub const fn base_temperature(&self) -> f32 {
        self.temperature
    }

    /// 计算给定位置的有效温度。
    ///
    /// # Parameters
    /// - `x`、`z`：世界坐标。
    /// - `y`：该位置的 Y 层高度。
    /// - `sea_level`：世界中的海平面。
    ///
    /// # Returns
    /// - 以 `f32` 表示的该位置的温度。
    ///
    /// # Notes
    /// - 此函数计算开销较大，应当缓存。
    /// - 温度受 `TemperatureModifier` 和噪声采样器影响。
    pub fn compute_temperature(&self, x: f64, y: i32, z: f64, sea_level: i32) -> f32 {
        let modified_temperature =
            self.temperature_modifier
                .convert_temperature(x, z, self.temperature);
        let offset_sea_level = sea_level + 17;

        if y > offset_sea_level {
            let temperature_noise =
                (TEMPERATURE_NOISE.sample(x / 8.0, z / 8.0, false) * 8.0) as f32;

            modified_temperature
                - (temperature_noise + y as f32 - offset_sea_level as f32) * 0.05f32 / 40.0f32
        } else {
            modified_temperature
        }
    }

    #[must_use]
    pub fn warm_enough_to_rain(&self, x: i32, y: i32, z: i32, sea_level: i32) -> bool {
        self.compute_temperature(f64::from(x), y, f64::from(z), sea_level) >= 0.15
    }

    #[must_use]
    pub fn is_rain_at(&self, x: i32, y: i32, z: i32, sea_level: i32) -> bool {
        self.has_precipitation && self.warm_enough_to_rain(x, y, z, sea_level)
    }
}

#[cfg(test)]
mod tests {
    use super::{TemperatureModifier, Weather};

    #[test]
    fn precipitation_controls_rain() {
        let weather = Weather::new(false, 1.0, TemperatureModifier::None, 0.0);
        assert!(!weather.is_rain_at(0, 64, 0, 63));
    }

    #[test]
    fn warm_precipitation_is_rain() {
        let weather = Weather::new(true, 1.0, TemperatureModifier::None, 0.0);
        assert!(weather.is_rain_at(0, 64, 0, 63));
    }

    #[test]
    fn cold_precipitation_is_snow() {
        let weather = Weather::new(true, 0.0, TemperatureModifier::None, 0.0);
        assert!(!weather.is_rain_at(0, 64, 0, 63));
    }
}
