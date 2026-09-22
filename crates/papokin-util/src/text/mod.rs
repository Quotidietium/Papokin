use crate::text::color::{ARGBColor, hsv_to_rgb};
use crate::translation::{
    Locale, get_translation, get_translation_text, reorder_substitutions, translation_to_pretty,
};
use crate::version::JavaMinecraftVersion;
use click::ClickEvent;
use color::Color;
use colored::Colorize;
use core::str;
use hover::HoverEvent;
use serde::de::{Error, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::borrow::Cow;
use std::fmt::Formatter;
use std::sync::LazyLock;
use style::Style;

pub mod click;
pub mod color;
pub mod hover;
pub mod legacy;
pub mod sign;
pub mod style;

/// 表示一个 Minecraft 聊天组件。
///
/// 文本组件是 Minecraft 聊天系统的构建单元，允许
/// 带颜色、样式、点击事件和悬停提示的富文本，以及
/// 翻译。它们可以嵌套与组合，以构建复杂的消息。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TextComponent(pub TextComponentBase);

impl<'de> Deserialize<'de> for TextComponent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct TextComponentVisitor;

        impl<'de> Visitor<'de> for TextComponentVisitor {
            type Value = TextComponentBase;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str("a TextComponentBase or a sequence of TextComponentBase")
            }

            fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(TextComponentBase {
                    content: Box::new(TextContent::Text {
                        text: Cow::from(v.to_string()),
                    }),
                    style: Box::default(),
                    extra: vec![],
                })
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut bases = Vec::new();
                while let Some(element) = seq.next_element::<TextComponent>()? {
                    bases.push(element.0);
                }

                Ok(TextComponentBase {
                    content: Box::new(TextContent::Text { text: "".into() }),
                    style: Box::default(),
                    extra: bases,
                })
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                TextComponentBase::deserialize(serde::de::value::MapAccessDeserializer::new(map))
            }
        }

        deserializer
            .deserialize_any(TextComponentVisitor)
            .map(TextComponent)
    }
}

impl Serialize for TextComponent {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_newtype_struct("TextComponent", &self.0.clone().to_translated())
    }
}

/// 文本组件的基础结构，包含内容、样式和子组件。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct TextComponentBase {
    /// 此组件的实际内容（文本、翻译等）。
    #[serde(flatten)]
    pub content: Box<TextContent>,
    /// 应用于此组件的样式（颜色、粗体、点击事件等）。
    #[serde(flatten)]
    pub style: Box<Style>,
    /// 附加在此组件内容之后的子文本组件。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<Self>,
}

impl TextComponentBase {
    /// 将此组件转换为最新 Minecraft 版本的 NBT 复合标签。
    #[must_use]
    pub fn to_nbt_compound(&self) -> papokin_nbt::NbtCompound {
        self.to_nbt_compound_for_version(&JavaMinecraftVersion::V_1_21_11)
    }

    /// 将此组件转换为特定 Minecraft 版本的 NBT 复合标签。
    #[expect(clippy::too_many_lines)]
    #[must_use]
    pub fn to_nbt_compound_for_version(
        &self,
        version: &JavaMinecraftVersion,
    ) -> papokin_nbt::NbtCompound {
        let mut compound = papokin_nbt::NbtCompound::new();
        match &*self.content {
            TextContent::Text { text } => {
                compound.put_string("text", text.to_string());
            }
            TextContent::Translate {
                translate, with, ..
            } => {
                compound.put_string("translate", translate.to_string());
                if !with.is_empty() {
                    let list = with
                        .iter()
                        .map(|w| w.to_nbt_tag_for_version(version))
                        .collect();
                    compound.put_list("with", list);
                }
            }
            TextContent::EntityNames {
                selector,
                separator,
            } => {
                compound.put_string("selector", selector.to_string());
                if let Some(sep) = separator {
                    compound.put_string("separator", sep.to_string());
                }
            }
            TextContent::Keybind { keybind } => {
                compound.put_string("keybind", keybind.to_string());
            }
            TextContent::Custom { key, with, .. } => {
                compound.put_string("translate", key.to_string());
                if !with.is_empty() {
                    let list = with
                        .iter()
                        .map(|w| w.to_nbt_tag_for_version(version))
                        .collect();
                    compound.put_list("with", list);
                }
            }
            TextContent::PlayerSprite {
                type_name,
                profile,
                hat,
            } => {
                if *version >= JavaMinecraftVersion::V_26_1 {
                    let full_type = if type_name.contains(':') {
                        type_name.to_string()
                    } else {
                        format!("minecraft:{type_name}")
                    };
                    compound.put_string("type", full_type);
                    compound.put_compound("player", profile.0.clone());
                    compound.put_byte("hat", i8::from(*hat));
                } else {
                    let name = profile.0.get_string("name").unwrap_or("player_sprite");
                    compound.put_string("text", name.to_string());
                }
            }
        }

        if let Some(ref color) = self.style.color {
            let color_str = match color {
                Color::Reset => Some("reset".to_string()),
                Color::Named(c) => Some(c.name().to_string()),
                Color::Rgb(rgb) => {
                    if *version >= JavaMinecraftVersion::V_1_16 {
                        Some(format!("#{:02X}{:02X}{:02X}", rgb.red, rgb.green, rgb.blue))
                    } else {
                        Some(rgb.to_nearest_named().name().to_string())
                    }
                }
            };
            if let Some(cs) = color_str {
                compound.put_string("color", cs);
            }
        }

        if let Some(bold) = self.style.bold {
            compound.put_byte("bold", i8::from(bold));
        }
        if let Some(italic) = self.style.italic {
            compound.put_byte("italic", i8::from(italic));
        }
        if let Some(underlined) = self.style.underlined {
            compound.put_byte("underlined", i8::from(underlined));
        }
        if let Some(strikethrough) = self.style.strikethrough {
            compound.put_byte("strikethrough", i8::from(strikethrough));
        }
        if let Some(obfuscated) = self.style.obfuscated {
            compound.put_byte("obfuscated", i8::from(obfuscated));
        }
        if let Some(ref insertion) = self.style.insertion {
            compound.put_string("insertion", insertion.clone());
        }
        if let Some(ref font) = self.style.font {
            compound.put_string("font", font.clone());
        }

        if *version >= JavaMinecraftVersion::V_1_21_4
            && let Some(ref shadow) = self.style.shadow_color
        {
            compound.put_int("shadow_color", shadow.to_argb_int());
        }

        if let Some(ref click) = self.style.click_event {
            let mut click_tag = papokin_nbt::NbtCompound::new();
            match click {
                ClickEvent::OpenUrl { url } => {
                    click_tag.put_string("action", "open_url".to_string());
                    if *version >= JavaMinecraftVersion::V_1_21_5 {
                        click_tag.put_string("url", url.to_string());
                    } else {
                        click_tag.put_string("value", url.to_string());
                    }
                }
                ClickEvent::OpenFile { path } => {
                    click_tag.put_string("action", "open_file".to_string());
                    if *version >= JavaMinecraftVersion::V_1_21_5 {
                        click_tag.put_string("path", path.to_string());
                    } else {
                        click_tag.put_string("value", path.to_string());
                    }
                }
                ClickEvent::RunCommand { command } => {
                    click_tag.put_string("action", "run_command".to_string());
                    if *version >= JavaMinecraftVersion::V_1_21_5 {
                        click_tag.put_string("command", command.to_string());
                    } else {
                        click_tag.put_string("value", command.to_string());
                    }
                }
                ClickEvent::SuggestCommand { command } => {
                    click_tag.put_string("action", "suggest_command".to_string());
                    if *version >= JavaMinecraftVersion::V_1_21_5 {
                        click_tag.put_string("command", command.to_string());
                    } else {
                        click_tag.put_string("value", command.to_string());
                    }
                }
                ClickEvent::ChangePage { page } => {
                    click_tag.put_string("action", "change_page".to_string());
                    if *version >= JavaMinecraftVersion::V_1_21_6 {
                        click_tag.put_int("page", *page as i32);
                    } else if *version >= JavaMinecraftVersion::V_1_21_5 {
                        click_tag.put_string("page", page.to_string());
                    } else {
                        click_tag.put_string("value", page.to_string());
                    }
                }
                ClickEvent::CopyToClipboard { value } => {
                    click_tag.put_string("action", "copy_to_clipboard".to_string());
                    click_tag.put_string("value", value.to_string());
                }
            }
            let click_key = if *version >= JavaMinecraftVersion::V_1_21_5 {
                "click_event"
            } else {
                "clickEvent"
            };
            compound.put_compound(click_key, click_tag);
        }

        if let Some(ref hover) = self.style.hover_event {
            let mut hover_tag = papokin_nbt::NbtCompound::new();
            if *version >= JavaMinecraftVersion::V_1_21_5 {
                match hover {
                    HoverEvent::ShowText { value } => {
                        hover_tag.put_string("action", "show_text".to_string());
                        if value.len() == 1 {
                            hover_tag.put("value", value[0].to_nbt_tag_for_version(version));
                        } else {
                            let list = value
                                .iter()
                                .map(|e| e.to_nbt_tag_for_version(version))
                                .collect();
                            hover_tag.put_list("value", list);
                        }
                    }
                    HoverEvent::ShowItem { id, count } => {
                        hover_tag.put_string("action", "show_item".to_string());
                        hover_tag.put_string("id", id.to_string());
                        if let Some(cnt) = count {
                            hover_tag.put_int("count", *cnt);
                        }
                    }
                    HoverEvent::ShowEntity { id, uuid, name } => {
                        hover_tag.put_string("action", "show_entity".to_string());
                        hover_tag.put_string("id", id.to_string());
                        hover_tag.put_string("uuid", uuid.to_string());
                        if let Some(n) = name {
                            if n.len() == 1 {
                                hover_tag.put("name", n[0].to_nbt_tag_for_version(version));
                            } else {
                                let list = n
                                    .iter()
                                    .map(|e| e.to_nbt_tag_for_version(version))
                                    .collect();
                                hover_tag.put_list("name", list);
                            }
                        }
                    }
                }
            } else if *version >= JavaMinecraftVersion::V_1_16 {
                match hover {
                    HoverEvent::ShowText { value } => {
                        hover_tag.put_string("action", "show_text".to_string());
                        if value.len() == 1 {
                            hover_tag.put("contents", value[0].to_nbt_tag_for_version(version));
                        } else {
                            let list = value
                                .iter()
                                .map(|e| e.to_nbt_tag_for_version(version))
                                .collect();
                            hover_tag.put_list("contents", list);
                        }
                    }
                    HoverEvent::ShowItem { id, count } => {
                        hover_tag.put_string("action", "show_item".to_string());
                        let mut contents = papokin_nbt::NbtCompound::new();
                        contents.put_string("id", id.to_string());
                        if let Some(cnt) = count {
                            contents.put_int("count", *cnt);
                        }
                        hover_tag.put_compound("contents", contents);
                    }
                    HoverEvent::ShowEntity { id, uuid, name } => {
                        hover_tag.put_string("action", "show_entity".to_string());
                        let mut contents = papokin_nbt::NbtCompound::new();
                        contents.put_string("type", id.to_string());
                        contents.put_string("id", uuid.to_string());
                        if let Some(n) = name {
                            if n.len() == 1 {
                                contents.put("name", n[0].to_nbt_tag_for_version(version));
                            } else {
                                let list = n
                                    .iter()
                                    .map(|e| e.to_nbt_tag_for_version(version))
                                    .collect();
                                contents.put_list("name", list);
                            }
                        }
                        hover_tag.put_compound("contents", contents);
                    }
                }
            } else {
                match hover {
                    HoverEvent::ShowText { value } => {
                        hover_tag.put_string("action", "show_text".to_string());
                        if value.len() == 1 {
                            hover_tag.put("value", value[0].to_nbt_tag_for_version(version));
                        } else {
                            let list = value
                                .iter()
                                .map(|e| e.to_nbt_tag_for_version(version))
                                .collect();
                            hover_tag.put_list("value", list);
                        }
                    }
                    HoverEvent::ShowItem { id, count } => {
                        hover_tag.put_string("action", "show_item".to_string());
                        let count_val = count.unwrap_or(1);
                        hover_tag
                            .put_string("value", format!("{{id:\"{id}\",Count:{count_val}b}}"));
                    }
                    HoverEvent::ShowEntity { id, uuid, name } => {
                        hover_tag.put_string("action", "show_entity".to_string());
                        let name_str = name.as_ref().map_or_else(String::new, |n| {
                            n.iter()
                                .map(|e| e.clone().get_text(Locale::EnUs))
                                .collect::<String>()
                        });
                        hover_tag.put_string(
                            "value",
                            format!("{{id:\"{uuid}\",type:\"{id}\",name:\"{name_str}\"}}"),
                        );
                    }
                }
            }
            let hover_key = if *version >= JavaMinecraftVersion::V_1_21_5 {
                "hover_event"
            } else {
                "hoverEvent"
            };
            compound.put_compound(hover_key, hover_tag);
        }

        if !self.extra.is_empty() {
            let list = self
                .extra
                .iter()
                .map(|e| e.to_nbt_tag_for_version(version))
                .collect();
            compound.put_list("extra", list);
        }

        compound
    }

