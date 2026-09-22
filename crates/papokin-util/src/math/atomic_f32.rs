use std::sync::atomic::{AtomicU32, Ordering};

/// `f32` 值的线程安全原子包装器。
pub struct AtomicF32 {
    /// 以 `u32` 位形式表示的浮点数底层原子存储。
    storage: AtomicU32,
}

impl AtomicF32 {
    /// 创建以 `value` 初始化的新 `AtomicF32`。
    ///
    /// # Arguments
    /// * `value` – 初始浮点值。
    ///
    /// # Returns
    /// 一个新的 `AtomicF32` 实例。
    #[must_use]
    pub const fn new(value: f32) -> Self {
        let as_u32 = value.to_bits();
        Self {
            storage: AtomicU32::new(as_u32),
        }
    }

    /// 将新值存入原子浮点数。
    ///
    /// # Arguments
    /// * `value` – 要存入的新浮点值。
    /// * `ordering` – 存储操作的内存序。
    pub fn store(&self, value: f32, ordering: Ordering) {
        let as_u32 = value.to_bits();
        self.storage.store(as_u32, ordering);
    }

    /// 加载原子浮点数的当前值。
    ///
    /// # Arguments
    /// * `ordering` – 加载操作的内存序。
    ///
    /// # Returns
    /// 当前的 `f32` 值。
    pub fn load(&self, ordering: Ordering) -> f32 {
        let as_u32 = self.storage.load(ordering);
        f32::from_bits(as_u32)
    }

    /// 对原子浮点数执行比较并交换操作。
    ///
    /// # Arguments
    /// * `current` – 期望当前存储的值。
    /// * `new` – 当 `current` 与存储值匹配时要存入的值。
    /// * `success` – 成功时使用的内存序。
    /// * `failure` – 失败时使用的内存序。
    ///
    /// # Returns
    /// 交换成功时返回包含先前值的 `Ok(f32)`，
    /// 交换失败时返回包含当前值的 `Err(f32)`。
    pub fn compare_exchange(
        &self,
        current: f32,
        new: f32,
        success: Ordering,
        failure: Ordering,
    ) -> Result<f32, f32> {
        let current_bits = current.to_bits();
        let new_bits = new.to_bits();
        self.storage
            .compare_exchange(current_bits, new_bits, success, failure)
            .map(f32::from_bits)
            .map_err(f32::from_bits)
    }
}
