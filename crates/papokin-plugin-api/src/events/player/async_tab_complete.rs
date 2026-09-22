use crate::wit::papokin::plugin::event::{AsyncTabCompleteEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 计算 tab 补全时异步触发的事件。
/// 此事件可取消；补全内容可以被修改。
pub struct AsyncTabCompleteEvent;
impl FromIntoEvent for AsyncTabCompleteEvent {
    const EVENT_TYPE: EventType = EventType::AsyncTabCompleteEvent;
    type Data = AsyncTabCompleteEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::AsyncTabCompleteEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::AsyncTabCompleteEvent(data)
    }
}
