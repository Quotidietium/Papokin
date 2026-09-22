use std::any::Any;

use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use papokin_data::item::Item;
use papokin_data::sound::{Sound, SoundCategory};
use papokin_protocol::codec::var_int::VarInt;
use papokin_protocol::java::client::play::COpenBook;

pub struct WritableBookItem;

impl ItemMetadata for WritableBookItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::WRITABLE_BOOK.id, Item::WRITTEN_BOOK.id])
    }
}

impl ItemBehaviour for WritableBookItem {
    fn normal_use(&self, _item: &Item, player: &Player) {
        player.try_send_client_packet(&COpenBook::new(VarInt(0)));
        player.world().play_sound(
            Sound::ItemBookPageTurn,
            SoundCategory::Players,
            &player.position(),
        );
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
