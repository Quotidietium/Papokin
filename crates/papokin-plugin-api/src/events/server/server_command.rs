use crate::wit::papokin::plugin::event::{Event, EventType, ServerCommandEventData};

use super::super::FromIntoEvent;

/// 从服务器控制台执行命令时触发的事件。
///
/// 关联的 [`ServerCommandEventData`] 包含命令字符串。
/// 此事件可取消。
pub struct ServerCommandEvent;
impl FromIntoEvent for ServerCommandEvent {
    const EVENT_TYPE: EventType = EventType::ServerCommandEvent;
    type Data = ServerCommandEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ServerCommandEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ServerCommandEvent(data)
    }
}
