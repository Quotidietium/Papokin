use papokin_data::packet::CURRENT_MC_VERSION;
use papokin_util::text::click::ClickEvent;
use papokin_util::text::hover::HoverEvent;
use papokin_util::text::{TextComponent, color::NamedColor};
use papokin_util::translation::get_translation_text;
use serde::Deserialize;
use std::borrow::Cow;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use papokin_util::PermissionLvl;
use papokin_util::permission::{Permission, PermissionDefault, PermissionRegistry};

use crate::command::argument_builder::{ArgumentBuilder, command};
use crate::command::context::command_context::CommandContext;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::node::{CommandExecutor, CommandExecutorResult};

const NAMES: [&str; 3] = ["papokin", "version", "ver"];
const DESCRIPTION: &str = "显示关于 Papokin 的信息。";
const PERMISSION: &str = "papokin:command.papokin";

const CACHE_DURATION: Duration = Duration::from_hours(24);

struct Executor;

const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
const GIT_HASH: &str = env!("GIT_HASH");
const GIT_HASH_FULL: &str = env!("GIT_HASH_FULL");

#[derive(Deserialize, Clone)]
struct Contributor {
    login: String,
}

struct ContributorCache {
    fetched_at: Instant,
    data: Vec<Contributor>,
}

static CONTRIBUTORS_CACHE: LazyLock<Mutex<Option<ContributorCache>>> =
    LazyLock::new(|| Mutex::new(None));

struct DonatorCache {
    fetched_at: Instant,
    data: TextComponent,
}

static DONATORS_CACHE: LazyLock<Mutex<Option<DonatorCache>>> = LazyLock::new(|| Mutex::new(None));

fn fetch_all_contributors_cached() -> Vec<Contributor> {
    if let Ok(guard) = CONTRIBUTORS_CACHE.lock()
        && let Some(cache) = guard.as_ref()
        && cache.fetched_at.elapsed() < CACHE_DURATION
    {
        return cache.data.clone();
    }

    let contributors = tokio::runtime::Handle::try_current().map_or_else(
        |_| {
            tokio::runtime::Runtime::new()
                .map_or_else(|_| Vec::new(), |rt| rt.block_on(fetch_all_contributors()))
        },
        |handle| tokio::task::block_in_place(|| handle.block_on(fetch_all_contributors())),
    );

    if !contributors.is_empty() {
        if let Ok(mut guard) = CONTRIBUTORS_CACHE.lock() {
            *guard = Some(ContributorCache {
                fetched_at: Instant::now(),
                data: contributors.clone(),
            });
        }
    } else if let Ok(guard) = CONTRIBUTORS_CACHE.lock()
        && let Some(cache) = guard.as_ref()
    {
        return cache.data.clone();
    }

    contributors
}

async fn fetch_all_contributors() -> Vec<Contributor> {
    let client = crate::http_client::client_builder()
        .user_agent("Papokin")
        .build()
        .unwrap_or_default();

    let mut all_contributors = Vec::new();
    let mut next_url = Some(
        "https://api.github.com/repos/Quotidietium/Papokin/contributors?per_page=100".to_string(),
    );

    while let Some(url) = next_url {
        let response = client.get(&url).send().await;

        match response {
            Ok(res) => {
                let link_header = res
                    .headers()
                    .get("link")
                    .and_then(|s| s.to_str().ok())
                    .map(String::from);

                if let Ok(contributors) = res.json::<Vec<Contributor>>().await {
                    all_contributors.extend(contributors);
                } else {
                    break;
                }

                next_url = link_header.as_deref().and_then(extract_next_url);
            }
            Err(_) => break,
        }
    }

    if all_contributors.is_empty() {
        return vec![];
    }

    all_contributors
}

fn extract_next_url(header: &str) -> Option<String> {
    header
        .split(',')
        .find(|part| part.contains("rel=\"next\""))
        .and_then(|part| {
            let start = part.find('<')? + 1;
            let end = part.find('>')?;
            Some(part[start..end].to_string())
        })
}

#[derive(Deserialize)]
struct Donator {
    name: String,
    tier: String,
    tier_slug: String,
}

#[derive(Deserialize)]
struct DonatorResponse {
    current: Vec<Donator>,
    #[allow(dead_code)]
    past: Vec<Donator>,
}

fn tier_priority(tier_slug: &str, tier_name: &str) -> u32 {
    let s = format!("{tier_slug} {tier_name}").to_lowercase();

    if s.contains("corporate platinum") || s.contains("corp platinum") {
        1
    } else if s.contains("corporate gold") || s.contains("corp gold") {
        2
    } else if s.contains("corporate silver") || s.contains("corp silver") {
        3
    } else if s.contains("corporate bronze") || s.contains("corp bronze") {
        4
    } else if s.contains("diamond") {
        5
    } else if s.contains("titanium") {
        6
    } else if s.contains("platinum") {
        7
    } else if s.contains("gold") {
        8
    } else if s.contains("silver") {
        9
    } else if s.contains("bronze") {
        10
    } else {
        11
    }
}