    /// 将此组件转换为指定 Minecraft 版本的 `NbtTag`。
    ///
    /// 对于 >= 1.20.3 的版本，尽可能使用紧凑表示（纯字符串标签）。
    #[must_use]
    pub fn to_nbt_tag_for_version(
        &self,
        version: &JavaMinecraftVersion,
    ) -> papokin_nbt::tag::NbtTag {
        if *version >= JavaMinecraftVersion::V_1_20_3
            && self.style.is_empty()
            && self.extra.is_empty()
            && let TextContent::Text { text } = &*self.content
        {
            papokin_nbt::tag::NbtTag::String(text.to_string().into_boxed_str())
        } else {
            papokin_nbt::tag::NbtTag::Compound(self.to_nbt_compound_for_version(version))
        }
    }

    /// 将此组件转换为特定 Minecraft 版本的 `serde_json::Value`。
    #[expect(clippy::too_many_lines)]
    #[must_use]
    pub fn to_json_value_for_version(&self, version: &JavaMinecraftVersion) -> serde_json::Value {
        if *version >= JavaMinecraftVersion::V_1_20_3
            && self.style.is_empty()
            && self.extra.is_empty()
            && let TextContent::Text { text } = &*self.content
        {
            return serde_json::Value::String(text.to_string());
        }

        let mut map = serde_json::Map::new();

        match &*self.content {
            TextContent::Text { text } => {
                map.insert(
                    "text".to_string(),
                    serde_json::Value::String(text.to_string()),
                );
            }
            TextContent::Translate {
                translate, with, ..
            } => {
                map.insert(
                    "translate".to_string(),
                    serde_json::Value::String(translate.to_string()),
                );
                if !with.is_empty() {
                    let list: Vec<serde_json::Value> = with
                        .iter()
                        .map(|w| w.to_json_value_for_version(version))
                        .collect();
                    map.insert("with".to_string(), serde_json::Value::Array(list));
                }
            }
            TextContent::EntityNames {
                selector,
                separator,
            } => {
                map.insert(
                    "selector".to_string(),
                    serde_json::Value::String(selector.to_string()),
                );
                if let Some(sep) = separator {
                    map.insert(
                        "separator".to_string(),
                        serde_json::Value::String(sep.to_string()),
                    );
                }
            }
            TextContent::Keybind { keybind } => {
                map.insert(
                    "keybind".to_string(),
                    serde_json::Value::String(keybind.to_string()),
                );
            }
            TextContent::Custom { key, with, .. } => {
                map.insert(
                    "translate".to_string(),
                    serde_json::Value::String(key.to_string()),
                );
                if !with.is_empty() {
                    let list: Vec<serde_json::Value> = with
                        .iter()
                        .map(|w| w.to_json_value_for_version(version))
                        .collect();
                    map.insert("with".to_string(), serde_json::Value::Array(list));
                }
            }
            TextContent::PlayerSprite {
                type_name,
                profile,
                hat,
            } => {
                if *version >= JavaMinecraftVersion::V_26_1 {
                    let full_type = if type_name.contains(':') {
                        type_name.to_string()
                    } else {
                        format!("minecraft:{type_name}")
                    };
                    map.insert("type".to_string(), serde_json::Value::String(full_type));
                    map.insert("player".to_string(), nbt_compound_to_json(&profile.0));
                    map.insert("hat".to_string(), serde_json::Value::Bool(*hat));
                } else {
                    let name = profile.0.get_string("name").unwrap_or("player_sprite");
                    map.insert(
                        "text".to_string(),
                        serde_json::Value::String(name.to_string()),
                    );
                }
            }
        }

        if let Some(ref color) = self.style.color {
            let color_str = match color {
                Color::Reset => Some("reset".to_string()),
                Color::Named(c) => Some(c.name().to_string()),
                Color::Rgb(rgb) => {
                    if *version >= JavaMinecraftVersion::V_1_16 {
                        Some(format!("#{:02X}{:02X}{:02X}", rgb.red, rgb.green, rgb.blue))
                    } else {
                        Some(rgb.to_nearest_named().name().to_string())
                    }
                }
            };
            if let Some(cs) = color_str {
                map.insert("color".to_string(), serde_json::Value::String(cs));
            }
        }

        if let Some(bold) = self.style.bold {
            map.insert("bold".to_string(), serde_json::Value::Bool(bold));
        }
        if let Some(italic) = self.style.italic {
            map.insert("italic".to_string(), serde_json::Value::Bool(italic));
        }
        if let Some(underlined) = self.style.underlined {
            map.insert(
                "underlined".to_string(),
                serde_json::Value::Bool(underlined),
            );
        }
        if let Some(strikethrough) = self.style.strikethrough {
            map.insert(
                "strikethrough".to_string(),
                serde_json::Value::Bool(strikethrough),
            );
        }
        if let Some(obfuscated) = self.style.obfuscated {
            map.insert(
                "obfuscated".to_string(),
                serde_json::Value::Bool(obfuscated),
            );
        }
        if let Some(ref insertion) = self.style.insertion {
            map.insert(
                "insertion".to_string(),
                serde_json::Value::String(insertion.clone()),
            );
        }
        if let Some(ref font) = self.style.font {
            map.insert("font".to_string(), serde_json::Value::String(font.clone()));
        }

        if *version >= JavaMinecraftVersion::V_1_21_4
            && let Some(ref shadow) = self.style.shadow_color
        {
            map.insert(
                "shadow_color".to_string(),
                serde_json::json!(shadow.to_argb_int()),
            );
        }

        if let Some(ref click) = self.style.click_event {
            let mut click_map = serde_json::Map::new();
            match click {
                ClickEvent::OpenUrl { url } => {
                    click_map.insert(
                        "action".to_string(),
                        serde_json::Value::String("open_url".to_string()),
                    );
                    if *version >= JavaMinecraftVersion::V_1_21_5 {
                        click_map.insert(
                            "url".to_string(),
                            serde_json::Value::String(url.to_string()),
                        );
                    } else {
                        click_map.insert(
                            "value".to_string(),
                            serde_json::Value::String(url.to_string()),
                        );
                    }
                }
                ClickEvent::OpenFile { path } => {
                    click_map.insert(
                        "action".to_string(),
                        serde_json::Value::String("open_file".to_string()),
                    );
                    if *version >= JavaMinecraftVersion::V_1_21_5 {
                        click_map.insert(
                            "path".to_string(),
                            serde_json::Value::String(path.to_string()),
                        );
                    } else {
                        click_map.insert(
                            "value".to_string(),
                            serde_json::Value::String(path.to_string()),
                        );
                    }
                }
                ClickEvent::RunCommand { command } => {
                    click_map.insert(
                        "action".to_string(),
                        serde_json::Value::String("run_command".to_string()),
                    );
                    if *version >= JavaMinecraftVersion::V_1_21_5 {
                        click_map.insert(
                            "command".to_string(),
                            serde_json::Value::String(command.to_string()),
                        );
                    } else {
                        click_map.insert(
                            "value".to_string(),
                            serde_json::Value::String(command.to_string()),
                        );
                    }
                }
                ClickEvent::SuggestCommand { command } => {
                    click_map.insert(
                        "action".to_string(),
                        serde_json::Value::String("suggest_command".to_string()),
                    );
                    if *version >= JavaMinecraftVersion::V_1_21_5 {
                        click_map.insert(
                            "command".to_string(),
                            serde_json::Value::String(command.to_string()),
                        );
                    } else {
                        click_map.insert(
                            "value".to_string(),
                            serde_json::Value::String(command.to_string()),
                        );
                    }
                }
                ClickEvent::ChangePage { page } => {
                    click_map.insert(
                        "action".to_string(),
                        serde_json::Value::String("change_page".to_string()),
                    );
                    if *version >= JavaMinecraftVersion::V_1_21_6 {
                        click_map.insert("page".to_string(), serde_json::json!(*page as i32));
                    } else if *version >= JavaMinecraftVersion::V_1_21_5 {
                        click_map.insert(
                            "page".to_string(),
                            serde_json::Value::String(page.to_string()),
                        );
                    } else {
                        click_map.insert(
                            "value".to_string(),
                            serde_json::Value::String(page.to_string()),
                        );
                    }
                }
                ClickEvent::CopyToClipboard { value } => {
                    click_map.insert(
                        "action".to_string(),
                        serde_json::Value::String("copy_to_clipboard".to_string()),
                    );
                    click_map.insert(
                        "value".to_string(),
                        serde_json::Value::String(value.to_string()),
                    );
                }
            }
            let click_key = if *version >= JavaMinecraftVersion::V_1_21_5 {
                "click_event"
            } else {
                "clickEvent"
            };
            map.insert(click_key.to_string(), serde_json::Value::Object(click_map));
        }

        if let Some(ref hover) = self.style.hover_event {
            let mut hover_map = serde_json::Map::new();
            if *version >= JavaMinecraftVersion::V_1_21_5 {
                match hover {
                    HoverEvent::ShowText { value } => {
                        hover_map.insert(
                            "action".to_string(),
                            serde_json::Value::String("show_text".to_string()),
                        );
                        if value.len() == 1 {
                            hover_map.insert(
                                "value".to_string(),
                                value[0].to_json_value_for_version(version),
                            );
                        } else {
                            let list = value
                                .iter()
                                .map(|e| e.to_json_value_for_version(version))
                                .collect();
                            hover_map.insert("value".to_string(), serde_json::Value::Array(list));
                        }
                    }
                    HoverEvent::ShowItem { id, count } => {
                        hover_map.insert(
                            "action".to_string(),
                            serde_json::Value::String("show_item".to_string()),
                        );
                        hover_map
                            .insert("id".to_string(), serde_json::Value::String(id.to_string()));
                        if let Some(cnt) = count {
                            hover_map.insert("count".to_string(), serde_json::json!(*cnt));
                        }
                    }
                    HoverEvent::ShowEntity { id, uuid, name } => {
                        hover_map.insert(
                            "action".to_string(),
                            serde_json::Value::String("show_entity".to_string()),
                        );
                        hover_map
                            .insert("id".to_string(), serde_json::Value::String(id.to_string()));
                        hover_map.insert(
                            "uuid".to_string(),
                            serde_json::Value::String(uuid.to_string()),
                        );
                        if let Some(n) = name {
                            if n.len() == 1 {
                                hover_map.insert(
                                    "name".to_string(),
                                    n[0].to_json_value_for_version(version),
                                );
                            } else {
                                let list = n
                                    .iter()
                                    .map(|e| e.to_json_value_for_version(version))
                                    .collect();
                                hover_map
                                    .insert("name".to_string(), serde_json::Value::Array(list));
                            }
                        }
                    }
                }
            } else if *version >= JavaMinecraftVersion::V_1_16 {
                match hover {
                    HoverEvent::ShowText { value } => {
                        hover_map.insert(
                            "action".to_string(),
                            serde_json::Value::String("show_text".to_string()),
                        );
                        if value.len() == 1 {
                            hover_map.insert(
                                "contents".to_string(),
                                value[0].to_json_value_for_version(version),
                            );
                        } else {
                            let list = value
                                .iter()
                                .map(|e| e.to_json_value_for_version(version))
                                .collect();
                            hover_map
                                .insert("contents".to_string(), serde_json::Value::Array(list));
                        }
                    }
                    HoverEvent::ShowItem { id, count } => {
                        hover_map.insert(
                            "action".to_string(),
                            serde_json::Value::String("show_item".to_string()),
                        );
                        let mut contents = serde_json::Map::new();
                        contents
                            .insert("id".to_string(), serde_json::Value::String(id.to_string()));
                        if let Some(cnt) = count {
                            contents.insert("count".to_string(), serde_json::json!(*cnt));
                        }
                        hover_map
                            .insert("contents".to_string(), serde_json::Value::Object(contents));
                    }
                    HoverEvent::ShowEntity { id, uuid, name } => {
                        hover_map.insert(
                            "action".to_string(),
                            serde_json::Value::String("show_entity".to_string()),
                        );
                        let mut contents = serde_json::Map::new();
                        contents.insert(
                            "type".to_string(),
                            serde_json::Value::String(id.to_string()),
                        );
                        contents.insert(
                            "id".to_string(),
                            serde_json::Value::String(uuid.to_string()),
                        );
                        if let Some(n) = name {
                            if n.len() == 1 {
                                contents.insert(
                                    "name".to_string(),
                                    n[0].to_json_value_for_version(version),
                                );
                            } else {
                                let list = n
                                    .iter()
                                    .map(|e| e.to_json_value_for_version(version))
                                    .collect();
                                contents.insert("name".to_string(), serde_json::Value::Array(list));
                            }
                        }
                        hover_map
                            .insert("contents".to_string(), serde_json::Value::Object(contents));
                    }
                }
            } else {
                match hover {
                    HoverEvent::ShowText { value } => {
                        hover_map.insert(
                            "action".to_string(),
                            serde_json::Value::String("show_text".to_string()),
                        );
                        if value.len() == 1 {
                            hover_map.insert(
                                "value".to_string(),
                                value[0].to_json_value_for_version(version),
                            );
                        } else {
                            let list = value
                                .iter()
                                .map(|e| e.to_json_value_for_version(version))
                                .collect();
                            hover_map.insert("value".to_string(), serde_json::Value::Array(list));
                        }
                    }
                    HoverEvent::ShowItem { id, count } => {
                        hover_map.insert(
                            "action".to_string(),
                            serde_json::Value::String("show_item".to_string()),
                        );
                        let count_val = count.unwrap_or(1);
                        hover_map.insert(
                            "value".to_string(),
                            serde_json::Value::String(format!(
                                "{{id:\"{id}\",Count:{count_val}b}}"
                            )),
                        );
                    }
                    HoverEvent::ShowEntity { id, uuid, name } => {
                        hover_map.insert(
                            "action".to_string(),
                            serde_json::Value::String("show_entity".to_string()),
                        );
                        let name_str = name.as_ref().map_or_else(String::new, |n| {
                            n.iter()
                                .map(|e| e.clone().get_text(Locale::EnUs))
                                .collect::<String>()
                        });
                        hover_map.insert(
                            "value".to_string(),
                            serde_json::Value::String(format!(
                                "{{id:\"{uuid}\",type:\"{id}\",name:\"{name_str}\"}}"
                            )),
                        );
                    }
                }
            }
            let hover_key = if *version >= JavaMinecraftVersion::V_1_21_5 {
                "hover_event"
            } else {
                "hoverEvent"
            };
            map.insert(hover_key.to_string(), serde_json::Value::Object(hover_map));
        }

        if !self.extra.is_empty() {
            let list: Vec<serde_json::Value> = self
                .extra
                .iter()
                .map(|e| e.to_json_value_for_version(version))
                .collect();
            map.insert("extra".to_string(), serde_json::Value::Array(list));
        }

        serde_json::Value::Object(map)
    }

