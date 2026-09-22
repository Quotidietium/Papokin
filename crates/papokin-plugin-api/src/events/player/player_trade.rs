use crate::wit::papokin::plugin::event::{Event, EventType, PlayerTradeEventData};

use super::super::FromIntoEvent;

/// 玩家与村民或流浪商人交易时触发的事件。
/// 交易者。此事件可取消。
pub struct PlayerTradeEvent;
impl FromIntoEvent for PlayerTradeEvent {
    const EVENT_TYPE: EventType = EventType::PlayerTradeEvent;
    type Data = PlayerTradeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerTradeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerTradeEvent(data)
    }
}
