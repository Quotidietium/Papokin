use crate::wit::papokin::plugin::event::{EntityPathfindEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 实体开始朝目标寻路时触发的事件。
pub struct EntityPathfindEvent;
impl FromIntoEvent for EntityPathfindEvent {
    const EVENT_TYPE: EventType = EventType::EntityPathfindEvent;
    type Data = EntityPathfindEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::EntityPathfindEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::EntityPathfindEvent(data)
    }
}
