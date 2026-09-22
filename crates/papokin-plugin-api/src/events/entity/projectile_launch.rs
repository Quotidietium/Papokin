use crate::wit::papokin::plugin::event::{Event, EventType, ProjectileLaunchEventData};

use super::super::FromIntoEvent;

/// 弹射物发射时触发的事件。
pub struct ProjectileLaunchEvent;
impl FromIntoEvent for ProjectileLaunchEvent {
    const EVENT_TYPE: EventType = EventType::ProjectileLaunchEvent;
    type Data = ProjectileLaunchEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::ProjectileLaunchEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::ProjectileLaunchEvent(data)
    }
}
