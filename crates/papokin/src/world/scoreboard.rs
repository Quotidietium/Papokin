use rustc_hash::FxHashMap;

use papokin_data::scoreboard::ScoreboardDisplaySlot;
use papokin_protocol::{
    ClientPacket, NumberFormat,
    codec::var_int::VarInt,
    java::client::play::{
        CDisplayObjective, CResetScore, CSetPlayerTeam, CUpdateObjectives, CUpdateScore, Mode,
        RenderType, TeamMethod, TeamParameters,
    },
};
use papokin_util::text::{TextComponent, color::NamedColor};
use tracing::warn;

use super::World;
use crate::entity::player::Player;

pub trait ScoreboardTarget: Send + Sync {
    fn send_je<J: ClientPacket + Sync>(&self, je_packet: &J);
}

impl ScoreboardTarget for World {
    fn send_je<J: ClientPacket + Sync>(&self, je_packet: &J) {
        self.broadcast_packet_all(je_packet);
    }
}

impl<T: ScoreboardTarget + ?Sized> ScoreboardTarget for &T {
    fn send_je<J: ClientPacket + Sync>(&self, je_packet: &J) {
        (*self).send_je(je_packet);
    }
}

impl ScoreboardTarget for std::sync::Arc<World> {
    fn send_je<J: ClientPacket + Sync>(&self, je_packet: &J) {
        self.broadcast_packet_all(je_packet);
    }
}

impl ScoreboardTarget for Player {
    fn send_je<J: ClientPacket + Sync>(&self, je_packet: &J) {
        self.try_send_client_packet(je_packet);
    }
}

impl ScoreboardTarget for std::sync::Arc<Player> {
    fn send_je<J: ClientPacket + Sync>(&self, je_packet: &J) {
        self.try_send_client_packet(je_packet);
    }
}

pub struct NoTarget;

impl ScoreboardTarget for NoTarget {
    fn send_je<J: ClientPacket + Sync>(&self, _je_packet: &J) {}
}

#[derive(Clone, Debug, Default)]
pub struct Scoreboard {
    objectives: FxHashMap<String, ScoreboardObjective>,
    display_slots: FxHashMap<ScoreboardDisplaySlot, String>,
    scores: FxHashMap<String, FxHashMap<String, ScoreboardScore>>,
    teams: FxHashMap<String, Team>,
}

impl Scoreboard {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn get_objectives(&self) -> &FxHashMap<String, ScoreboardObjective> {
        &self.objectives
    }

    #[must_use]
    pub fn get_objective(&self, name: &str) -> Option<&ScoreboardObjective> {
        self.objectives.get(name)
    }

    pub fn get_objective_mut(&mut self, name: &str) -> Option<&mut ScoreboardObjective> {
        self.objectives.get_mut(name)
    }

    #[must_use]
    pub const fn get_display_slots(&self) -> &FxHashMap<ScoreboardDisplaySlot, String> {
        &self.display_slots
    }

    #[must_use]
    pub fn get_display_objective(&self, slot: ScoreboardDisplaySlot) -> Option<&str> {
        self.display_slots.get(&slot).map(String::as_str)
    }

    #[must_use]
    pub const fn get_scores(&self) -> &FxHashMap<String, FxHashMap<String, ScoreboardScore>> {
        &self.scores
    }

    #[must_use]
    pub fn get_score(&self, entity_name: &str, objective_name: &str) -> Option<&ScoreboardScore> {
        self.scores.get(objective_name)?.get(entity_name)
    }

    #[must_use]
    pub fn get_score_value(&self, entity_name: &str, objective_name: &str) -> Option<i32> {
        self.get_score(entity_name, objective_name)
            .map(|s| s.value.0)
    }

    #[must_use]
    pub fn get_scores_for_objective(
        &self,
        objective_name: &str,
    ) -> Option<&FxHashMap<String, ScoreboardScore>> {
        self.scores.get(objective_name)
    }

    #[must_use]
    pub fn get_scores_for_entity(&self, entity_name: &str) -> FxHashMap<String, &ScoreboardScore> {
        let mut entity_scores = FxHashMap::default();
        for (obj_name, obj_scores) in &self.scores {
            if let Some(score) = obj_scores.get(entity_name) {
                entity_scores.insert(obj_name.clone(), score);
            }
        }
        entity_scores
    }

    #[must_use]
    pub const fn get_teams(&self) -> &FxHashMap<String, Team> {
        &self.teams
    }

    #[must_use]
    pub fn get_team(&self, name: &str) -> Option<&Team> {
        self.teams.get(name)
    }

    pub fn get_team_mut(&mut self, name: &str) -> Option<&mut Team> {
        self.teams.get_mut(name)
    }

    #[must_use]
    pub fn get_entity_team(&self, entity_name: &str) -> Option<&Team> {
        self.teams
            .values()
            .find(|team| team.players.iter().any(|p| p == entity_name))
    }

