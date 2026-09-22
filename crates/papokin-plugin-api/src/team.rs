use crate::wit::papokin::plugin::common::NamedColor;
use crate::wit::papokin::plugin::player::Player;
use crate::wit::papokin::plugin::scoreboard::{
    CollisionRule, NametagVisibility, Scoreboard, TeamSettings,
};
use crate::wit::papokin::plugin::text::TextComponent;

/// 用于构建 [`TeamSettings`] 的构建器。
pub struct TeamSettingsBuilder {
    display_name: Option<TextComponent>,
    friendly_fire: bool,
    see_friendly_invisibles: bool,
    nametag_visibility: NametagVisibility,
    collision_rule: CollisionRule,
    color: NamedColor,
    prefix: Option<TextComponent>,
    suffix: Option<TextComponent>,
}

impl Default for TeamSettingsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TeamSettingsBuilder {
    /// 以默认设置创建新的 `TeamSettingsBuilder`。
    #[must_use]
    pub fn new() -> Self {
        Self {
            display_name: None,
            friendly_fire: true,
            see_friendly_invisibles: false,
            nametag_visibility: NametagVisibility::Always,
            collision_rule: CollisionRule::Always,
            color: NamedColor::White,
            prefix: None,
            suffix: None,
        }
    }

    /// 设置队伍的显示名称。
    #[must_use]
    pub fn display_name(mut self, name: impl Into<TextComponent>) -> Self {
        self.display_name = Some(name.into());
        self
    }

    /// 设置该队伍成员是否启用友军伤害。
    #[must_use]
    pub fn friendly_fire(mut self, allow: bool) -> Self {
        self.friendly_fire = allow;
        self
    }

    /// 设置队友能否看到隐身的友方玩家。
    #[must_use]
    pub fn see_friendly_invisibles(mut self, see: bool) -> Self {
        self.see_friendly_invisibles = see;
        self
    }

    /// 设置该队伍的名称标签可见性。
    #[must_use]
    pub fn nametag_visibility(mut self, vis: NametagVisibility) -> Self {
        self.nametag_visibility = vis;
        self
    }

    /// 设置该队伍成员的碰撞规则。
    #[must_use]
    pub fn collision_rule(mut self, rule: CollisionRule) -> Self {
        self.collision_rule = rule;
        self
    }

    /// 设置该队伍的显示颜色与发光颜色。
    #[must_use]
    pub fn color(mut self, color: NamedColor) -> Self {
        self.color = color;
        self
    }

    /// 设置显示在成员名称之前的前缀。
    #[must_use]
    pub fn prefix(mut self, prefix: impl Into<TextComponent>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// 设置显示在成员名称之后的后缀。
    #[must_use]
    pub fn suffix(mut self, suffix: impl Into<TextComponent>) -> Self {
        self.suffix = Some(suffix.into());
        self
    }

    /// 构建 [`TeamSettings`]。
    #[must_use]
    pub fn build(self) -> TeamSettings {
        TeamSettings {
            display_name: self.display_name.unwrap_or_else(|| TextComponent::text("")),
            friendly_fire: self.friendly_fire,
            see_friendly_invisibles: self.see_friendly_invisibles,
            nametag_visibility: self.nametag_visibility,
            collision_rule: self.collision_rule,
            color: self.color,
            prefix: self.prefix.unwrap_or_else(|| TextComponent::text("")),
            suffix: self.suffix.unwrap_or_else(|| TextComponent::text("")),
        }
    }
}

/// 记分板队伍的高层表示。
pub struct Team<'a> {
    scoreboard: &'a Scoreboard,
    name: String,
}

impl<'a> Team<'a> {
    /// 创建给定记分板上某队伍的句柄。
    #[must_use]
    pub fn new(scoreboard: &'a Scoreboard, name: impl Into<String>) -> Self {
        Self {
            scoreboard,
            name: name.into(),
        }
    }

