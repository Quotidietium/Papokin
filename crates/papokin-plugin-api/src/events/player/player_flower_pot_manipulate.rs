use crate::wit::papokin::plugin::event::{Event, EventType, PlayerFlowerPotManipulateEventData};

use super::super::FromIntoEvent;

/// 玩家操作花盆内容物时触发的事件（在
/// 花盆。此事件可取消。
pub struct PlayerFlowerPotManipulateEvent;
impl FromIntoEvent for PlayerFlowerPotManipulateEvent {
    const EVENT_TYPE: EventType = EventType::PlayerFlowerPotManipulateEvent;
    type Data = PlayerFlowerPotManipulateEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerFlowerPotManipulateEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerFlowerPotManipulateEvent(data)
    }
}
