use crate::wit::papokin::plugin::event::{Event, EventType, PlayerBedFailEnterEventData};

use super::super::FromIntoEvent;

/// 玩家上床失败时触发的事件。此事件
/// 可取消；取消会抑制失败并让玩家进入
/// 反正仍会睡这张床。
pub struct PlayerBedFailEnterEvent;
impl FromIntoEvent for PlayerBedFailEnterEvent {
    const EVENT_TYPE: EventType = EventType::PlayerBedFailEnterEvent;
    type Data = PlayerBedFailEnterEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerBedFailEnterEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerBedFailEnterEvent(data)
    }
}