    /// 将此组件转换为特定 Minecraft 版本的 JSON 字符串。
    #[must_use]
    pub fn to_json_for_version(&self, version: &JavaMinecraftVersion) -> String {
        self.to_json_value_for_version(version).to_string()
    }
}

fn nbt_compound_to_json(compound: &papokin_nbt::NbtCompound) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (k, v) in &compound.child_tags {
        map.insert(k.to_string(), nbt_tag_to_json(v));
    }
    serde_json::Value::Object(map)
}

fn nbt_tag_to_json(tag: &papokin_nbt::tag::NbtTag) -> serde_json::Value {
    match tag {
        papokin_nbt::tag::NbtTag::End => serde_json::Value::Null,
        papokin_nbt::tag::NbtTag::Byte(b) => serde_json::json!(*b),
        papokin_nbt::tag::NbtTag::Short(s) => serde_json::json!(*s),
        papokin_nbt::tag::NbtTag::Int(i) => serde_json::json!(*i),
        papokin_nbt::tag::NbtTag::Long(l) => serde_json::json!(*l),
        papokin_nbt::tag::NbtTag::Float(f) => serde_json::json!(*f),
        papokin_nbt::tag::NbtTag::Double(d) => serde_json::json!(*d),
        papokin_nbt::tag::NbtTag::ByteArray(arr) => {
            serde_json::Value::Array(arr.iter().map(|&x| serde_json::json!(x)).collect())
        }
        papokin_nbt::tag::NbtTag::String(s) => serde_json::Value::String(s.to_string()),
        papokin_nbt::tag::NbtTag::List(list) => {
            serde_json::Value::Array(list.iter().map(nbt_tag_to_json).collect())
        }
        papokin_nbt::tag::NbtTag::Compound(c) => nbt_compound_to_json(c),
        papokin_nbt::tag::NbtTag::IntArray(arr) => {
            serde_json::Value::Array(arr.iter().map(|&x| serde_json::json!(x)).collect())
        }
        papokin_nbt::tag::NbtTag::LongArray(arr) => {
            serde_json::Value::Array(arr.iter().map(|&x| serde_json::json!(x)).collect())
        }
    }
}

