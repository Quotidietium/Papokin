use crate::wit::papokin::plugin::event::{Event, EventType, ServerTickStartEventData};

use super::super::FromIntoEvent;

/// 每个服务器刻开始时触发的事件（在
/// 默认刻速率）。
///
/// 关联的 [`ServerTickStartEventData`] 携带从 0 开始计数的 `tick`
/// 即将运行的刻的编号。此事件不可取消。
pub struct ServerTickStartEvent;
impl FromIntoEvent for ServerTickStartEvent {
    const EVENT_TYPE: EventType = EventType::ServerTickStartEvent;
    type Data = ServerTickStartEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ServerTickStartEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ServerTickStartEvent(data)
    }
}
