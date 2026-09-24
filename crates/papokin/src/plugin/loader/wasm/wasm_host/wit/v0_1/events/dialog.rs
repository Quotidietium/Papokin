use bytes::Bytes;
use papokin_protocol::java::client::dialog::{
    ActionButton as ProtocolActionButton, Dialog as ProtocolDialog, DialogAction,
    DialogBody as ProtocolDialogBody, DialogInput as ProtocolDialogInput, DialogLink,
};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::plugin::api::events::dialog::{
    DialogClearEvent, DialogClickActionEvent, DialogShowEvent,
};
use crate::plugin::loader::wasm::wasm_host::{
    state::PluginHostState,
    wit::v0_1::{
        events::{ToFromWasmEvent, consume_player},
        papokin::plugin::{
            event::{DialogClearEventData, DialogClickActionEventData, DialogShowEventData, Event},
            java_dialogs::{
                Action, ActionButton, AfterAction, CustomClickAction, Dialog, DialogBody,
                DialogInput, DialogInputBool, DialogInputNumberRange, DialogInputSingleOption,
                DialogInputText, DialogType, Link, LinkLabel, LinkType,
            },
        },
        player::text_component_from_resource,
    },
};

/// 事件回读路径的容错转换：句柄无效时记录错误并以空文本占位，
/// 避免恶意/异常的插件句柄让 panic 跨越宿主调用边界中断事件分发。
fn text_or_empty(
    state: &PluginHostState,
    res: &wasmtime::component::Resource<
        crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::text::TextComponent,
    >,
) -> papokin_util::text::TextComponent {
    match text_component_from_resource(state, res) {
        Ok(component) => component,
        Err(error) => {
            tracing::error!("对话框文本资源句柄无效，已用空文本占位：{error}");
            papokin_util::text::TextComponent::text("")
        }
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) fn protocol_dialog_from_wasm(
    state: &PluginHostState,
    dialog: &Dialog,
) -> ProtocolDialog {
    let title = text_or_empty(state, &dialog.title);

    let body: Vec<_> = dialog
        .body
        .iter()
        .map(|b| match b {
            DialogBody::PlainMessage(c) => ProtocolDialogBody::PlainMessage {
                contents: text_or_empty(state, c),
            },
            DialogBody::Item(_i) => ProtocolDialogBody::Item { item: 0 },
        })
        .collect();

    let inputs: Vec<_> = dialog
        .inputs
        .iter()
        .map(|i| match i {
            DialogInput::Bool(b) => ProtocolDialogInput::Boolean {
                label: text_or_empty(state, &b.label),
                default_value: b.default_value,
            },
            DialogInput::Text(t) => ProtocolDialogInput::Text {
                label: text_or_empty(state, &t.label),
                placeholder: text_or_empty(state, &t.placeholder),
                default_value: t.default_value.clone(),
            },
            DialogInput::NumberRange(n) => ProtocolDialogInput::NumberRange {
                label: text_or_empty(state, &n.label),
                min: n.min_value,
                max: n.max_value,
                initial: n.initial_value,
                step: n.step,
                label_format: n.label_format.clone(),
            },
            DialogInput::SingleOption(s) => ProtocolDialogInput::SingleOption {
                label: text_or_empty(state, &s.label),
                options: s.options.iter().map(|o| text_or_empty(state, o)).collect(),
                initial_index: s.initial_index,
            },
        })
        .collect();

    let buttons: Vec<_> = dialog
        .buttons
        .iter()
        .map(|b| ProtocolActionButton {
            text: text_or_empty(state, &b.text),
            tooltip: b.tooltip.as_ref().map(|t| text_or_empty(state, t)),
            width: b.width,
            action: match &b.action {
                Action::OpenUrl(u) => DialogAction::OpenUrl { url: u.clone() },
                Action::CustomClick(c) => DialogAction::Custom {
                    id: c.id.clone(),
                    payload: c.payload.clone(),
                },
            },
        })
        .collect();

    let links: Vec<_> = dialog
        .links
        .iter()
        .map(|l| {
            let label = match &l.label {
                LinkLabel::BuiltIn(t) => {
                    let link_type = match t {
                        LinkType::BugReport => papokin_protocol::LinkType::BugReport,
                        LinkType::CommunityGuidelines => {
                            papokin_protocol::LinkType::CommunityGuidelines
                        }
                        LinkType::Support => papokin_protocol::LinkType::Support,
                        LinkType::Status => papokin_protocol::LinkType::Status,
                        LinkType::Feedback => papokin_protocol::LinkType::Feedback,
                        LinkType::Community => papokin_protocol::LinkType::Community,
                        LinkType::Website => papokin_protocol::LinkType::Website,
                        LinkType::Forums => papokin_protocol::LinkType::Forums,
                        LinkType::News => papokin_protocol::LinkType::News,
                        LinkType::Announcements => papokin_protocol::LinkType::Announcements,
                    };
                    papokin_protocol::Label::BuiltIn(link_type)
                }
                LinkLabel::Custom(c) => {
                    papokin_protocol::Label::TextComponent(Box::new(text_or_empty(state, c)))
                }
            };
            DialogLink {
                label,
                url: l.url.clone(),
            }
        })
        .collect();

    ProtocolDialog {
        r#type: match dialog.type_ {
            DialogType::Notice => "minecraft:notice".to_string(),
            DialogType::Confirmation => "minecraft:confirmation".to_string(),
            DialogType::MultiAction => "minecraft:multi_action".to_string(),
            DialogType::DialogList => "minecraft:dialog_list".to_string(),
            DialogType::ServerLinks => "minecraft:server_links".to_string(),
        },
        title,
        body,
        inputs,
        buttons,
        links,
        exit_action: None,
        after_action: dialog.after_action.map(|a| match a {
            AfterAction::Peek => "peek".to_string(),
            AfterAction::Pop => "pop".to_string(),
        }),
        can_close_with_escape: dialog.can_close_with_escape,
        external_title: dialog
            .external_title
            .as_ref()
            .map(|t| text_or_empty(state, t)),
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) fn protocol_dialog_to_wasm(
    state: &mut PluginHostState,
    dialog: &ProtocolDialog,
) -> Dialog {
    let title = state
        .add_text_component(dialog.title.clone())
        .expect("添加文本组件失败");

    let type_ = match dialog.r#type.as_str() {
        "minecraft:confirmation" => DialogType::Confirmation,
        "minecraft:multi_action" => DialogType::MultiAction,
        "minecraft:dialog_list" => DialogType::DialogList,
        "minecraft:server_links" => DialogType::ServerLinks,
        _ => DialogType::Notice,
    };

    let body = dialog
        .body
        .iter()
        .map(|b| match b {
            ProtocolDialogBody::PlainMessage { contents } => {
                let comp = state
                    .add_text_component(contents.clone())
                    .expect("添加文本组件失败");
                DialogBody::PlainMessage(comp)
            }
            ProtocolDialogBody::Item { item: _ } => {
                let item_res = state
                    .add_item_stack(Arc::new(Mutex::new(
                        papokin_data::item_stack::ItemStack::new(0, &papokin_data::item::Item::AIR),
                    )))
                    .expect("添加物品堆资源失败");
                DialogBody::Item(item_res)
            }
        })
        .collect();

    let inputs = dialog
        .inputs
        .iter()
        .map(|i| match i {
            ProtocolDialogInput::Boolean {
                label,
                default_value,
            } => {
                let lbl = state
                    .add_text_component(label.clone())
                    .expect("添加文本组件失败");
                DialogInput::Bool(DialogInputBool {
                    label: lbl,
                    default_value: *default_value,
                })
            }
            ProtocolDialogInput::Text {
                label,
                placeholder,
                default_value,
            } => {
                let lbl = state
                    .add_text_component(label.clone())
                    .expect("添加文本组件失败");
                let ph = state
                    .add_text_component(placeholder.clone())
                    .expect("添加文本组件失败");
                DialogInput::Text(DialogInputText {
                    label: lbl,
                    placeholder: ph,
                    default_value: default_value.clone(),
                })
            }
            ProtocolDialogInput::NumberRange {
                label,
                min,
                max,
                initial,
                step,
                label_format,
            } => {
                let lbl = state
                    .add_text_component(label.clone())
                    .expect("添加文本组件失败");
                DialogInput::NumberRange(DialogInputNumberRange {
                    label: lbl,
                    min_value: *min,
                    max_value: *max,
                    initial_value: *initial,
                    step: *step,
                    label_format: label_format.clone(),
                })
            }
            ProtocolDialogInput::SingleOption {
                label,
                options,
                initial_index,
            } => {
                let lbl = state
                    .add_text_component(label.clone())
                    .expect("添加文本组件失败");
                let opts = options
                    .iter()
                    .map(|o| {
                        state
                            .add_text_component(o.clone())
                            .expect("添加文本组件失败")
                    })
                    .collect();
                DialogInput::SingleOption(DialogInputSingleOption {
                    label: lbl,
                    options: opts,
                    initial_index: *initial_index,
                })
            }
        })
        .collect();

    let buttons = dialog
        .buttons
        .iter()
        .map(|b| {
            let text = state
                .add_text_component(b.text.clone())
                .expect("添加文本组件失败");
            let tooltip = b.tooltip.as_ref().map(|t| {
                state
                    .add_text_component(t.clone())
                    .expect("添加文本组件失败")
            });
            let action = match &b.action {
                DialogAction::OpenUrl { url } => Action::OpenUrl(url.clone()),
                DialogAction::Custom { id, payload } => Action::CustomClick(CustomClickAction {
                    id: id.clone(),
                    payload: payload.clone(),
                }),
            };
            ActionButton {
                text,
                tooltip,
                width: b.width,
                action,
            }
        })
        .collect();

    let links = dialog
        .links
        .iter()
        .map(|l| {
            let label = match &l.label {
                papokin_protocol::Label::BuiltIn(t) => LinkLabel::BuiltIn(match t {
                    papokin_protocol::LinkType::BugReport => LinkType::BugReport,
                    papokin_protocol::LinkType::CommunityGuidelines => {
                        LinkType::CommunityGuidelines
                    }
                    papokin_protocol::LinkType::Support => LinkType::Support,
                    papokin_protocol::LinkType::Status => LinkType::Status,
                    papokin_protocol::LinkType::Feedback => LinkType::Feedback,
                    papokin_protocol::LinkType::Community => LinkType::Community,
                    papokin_protocol::LinkType::Website => LinkType::Website,
                    papokin_protocol::LinkType::Forums => LinkType::Forums,
                    papokin_protocol::LinkType::News => LinkType::News,
                    papokin_protocol::LinkType::Announcements => LinkType::Announcements,
                }),
                papokin_protocol::Label::TextComponent(c) => {
                    let comp = state
                        .add_text_component((**c).clone())
                        .expect("添加文本组件失败");
                    LinkLabel::Custom(comp)
                }
            };
            Link {
                label,
                url: l.url.clone(),
            }
        })
        .collect();

    let external_title = dialog.external_title.as_ref().map(|t| {
        state
            .add_text_component(t.clone())
            .expect("添加文本组件失败")
    });

    Dialog {
        title,
        type_,
        body,
        inputs,
        buttons,
        links,
        after_action: dialog.after_action.as_deref().map(|a| match a {
            "peek" => AfterAction::Peek,
            _ => AfterAction::Pop,
        }),
        can_close_with_escape: dialog.can_close_with_escape,
        external_title,
    }
}