impl TextComponentBase {
    /// 将此组件转换为人类可读的字符串，用于控制台输出。
    ///
    /// # Returns
    /// 一个已格式化、可直接输出到控制台的字符串。
    #[must_use]
    pub fn to_pretty_console(self) -> String {
        self.to_pretty_console_inner(&Style::default())
    }

    #[expect(clippy::wrong_self_convention)]
    fn to_pretty_console_inner(self, current_styles: &Style) -> String {
        fn osc8_link(url: &str, text: &str) -> String {
            format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
        }
        /// 优先采用 `a`。
        fn combine_styles(a: Style, b: &Style) -> Style {
            let mut style = Style::default();
            if let Some(color) = a.color {
                style.color = Some(color);
            } else {
                style.color = b.color;
            }
            if let Some(bold) = a.bold {
                style.bold = Some(bold);
            } else {
                style.bold = b.bold;
            }
            if let Some(italic) = a.italic {
                style.italic = Some(italic);
            } else {
                style.italic = b.italic;
            }
            if let Some(underlined) = a.underlined {
                style.underlined = Some(underlined);
            } else {
                style.underlined = b.underlined;
            }
            if let Some(strikethrough) = a.strikethrough {
                style.strikethrough = Some(strikethrough);
            } else {
                style.strikethrough = b.strikethrough;
            }
            if let Some(click_event) = a.click_event {
                style.click_event = Some(click_event);
            } else {
                style.click_event.clone_from(&b.click_event);
            }
            style
        }

        let mut text = match *self.content {
            TextContent::Text { text } => text.into_owned(),
            TextContent::Translate { translate, with } => {
                translation_to_pretty(format!("minecraft:{translate}"), Locale::EnUs, with)
            }
            TextContent::EntityNames {
                selector,
                separator: _,
            } => selector.into_owned(),
            TextContent::Keybind { keybind } => keybind.into_owned(),
            TextContent::Custom { key, with, .. } => translation_to_pretty(key, Locale::EnUs, with),
            TextContent::PlayerSprite { ref profile, .. } => profile
                .0
                .get_string("name")
                .map_or_else(|| "player_sprite".to_string(), ToString::to_string),
        };
        let style = combine_styles(*self.style, current_styles);
        let color = style.color;
        if let Some(color) = color {
            text = color.console_color(&text).to_string();
        }
        if style.bold.is_some() {
            text = text.bold().to_string();
        }
        if style.italic.is_some() {
            text = text.italic().to_string();
        }
        if style.underlined.is_some() {
            text = text.underline().to_string();
        }
        if style.strikethrough.is_some() {
            text = text.strikethrough().to_string();
        }
        if let Some(ClickEvent::OpenUrl { url }) = style.click_event.as_ref() {
            text = osc8_link(url, &text);
        }
        if let Some(ClickEvent::OpenFile { path }) = style.click_event.as_ref() {
            text = osc8_link(&format!("file://{path}"), &text);
        }

        for child in self.extra {
            text += &*child.to_pretty_console_inner(&style);
        }
        text
    }

