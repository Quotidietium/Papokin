use thiserror::Error;

/// 物品栏操作期间可能发生的错误。
///
/// 这些错误表示处理物品栏时出现的各种失败情况
/// 交互，例如无效的槽位索引、权限问题或协议错误。
#[derive(Error, Debug)]
pub enum InventoryError {
    /// 获取物品栏或槽位的锁失败。
    #[error("Unable to lock")]
    LockError,
    /// 指定的槽位索引无效或越界。
    #[error("Invalid slot")]
    InvalidSlot,
    /// 玩家尝试与已关闭的容器进行交互。
    ///
    /// 该参数为玩家的实体 ID。
    #[error("Player '{0}' tried to interact with a closed container")]
    ClosedContainerInteract(i32),
    /// 多个玩家试图同时在同一容器中拖动物品。
    #[error("Multiple players dragging in a container at once")]
    MultiplePlayersDragging,
    /// 拖动操作顺序不正确（例如在开始之前就结束）。
    #[error("Out of order dragging")]
    OutOfOrderDragging,
    /// 收到的物品栏数据包格式错误或无效。
    #[error("Invalid inventory packet")]
    InvalidPacket,
    /// 玩家缺少执行此物品栏操作的权限。
    #[error("Player does not have enough permissions")]
    PermissionError,
}