    pub fn add_objective(
        &mut self,
        target: &impl ScoreboardTarget,
        objective: ScoreboardObjective,
    ) {
        if self.objectives.contains_key(&objective.name) {
            warn!("尝试创建已存在的记分项：{}", &objective.name);
            return;
        }

        let je_update = CUpdateObjectives::new(
            objective.name.clone(),
            Mode::Add,
            objective.display_name.clone(),
            objective.render_type,
            objective.number_format.clone(),
        );

        target.send_je(&je_update);

        self.objectives.insert(objective.name.clone(), objective);
    }

    pub fn update_objective(
        &mut self,
        target: &impl ScoreboardTarget,
        objective: ScoreboardObjective,
    ) {
        if !self.objectives.contains_key(&objective.name) {
            warn!("尝试更新不存在的记分项：{}", &objective.name);
            return;
        }

        let je_update = CUpdateObjectives::new(
            objective.name.clone(),
            Mode::Update,
            objective.display_name.clone(),
            objective.render_type,
            objective.number_format.clone(),
        );

        target.send_je(&je_update);

        self.objectives.insert(objective.name.clone(), objective);
    }

    pub fn set_display_objective(
        &mut self,
        target: &impl ScoreboardTarget,
        slot: ScoreboardDisplaySlot,
        objective_name: Option<&str>,
    ) {
        let obj_name_str = objective_name.unwrap_or("");

        let je_display = CDisplayObjective::new(slot, obj_name_str.to_string());

        target.send_je(&je_display);

        if let Some(name) = objective_name {
            self.display_slots.insert(slot, name.to_string());
        } else {
            self.display_slots.remove(&slot);
        }
    }

    pub fn clear_display_objective(
        &mut self,
        target: &impl ScoreboardTarget,
        slot: ScoreboardDisplaySlot,
    ) {
        self.set_display_objective(target, slot, None);
    }

    pub fn remove_objective(&mut self, target: &impl ScoreboardTarget, name: &str) {
        if !self.objectives.contains_key(name) {
            warn!("尝试移除不存在的记分项：{}", name);
            return;
        }

        let je_packet = CUpdateObjectives::new(
            name.to_string(),
            Mode::Remove,
            TextComponent::empty(),
            RenderType::Integer,
            None,
        );

        target.send_je(&je_packet);

        self.objectives.remove(name);
        self.scores.remove(name);
        self.display_slots.retain(|_, obj| obj != name);
    }

    pub fn update_score(&mut self, target: &impl ScoreboardTarget, score: ScoreboardScore) {
        if !self.objectives.contains_key(&score.objective_name) {
            warn!("尝试向不存在的记分项写入分数：{}", &score.objective_name);
            return;
        }

        let je_packet = CUpdateScore::new(
            score.entity_name.clone(),
            score.objective_name.clone(),
            score.value,
            score.display_name.clone(),
            score.number_format.clone(),
        );

        target.send_je(&je_packet);

        self.scores
            .entry(score.objective_name.clone())
            .or_default()
            .insert(score.entity_name.clone(), score);
    }

    pub fn set_score_value(
        &mut self,
        target: &impl ScoreboardTarget,
        entity_name: impl Into<String>,
        objective_name: impl Into<String>,
        value: i32,
    ) {
        let entity_s = entity_name.into();
        let obj_s = objective_name.into();
        let existing = self.get_score(&entity_s, &obj_s).cloned();
        let score = ScoreboardScore {
            entity_name: entity_s,
            objective_name: obj_s,
            value: VarInt(value),
            display_name: existing.as_ref().and_then(|s| s.display_name.clone()),
            number_format: existing.as_ref().and_then(|s| s.number_format.clone()),
            locked: existing.as_ref().is_none_or(|s| s.locked),
        };
        self.update_score(target, score);
    }

    pub fn add_score(
        &mut self,
        target: &impl ScoreboardTarget,
        entity_name: impl Into<String>,
        objective_name: impl Into<String>,
        delta: i32,
    ) -> i32 {
        let entity_s = entity_name.into();
        let obj_s = objective_name.into();
        let current_val = self.get_score_value(&entity_s, &obj_s).unwrap_or(0);
        let new_val = current_val + delta;
        self.set_score_value(target, entity_s, obj_s, new_val);
        new_val
    }

    pub fn remove_score(
        &mut self,
        target: &impl ScoreboardTarget,
        entity_name: &str,
        objective_name: &str,
    ) {
        let je_packet = CResetScore::new(entity_name.to_string(), Some(objective_name.to_string()));

        target.send_je(&je_packet);

        if let Some(objective_scores) = self.scores.get_mut(objective_name) {
            objective_scores.remove(entity_name);
        }
    }