    /// 提取此组件在给定区域设置下的原始文本内容。
    ///
    /// # Arguments
    /// - `locale` – 用于翻译的区域设置。
    ///
    /// # Returns
    /// 组件的纯文本内容。
    #[must_use]
    pub fn get_text(self, locale: Locale) -> String {
        let mut text = match *self.content {
            TextContent::Text { text } => text.into_owned(),
            TextContent::Translate { translate, with } => {
                get_translation_text(format!("minecraft:{translate}"), locale, with)
            }
            TextContent::EntityNames {
                selector,
                separator: _,
            } => selector.into_owned(),
            TextContent::Keybind { keybind } => keybind.into_owned(),
            TextContent::Custom { key, with, .. } => get_translation_text(key, locale, with),
            TextContent::PlayerSprite { profile, .. } => profile
                .0
                .get_string("name")
                .map(ToString::to_string)
                .unwrap_or_default(),
        };

        // 递归追加所有子组件的文本
        for child in self.extra {
            text += &child.get_text(locale);
        }

        text
    }

    /// 通过解析所有翻译来转换此组件。
    ///
    /// # Returns
    /// 一个所有翻译均已解析的新组件。
    fn translate_hover_event(style: &mut Style) {
        if let Some(ref hover) = style.hover_event {
            style.hover_event = match hover {
                HoverEvent::ShowText { value } => {
                    let mut hover_components = vec![];
                    for hover_component in value {
                        hover_components.push(hover_component.to_owned().to_translated());
                    }
                    Some(HoverEvent::ShowText {
                        value: hover_components,
                    })
                }
                HoverEvent::ShowEntity { name, id, uuid } => name.as_ref().map_or_else(
                    || {
                        Some(HoverEvent::ShowEntity {
                            name: None,
                            id: id.clone(),
                            uuid: uuid.clone(),
                        })
                    },
                    |name| {
                        Some(HoverEvent::ShowEntity {
                            name: Some(name.iter().map(|x| x.to_owned().to_translated()).collect()),
                            id: id.clone(),
                            uuid: uuid.clone(),
                        })
                    },
                ),
                HoverEvent::ShowItem { id, count } => Some(HoverEvent::ShowItem {
                    id: id.clone(),
                    count: count.to_owned(),
                }),
            };
        }
    }

    /// 通过解析所有翻译来转换此组件。
    ///
    /// # Returns
    /// 一个所有翻译均已解析的新组件。
    #[must_use]
    pub fn to_translated(self) -> Self {
        // NOTE: 将平移切分为多个片段并插入替换内容。
        let component = match *self.content {
            TextContent::Translate { translate, with } => {
                let mut translated_with = vec![];
                for w in with {
                    translated_with.push(w.to_translated());
                }
                Self {
                    content: Box::new(TextContent::Translate {
                        translate,
                        with: translated_with,
                    }),
                    style: self.style,
                    extra: self.extra,
                }
            }
            TextContent::Custom { key, with, locale } => {
                let translation = get_translation(&key, locale);
                let mut translation_parent = translation.clone();
                let mut translation_slices = vec![];

                if translation.contains('%') {
                    let (substitutions, ranges) = reorder_substitutions(&translation, with);
                    for (idx, &range) in ranges.iter().enumerate() {
                        if idx == 0 {
                            translation_parent = translation[..range.start].to_string();
                        }
                        translation_slices.push(substitutions[idx].clone());
                        if range.end >= translation.len() - 1 {
                            continue;
                        }

                        translation_slices.push(Self {
                            content: Box::new(TextContent::Text {
                                text: if idx == ranges.len() - 1 {
                                    // 最后一次替换，追加译文的其余部分
                                    Cow::Owned(translation[range.end + 1..].to_string())
                                } else {
                                    Cow::Owned(
                                        translation[range.end + 1..ranges[idx + 1].start]
                                            .to_string(),
                                    )
                                },
                            }),
                            style: Box::new(Style::default()),
                            extra: vec![],
                        });
                    }
                }
                for i in self.extra {
                    translation_slices.push(i);
                }
                Self {
                    content: Box::new(TextContent::Text {
                        text: translation_parent.into(),
                    }),
                    style: self.style,
                    extra: translation_slices,
                }
            }
            _ => self, // 如果不是翻译，则原样返回
        };
        // 确保额外组件也被转换
        let extra = component
            .extra
            .into_iter()
            .map(Self::to_translated)
            .collect();

        // 如果存在悬停事件，它也会被翻译
        let mut style = component.style;
        Self::translate_hover_event(&mut style);

        Self {
            content: component.content,
            style,
            extra,
        }
    }
}

impl TextComponent {
    /// 创建一个不含任何文本内容的新文本组件。
    ///
    /// 可用于将多个文本组件合并为一个
    /// 将它们全部作为空文本组件的子组件
    /// 按要求的顺序。
    ///
    /// # Returns
    /// 一个空的 `TextComponent`。
    #[must_use]
    pub fn empty() -> Self {
        Self::text("")
    }

    /// 从 NBT 表示形式解析文本组件
    #[must_use]
    pub fn from_nbt(tag: &papokin_nbt::tag::NbtTag) -> Self {
        serde_json::from_value(nbt_tag_to_json(tag)).unwrap_or_else(|_| Self::empty())
    }