    /// 返回队伍的内部标识名称。
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 获取此队伍的当前设置（若记分板上存在）。
    #[must_use]
    pub fn get_settings(&self) -> Option<TeamSettings> {
        self.scoreboard.get_team(&self.name)
    }

    /// 在记分板上更新此队伍的设置。
    pub fn update_settings(&self, settings: TeamSettings) {
        self.scoreboard.update_team(&self.name, settings);
    }

    /// 获取此队伍的显示名。
    #[must_use]
    pub fn display_name(&self) -> Option<TextComponent> {
        self.get_settings().map(|s| s.display_name)
    }

    /// 设置该队伍的显示名称。
    pub fn set_display_name(&self, name: TextComponent) {
        if let Some(mut s) = self.get_settings() {
            s.display_name = name;
            self.update_settings(s);
        }
    }

    /// 获取此队伍的前缀。
    #[must_use]
    pub fn prefix(&self) -> Option<TextComponent> {
        self.get_settings().map(|s| s.prefix)
    }

    /// 设置该队伍的前缀。
    pub fn set_prefix(&self, prefix: TextComponent) {
        if let Some(mut s) = self.get_settings() {
            s.prefix = prefix;
            self.update_settings(s);
        }
    }

    /// 获取此队伍的后缀。
    #[must_use]
    pub fn suffix(&self) -> Option<TextComponent> {
        self.get_settings().map(|s| s.suffix)
    }

    /// 设置该队伍的后缀。
    pub fn set_suffix(&self, suffix: TextComponent) {
        if let Some(mut s) = self.get_settings() {
            s.suffix = suffix;
            self.update_settings(s);
        }
    }

    /// 获取队伍颜色。
    #[must_use]
    pub fn color(&self) -> Option<NamedColor> {
        self.get_settings().map(|s| s.color)
    }

    /// 设置队伍颜色。
    pub fn set_color(&self, color: NamedColor) {
        if let Some(mut s) = self.get_settings() {
            s.color = color;
            self.update_settings(s);
        }
    }

    /// 获取是否启用友军伤害。
    #[must_use]
    pub fn allow_friendly_fire(&self) -> bool {
        self.get_settings().map_or(true, |s| s.friendly_fire)
    }

    /// 设置是否启用友军伤害。
    pub fn set_allow_friendly_fire(&self, allow: bool) {
        if let Some(mut s) = self.get_settings() {
            s.friendly_fire = allow;
            self.update_settings(s);
        }
    }

    /// 获取队友能否看见友方隐身玩家。
    #[must_use]
    pub fn can_see_friendly_invisibles(&self) -> bool {
        self.get_settings()
            .map_or(false, |s| s.see_friendly_invisibles)
    }

    /// 设置队友能否看到友方的隐身玩家。
    pub fn set_can_see_friendly_invisibles(&self, see: bool) {
        if let Some(mut s) = self.get_settings() {
            s.see_friendly_invisibles = see;
            self.update_settings(s);
        }
    }

    /// 获取此队伍的命名牌可见性。
    #[must_use]
    pub fn nametag_visibility(&self) -> Option<NametagVisibility> {
        self.get_settings().map(|s| s.nametag_visibility)
    }

    /// 设置该队伍的名称标签可见性。
    pub fn set_nametag_visibility(&self, vis: NametagVisibility) {
        if let Some(mut s) = self.get_settings() {
            s.nametag_visibility = vis;
            self.update_settings(s);
        }
    }

    /// 获取此队伍的碰撞规则。
    #[must_use]
    pub fn collision_rule(&self) -> Option<CollisionRule> {
        self.get_settings().map(|s| s.collision_rule)
    }

    /// 设置该队伍的碰撞规则。
    pub fn set_collision_rule(&self, rule: CollisionRule) {
        if let Some(mut s) = self.get_settings() {
            s.collision_rule = rule;
            self.update_settings(s);
        }
    }

