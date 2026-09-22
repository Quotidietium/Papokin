use papokin_codecs::codec::list::validate_fixed_size;
use papokin_codecs::{DataResult, FlatTryFrom, comap_flat_map_codec_impl};
use papokin_nbt::tag::NbtTag;
use serde::{Deserialize, Serialize};

/// 表示使用欧拉角（以度为单位）的三维旋转。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EulerAngle {
    /// 绕 X 轴的旋转角度（单位：度）。
    pub pitch: f32,
    /// 绕 Y 轴的旋转角度（单位：度）。
    pub yaw: f32,
    /// 绕 Z 轴的旋转角度（单位：度）。
    pub roll: f32,
}

impl EulerAngle {
    /// 使用给定的俯仰角、偏航角和翻滚角（单位：度）创建新的 `EulerAngle`。
    ///
    /// 值会被归一化到 [0, 360] 范围。
    ///
    /// # Arguments
    /// * `pitch` – 绕 X 轴的旋转。
    /// * `yaw` – 绕 Y 轴的旋转。
    /// * `roll` – 绕 Z 轴的旋转。
    #[must_use]
    pub fn new(pitch: f32, yaw: f32, roll: f32) -> Self {
        let pitch = pitch % 360.0;
        let yaw = yaw % 360.0;
        let roll = roll % 360.0;

        Self { pitch, yaw, roll }
    }

    /// 一个常量，表示所有轴上的零旋转。
    pub const ZERO: Self = Self {
        pitch: 0.0,
        yaw: 0.0,
        roll: 0.0,
    };
}

impl Default for EulerAngle {
    fn default() -> Self {
        Self::ZERO
    }
}

impl From<EulerAngle> for NbtTag {
    fn from(val: EulerAngle) -> Self {
        Self::List(vec![
            Self::Float(val.pitch),
            Self::Float(val.yaw),
            Self::Float(val.roll),
        ])
    }
}

impl From<NbtTag> for EulerAngle {
    fn from(tag: NbtTag) -> Self {
        if let NbtTag::List(list) = tag
            && list.len() == 3
        {
            let pitch = if let NbtTag::Float(f) = list[0] {
                f
            } else {
                0.0
            };
            let yaw = if let NbtTag::Float(f) = list[1] {
                f
            } else {
                0.0
            };
            let roll = if let NbtTag::Float(f) = list[2] {
                f
            } else {
                0.0
            };

            return Self::new(pitch, yaw, roll);
        }

        Self::ZERO
    }
}

impl From<&EulerAngle> for Vec<f32> {
    fn from(value: &EulerAngle) -> Self {
        let EulerAngle { pitch, yaw, roll } = value;
        vec![*pitch, *yaw, *roll]
    }
}

impl FlatTryFrom<Vec<f32>> for EulerAngle {
    fn flat_try_from(value: Vec<f32>) -> DataResult<Self> {
        validate_fixed_size(value, 3).flat_map(|v| {
            v.try_into().map_or_else(
                |_| DataResult::new_error("Expected 3 elements"),
                |arr| {
                    let [x, y, z]: [f32; 3] = arr;
                    DataResult::new_success(Self {
                        pitch: x,
                        yaw: y,
                        roll: z,
                    })
                },
            )
        })
    }
}

comap_flat_map_codec_impl!(Vec<f32> => EulerAngle, EulerAngle::flat_try_from, Vec::<f32>::from);