    /// 创建一个带纯文本内容的新文本组件。
    ///
    /// # Arguments
    /// - `plain` – 文本内容（可为 `String`、`&str` 或 `Cow <'static, str>`）。
    ///
    /// # Returns
    /// 一个包含给定文本的新 `TextComponent`。
    #[must_use]
    pub fn text<P: Into<Cow<'static, str>>>(plain: P) -> Self {
        Self(TextComponentBase {
            content: Box::new(TextContent::Text { text: plain.into() }),
            style: Box::new(Style::default()),
            extra: vec![],
        })
    }

    /// 创建一个带翻译键的新文本组件。
    ///
    /// # Arguments
    /// - `key` – 翻译键（例如 "multiplayer.player.joined"）。
    /// - `with` – 用于翻译的替换参数。
    ///
    /// # Returns
    /// 一个将由客户端进行翻译的新 `TextComponent`。
    #[must_use]
    pub fn translate<K: Into<Cow<'static, str>>, W: Into<Vec<Self>>>(key: K, with: W) -> Self {
        Self(TextComponentBase {
            content: Box::new(TextContent::Translate {
                translate: key.into(),
                with: with.into().into_iter().map(|x| x.0).collect(),
            }),
            style: Box::new(Style::default()),
            extra: vec![],
        })
    }

    /// 创建一个使用自定义翻译键的新文本组件。
    ///
    /// # Arguments
    /// - `namespace` – 翻译所属的命名空间（例如 "papokinplus"）。
    /// - `key` – 命名空间内的翻译键。
    /// - `locale` – 用于翻译的区域设置。
    /// - `with` – 用于翻译的替换参数。
    ///
    /// # Returns
    /// 一个带有自定义翻译的新 `TextComponent`。
    #[must_use]
    pub fn custom<K: Into<Cow<'static, str>>, W: Into<Vec<Self>>>(
        namespace: K,
        key: K,
        locale: Locale,
        with: W,
    ) -> Self {
        Self(TextComponentBase {
            content: Box::new(TextContent::Custom {
                key: format!("{}:{}", namespace.into(), key.into())
                    .to_lowercase()
                    .into(),
                locale,
                with: with.into().into_iter().map(|x| x.0).collect(),
            }),
            style: Box::new(Style::default()),
            extra: vec![],
        })
    }

    /// 创建一个显示选择器所找到的一个或多个实体名称的新文本组件。
    ///
    /// # Arguments
    /// - `selector` – 实体选择器字符串（例如 `@e[type=pig]`）。
    /// - `separator` – 多个实体名称之间的可选分隔符字符串。
    ///
    /// # Returns
    /// 一个显示实体名称的新 `TextComponent`。
    #[must_use]
    pub fn entity_names<S: Into<Cow<'static, str>>, P: Into<Cow<'static, str>>>(
        selector: S,
        separator: Option<P>,
    ) -> Self {
        Self(TextComponentBase {
            content: Box::new(TextContent::EntityNames {
                selector: selector.into(),
                separator: separator.map(Into::into),
            }),
            style: Box::new(Style::default()),
            extra: vec![],
        })
    }

    /// 创建一个显示按键绑定标识符的新文本组件。
    ///
    /// # Arguments
    /// - `keybind` – 按键绑定标识符（例如 `key.jump`、`key.forward`）。
    ///
    /// # Returns
    /// 一个显示所配置按键的新 `TextComponent`。
    #[must_use]
    pub fn keybind<K: Into<Cow<'static, str>>>(keybind: K) -> Self {
        Self(TextComponentBase {
            content: Box::new(TextContent::Keybind {
                keybind: keybind.into(),
            }),
            style: Box::new(Style::default()),
            extra: vec![],
        })
    }

    /// 向此组件追加一个子组件。
    ///
    /// # Arguments
    /// - `child` – 要追加的组件。
    ///
    /// # Returns
    /// 添加了子组件的组件。
    #[must_use]
    pub fn add_child(mut self, child: Self) -> Self {
        self.0.extra.push(child.0);
        self
    }

    /// 根据原始内容创建新组件。
    ///
    /// # Arguments
    /// - `content` – 文本内容。
    ///
    /// # Returns
    /// 一个包含给定内容的新组件。
    #[must_use]
    pub fn from_content(content: TextContent) -> Self {
        Self(TextComponentBase {
            content: Box::new(content),
            style: Box::new(Style::default()),
            extra: vec![],
        })
    }

    /// 向此组件追加纯文本。
    ///
    /// # Arguments
    /// - `text` – 要追加的文本。
    ///
    /// # Returns
    /// 追加了文本的组件。
    #[must_use]
    pub fn add_text<P: Into<Cow<'static, str>>>(mut self, text: P) -> Self {
        self.0.extra.push(TextComponentBase {
            content: Box::new(TextContent::Text { text: text.into() }),
            style: Box::new(Style::default()),
            extra: vec![],
        });
        self
    }

    /// 提取英语（美国）的原始文本内容。
    ///
    /// # Returns
    /// 纯文本内容。
    #[must_use]
    pub fn get_text(self) -> String {
        self.0.get_text(Locale::EnUs)
    }

    /// 创建已替换格式化占位符的聊天消息。
    ///
    /// 替换：
    /// - `&` 与 `§` 用于旧版格式化
    /// - `{DISPLAYNAME}` 替换为玩家名称
    /// - `{MESSAGE}` 替换为聊天消息内容
    ///
    /// # Arguments
    /// - `format` – 消息格式字符串。
    /// - `player_name` – 玩家的显示名称。
    /// - `content` – 聊天消息内容。
    ///
    /// # Returns
    /// 一个格式化后的聊天组件。
    #[must_use]
    pub fn chat_decorated(format: &str, player_name: &str, content: &str) -> Self {
        // Todo: 或许可以视权限允许玩家在聊天中使用 &
        let with_resolved_fields = format
            .replace('&', "§")
            .replace("{DISPLAYNAME}", player_name)
            .replace("{MESSAGE}", content);

        Self(TextComponentBase {
            content: Box::new(TextContent::Text {
                text: Cow::Owned(with_resolved_fields),
            }),
            style: Box::new(Style::default()),
            extra: vec![],
        })
    }

    /// 将此组件转换为美观的控制台字符串。
    ///
    /// # Returns
    /// 一个已格式化、可直接输出到控制台的字符串。
    #[must_use]
    pub fn to_pretty_console(self) -> String {
        self.0.to_pretty_console()
    }
}

impl TextComponent {
    /// 创建一个玩家精灵组件。
    #[must_use]
    pub fn player_sprite(profile: papokin_nbt::NbtCompound, hat: bool) -> Self {
        Self(TextComponentBase {
            content: Box::new(TextContent::PlayerSprite {
                type_name: Cow::Borrowed("minecraft:player_sprite"),
                profile: ProfileNbt(profile),
                hat,
            }),
            style: Box::default(),
            extra: vec![],
        })
    }

    /// 使用针对最新 Minecraft 版本的 NBT 序列化，将此组件编码为字节数组。
    ///
    /// # Returns
    /// 一个装箱的字节切片，包含 NBT 编码的组件。
    #[must_use]
    pub fn encode(&self) -> Box<[u8]> {
        self.encode_for_version(&JavaMinecraftVersion::V_1_21_11)
    }

    /// 使用针对特定 Minecraft 版本的 NBT 序列化，将此组件编码为字节数组。
    ///
    /// # Arguments
    /// - `version` – 要为其编码的 Minecraft 版本。
    ///
    /// # Returns
    /// 一个装箱的字节切片，包含 NBT 编码的组件。
    #[must_use]
    pub fn encode_for_version(&self, version: &JavaMinecraftVersion) -> Box<[u8]> {
        let tag = self
            .0
            .clone()
            .to_translated()
            .to_nbt_tag_for_version(version);
        let mut bytes = Vec::new();
        let mut writer = papokin_nbt::serializer::NbtWriteHelperJava::new(&mut bytes);
        let _ = tag.serialize(&mut writer);
        bytes.into_boxed_slice()
    }

    /// 将此组件转换为特定 Minecraft 版本的 NBT 复合标签。
    #[must_use]
    pub fn to_nbt_compound_for_version(
        &self,
        version: &JavaMinecraftVersion,
    ) -> papokin_nbt::NbtCompound {
        self.0
            .clone()
            .to_translated()
            .to_nbt_compound_for_version(version)
    }

    /// 将此组件转换为特定 Minecraft 版本的 `NbtTag`。
    #[must_use]
    pub fn to_nbt_tag_for_version(
        &self,
        version: &JavaMinecraftVersion,
    ) -> papokin_nbt::tag::NbtTag {
        self.0
            .clone()
            .to_translated()
            .to_nbt_tag_for_version(version)
    }

    /// 将此组件转换为特定 Minecraft 版本的 JSON 字符串。
    #[must_use]
    pub fn to_json_for_version(&self, version: &JavaMinecraftVersion) -> String {
        self.0.clone().to_translated().to_json_for_version(version)
    }

    /// 将此组件转换为特定 Minecraft 版本的 `serde_json::Value`。
    #[must_use]
    pub fn to_json_value_for_version(&self, version: &JavaMinecraftVersion) -> serde_json::Value {
        self.0
            .clone()
            .to_translated()
            .to_json_value_for_version(version)
    }

