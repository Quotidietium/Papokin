use super::super::FromIntoEvent;
use crate::wit::papokin::plugin::event::{Event, EventType, TradeSelectEventData};

/// 玩家选择商人交易选项时触发的事件。
pub struct TradeSelectEvent;
impl FromIntoEvent for TradeSelectEvent {
    const EVENT_TYPE: EventType = EventType::TradeSelectEvent;
    type Data = TradeSelectEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::TradeSelectEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::TradeSelectEvent(data)
    }
}