impl ToFromWasmEvent for DialogClickActionEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        Event::DialogClickActionEvent(DialogClickActionEventData {
            player: state
                .add_player(self.player.clone())
                .expect("添加玩家资源失败"),
            id: self.id.clone(),
            payload: self.payload.as_ref().map(|p| p.to_vec()),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::DialogClickActionEvent(data) => Self {
                player: consume_player(state, &data.player),
                id: data.id,
                payload: data.payload.map(Bytes::from),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for DialogClearEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        Event::DialogClearEvent(DialogClearEventData {
            player: state
                .add_player(self.player.clone())
                .expect("添加玩家资源失败"),
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::DialogClearEvent(data) => Self {
                player: consume_player(state, &data.player),
                cancelled: data.cancelled,
            },
            _ => panic!("意外的事件类型"),
        }
    }
}

impl ToFromWasmEvent for DialogShowEvent {
    fn to_wasm_event(&self, state: &mut PluginHostState) -> Event {
        let dialog = protocol_dialog_to_wasm(state, &self.dialog);
        Event::DialogShowEvent(DialogShowEventData {
            player: state
                .add_player(self.player.clone())
                .expect("添加玩家资源失败"),
            dialog,
            cancelled: self.cancelled,
        })
    }

    fn from_wasm_event(event: Event, state: &mut PluginHostState) -> Self {
        match event {
            Event::DialogShowEvent(data) => {
                let dialog = protocol_dialog_from_wasm(state, &data.dialog);
                Self {
                    player: consume_player(state, &data.player),
                    dialog,
                    cancelled: data.cancelled,
                }
            }
            _ => panic!("意外的事件类型"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_dialog_text_handle_falls_back_to_empty() {
        let mut state = PluginHostState::new();
        let valid = state
            .add_text_component::<crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::text::TextComponent>(
                papokin_util::text::TextComponent::text("标题"),
            )
            .expect("添加文本组件资源失败");
        assert_eq!(text_or_empty(&state, &valid).get_text(), "标题");

        // 伪造的未注册句柄：容错为空文本而非 panic 跨越宿主边界
        let forged = wasmtime::component::Resource::<
            crate::plugin::loader::wasm::wasm_host::wit::v0_1::papokin::plugin::text::TextComponent,
        >::new_own(u32::MAX - 1);
        assert_eq!(text_or_empty(&state, &forged).get_text(), "");
    }
}