    /// 设置文本颜色。
    ///
    /// # Arguments
    /// - `color` – 要应用的颜色。
    ///
    /// # Returns
    /// 设置了颜色的组件。
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.0.style.color = Some(color);
        self
    }

    /// 使用 Minecraft 命名颜色设置文本颜色。
    ///
    /// # Arguments
    /// - `color` – 要应用的命名颜色。
    ///
    /// # Returns
    /// 设置了颜色的组件。
    #[must_use]
    pub fn color_named(mut self, color: color::NamedColor) -> Self {
        self.0.style.color = Some(Color::Named(color));
        self
    }

    /// 使用 RGB 颜色设置文本颜色。
    ///
    /// # Arguments
    /// - `color` – 要应用的 RGB 颜色。
    ///
    /// # Returns
    /// 设置了颜色的组件。
    #[must_use]
    pub fn color_rgb(mut self, color: color::RGBColor) -> Self {
        self.0.style.color = Some(Color::Rgb(color));
        self
    }

    /// 追加一个新行/换行符。
    ///
    /// # Returns
    /// 追加了换行符的组件。
    #[must_use]
    pub fn new_line(self) -> Self {
        self.add_child(Self::text("\n"))
    }

    /// 使用命名颜色为文本应用颜色渐变。
    ///
    /// # Arguments
    /// - `colors` – 要应用的渐变颜色。
    ///
    /// # Returns
    /// 应用了渐变效果的组件。
    #[must_use]
    pub fn gradient_named(self, colors: &[color::NamedColor]) -> Self {
        let rgb_colors: Vec<color::RGBColor> =
            colors.iter().map(color::NamedColor::to_rgb).collect();
        self.gradient(&rgb_colors)
    }

    /// 使用 RGB 颜色为文本应用颜色渐变。
    ///
    /// # Arguments
    /// - `colors` – 要应用的渐变颜色。
    ///
    /// # Returns
    /// 应用了渐变效果的组件。
    #[must_use]
    pub fn gradient(self, colors: &[color::RGBColor]) -> Self {
        if colors.len() < 2 {
            return self;
        }

        self.apply_color_effect(|i, len| {
            if len <= 1 {
                return colors[0];
            }
            let total_segments = colors.len() - 1;
            let position = i as f32 / (len - 1) as f32;
            let segment_f = position * total_segments as f32;
            let segment_index = (segment_f.floor() as usize).min(total_segments - 1);

            let local_t = segment_f - segment_index as f32;
            let start = colors[segment_index];
            let end = colors[segment_index + 1];

            // LERP 逻辑
            color::RGBColor::new(
                (f32::from(end.red) - f32::from(start.red)).mul_add(local_t, f32::from(start.red))
                    as u8,
                (f32::from(end.green) - f32::from(start.green))
                    .mul_add(local_t, f32::from(start.green)) as u8,
                (f32::from(end.blue) - f32::from(start.blue))
                    .mul_add(local_t, f32::from(start.blue)) as u8,
            )
        })
    }

    /// 为文本应用彩虹效果。
    ///
    /// 每个字符获得不同的色相，形成平滑的彩虹过渡。
    ///
    /// # Returns
    /// 应用了彩虹效果的组件。
    #[must_use]
    pub fn rainbow(self) -> Self {
        self.apply_color_effect(|i, len| {
            let hue = (i as f32 / len as f32) * 360.0;
            let (r, g, b) = hsv_to_rgb(hue, 1.0, 1.0);
            color::RGBColor::new(r, g, b)
        })
    }

    /// 对文本内容应用逐字符着色效果。
    ///
    /// # Arguments
    /// - `color_gen` – 接收字符索引和总长度的函数
    ///   并为该字符返回一个 RGB 颜色。
    ///
    /// # Returns
    /// 一个新的文本组件，其中每个字符会根据以下内容被单独着色
    /// 传给生成器函数。原组件的内容将变为空，
    /// 而带颜色的字符则被放入 `extra` 字段。
    fn apply_color_effect<F>(mut self, color_gen: F) -> Self
    where
        F: Fn(usize, usize) -> color::RGBColor,
    {
        let raw_text = self.0.clone().get_text(Locale::EnUs);
        let chars: Vec<char> = raw_text.chars().collect();
        let len = chars.len();

        if len == 0 {
            return self;
        }

        let mut colored_extra = Vec::new();
        for (i, c) in chars.into_iter().enumerate() {
            let rgb = color_gen(i, len);

            let mut char_base = TextComponentBase {
                content: Box::new(TextContent::Text {
                    text: Cow::Owned(c.to_string()),
                }),
                style: self.0.style.clone(),
                extra: vec![],
            };
            char_base.style.color = Some(Color::Rgb(rgb));
            colored_extra.push(char_base);
        }

        self.0.content = Box::new(TextContent::Text { text: "".into() });
        self.0.extra = colored_extra;
        self.0.style.click_event = None;
        self.0.style.hover_event = None;
        self
    }

    /// 将组件包裹在方括号中。
    ///
    /// # Returns
    /// 新的组件。
    #[allow(deprecated)]
    #[must_use]
    pub fn wrap_in_square_brackets(self) -> Self {
        Self::translate("chat.square_brackets", [self])
    }

    /// 使文本变为粗体。
    ///
    /// # Returns
    /// 启用了粗体的组件。
    #[must_use]
    pub fn bold(mut self) -> Self {
        self.0.style.bold = Some(true);
        self
    }

    /// 使文本变为斜体。
    ///
    /// # Returns
    /// 启用了斜体的组件。
    #[must_use]
    pub fn italic(mut self) -> Self {
        self.0.style.italic = Some(true);
        self
    }

    /// 为文本添加下划线。
    ///
    /// # Returns
    /// 启用了下划线的组件。
    #[must_use]
    pub fn underlined(mut self) -> Self {
        self.0.style.underlined = Some(true);
        self
    }

    /// 为文本添加删除线。
    ///
    /// # Returns
    /// 启用了删除线的组件。
    #[must_use]
    pub fn strikethrough(mut self) -> Self {
        self.0.style.strikethrough = Some(true);
        self
    }

    /// 使文本变为乱码（随机字符）。
    ///
    /// # Returns
    /// 启用了模糊处理的组件。
    #[must_use]
    pub fn obfuscated(mut self) -> Self {
        self.0.style.obfuscated = Some(true);
        self
    }

    /// 设置按住 Shift 点击时插入玩家聊天输入框的文本。
    ///
    /// 当玩家按住 Shift 点击文本时，该字符串会被插入其
    /// 聊天输入。它不会覆盖玩家已在编写的任何文本。
    /// 这仅对聊天消息有效。
    ///
    /// # Arguments
    /// - `text` – 按住 Shift 点击时要插入的文本。
    ///
    /// # Returns
    /// 设置了插入文本的组件。
    #[must_use]
    pub fn insertion(mut self, text: String) -> Self {
        self.0.style.insertion = Some(text);
        self
    }

    /// 设置玩家点击文本时发生的事件。
    ///
    /// 允许执行诸如运行命令、打开 URL、建议命令等操作，
    /// 或将文本复制到剪贴板。仅在聊天中有效。
    ///
    /// # Arguments
    /// - `event` – 要触发的点击事件。
    ///
    /// # Returns
    /// 设置了点击事件的组件。
    #[must_use]
    pub fn click_event(mut self, event: ClickEvent) -> Self {
        self.0.style.click_event = Some(event);
        self
    }

    /// 设置玩家悬停在文本上时显示的工具提示。
    ///
    /// 可显示纯文本、物品信息或实体详情。
    ///
    /// # Arguments
    /// - `event` – 要显示的悬停事件。
    ///
    /// # Returns
    /// 设置了悬停事件的组件。
    #[must_use]
    pub fn hover_event(mut self, event: HoverEvent) -> Self {
        self.0.style.hover_event = Some(event);
        self
    }

    /// 设置用于渲染的字体资源位置。
    ///
    /// 允许更改文本的字体。默认字体包括：
    /// - `minecraft:default` - Minecraft 标准字体。
    /// - `minecraft:uniform` - 等宽字体。
    /// - `minecraft:alt` - 一种备选字体样式。
    /// - `minecraft:illageralt` - 灾厄村民主题字体。
    ///
    /// # Arguments
    /// - `resource_location` – 字体资源位置（例如 "minecraft:uniform"）。
    ///
    /// # Returns
    /// 设置了字体的组件。
    #[must_use]
    pub fn font(mut self, resource_location: String) -> Self {
        self.0.style.font = Some(resource_location);
        self
    }

    /// 覆盖文本的阴影颜色。
    ///
    /// # Arguments
    /// - `color` – 阴影的 ARGB 颜色值。
    ///
    /// # Returns
    /// 设置了阴影颜色的组件。
    #[must_use]
    pub fn shadow_color(mut self, color: ARGBColor) -> Self {
        self.0.style.shadow_color = Some(color);
        self
    }
}

