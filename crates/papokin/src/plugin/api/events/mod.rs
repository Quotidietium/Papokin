use std::any::Any;
use std::sync::Arc;

pub mod block;
pub mod dialog;
pub mod enchantment;
pub mod entity;
pub mod hanging;
pub mod inventory;
pub mod player;
pub mod raid;
pub mod server;
pub mod vehicle;
pub mod world;

/// 表示系统中一个事件的 trait。
///
/// 此 trait 提供获取事件名称和进行类型安全向下转换的方法。
pub trait Payload: Send + Sync {
    /// 返回事件类型的静态名称。
    ///
    /// # Returns
    /// 一个静态字符串切片，表示该负载类型的名称。
    fn get_name_static() -> &'static str
    where
        Self: Sized;

    /// 返回此负载实例的名称。
    ///
    /// # Returns
    /// 一个静态字符串切片，表示该负载实例的名称。
    fn get_name(&self) -> &'static str;

    /// 提供以 trait 对象形式表示的负载的不可变引用。
    ///
    /// 此方法允许对负载进行类型安全的向下转换。
    ///
    /// # Returns
    /// 以 `dyn Any` trait 对象形式对负载的不可变引用。
    fn as_any(&self) -> &dyn Any;

    /// 提供以 trait 对象形式表示的负载的可变引用。
    ///
    /// 此方法允许对负载进行类型安全的向下转换。
    ///
    /// # Returns
    /// 一个以 `dyn Any` trait 对象形式表示的负载可变引用。
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// 报告此事件的取消状态（若其可取消）。
    ///
    ///当事件带有 `cancelled` 标志且已被取消时，返回 `Some(true)`
    /// 当前已被取消，`Some(false)` 则表示带有该标志但尚未
    /// 被取消时给出，对不可取消的事件则给出 `None`。调度器
    /// 用它来跳过声明不处理已取消事件的处理函数
    /// （Bukkit 的 `ignoreCancelled` 语义）。
    fn cancelled_state(&self) -> Option<bool> {
        None
    }
}

/// 用于 Payload 实现安全向下转换的辅助函数。
impl dyn Payload + '_ {
    /// 尝试通过基于名称的类型检查，将 `Arc<dyn Payload>` 向下转换为 `Arc<T>`。
    ///
    /// 此方法可安全地跨编译边界使用，因为它使用基于字符串的
    /// 类型识别，而不是 `TypeId`。
    ///
    /// # Type Parameters
    /// - `T`：要向下转型到的目标类型。必须实现 Payload。
    ///
    /// # Arguments
    /// - `payload`：要向下转型的 `Arc<dyn Payload>`。
    ///
    /// # Returns
    /// 向下转型成功时返回 `Some(Arc<T>)`，否则返回 `None`。
    pub fn downcast_arc<T: Payload + 'static>(payload: Arc<dyn Payload>) -> Option<Arc<T>> {
        if payload.get_name() == T::get_name_static() {
            // SAFETY: 类型名与 `T::get_name_static()` 匹配，保证底层类型为 `T`。
            unsafe {
                let raw = Arc::into_raw(payload);
                let typed = raw.cast::<T>();
                Some(Arc::from_raw(typed))
            }
        } else {
            None
        }
    }

    /// 尝试通过基于名称的类型检查，将 &mut dyn Payload 向下转换为 &mut T。
    ///
    /// # Type Parameters
    /// - `T`：要向下转型到的目标类型。必须实现 Payload。
    ///
    /// # Returns
    /// 若向下转型成功则为 Some(&mut T)，否则为 None。
    pub fn downcast_mut<T: Payload + 'static>(&mut self) -> Option<&mut T> {
        if self.get_name() == T::get_name_static() {
            // SAFETY: 类型名与 `T::get_name_static()` 匹配，保证底层类型为 `T`。
            unsafe { Some(&mut *(std::ptr::from_mut::<dyn Payload>(self).cast::<T>())) }
        } else {
            None
        }
    }

    /// 尝试通过基于名称的类型检查，将 &dyn Payload 向下转换为 &T。
    ///
    /// # Type Parameters
    /// - `T`：要向下转型到的目标类型。必须实现 Payload。
    ///
    /// # Returns
    /// 若向下转型成功则为 Some(&T)，否则为 None。
    pub fn downcast_ref<T: Payload + 'static>(&self) -> Option<&T> {
        if self.get_name() == T::get_name_static() {
            // SAFETY: 类型名与 `T::get_name_static()` 匹配，保证底层类型为 `T`。
            unsafe { Some(&*(std::ptr::from_ref::<dyn Payload>(self).cast::<T>())) }
        } else {
            None
        }
    }
}

/// 用于可取消事件的 trait。
///
/// 此 trait 提供检查和设置事件取消状态的方法。
pub trait Cancellable: Send + Sync {
    /// 检查事件是否已被取消。
    ///
    /// # Returns
    /// 一个布尔值，表示事件是否被取消。
    fn cancelled(&self) -> bool;

    /// 设置事件的取消状态。
    ///
    /// # Arguments
    /// - `cancelled`：指示新取消状态的布尔值。
    fn set_cancelled(&mut self, cancelled: bool);
}
/// 表示事件优先级等级的枚举。
///
/// 以较低优先级注册的处理器在分发时优先被调用：
/// `Lowest` 先于 `Low` 运行，`Highest` 最后运行，从而让高优先级的
/// 处理程序的修改会覆盖先前处理程序所做的更改。共享同一
/// 相同优先级的按注册顺序运行（与 Bukkit 兼容的排序）。
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone, Copy)]
pub enum EventPriority {
    /// 最高优先级。
    Highest,

    /// 高优先级。
    High,

    /// 普通优先级。
    Normal,

    /// 低优先级。
    Low,

    /// 最低优先级。
    Lowest,
}
