use crate::wit::papokin::plugin::event::{CommandRegisteredEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 命令注册到服务器时触发的事件。
pub struct CommandRegisteredEvent;
impl FromIntoEvent for CommandRegisteredEvent {
    const EVENT_TYPE: EventType = EventType::CommandRegisteredEvent;
    type Data = CommandRegisteredEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::CommandRegisteredEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::CommandRegisteredEvent(data)
    }
}
