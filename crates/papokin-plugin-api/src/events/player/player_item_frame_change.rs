use crate::wit::papokin::plugin::event::{Event, EventType, PlayerItemFrameChangeEventData};

use super::super::FromIntoEvent;

/// 玩家在物品展示框中放置、移除或旋转物品时触发的事件（在
/// 一个物品展示框。此事件可取消。
pub struct PlayerItemFrameChangeEvent;
impl FromIntoEvent for PlayerItemFrameChangeEvent {
    const EVENT_TYPE: EventType = EventType::PlayerItemFrameChangeEvent;
    type Data = PlayerItemFrameChangeEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerItemFrameChangeEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerItemFrameChangeEvent(data)
    }
}
