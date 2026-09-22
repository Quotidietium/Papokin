use crate::wit::papokin::plugin::event::{Event, EventType, PlayerPickupExperienceEventData};

use super::super::FromIntoEvent;

/// 玩家拾取经验球时触发的事件。此事件
/// 此事件可取消；数量可以被修改。
pub struct PlayerPickupExperienceEvent;
impl FromIntoEvent for PlayerPickupExperienceEvent {
    const EVENT_TYPE: EventType = EventType::PlayerPickupExperienceEvent;
    type Data = PlayerPickupExperienceEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerPickupExperienceEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerPickupExperienceEvent(data)
    }
}
