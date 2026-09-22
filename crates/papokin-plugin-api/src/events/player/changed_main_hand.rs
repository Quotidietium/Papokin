use crate::wit::papokin::plugin::event::{Event, EventType, PlayerChangedMainHandEventData};

use super::super::FromIntoEvent;

/// 玩家在设置中更改主手时触发的事件。
///
/// 关联的 [`PlayerChangedMainHandEventData`] 包含玩家及其
/// 新选定的主手。
pub struct PlayerChangedMainHandEvent;
impl FromIntoEvent for PlayerChangedMainHandEvent {
    const EVENT_TYPE: EventType = EventType::PlayerChangedMainHandEvent;
    type Data = PlayerChangedMainHandEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerChangedMainHandEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerChangedMainHandEvent(data)
    }
}
