use crate::wit::papokin::plugin::event::{Event, EventType, PlayerTeleportEndGatewayEventData};

use super::super::FromIntoEvent;

/// 玩家通过末地折跃门传送时触发的事件。此
/// 事件可取消。
pub struct PlayerTeleportEndGatewayEvent;
impl FromIntoEvent for PlayerTeleportEndGatewayEvent {
    const EVENT_TYPE: EventType = EventType::PlayerTeleportEndGatewayEvent;
    type Data = PlayerTeleportEndGatewayEventData;

    fn data_from_event(event: Event) -> Self::Data {
        match event {
            Event::PlayerTeleportEndGatewayEvent(data) => data,
            _ => panic!("非预期的事件"),
        }
    }

    fn data_into_event(data: Self::Data) -> Event {
        Event::PlayerTeleportEndGatewayEvent(data)
    }
}
