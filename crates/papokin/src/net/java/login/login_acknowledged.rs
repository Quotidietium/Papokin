#[allow(clippy::wildcard_imports)]
use super::*;

impl PendingConnection {
    pub async fn handle_login_acknowledged(
        &mut self,
        server: &Server,
    ) -> Option<PacketHandlerResult> {
        debug!("正在处理登录确认");
        // 仅当 CLoginSuccess 已发送后才允许确认配置切换。没有该守卫，
        // 改过的客户端可在 SLoginStart 之后跳过 SEncryptionResponse
        // （即跳过认证）直接进入配置阶段，以伪造的名称/UUID 进服。
        if !self
            .login_success_sent
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            tracing::warn!("连接 {} 未完成登录即发送登录确认包，已断开", self.id);
            self.kick(TextComponent::text("未完成登录")).await;
            return Some(PacketHandlerResult::Stop);
        }
        if !self.version.load().supports_configuration_state() {
            self.kick(TextComponent::text("此版本不支持配置状态")).await;
            return Some(PacketHandlerResult::Stop);
        }
        self.connection_state.store(ConnectionState::Config);
        self.send_packet_now(&server.get_branding()).await;

        if server.advanced_config.server_links.enabled
            && self.version.load() >= JavaMinecraftVersion::V_1_21
        {
            let mut links: Vec<Link> = Vec::new();

            let bug_report = &server.advanced_config.server_links.bug_report;
            if !bug_report.is_empty() {
                links.push(Link::new(Label::BuiltIn(LinkType::BugReport), bug_report));
            }

            let support = &server.advanced_config.server_links.support;
            if !support.is_empty() {
                links.push(Link::new(Label::BuiltIn(LinkType::Support), support));
            }

            let status = &server.advanced_config.server_links.status;
            if !status.is_empty() {
                links.push(Link::new(Label::BuiltIn(LinkType::Status), status));
            }

            let feedback = &server.advanced_config.server_links.feedback;
            if !feedback.is_empty() {
                links.push(Link::new(Label::BuiltIn(LinkType::Feedback), feedback));
            }

            let community = &server.advanced_config.server_links.community;
            if !community.is_empty() {
                links.push(Link::new(Label::BuiltIn(LinkType::Community), community));
            }

            let website = &server.advanced_config.server_links.website;
            if !website.is_empty() {
                links.push(Link::new(Label::BuiltIn(LinkType::Website), website));
            }

            let forums = &server.advanced_config.server_links.forums;
            if !forums.is_empty() {
                links.push(Link::new(Label::BuiltIn(LinkType::Forums), forums));
            }

            let news = &server.advanced_config.server_links.news;
            if !news.is_empty() {
                links.push(Link::new(Label::BuiltIn(LinkType::News), news));
            }

            let announcements = &server.advanced_config.server_links.announcements;
            if !announcements.is_empty() {
                links.push(Link::new(
                    Label::BuiltIn(LinkType::Announcements),
                    announcements,
                ));
            }

            for (key, value) in &server.advanced_config.server_links.custom {
                links.push(Link::new(
                    Label::TextComponent(TextComponent::text(key.clone()).into()),
                    value,
                ));
            }

            self.send_packet_now(&CConfigServerLinks::new(&links)).await;
        }

        let resource_config = &server.advanced_config.resource_pack.java;
        if resource_config.enabled {
            let uuid = Uuid::new_v3(&uuid::Uuid::NAMESPACE_DNS, resource_config.url.as_bytes());
            // 记录已下发的资源包 UUID：后续资源包响应必须与之匹配，
            // 防止伪造响应驱动配置流程推进（与 config 阶段同一校验）。
            self.resource_pack_id.store(Some(uuid));
            let resource_pack = CConfigAddResourcePack::new(
                &uuid,
                &resource_config.url,
                &resource_config.sha1,
                resource_config.force,
                if resource_config.prompt_message.is_empty() {
                    None
                } else {
                    Some(TextComponent::text(resource_config.prompt_message.clone()))
                },
            );

            self.send_packet_now(&resource_pack).await;
        } else if self.version.load() >= JavaMinecraftVersion::V_1_20_5 {
            self.send_known_packs(server).await;
        } else {
            self.handle_known_packs(server).await;
        }
        debug!("登录已确认");
        None
    }

    pub async fn send_known_packs(&mut self, server: &Server) {
        let features = server.get_enabled_features();
        self.send_packet_now(&CFeatureFlags::new(&features)).await;
        let version_str = self.version.load().to_string();
        let loaded_packs = server.datapack_manager.get_loaded_packs();
        let known_packs = server.get_known_packs(&version_str, &loaded_packs);
        self.send_packet_now(&CKnownPacks::new(&known_packs)).await;
    }
}
