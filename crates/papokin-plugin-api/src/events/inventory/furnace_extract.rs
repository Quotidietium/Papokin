use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, FurnaceExtractEventData};

/// 玩家从熔炉输出槽取出物品时触发的事件。
pub struct FurnaceExtractEvent;
impl FromIntoEvent for FurnaceExtractEvent {
    const EVENT_TYPE: EventType = EventType::FurnaceExtractEvent;
    type Data = FurnaceExtractEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::FurnaceExtractEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::FurnaceExtractEvent(data)
    }
}
