use super::RandomImpl;

/// 一个扩展了 `RandomImpl` 并具备高斯（正态）分布生成能力的 trait。
pub trait GaussianGenerator: RandomImpl {
    /// 返回之前计算存储的高斯值（如有）。
    ///
    /// # Returns
    /// 上次存储的高斯值；如果未存储任何值，则为 `None`。
    fn stored_next_gaussian(&self) -> Option<f64>;

    /// 设置供下次调用使用的已存高斯值。
    ///
    /// # Arguments
    /// - `value` – 要存储的高斯值；传 `None` 则清除存储。
    fn set_stored_next_gaussian(&mut self, value: Option<f64>);

    /// Generates the next Gaussian-distributed random value.
    ///
    /// # Returns
    /// 来自标准高斯（正态）分布的随机值。
    fn calculate_gaussian(&mut self) -> f64 {
        if let Some(gaussian) = self.stored_next_gaussian() {
            self.set_stored_next_gaussian(None);
            gaussian
        } else {
            loop {
                let d = self.next_f64().mul_add(2.0, -1.0);
                let e = self.next_f64().mul_add(2.0, -1.0);
                let f = d * d + e * e;

                if f < 1f64 && f != 0f64 {
                    let g = (-2f64 * f.ln() / f).sqrt();
                    self.set_stored_next_gaussian(Some(e * g));
                    return d * g;
                }
            }
        }
    }
}