impl TextComponent {
    /// 用包含灰色逗号的分隔符将多个文本组件连接为一个
    /// 以及其后的一个空格。
    ///
    /// # Arguments
    /// - `elements` - 要连接的元素。
    ///
    /// # Returns
    /// 所有元素连接到其中后的最终文本组件。
    #[must_use]
    pub fn join_with_comma(elements: Vec<Self>) -> Self {
        static DEFAULT_SEPARATOR: LazyLock<TextComponent> = LazyLock::new(|| {
            TextComponent::text(", ").color(Color::Named(color::NamedColor::Gray))
        });

        Self::join(elements, &DEFAULT_SEPARATOR)
    }

    /// 用给定的分隔符文本组件将多个文本组件连接为一个。
    /// 如果只想用……连接文本组件，请改用 [`TextComponent::join_with_comma`]
    /// 中间加一个逗号。
    ///
    /// # Arguments
    /// - `elements` - 要连接的元素。
    /// - `separator` - 用于连接所提供元素的分隔符。
    ///
    /// # Returns
    /// 所有元素连接到其中后的最终文本组件。
    #[must_use]
    pub fn join(elements: Vec<Self>, separator: &Self) -> Self {
        let mut result = Self::empty();
        let mut first = true;

        for element in elements {
            if !first {
                result = result.add_child(separator.clone());
            }

            result = result.add_child(element);
            first = false;
        }

        result
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProfileNbt(pub papokin_nbt::NbtCompound);

impl Eq for ProfileNbt {}

impl std::hash::Hash for ProfileNbt {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_string().hash(state);
    }
}

/// 文本组件的内容类型。
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum TextContent {
    /// 原始的、未经翻译的文本。
    Text { text: Cow<'static, str> },
    /// 应在客户端上被翻译的文本。
    Translate {
        /// 翻译键（例如 "multiplayer.player.joined"）。
        translate: Cow<'static, str>,
        /// 翻译的替换参数。
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        with: Vec<TextComponentBase>,
    },
    /// 显示选择器所找到一个或多个实体的名称。
    EntityNames {
        /// 实体选择器字符串（例如 "@e[type=pig]"）。
        selector: Cow<'static, str>,
        /// 多个实体名称之间的可选分隔符。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        separator: Option<Cow<'static, str>>,
    },
    /// 可配置控件的按键绑定标识符。
    ///
    /// 可用的按键绑定参见 <https://minecraft.wiki/w/Controls#Configurable_controls>。
    Keybind {
        /// 按键绑定标识符（例如 "key.forward"）。
        keybind: Cow<'static, str>,
    },
    /// 用于模组内容的自定义翻译键。
    ///
    /// 此变体不会直接序列化；翻译将被解析
    /// 在序列化之前用 `to_translated()` 转换。
    #[serde(skip)]
    Custom {
        /// 含命名空间的完整翻译键（例如 "papokinplus:some.text"）。
        key: Cow<'static, str>,
        /// 用于翻译的区域设置。
        locale: Locale,
        /// 翻译的替换参数。
        with: Vec<TextComponentBase>,
    },
    /// 一个玩家精灵对象组件。
    #[serde(skip)]
    PlayerSprite {
        type_name: Cow<'static, str>,
        profile: ProfileNbt,
        hat: bool,
    },
}

/// 文本组件实现的测试。
#[cfg(test)]
mod test {
    use crate::text::click::ClickEvent;
    use crate::text::{TextComponent, color::NamedColor, hover::HoverEvent};
    use crate::version::JavaMinecraftVersion;
    use std::borrow::Cow;

    #[test]
    fn serialize_text_component() {
        #[allow(deprecated)]
        let msg_comp = TextComponent::translate(
            "multiplayer.player.joined",
            [TextComponent::text("NAME".to_string())],
        )
        .color_named(NamedColor::Yellow);

        let bytes = msg_comp.encode();

        let expected_compound = msg_comp.0.to_translated().to_nbt_compound();
        let mut cursor = std::io::Cursor::new(&bytes[..]);
        let mut reader = papokin_nbt::deserializer::NbtReadHelperJava::new(
            papokin_nbt::deserializer::NbtStreamReader(&mut cursor),
        );
        let decoded = papokin_nbt::Nbt::read_unnamed(&mut reader).unwrap();
        assert_eq!(decoded, expected_compound.into());
    }

    /// 客户端期望悬停事件的有效负载内联在 `action` 旁边。
    /// 将其嵌套在 `item`/`entity` 下会导致该组件解码失败，并且
    /// 将玩家踢出服务器。
    #[test]
    fn hover_event_payload_is_inlined() {
        let show_item = TextComponent::text("剑")
            .hover_event(HoverEvent::ShowItem {
                id: Cow::Borrowed("minecraft:diamond_sword"),
                count: Some(1),
            })
            .0
            .to_nbt_compound();
        let hover = show_item.get_compound("hover_event").unwrap();
        assert_eq!(hover.get_string("action"), Some("show_item"));
        assert_eq!(hover.get_string("id"), Some("minecraft:diamond_sword"));
        assert_eq!(hover.get_int("count"), Some(1));
        assert!(hover.get_compound("item").is_none());

        let show_entity = TextComponent::text("猪")
            .hover_event(HoverEvent::show_entity(
                "6ba1a740-9a3b-4b7c-8f2c-8f5a5c1a0a11",
                "minecraft:pig",
                None,
            ))
            .0
            .to_nbt_compound();
        let hover = show_entity.get_compound("hover_event").unwrap();
        assert_eq!(hover.get_string("action"), Some("show_entity"));
        assert_eq!(hover.get_string("id"), Some("minecraft:pig"));
        assert_eq!(
            hover.get_string("uuid"),
            Some("6ba1a740-9a3b-4b7c-8f2c-8f5a5c1a0a11")
        );
        assert!(hover.get_compound("entity").is_none());
    }

    /// `count` 对客户端而言是可选的，因此必须将其排除在载荷之外
    /// 当它从未被设置时。
    #[test]
    fn hover_show_item_omits_unset_count() {
        let compound = TextComponent::text("剑")
            .hover_event(HoverEvent::ShowItem {
                id: Cow::Borrowed("minecraft:diamond_sword"),
                count: None,
            })
            .0
            .to_nbt_compound();
        let hover = compound.get_compound("hover_event").unwrap();
        assert!(hover.get_int("count").is_none());
    }

    #[test]
    fn click_event_uses_legacy_value_before_1_21_5() {
        let compound = TextComponent::text("链接")
            .click_event(ClickEvent::OpenUrl {
                url: Cow::Borrowed("https://example.com"),
            })
            .0
            .to_nbt_compound_for_version(&JavaMinecraftVersion::V_1_21_4);
        let click = compound.get_compound("clickEvent").unwrap();
        assert_eq!(click.get_string("action"), Some("open_url"));
        assert_eq!(click.get_string("value"), Some("https://example.com"));
        assert!(click.get_string("url").is_none());

        let suggest = TextComponent::text("名称")
            .click_event(ClickEvent::SuggestCommand {
                command: Cow::Borrowed("/tell name"),
            })
            .0
            .to_nbt_compound_for_version(&JavaMinecraftVersion::V_1_21_4);
        let click = suggest.get_compound("clickEvent").unwrap();
        assert_eq!(click.get_string("command"), None);
        assert_eq!(click.get_string("value"), Some("/tell name"));
    }

    #[test]
    fn click_event_uses_modern_keys_from_1_21_5() {
        let compound = TextComponent::text("链接")
            .click_event(ClickEvent::OpenUrl {
                url: Cow::Borrowed("https://example.com"),
            })
            .0
            .to_nbt_compound_for_version(&JavaMinecraftVersion::V_1_21_5);
        let click = compound.get_compound("click_event").unwrap();
        assert_eq!(click.get_string("url"), Some("https://example.com"));
        assert!(click.get_string("value").is_none());
    }
}