fn tier_color(tier_slug: &str, tier_name: &str) -> NamedColor {
    let s = format!("{tier_slug} {tier_name}").to_lowercase();

    if s.contains("diamond") {
        NamedColor::Aqua
    } else if s.contains("titanium") {
        NamedColor::DarkAqua
    } else if s.contains("platinum") {
        NamedColor::LightPurple
    } else if s.contains("gold") {
        NamedColor::Gold
    } else if s.contains("silver") {
        NamedColor::Gray
    } else if s.contains("bronze") {
        NamedColor::DarkRed
    } else {
        NamedColor::White
    }
}

async fn fetch_donators_hover() -> TextComponent {
    let url = "https://market.pumpkinmc.org/api/v1/rest/donators";
    let client = crate::http_client::client_builder()
        .user_agent("Papokin")
        .build()
        .unwrap_or_default();
    let response = client.get(url).send().await;

    let mut donators_text = TextComponent::text("点击打开捐赠页面\n\n捐赠者：\n");

    if let Ok(res) = response
        && let Ok(data) = res.json::<DonatorResponse>().await
    {
        let mut current = data.current;
        current.sort_by(|a, b| {
            let prio_a = tier_priority(&a.tier_slug, &a.tier);
            let prio_b = tier_priority(&b.tier_slug, &b.tier);
            prio_a
                .cmp(&prio_b)
                .then_with(|| a.tier.cmp(&b.tier))
                .then_with(|| a.name.cmp(&b.name))
        });

        if current.is_empty() {
            donators_text = donators_text.add_child(TextComponent::text("暂无活跃的捐赠者"));
        } else {
            let mut current_tier = String::new();
            for donator in current {
                if donator.tier != current_tier {
                    current_tier.clone_from(&donator.tier);
                    let color = tier_color(&donator.tier_slug, &donator.tier);
                    donators_text = donators_text.add_child(
                        TextComponent::text(format!("\n{current_tier}:\n"))
                            .color_named(color)
                            .bold(),
                    );
                }
                let color = tier_color(&donator.tier_slug, &donator.tier);
                donators_text = donators_text.add_child(
                    TextComponent::text(format!("  • {}\n", donator.name)).color_named(color),
                );
            }
        }
        return donators_text;
    }

    donators_text.add_child(TextComponent::text("无法加载捐赠者列表"))
}

fn fetch_donators_hover_cached() -> TextComponent {
    if let Ok(guard) = DONATORS_CACHE.lock()
        && let Some(cache) = guard.as_ref()
        && cache.fetched_at.elapsed() < CACHE_DURATION
    {
        return cache.data.clone();
    }

    let donators = tokio::runtime::Handle::try_current().map_or_else(
        |_| {
            tokio::runtime::Runtime::new().map_or_else(
                |_| TextComponent::text("无法加载捐赠者列表"),
                |rt| rt.block_on(fetch_donators_hover()),
            )
        },
        |handle| tokio::task::block_in_place(|| handle.block_on(fetch_donators_hover())),
    );

    if let Ok(mut guard) = DONATORS_CACHE.lock() {
        *guard = Some(DonatorCache {
            fetched_at: Instant::now(),
            data: donators.clone(),
        });
    }

    donators
}

