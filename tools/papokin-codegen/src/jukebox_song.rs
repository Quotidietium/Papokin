use heck::ToPascalCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;

/// 来自同步注册表资源文件的唱片机歌曲条目的补充数据。
#[derive(Deserialize)]
struct JukeboxSongData {
    /// 歌曲的时长（秒）。
    length_in_seconds: f32,
    /// 播放此唱片时红石比较器的输出信号强度（0–15）。
    comparator_output: u8,
}

/// 生成 `JukeboxSong` 枚举及其时长、比较器和名称方法的 `TokenStream`。
pub fn build() -> TokenStream {
    let dir = std::path::Path::new("../../assets/datapacks/26_3/data/minecraft/jukebox_song");
    let mut song_data: BTreeMap<String, JukeboxSongData> = BTreeMap::new();
    let mut entries: Vec<_> = fs::read_dir(dir)
        .expect("缺少唱片机歌曲目录")
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .collect();
    entries.sort_by_key(|e| e.path());

    for entry in entries {
        let path = entry.path();
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        let content = fs::read_to_string(&path).expect("读取唱片机歌曲文件失败");
        let data: JukeboxSongData =
            serde_json::from_str(&content).expect("解析唱片机歌曲 JSON 失败");
        song_data.insert(stem, data);
    }

    let songs: BTreeMap<String, u32> = song_data
        .keys()
        .enumerate()
        .map(|(i, k)| (k.clone(), i as u32))
        .collect();

    let make_variant_ident = |name: &str| {
        let pascal = name.to_pascal_case();
        if pascal.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            format_ident!("Id{}", pascal)
        } else {
            format_ident!("{}", pascal)
        }
    };

    let variants = songs
        .keys()
        .map(|name| {
            let variant_name = make_variant_ident(name);
            quote! { #variant_name, }
        })
        .collect::<TokenStream>();

    let type_from_name = songs
        .keys()
        .map(|name| {
            let variant_name = make_variant_ident(name);
            quote! { #name => Some(Self::#variant_name), }
        })
        .collect::<TokenStream>();

    let type_to_name = songs
        .keys()
        .map(|name| {
            let variant_name = make_variant_ident(name);
            quote! { Self::#variant_name => #name, }
        })
        .collect::<TokenStream>();

    let type_to_id = songs
        .iter()
        .map(|(name, id)| {
            let variant_name = make_variant_ident(name);
            quote! { Self::#variant_name => #id, }
        })
        .collect::<TokenStream>();

    let type_to_length = songs
        .keys()
        .map(|name| {
            let variant_name = make_variant_ident(name);
            let length = song_data
                .get(name)
                .map_or(0, |d| d.length_in_seconds as u32);
            quote! { Self::#variant_name => #length, }
        })
        .collect::<TokenStream>();

    let type_to_comparator = songs
        .keys()
        .map(|name| {
            let variant_name = make_variant_ident(name);
            let output = song_data.get(name).map_or(0, |d| d.comparator_output);
            quote! { Self::#variant_name => #output, }
        })
        .collect::<TokenStream>();

    quote! {
        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(u32)]
        pub enum JukeboxSong {
            #variants
        }

        impl JukeboxSong {
            #[doc = r" Returns the `JukeboxSong` from the string name (e.g., 'pigstep')."]
            #[must_use]
            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    #type_from_name
                    _ => None
                }
            }

            #[doc = r" Returns the string name of the song."]
            #[must_use]
            pub const fn to_name(&self) -> &'static str {
                match self {
                    #type_to_name
                }
            }

            #[doc = r" Returns the numeric ID associated with the song."]
            #[must_use]
            pub const fn get_id(&self) -> u32 {
                match self {
                    #type_to_id
                }
            }

            #[doc = r" Returns the comparator output value (0-15) for this song."]
            #[must_use]
            pub const fn comparator_output(&self) -> u8 {
                #[allow(clippy::match_same_arms)]
                match self {
                    #type_to_comparator
                }
            }

            #[doc = r" Returns the song length in seconds."]
            #[must_use]
            pub const fn length_in_seconds(&self) -> u32 {
                #[allow(clippy::match_same_arms)]
                match self {
                    #type_to_length
                }
            }

            #[doc = r" Returns the song length in ticks (20 ticks per second)."]
            #[must_use]
            pub const fn length_in_ticks(&self) -> u64 {
                self.length_in_seconds() as u64 * 20
            }
        }
    }
}
