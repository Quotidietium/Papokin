//! 供目标（goal）使用的辅助函数，用于选择导航可到达的随机目的地。

pub mod air_and_water_random_pos;
pub mod default_random_pos;
pub mod goal_utils;
pub mod hover_random_pos;
pub mod land_random_pos;
pub mod random_pos;

/// 各目标在 [`rand::RngExt`] 之外所需的额外采样。
pub trait RandomExt: rand::RngExt {
    /// 以 `center` 为中心的三角分布。
    fn triangle(&mut self, center: f64, spread: f64) -> f64 {
        spread.mul_add(self.random::<f64>() - self.random::<f64>(), center)
    }
}

impl<T: rand::RngExt + ?Sized> RandomExt for T {}
