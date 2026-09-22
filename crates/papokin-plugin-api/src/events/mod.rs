#![allow(clippy::panic)]

use std::{
    collections::BTreeMap,
    marker::PhantomData,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
};

pub use crate::wit::papokin::plugin::event::{
    ClientboundPacket, Event, EventPriority, InteractAction, JavaClientboundPacket,
    JavaServerboundPacket, ServerboundPacket,
};
use crate::{Context, Result, Server, wit::papokin::plugin::event::EventType};

/// 方块事件。
pub mod block;
/// 对话框事件。
pub mod dialog;
/// 附魔事件。
pub mod enchantment;
/// 实体事件。
pub mod entity;
/// 悬挂实体事件。
pub mod hanging;
/// 物品栏事件。
pub mod inventory;
/// 网络数据包事件。
pub mod packet;
/// 玩家事件。
pub mod player;
/// 袭击事件。
pub mod raid;
/// 服务器生命周期事件。
pub mod server;
/// 载具事件。
pub mod vehicle;
/// 世界事件。
pub mod world;

pub use block::*;
pub use dialog::*;
pub use enchantment::*;
pub use entity::*;
pub use hanging::*;
pub use inventory::*;
pub use packet::*;
pub use player::*;
pub use raid::*;
pub use server::*;
pub use vehicle::*;
pub use world::*;

pub(crate) static NEXT_HANDLER_ID: AtomicU32 = AtomicU32::new(0);
pub(crate) static EVENT_HANDLERS: Mutex<BTreeMap<u32, Arc<dyn ErasedEventHandler>>> =
    Mutex::new(BTreeMap::new());

/// 将事件标记类型与其 WIT 生成的数据类型及 [`EventType`] 判别值关联起来。
///
/// 为单元结构体实现此 trait 以定义新事件。[`EventType`] 常量
/// 告诉宿主要订阅哪个事件，而 `data_from_event` 与 `data_into_event`
/// 提供不透明的 [`Event`] 变体与具体数据类型之间的转换。
pub trait FromIntoEvent: Sized {
    /// 宿主用于标识此事件的判别值。
    const EVENT_TYPE: EventType;

    /// 此事件携带的由 WIT 生成的数据记录。
    type Data;

    /// 从 [`Event`] 变体中提取事件数据。
    ///
    /// # Panics
    /// 若 [`Event`] 变体与 [`Self::EVENT_TYPE`] 不匹配则 panic。
    fn data_from_event(event: Event) -> Self::Data;

    /// 将事件数据重新包装为对应的 [`Event`] 变体。
    fn data_into_event(data: Self::Data) -> Event;
}

/// 事件关联数据类型的便捷别名。
pub type EventData<E> = <E as FromIntoEvent>::Data;

/// 固定（pinned）、装箱、动态分发的 future 的类型别名。
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// 特定事件类型的处理器。
///
/// 实现此 trait 以处理事件。`handle` 方法接收服务器
/// 句柄与事件数据，并返回（可能已被修改的）事件数据。
pub trait EventHandler<E: FromIntoEvent> {
    /// 处理事件并返回（可能被修改的）数据。
    fn handle(&self, server: Server, event: E::Data) -> E::Data;
}

pub(crate) trait ErasedEventHandler: Send + Sync {
    fn handle_erased(&self, server: Server, event: Event) -> Event;
}

struct HandlerWrapper<E: FromIntoEvent, H> {
    handler: H,
    _phantom: PhantomData<E>,
}

impl<E: FromIntoEvent + Send + Sync, H: EventHandler<E> + Send + Sync> ErasedEventHandler
    for HandlerWrapper<E, H>
{
    fn handle_erased(&self, server: Server, event: Event) -> Event {
        let data = E::data_from_event(event);
        let result = self.handler.handle(server, data);
        E::data_into_event(result)
    }
}

impl Context {
    /// 向插件注册一个事件处理器。
    ///
    /// 处理器必须实现 [`EventHandler`] trait。
    /// 若事件是阻塞式的，从处理器返回事件将修改该事件。
    pub fn register_event_handler<
        E: FromIntoEvent + Send + Sync + 'static,
        H: EventHandler<E> + Send + Sync + 'static,
    >(
        &self,
        handler: H,
        event_priority: EventPriority,
        blocking: bool,
        ignore_cancelled: bool,
    ) -> Result<u32> {
        let id = NEXT_HANDLER_ID.fetch_add(1, Ordering::Relaxed);
        let wrapped = HandlerWrapper {
            handler,
            _phantom: PhantomData::<E>,
        };
        EVENT_HANDLERS
            .lock()
            .map_err(|e| e.to_string())?
            .insert(id, Arc::new(wrapped));

        self.register_event(
            id,
            E::EVENT_TYPE,
            event_priority,
            blocking,
            ignore_cancelled,
        );
        Ok(id)
    }
}
