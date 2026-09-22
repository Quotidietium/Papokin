pub use crate::wit::papokin::plugin::display::{
    BillboardMode, BlockDisplayEntity, DisplayEntity, DisplayTransformation, InteractionEntity,
    ItemDisplayEntity, ItemDisplayMode, Quaternionf, TextAlignment, TextDisplayEntity, Vector3f,
};
use crate::wit::papokin::plugin::item_stack::ItemStack;
use crate::wit::papokin::plugin::text::TextComponent;
use crate::wit::papokin::plugin::world::Entity;

/// 用于构建 [`DisplayTransformation`] 的构建器。
#[derive(Clone, Copy, Debug)]
pub struct TransformationBuilder {
    translation: Vector3f,
    scale: Vector3f,
    left_rotation: Quaternionf,
    right_rotation: Quaternionf,
}

impl Default for TransformationBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TransformationBuilder {
    /// 创建新的恒等 `TransformationBuilder`。
    #[must_use]
    pub const fn new() -> Self {
        Self {
            translation: Vector3f {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            scale: Vector3f {
                x: 1.0,
                y: 1.0,
                z: 1.0,
            },
            left_rotation: Quaternionf {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
            right_rotation: Quaternionf {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            },
        }
    }

    /// 设置平移向量。
    #[must_use]
    pub const fn translation(mut self, x: f32, y: f32, z: f32) -> Self {
        self.translation = Vector3f { x, y, z };
        self
    }

    /// 设置缩放向量。
    #[must_use]
    pub const fn scale(mut self, x: f32, y: f32, z: f32) -> Self {
        self.scale = Vector3f { x, y, z };
        self
    }

    /// 设置所有轴上统一的缩放。
    #[must_use]
    pub const fn uniform_scale(mut self, scale: f32) -> Self {
        self.scale = Vector3f {
            x: scale,
            y: scale,
            z: scale,
        };
        self
    }

    /// 设置左旋转四元数。
    #[must_use]
    pub const fn left_rotation(mut self, x: f32, y: f32, z: f32, w: f32) -> Self {
        self.left_rotation = Quaternionf { x, y, z, w };
        self
    }

    /// 设置右旋转四元数。
    #[must_use]
    pub const fn right_rotation(mut self, x: f32, y: f32, z: f32, w: f32) -> Self {
        self.right_rotation = Quaternionf { x, y, z, w };
        self
    }

    /// 构建 [`DisplayTransformation`]。
    #[must_use]
    pub const fn build(self) -> DisplayTransformation {
        DisplayTransformation {
            translation: self.translation,
            scale: self.scale,
            left_rotation: self.left_rotation,
            right_rotation: self.right_rotation,
        }
    }
}

/// 在通用 [`Entity`] 上向下转换为专用 Display / Interaction 资源的扩展 trait。
pub trait EntityDisplayExt {
    /// 尝试将该实体视为基础 [`DisplayEntity`]。
    fn as_display(&self) -> Option<DisplayEntity>;
    /// 尝试将该实体视为 [`BlockDisplayEntity`]。
    fn as_block_display(&self) -> Option<BlockDisplayEntity>;
    /// 尝试将该实体视为 [`ItemDisplayEntity`]。
    fn as_item_display(&self) -> Option<ItemDisplayEntity>;
    /// 尝试将该实体视为 [`TextDisplayEntity`]。
    fn as_text_display(&self) -> Option<TextDisplayEntity>;
    /// 尝试将该实体视为 [`InteractionEntity`]。
    fn as_interaction(&self) -> Option<InteractionEntity>;
}

impl EntityDisplayExt for Entity {
    fn as_display(&self) -> Option<DisplayEntity> {
        DisplayEntity::from_entity(self)
    }

    fn as_block_display(&self) -> Option<BlockDisplayEntity> {
        BlockDisplayEntity::from_entity(self)
    }

    fn as_item_display(&self) -> Option<ItemDisplayEntity> {
        ItemDisplayEntity::from_entity(self)
    }

    fn as_text_display(&self) -> Option<TextDisplayEntity> {
        TextDisplayEntity::from_entity(self)
    }

    fn as_interaction(&self) -> Option<InteractionEntity> {
        InteractionEntity::from_entity(self)
    }
}

/// 为 [`DisplayEntity`] 提供便捷修改辅助的扩展 trait。
pub trait DisplayEntityExt {
    /// 直接设置平移，无需手动构造 `DisplayTransformation`。
    fn set_translation(&self, x: f32, y: f32, z: f32);
    /// 直接设置缩放，无需手动构造 `DisplayTransformation`。
    fn set_scale(&self, x: f32, y: f32, z: f32);
}

impl DisplayEntityExt for DisplayEntity {
    fn set_translation(&self, x: f32, y: f32, z: f32) {
        let mut transform = self.get_transformation();
        transform.translation = Vector3f { x, y, z };
        self.set_transformation(transform);
    }

    fn set_scale(&self, x: f32, y: f32, z: f32) {
        let mut transform = self.get_transformation();
        transform.scale = Vector3f { x, y, z };
        self.set_transformation(transform);
    }
}

/// 为 [`ItemDisplayEntity`] 提供便捷方法的扩展 trait。
pub trait ItemDisplayEntityExt {
    /// 设置所显示的物品堆。
    fn set_item_stack(&self, item: Option<ItemStack>);
}

impl ItemDisplayEntityExt for ItemDisplayEntity {
    fn set_item_stack(&self, item: Option<ItemStack>) {
        self.set_item(item);
    }
}

/// 为 [`TextDisplayEntity`] 提供便捷方法的扩展 trait。
pub trait TextDisplayEntityExt {
    /// 从纯文本字符串设置文本。
    fn set_plain_text(&self, text: &str);
}

impl TextDisplayEntityExt for TextDisplayEntity {
    fn set_plain_text(&self, text: &str) {
        self.set_text(TextComponent::text(text));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transformation_builder() {
        let transform = TransformationBuilder::new()
            .translation(1.0, 2.0, 3.0)
            .scale(2.0, 2.0, 2.0)
            .left_rotation(0.0, 0.7071, 0.0, 0.7071)
            .build();

        assert_eq!(transform.translation.x, 1.0);
        assert_eq!(transform.translation.y, 2.0);
        assert_eq!(transform.translation.z, 3.0);
        assert_eq!(transform.scale.x, 2.0);
        assert_eq!(transform.scale.y, 2.0);
        assert_eq!(transform.scale.z, 2.0);
        assert_eq!(transform.left_rotation.y, 0.7071);
    }
}