    pub fn reset_scores_for_entity(&mut self, target: &impl ScoreboardTarget, entity_name: &str) {
        let je_packet = CResetScore::new(entity_name.to_string(), None);

        target.send_je(&je_packet);

        for obj_scores in self.scores.values_mut() {
            obj_scores.remove(entity_name);
        }
    }

    pub fn add_team(&mut self, target: &impl ScoreboardTarget, team: Team) {
        if self.teams.contains_key(&team.name) {
            warn!("尝试创建已存在的队伍 {}", team.name);
            return;
        }

        let parameters = TeamParameters {
            display_name: &team.display_name,
            options: team.options,
            nametag_visibility: team.nametag_visibility.to_str(),
            collision_rule: team.collision_rule.to_str(),
            color: team.color as i32,
            player_prefix: &team.player_prefix,
            player_suffix: &team.player_suffix,
        };

        target.send_je(&CSetPlayerTeam {
            team_name: team.name.clone(),
            method: TeamMethod::Create,
            parameters: Some(parameters),
            players: team.players.clone().into(),
        });

        self.teams.insert(team.name.clone(), team);
    }

    pub fn create_team(&mut self, target: &impl ScoreboardTarget, team: Team) {
        self.add_team(target, team);
    }

    pub fn update_team(&mut self, target: &impl ScoreboardTarget, team: Team) {
        if !self.teams.contains_key(&team.name) {
            warn!("尝试更新不存在的队伍 {}", team.name);
            return;
        }

        let parameters = TeamParameters {
            display_name: &team.display_name,
            options: team.options,
            nametag_visibility: team.nametag_visibility.to_str(),
            collision_rule: team.collision_rule.to_str(),
            color: team.color as i32,
            player_prefix: &team.player_prefix,
            player_suffix: &team.player_suffix,
        };

        target.send_je(&CSetPlayerTeam {
            team_name: team.name.clone(),
            method: TeamMethod::Update,
            parameters: Some(parameters),
            players: Box::new([]),
        });

        self.teams.insert(team.name.clone(), team);
    }

    pub fn remove_team(&mut self, target: &impl ScoreboardTarget, name: &str) {
        if !self.teams.contains_key(name) {
            warn!("尝试移除不存在的队伍 {}", name);
            return;
        }

        target.send_je(&CSetPlayerTeam {
            team_name: name.to_string(),
            method: TeamMethod::Remove,
            parameters: None,
            players: Box::new([]),
        });

        self.teams.remove(name);
    }

    pub fn add_player_to_team(
        &mut self,
        target: &impl ScoreboardTarget,
        team_name: &str,
        player: String,
    ) {
        let Some(team) = self.teams.get_mut(team_name) else {
            warn!("尝试将玩家加入不存在的队伍 {}", team_name);
            return;
        };

        if team.players.contains(&player) {
            return;
        }

        target.send_je(&CSetPlayerTeam {
            team_name: team_name.to_string(),
            method: TeamMethod::AddPlayers,
            parameters: None,
            players: vec![player.clone()].into(),
        });

        team.players.push(player);
    }

    pub fn remove_player_from_team(
        &mut self,
        target: &impl ScoreboardTarget,
        team_name: &str,
        player: &str,
    ) {
        let Some(team) = self.teams.get_mut(team_name) else {
            warn!("尝试将玩家移出不存在的队伍 {}", team_name);
            return;
        };

        if !team.players.contains(&player.to_string()) {
            return;
        }

        target.send_je(&CSetPlayerTeam {
            team_name: team_name.to_string(),
            method: TeamMethod::RemovePlayers,
            parameters: None,
            players: vec![player.to_string()].into(),
        });

        team.players.retain(|p| p != player);
    }

    pub fn clear_team_players(&mut self, target: &impl ScoreboardTarget, team_name: &str) {
        let Some(team) = self.teams.get_mut(team_name) else {
            warn!("尝试清空不存在的队伍 {} 的成员", team_name);
            return;
        };

        if team.players.is_empty() {
            return;
        }

        let players_to_remove = team.players.clone();
        target.send_je(&CSetPlayerTeam {
            team_name: team_name.to_string(),
            method: TeamMethod::RemovePlayers,
            parameters: None,
            players: players_to_remove.into(),
        });

        team.players.clear();
    }

