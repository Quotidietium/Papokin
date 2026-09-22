use crate::wit::papokin::plugin::event::{
    Event, EventType, PlayerNaturallySpawnCreaturesEventData,
};

use super::super::FromIntoEvent;

/// 生物在玩家周围自然生成时触发的事件。
/// 此事件可取消。
pub struct PlayerNaturallySpawnCreaturesEvent;
impl FromIntoEvent for PlayerNaturallySpawnCreaturesEvent {
    const EVENT_TYPE: EventType = EventType::PlayerNaturallySpawnCreaturesEvent;
    type Data = PlayerNaturallySpawnCreaturesEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerNaturallySpawnCreaturesEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerNaturallySpawnCreaturesEvent(data)
    }
}
