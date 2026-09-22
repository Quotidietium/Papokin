use crate::wit::papokin::plugin::event::{BlockBrushEventData, Event, EventType};

use super::super::FromIntoEvent;

/// 玩家刷扫方块时触发的事件。
pub struct BlockBrushEvent;
impl FromIntoEvent for BlockBrushEvent {
    const EVENT_TYPE: EventType = EventType::BlockBrushEvent;
    type Data = BlockBrushEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::BlockBrushEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::BlockBrushEvent(data)
    }
}
