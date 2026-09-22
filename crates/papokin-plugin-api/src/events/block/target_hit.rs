use crate::wit::papokin::plugin::event::{Event, EventType, TargetHitEventData};

use super::super::FromIntoEvent;

/// 标靶方块被弹射物或实体击中时触发的事件。
pub struct TargetHitEvent;
impl FromIntoEvent for TargetHitEvent {
    const EVENT_TYPE: EventType = EventType::TargetHitEvent;
    type Data = TargetHitEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::TargetHitEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::TargetHitEvent(data)
    }
}