    pub fn send_to_player(&self, player: &Player) {
        for objective in self.objectives.values() {
            let je_update = CUpdateObjectives::new(
                objective.name.clone(),
                Mode::Add,
                objective.display_name.clone(),
                objective.render_type,
                objective.number_format.clone(),
            );
            player.try_send_client_packet(&je_update);
        }

        for (slot, objective_name) in &self.display_slots {
            let je_display = CDisplayObjective::new(*slot, objective_name.clone());
            player.try_send_client_packet(&je_display);
        }

        for objective_scores in self.scores.values() {
            for score in objective_scores.values() {
                let je_packet = CUpdateScore::new(
                    score.entity_name.clone(),
                    score.objective_name.clone(),
                    score.value,
                    score.display_name.clone(),
                    score.number_format.clone(),
                );
                player.try_send_client_packet(&je_packet);
            }
        }

        for team in self.teams.values() {
            let parameters = TeamParameters {
                display_name: &team.display_name,
                options: team.options,
                nametag_visibility: team.nametag_visibility.to_str(),
                collision_rule: team.collision_rule.to_str(),
                color: team.color as i32,
                player_prefix: &team.player_prefix,
                player_suffix: &team.player_suffix,
            };
            let je_packet = CSetPlayerTeam {
                team_name: team.name.clone(),
                method: TeamMethod::Create,
                parameters: Some(parameters),
                players: team.players.clone().into(),
            };
            player.try_send_client_packet(&je_packet);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoreboardObjective {
    pub name: String,
    pub display_name: TextComponent,
    pub render_type: RenderType,
    pub number_format: Option<NumberFormat>,
    pub criterion: String,
}

impl ScoreboardObjective {
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        display_name: TextComponent,
        render_type: RenderType,
        number_format: Option<NumberFormat>,
        criterion: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            display_name,
            render_type,
            number_format,
            criterion: criterion.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoreboardScore {
    pub entity_name: String,
    pub objective_name: String,
    pub value: VarInt,
    pub display_name: Option<TextComponent>,
    pub number_format: Option<NumberFormat>,
    pub locked: bool,
}

impl ScoreboardScore {
    #[must_use]
    pub fn new(
        entity_name: impl Into<String>,
        objective_name: impl Into<String>,
        value: VarInt,
        display_name: Option<TextComponent>,
        number_format: Option<NumberFormat>,
    ) -> Self {
        Self {
            entity_name: entity_name.into(),
            objective_name: objective_name.into(),
            value,
            display_name,
            number_format,
            locked: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameTagVisibility {
    Always,
    Never,
    HideForOtherTeams,
    HideForOwnTeam,
}

impl NameTagVisibility {
    #[must_use]
    pub const fn to_str(&self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::Never => "never",
            Self::HideForOtherTeams => "hideForOtherTeams",
            Self::HideForOwnTeam => "hideForOwnTeam",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollisionRule {
    Always,
    Never,
    PushOtherTeams,
    PushOwnTeam,
}

impl CollisionRule {
    #[must_use]
    pub const fn to_str(&self) -> &'static str {
        match self {
            Self::Always => "always",
            Self::Never => "never",
            Self::PushOtherTeams => "pushOtherTeams",
            Self::PushOwnTeam => "pushOwnTeam",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Team {
    pub name: String,
    pub display_name: TextComponent,
    pub options: i8,
    pub nametag_visibility: NameTagVisibility,
    pub collision_rule: CollisionRule,
    pub color: NamedColor,
    pub player_prefix: TextComponent,
    pub player_suffix: TextComponent,
    pub players: Vec<String>,
}

#[derive(Default, Debug)]
pub struct ScoreboardBuilder {
    scoreboard: Scoreboard,
}

impl ScoreboardBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn objective(mut self, name: impl Into<String>, display_name: TextComponent) -> Self {
        let obj = ScoreboardObjective::new(
            name.into(),
            display_name,
            RenderType::Integer,
            None,
            "dummy",
        );
        self.scoreboard.objectives.insert(obj.name.clone(), obj);
        self
    }

    #[must_use]
    pub fn objective_with_render(
        mut self,
        name: impl Into<String>,
        display_name: TextComponent,
        render_type: RenderType,
    ) -> Self {
        let obj = ScoreboardObjective::new(name.into(), display_name, render_type, None, "dummy");
        self.scoreboard.objectives.insert(obj.name.clone(), obj);
        self
    }

    #[must_use]
    pub fn display_slot(
        mut self,
        slot: ScoreboardDisplaySlot,
        objective_name: impl Into<String>,
    ) -> Self {
        self.scoreboard
            .display_slots
            .insert(slot, objective_name.into());
        self
    }

    #[must_use]
    pub fn score(
        mut self,
        entity_name: impl Into<String>,
        objective_name: impl Into<String>,
        value: i32,
    ) -> Self {
        let entity_s = entity_name.into();
        let obj_s = objective_name.into();
        let score =
            ScoreboardScore::new(entity_s.clone(), obj_s.clone(), VarInt(value), None, None);
        self.scoreboard
            .scores
            .entry(entity_s)
            .or_default()
            .insert(obj_s, score);
        self
    }

    #[must_use]
    pub fn team(mut self, team: Team) -> Self {
        self.scoreboard.teams.insert(team.name.clone(), team);
        self
    }

    #[must_use]
    pub fn build(self) -> Scoreboard {
        self.scoreboard
    }
}
