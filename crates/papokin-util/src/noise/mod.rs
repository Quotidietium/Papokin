pub mod perlin;
pub mod simplex;
pub mod volume;

/// 用于噪声计算的三维梯度向量。
pub struct Gradient {
    /// 梯度向量的 X 分量。
    x: f64,
    /// 梯度向量的 Y 分量。
    y: f64,
    /// 梯度向量的 Z 分量。
    z: f64,
}

/// 一组预先计算的 16 个梯度向量，用于 3D 噪声生成。
pub const GRADIENTS: [Gradient; 16] = [
    Gradient {
        x: 1f64,
        y: 1f64,
        z: 0f64,
    },
    Gradient {
        x: -1f64,
        y: 1f64,
        z: 0f64,
    },
    Gradient {
        x: 1f64,
        y: -1f64,
        z: 0f64,
    },
    Gradient {
        x: -1f64,
        y: -1f64,
        z: 0f64,
    },
    Gradient {
        x: 1f64,
        y: 0f64,
        z: 1f64,
    },
    Gradient {
        x: -1f64,
        y: 0f64,
        z: 1f64,
    },
    Gradient {
        x: 1f64,
        y: 0f64,
        z: -1f64,
    },
    Gradient {
        x: -1f64,
        y: 0f64,
        z: -1f64,
    },
    Gradient {
        x: 0f64,
        y: 1f64,
        z: 1f64,
    },
    Gradient {
        x: 0f64,
        y: -1f64,
        z: 1f64,
    },
    Gradient {
        x: 0f64,
        y: 1f64,
        z: -1f64,
    },
    Gradient {
        x: 0f64,
        y: -1f64,
        z: -1f64,
    },
    Gradient {
        x: 1f64,
        y: 1f64,
        z: 0f64,
    },
    Gradient {
        x: 0f64,
        y: -1f64,
        z: 1f64,
    },
    Gradient {
        x: -1f64,
        y: 1f64,
        z: 0f64,
    },
    Gradient {
        x: 0f64,
        y: -1f64,
        z: -1f64,
    },
];

impl Gradient {
    /// 计算此梯度向量与给定坐标的点积。
    ///
    /// # Arguments
    /// - `x` – 用于点乘的 X 坐标。
    /// - `y` – 用于点乘的 Y 坐标。
    /// - `z` – 用于点乘的 Z 坐标。
    ///
    /// # Returns
    /// 点积 `self.x * x + self.y * y + self.z * z`。
    #[inline]
    #[must_use]
    pub const fn dot(&self, x: f64, y: f64, z: f64) -> f64 {
        // 在没有 target-feature=+fma 的情况下使用 mul_add 会带来巨大的性能损失
        // 因为它会为每个 Perlin 采样降级为 16 次 libm 调用。
        //
        // 这带来了 15% 左右的惊人性能提升
        self.x * x + self.y * y + self.z * z
    }
}
