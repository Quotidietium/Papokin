use rustc_hash::FxHashSet;

use crate::entity::Entity;

/// 每刻缓存生物进行的视线检查，这样多个目标询问
/// 同一目标的调用只需支付一次射线检测的开销。
#[derive(Default)]
pub struct Sensing {
    seen: FxHashSet<i32>,
    unseen: FxHashSet<i32>,
}

impl Sensing {
    pub fn tick(&mut self) {
        self.seen.clear();
        self.unseen.clear();
    }

    pub fn has_line_of_sight(&mut self, mob: &Entity, target: &Entity) -> bool {
        let target_id = target.entity_id;
        if self.seen.contains(&target_id) {
            return true;
        }
        if self.unseen.contains(&target_id) {
            return false;
        }

        let has_line_of_sight = mob.has_line_of_sight(target);
        if has_line_of_sight {
            self.seen.insert(target_id);
        } else {
            self.unseen.insert(target_id);
        }
        has_line_of_sight
    }
}
