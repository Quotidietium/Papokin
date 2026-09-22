use std::{borrow::Cow, vec};

use serde::{Deserialize, Serialize};

use super::{TextComponent, TextComponentBase};

/// 表示聊天组件中的悬浮事件动作。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum HoverEvent {
    /// 显示带有给定文本的工具提示。
    ShowText { value: Vec<TextComponentBase> },
    /// 显示一个物品。
    ShowItem {
        /// 物品的资源标识符。
        id: Cow<'static, str>,
        /// 物品堆中的物品数量。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        count: Option<i32>,
        // #[serde(default, skip_serializing_if = "Option::is_none")]
        // components: Option<Cow<'static, str>>,
    },
    /// 显示一个实体。
    ShowEntity {
        /// 实体的 ID 与实体类型。
        id: Cow<'static, str>,
        /// 实体的 UUID
        /// UUID 不能使用 `uuid::Uuid`，因为其序列化会把它解析成字节，导致字节被重复序列化。
        uuid: Cow<'static, str>,
        /// 实体的可选自定义名称。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<Vec<TextComponentBase>>,
    },
}

impl HoverEvent {
    /// 创建显示文本的新悬停事件。
    ///
    /// # Arguments
    /// - `text` – 要在工具提示中显示的文本组件。
    ///
    /// # Returns
    /// 一个包含所提供文本的 `HoverEvent::ShowText` 变体。
    #[must_use]
    pub fn show_text(text: TextComponent) -> Self {
        Self::ShowText {
            value: vec![text.0],
        }
    }

    /// 创建显示实体信息的新悬停事件。
    ///
    /// # Arguments
    /// - `uuid` – 实体的 UUID 字符串。
    /// - `kind` – 实体类型标识符（例如 "minecraft:pig"）。
    /// - `name` – 实体的可选自定义名称。
    ///
    /// # Returns
    /// 一个包含实体信息的 `HoverEvent::ShowEntity` 变体。
    pub fn show_entity<P: Into<Cow<'static, str>>>(
        uuid: P,
        kind: P,
        name: Option<TextComponent>,
    ) -> Self {
        Self::ShowEntity {
            id: kind.into(),
            uuid: uuid.into(),
            name: match name {
                Some(name) => Some(vec![name.0]),
                None => None,
            },
        }
    }
}