#[expect(clippy::too_many_lines)]
impl CommandExecutor for Executor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let contributors = fetch_all_contributors_cached();
        let contributor_names = contributors
            .iter()
            .map(|c| c.login.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let locale = context.source.output.get_locale();
        let profile = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };
        let version_string = format!(
            "{}（提交：{}/{}）- {} 位贡献者",
            CARGO_PKG_VERSION,
            GIT_HASH,
            profile,
            contributors.len()
        );
        let mut msg = TextComponent::text("");

        let version_translation = get_translation_text(
            "papokin:commands.papokin.version",
            locale,
            vec![TextComponent::text(version_string).0],
        );
        msg = msg.add_child(
            TextComponent::text(version_translation.clone())
                .hover_event(HoverEvent::show_text(
                    TextComponent::text(format!("提交：{GIT_HASH_FULL}\n\n贡献者：\n")).add_child(
                        TextComponent::text(contributor_names)
                            .gradient_named(&[NamedColor::DarkGreen, NamedColor::Green])
                            .new_line(),
                    ),
                ))
                .click_event(ClickEvent::CopyToClipboard {
                    value: Cow::from(version_translation.replace('\n', "")),
                })
                .color_named(NamedColor::Green),
        );

        let desc_translation =
            get_translation_text("papokin:commands.papokin.description", locale, vec![]);
        let desc_hover_translation =
            get_translation_text("papokin:commands.papokin.description.hover", locale, vec![]);
        msg = msg.add_child(
            TextComponent::text(desc_translation.clone())
                .click_event(ClickEvent::CopyToClipboard {
                    value: Cow::from(desc_translation.replace('\n', "")),
                })
                .hover_event(HoverEvent::show_text(TextComponent::text(
                    desc_hover_translation,
                )))
                .color_named(NamedColor::White),
        );

        let mc_version_translation = get_translation_text(
            "papokin:commands.papokin.minecraft_version",
            locale,
            vec![
                TextComponent::text(CURRENT_MC_VERSION.to_string()).0,
                TextComponent::text(CURRENT_MC_VERSION.protocol_version().to_string()).0,
            ],
        );
        let mc_version_hover_translation = get_translation_text(
            "papokin:commands.papokin.minecraft_version.hover",
            locale,
            vec![],
        );
        msg = msg.add_child(
            TextComponent::text(mc_version_translation.clone())
                .click_event(ClickEvent::CopyToClipboard {
                    value: Cow::from(mc_version_translation.replace('\n', "")),
                })
                .hover_event(HoverEvent::show_text(TextComponent::text(
                    mc_version_hover_translation,
                )))
                .color_named(NamedColor::Gold),
        );

        let github_translation =
            get_translation_text("papokin:commands.papokin.github", locale, vec![]);
        let github_hover_translation =
            get_translation_text("papokin:commands.papokin.github.hover", locale, vec![]);
        msg = msg.add_child(
            TextComponent::text(github_translation)
                .click_event(ClickEvent::OpenUrl {
                    url: Cow::from("https://github.com/Quotidietium/Papokin"),
                })
                .hover_event(HoverEvent::show_text(TextComponent::text(
                    github_hover_translation,
                )))
                .color_named(NamedColor::Blue)
                .bold()
                .underlined(),
        );

        msg = msg.add_child(TextComponent::text("  "));

        let donators_hover = fetch_donators_hover_cached();
        msg = msg.add_child(
            TextComponent::text("[捐赠]")
                .click_event(ClickEvent::OpenUrl {
                    url: Cow::from("https://pumpkinmc.org/donate/"),
                })
                .hover_event(HoverEvent::show_text(donators_hover))
                .rainbow()
                .bold()
                .underlined(),
        );

        msg = msg.add_child(TextComponent::text("  "));

        let website_translation =
            get_translation_text("papokin:commands.papokin.website", locale, vec![]);
        let website_hover_translation =
            get_translation_text("papokin:commands.papokin.website.hover", locale, vec![]);
        msg = msg.add_child(
            TextComponent::text(website_translation)
                .click_event(ClickEvent::OpenUrl {
                    url: Cow::from("https://pumpkinmc.org/"),
                })
                .hover_event(HoverEvent::show_text(TextComponent::text(
                    website_hover_translation,
                )))
                .color_named(NamedColor::Blue)
                .bold()
                .underlined(),
        );

        context.source.send_message(msg);

        // 返回……的数量完全合理
        // 以 i32 形式作为此命令的结果返回。
        Ok(contributors.len() as i32)
    }
}

pub fn register(dispatcher: &mut CommandDispatcher, registry: &PermissionRegistry) {
    registry.register_permission_or_panic(Permission::new(
        PERMISSION,
        DESCRIPTION,
        PermissionDefault::Op(PermissionLvl::Zero),
    ));

    for name in NAMES {
        dispatcher.register(
            command(name, DESCRIPTION)
                .requires(PERMISSION)
                .executes(Executor),
        );
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn cache_duration_is_24_hours() {
        assert_eq!(CACHE_DURATION, Duration::from_hours(24));
    }

    #[test]
    fn contributor_cache_updates_and_retrieves() {
        let mut guard = CONTRIBUTORS_CACHE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = Some(ContributorCache {
            fetched_at: Instant::now(),
            data: vec![Contributor {
                login: "test_user".to_string(),
            }],
        });
        drop(guard);

        let contributors = fetch_all_contributors_cached();
        assert_eq!(contributors.len(), 1);
        assert_eq!(contributors[0].login, "test_user");
    }

    #[test]
    fn donator_cache_updates_and_retrieves() {
        let expected = TextComponent::text("缓存捐赠者测试");
        let mut guard = DONATORS_CACHE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = Some(DonatorCache {
            fetched_at: Instant::now(),
            data: expected.clone(),
        });
        drop(guard);

        let cached = fetch_donators_hover_cached();
        assert_eq!(cached, expected);
    }
}
