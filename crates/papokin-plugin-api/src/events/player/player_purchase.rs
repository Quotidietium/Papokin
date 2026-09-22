use crate::wit::papokin::plugin::event::{Event, EventType, PlayerPurchaseEventData};

use super::super::FromIntoEvent;

/// 玩家与商人完成交易时触发的事件。
/// 此事件可取消。
pub struct PlayerPurchaseEvent;
impl FromIntoEvent for PlayerPurchaseEvent {
    const EVENT_TYPE: EventType = EventType::PlayerPurchaseEvent;
    type Data = PlayerPurchaseEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerPurchaseEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerPurchaseEvent(data)
    }
}
