use std::borrow::Cow;

use serde::{Deserialize, Serialize};

/// 点击文本时要采取的动作。
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Eq, Hash)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ClickEvent {
    /// 打开一个 URL。
    OpenUrl { url: Cow<'static, str> },
    /// 打开一个文件。
    OpenFile { path: Cow<'static, str> },
    /// 在告示牌中生效，但仅作用于根文本组件。
    RunCommand { command: Cow<'static, str> },
    /// 用该文本替换聊天框的内容，不一定是
    /// 命令。
    SuggestCommand { command: Cow<'static, str> },
    /// 仅可在成书中使用。更改书的页面。索引
    /// 从 1 开始。
    ChangePage { page: u32 },
    /// 将给定文本复制到系统剪贴板。
    CopyToClipboard { value: Cow<'static, str> },
}
