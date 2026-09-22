use crate::wit::papokin::plugin::event::{EntityEnterLoveModeEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体进入求偶模式时触发的事件。
pub struct EntityEnterLoveModeEvent;
impl FromIntoEvent for EntityEnterLoveModeEvent {
    const EVENT_TYPE: EventType = EventType::EntityEnterLoveModeEvent;
    type Data = EntityEnterLoveModeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityEnterLoveModeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityEnterLoveModeEvent(data)
    }
}