    ///返回该队伍中所有玩家/实体名称的列表。
    #[must_use]
    pub fn get_players(&self) -> Vec<String> {
        self.scoreboard.get_team_players(&self.name)
    }

    /// 将玩家或实体名加入此队伍。
    pub fn add_player(&self, player_name: &str) {
        self.scoreboard.add_player_to_team(&self.name, player_name);
    }

    /// 从此队伍中移除一个玩家或实体名称。
    pub fn remove_player(&self, player_name: &str) {
        self.scoreboard
            .remove_player_from_team(&self.name, player_name);
    }

    /// 检查玩家或实体名是否在此队伍中。
    #[must_use]
    pub fn has_player(&self, player_name: &str) -> bool {
        self.get_players().iter().any(|p| p == player_name)
    }

    /// 移除此队伍的所有成员。
    pub fn clear_players(&self) {
        self.scoreboard.clear_team_players(&self.name);
    }

    /// 从记分板中移除该队伍。
    pub fn unregister(self) {
        self.scoreboard.remove_team(&self.name);
    }
}

/// 为 [`Scoreboard`] 提供队伍操作的扩展 trait。
pub trait ScoreboardTeamExt {
    /// 在记分板上注册并创建一个新队伍。
    fn register_new_team(&self, name: &str, settings: TeamSettings) -> Team<'_>;
    /// 按名称获取队伍（若记分板上存在）。
    fn get_team_handle(&self, name: &str) -> Option<Team<'_>>;
    ///返回记分板上的所有队伍。
    fn get_all_teams(&self) -> Vec<Team<'_>>;
    /// 获取玩家所属的队伍（如果有）。
    fn get_player_team_handle(&self, player_name: &str) -> Option<Team<'_>>;
}

impl ScoreboardTeamExt for Scoreboard {
    fn register_new_team(&self, name: &str, settings: TeamSettings) -> Team<'_> {
        self.create_team(name, settings);
        Team::new(self, name)
    }

    fn get_team_handle(&self, name: &str) -> Option<Team<'_>> {
        self.get_team(name).is_some().then(|| Team::new(self, name))
    }

    fn get_all_teams(&self) -> Vec<Team<'_>> {
        self.get_teams()
            .into_iter()
            .map(|name| Team::new(self, name))
            .collect()
    }

    fn get_player_team_handle(&self, player_name: &str) -> Option<Team<'_>> {
        self.get_player_team(player_name)
            .map(|name| Team::new(self, name))
    }
}

/// 为 [`Player`] 提供队伍操作的扩展 trait。
pub trait PlayerTeamExt {
    /// 获取此玩家当前所在队伍名（如果有）。
    fn get_team_name(&self) -> Option<String>;
}

impl PlayerTeamExt for Player {
    fn get_team_name(&self) -> Option<String> {
        self.get_team()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn team_settings_builder_defaults() {
        let dummy_text: TextComponent = unsafe { std::mem::zeroed() };
        let dummy_text2: TextComponent = unsafe { std::mem::zeroed() };
        let dummy_text3: TextComponent = unsafe { std::mem::zeroed() };
        let settings = TeamSettingsBuilder::new()
            .display_name(dummy_text)
            .color(NamedColor::Red)
            .prefix(dummy_text2)
            .suffix(dummy_text3)
            .friendly_fire(false)
            .see_friendly_invisibles(true)
            .nametag_visibility(NametagVisibility::HideForOtherTeams)
            .collision_rule(CollisionRule::PushOwnTeam)
            .build();

        assert!(!settings.friendly_fire);
        assert!(settings.see_friendly_invisibles);
        assert_eq!(settings.color, NamedColor::Red);
        assert_eq!(
            settings.nametag_visibility,
            NametagVisibility::HideForOtherTeams
        );
        assert_eq!(settings.collision_rule, CollisionRule::PushOwnTeam);
        std::mem::forget(settings);
    }
}
