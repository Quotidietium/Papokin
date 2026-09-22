use crate::wit::papokin::plugin::event::{Event, EventType, ServerTickEndEventData};

use super::super::FromIntoEvent;

/// 每个服务器刻结束时触发的事件。
///
/// 关联的 [`ServerTickEndEventData`] 携带：
/// - `tick`：刚结束的刻的编号（从 0 开始）。
/// - `duration_nanos`：本刻耗时，从
///   本刻迭代开始量到 `Server::tick` 返回之后。
///
/// 此事件不可取消。
pub struct ServerTickEndEvent;
impl FromIntoEvent for ServerTickEndEvent {
    const EVENT_TYPE: EventType = EventType::ServerTickEndEvent;
    type Data = ServerTickEndEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ServerTickEndEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ServerTickEndEvent(data)
    }
}
