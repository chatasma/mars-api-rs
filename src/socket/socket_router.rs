use std::{collections::HashMap};


use futures::future::join_all;
use mongodb::bson::doc;
use mongodb::options::UpdateOptions;
use rocket::serde::{json::{serde_json, Value}, DeserializeOwned};

use uuid::Uuid;

use crate::{database::models::{death::Death, achievement::Achievement, r#match::{FirstBlood, MatchState}, participant::{Participant, SimpleParticipant}, player::{AchievementData, Player}}, socket::r#match::match_phase_listener::MatchPhaseListener, util::{r#macro::unwrap_helper, time::get_u64_time_millis}};

use super::{event_type::EventType, leaderboard::leaderboard_listener::LeaderboardListener, map::map_record_listener::MapRecordListener, r#match::match_events::{MatchEndData, MatchStartData}, objective::objective_events::{ControlPointCaptureData, CoreLeakData, DestroyableDamageData, DestroyableDestroyData, FlagDropData, FlagEventData, WoolDropData, WoolEventData}, participant::{participant_party_listener::ParticipantPartyListener, participant_stat_listener::ParticipantStatListener}, player::{player_events::{KillstreakData, PartyJoinData, PartyLeaveData, PlayerAchievementData, PlayerChatData, PlayerDeathData}, player_gamemode_stat_listener::PlayerGamemodeStatListener, player_listener::PlayerListener, player_record_listener::PlayerRecordListener, player_stat_listener::PlayerStatListener, player_xp_listener::PlayerXPListener}, server::{server_context::ServerContext, server_events::MatchLoadData}, update::player_update_listener::PlayerUpdateListener};
use crate::database::Database;

pub struct SocketRouter {
    pub server: ServerContext,
    pub participant_listeners: Vec<Box<dyn PlayerListener<Context = Participant> + Send + Sync>>,
    pub player_listeners: Vec<Box<dyn PlayerListener<Context = Player> + Send + Sync>>
}

pub enum SocketError {
    InvalidMatchState,
    Unknown(String)
}

impl SocketError {
    fn message(&self) -> String {
        match self {
            Self::InvalidMatchState => String::from("Encountered invalid match state or missing match"),
            Self::Unknown(msg) => msg.clone()
        }
    }
}

impl SocketRouter {
    pub fn new(server_context: ServerContext) -> Self {
        Self { 
            server: server_context,
            participant_listeners: vec![
                Box::new(ParticipantStatListener {}),
                Box::new(ParticipantPartyListener {}),
                Box::new(MapRecordListener {}),
                Box::new(LeaderboardListener {})
            ],
            player_listeners: vec![
                Box::new(PlayerStatListener {}),
                Box::new(PlayerGamemodeStatListener {}),
                Box::new(PlayerXPListener {}),
                Box::new(PlayerRecordListener {}),
                Box::new(PlayerUpdateListener {}),
            ]
        }
    }

    pub async fn route(&mut self, event_type: &EventType, data: Value) {
        let response : anyhow::Result<(), SocketError> = match event_type {
            EventType::MatchLoad =>                             self.on_match_load(Self::parse_data(data)).await,
            EventType::MatchStart =>                            self.on_match_start(Self::parse_data(data)).await,
            EventType::MatchEnd =>                              self.on_match_end(Self::parse_data(data)).await,
            EventType::PlayerDeath =>                           self.on_player_death(Self::parse_data(data)).await,
            EventType::PlayerChat =>                            self.on_player_chat(Self::parse_data(data)).await,
            EventType::Killstreak =>                            self.on_killstreak(Self::parse_data(data)).await,
            EventType::PartyJoin =>                             self.on_party_join(Self::parse_data(data)).await,
            EventType::PartyLeave =>                            self.on_party_leave(Self::parse_data(data)).await,
            EventType::DestroyableDestroy =>                    self.on_destroyable_destroy(Self::parse_data(data)).await,
            EventType::DestroyableDamage =>                     self.on_destroyable_damage(Self::parse_data(data)).await,
            EventType::CoreLeak =>                              self.on_core_leak(Self::parse_data(data)).await,
            EventType::FlagCapture =>                           self.on_flag_place(Self::parse_data(data)).await,
            EventType::FlagPickup =>                            self.on_flag_pickup(Self::parse_data(data)).await,
            EventType::FlagDrop =>                              self.on_flag_drop(Self::parse_data(data)).await,
            EventType::FlagDefend =>                            self.on_flag_defend(Self::parse_data(data)).await,
            EventType::WoolCapture =>                           self.on_wool_place(Self::parse_data(data)).await,
            EventType::WoolPickup =>                            self.on_wool_pickup(Self::parse_data(data)).await,
            EventType::WoolDrop =>                              self.on_wool_drop(Self::parse_data(data)).await,
            EventType::WoolDefend =>                            self.on_wool_defend(Self::parse_data(data)).await,
            EventType::ControlPointCapture =>                   self.on_control_point_capture(Self::parse_data(data)).await,
            EventType::AchievementEarn =>                       self.on_achievement_complete(Self::parse_data(data)).await,
            _ => {warn!("Event (srv {}) fell through router: {} - {}", self.server.id, event_type, data.to_string()); return}
        };
        match response {
            Err(socket_error) => {
                match socket_error {
                    SocketError::InvalidMatchState => {
                        self.server.call(&EventType::ForceMatchEnd, ()).await;
                        let match_id = self.get_match_id().await;
                        warn!("Forcing match end for Match ID: {}. Caused by {}: {}", match_id, event_type.to_string(), socket_error.message());
                    },
                    SocketError::Unknown(_text) => {
                    }
                }
            }
            _ => {}
        };
    }

    async fn on_match_load(&mut self, data: MatchLoadData) -> Result<(), SocketError> {
        MatchPhaseListener { server: &mut self.server }.on_load(data).await
    }

    async fn on_match_start(&mut self, data: MatchStartData) -> Result<(), SocketError> {
        let mut current_match = unwrap_helper::return_default!(self.server.get_match().await, Err(SocketError::InvalidMatchState));
        if current_match.get_state() != MatchState::Pre {
            return Err(SocketError::InvalidMatchState);
        };
        current_match = match (MatchPhaseListener { server: &mut self.server }.on_start(data, current_match)) {
            Ok(current_match) => current_match,
            Err(socket_error) => return Err(socket_error)
        };
        self.server.api_state.match_cache.set(&self.server.api_state.database, &current_match.id, &current_match, false).await;
        Ok(())
    }

    async fn on_match_end(&mut self, mut data: MatchEndData) -> Result<(), SocketError> {
        let mut current_match = unwrap_helper::return_default!(self.server.get_match().await, Err(SocketError::InvalidMatchState));
        if current_match.get_state() != MatchState::InProgress {
            return Err(SocketError::InvalidMatchState);
        };
        current_match = match (MatchPhaseListener { server: &mut self.server }.on_end(&data, current_match)) {
            Ok(current_match) => current_match,
            Err(socket_error) => return Err(socket_error)
        };

        // swap to avoid partial move
        let participants = current_match.participants;
        current_match.participants = HashMap::new();

        let mut profiles : Vec<Player> = Vec::new();

        for (_participant_id, mut participant) in participants.into_iter() {
            {
                for participant_listener in self.participant_listeners.iter() {
                     participant_listener.on_match_end_v2(&mut self.server, &mut current_match, &mut participant, &mut data).await;
                };
                current_match.save_participants(vec![participant.clone()]);
            };
            let mut player = participant.get_player(&*self.server.api_state).await;
            {
                for player_listener in self.player_listeners.iter() {
                    player_listener.on_match_end_v2(&mut self.server, &mut current_match, &mut player, &mut data).await;
                };

                participant.set_player(&*self.server.api_state, &player).await;
            };
            profiles.push(player);
        }

        if profiles.len() > 0 {
            let mut tasks : Vec<_> = Vec::new();
            for profile in profiles.iter() {
                tasks.push(self.server.api_state.database.save(profile));
            };
            join_all(tasks).await;
        };

        {
            self.server.api_state.database.save(&current_match.level).await;
            self.server.api_state.match_cache.set_with_expiry(&self.server.api_state.database, &current_match.id, &current_match, true, Some(3_600_000)).await;
        };
        Ok(())
    }
    
    async fn on_player_death(&mut self, mut data: PlayerDeathData) -> Result<(), SocketError> {
        println!("Player death! {}", data.victim.name.clone());
        let mut current_match = unwrap_helper::return_default!(self.server.get_match().await, Err(SocketError::InvalidMatchState));
        if current_match.get_state() != MatchState::InProgress {
            return Err(SocketError::InvalidMatchState);
        };

        let is_first_blood = current_match.first_blood.is_none() && data.is_murder();
        if is_first_blood {
            current_match.first_blood = Some(FirstBlood { attacker: data.attacker.as_ref().unwrap().clone(), victim: data.victim.clone(), date: get_u64_time_millis() } );
        };

        if data.is_murder() {
            let mut attacker = {
                match data.attacker.as_ref() {
                    Some(attacker) => { current_match.participants.get(&attacker.id).unwrap().clone() }
                    None => { panic!("Attacker not in participants map") }
                }
            };
            {
                for participant_listener in self.participant_listeners.iter() {
                     participant_listener.on_kill(&mut self.server, &mut current_match, &mut attacker, &mut data, is_first_blood).await;
                };
                current_match.save_participants(vec![attacker.clone()]);
            };
            {
                // let mut player_context = PlayerContext { profile: attacker.get_player(&*self.server.api_state).await, current_match: &mut current_match };
                // player_context = PlayerStatListener::on_kill(&mut self.server, player_context, &mut data, is_first_blood).await;
                // attacker.set_player(&*self.server.api_state, &player_context.profile).await;

                let mut player = attacker.get_player(&*self.server.api_state).await;
                for player_listener in self.player_listeners.iter() {
                    player_listener.on_kill(&mut self.server, &mut current_match, &mut player, &mut data, is_first_blood).await;
                };
                attacker.set_player(&*self.server.api_state, &player).await;

            };
        };

        let mut victim = current_match.participants.get(&data.victim.id).unwrap().to_owned();

        {
            for participant_listener in self.participant_listeners.iter() {
                 participant_listener.on_death(&mut self.server, &mut current_match, &mut victim, &mut data, is_first_blood).await;
            };
            current_match.save_participants(vec![victim.clone()]);
        };
        {
            let mut player = victim.get_player(&*self.server.api_state).await;
            for player_listener in self.player_listeners.iter() {
                player_listener.on_death(&mut self.server, &mut current_match, &mut player, &mut data, is_first_blood).await;
            };
            victim.set_player(&*self.server.api_state, &player).await;
        };

        {
            self.server.api_state.database.insert_one(&Death {
                id: Uuid::new_v4().to_string(),
                victim: data.victim.clone(),
                attacker: data.attacker.clone(),
                weapon: data.weapon.clone(),
                entity: data.entity.clone(),
                distance: data.distance.clone(),
                key: data.key.clone(),
                cause: data.cause.clone(),
                server_id: self.server.id.clone(),
                match_id: current_match.id.clone(),
                created_at: get_u64_time_millis(),
            }).await;
            self.server.api_state.match_cache.set(&self.server.api_state.database, &current_match.id, &current_match, false).await;
        };


        Ok(())
    }

    async fn on_player_chat(&mut self, mut data: PlayerChatData) -> Result<(), SocketError> {
        let mut current_match = unwrap_helper::return_default!(self.server.get_match().await, Err(SocketError::InvalidMatchState));
        let participant = match current_match.participants.get(&data.player.id) {
            Some(participant_ref) => Some(participant_ref.to_owned()),
            None => None
        };

        if participant.is_some() {
            let mut participant = participant.unwrap();

            for participant_listener in self.participant_listeners.iter() {
                 participant_listener.on_chat(&mut self.server, &mut current_match, &mut participant, &mut data).await;
            };
            current_match.save_participants(vec![participant.clone()]);
        };

        {
            let mut player = unwrap_helper::return_default!(self.server.api_state.player_cache.get(&self.server.api_state.database, &data.player.name).await, Ok(()));
            for player_listener in self.player_listeners.iter() {
                player_listener.on_chat(&mut self.server, &mut current_match, &mut player, &mut data).await;
            };
            self.server.api_state.player_cache.set(&self.server.api_state.database, &player.name, &player, false).await;
        };

        {
            self.server.api_state.match_cache.set(&self.server.api_state.database, &current_match.id, &current_match, false).await;
        };

        Ok(())
    }

    async fn on_killstreak(&mut self, data: KillstreakData) -> Result<(), SocketError> {
        let mut current_match = unwrap_helper::return_default!(self.server.get_match().await, Err(SocketError::InvalidMatchState));
        if current_match.get_state() != MatchState::InProgress {
            return Err(SocketError::InvalidMatchState);
        };
        let mut participant = current_match.participants.get(&data.player.id).unwrap().to_owned();
        let mut player = participant.get_player(&*self.server.api_state).await;
        if data.ended {
            for participant_listener in self.participant_listeners.iter() {
                 participant_listener.on_killstreak_end(&mut self.server, &mut current_match, &mut participant, data.amount).await;
            };
            for player_listener in self.player_listeners.iter() {
                player_listener.on_killstreak_end(&mut self.server, &mut current_match, &mut player, data.amount).await;
            };
        } else {
            for participant_listener in self.participant_listeners.iter() {
                 participant_listener.on_killstreak(&mut self.server, &mut current_match, &mut participant, data.amount).await;
            };
            for player_listener in self.player_listeners.iter() {
                player_listener.on_killstreak(&mut self.server, &mut current_match, &mut player, data.amount).await;
            };
        };

        {
            current_match.save_participants(vec![participant.clone()]);
            participant.set_player(&*self.server.api_state, &player).await;
            self.server.api_state.match_cache.set(&self.server.api_state.database, &current_match.id, &current_match, false).await;
        };
        Ok(())
    }

    async fn on_party_join(&mut self, data: PartyJoinData) -> Result<(), SocketError> {
        let mut current_match = unwrap_helper::return_default!(self.server.get_match().await, Err(SocketError::InvalidMatchState));
        if current_match.get_state() != MatchState::InProgress {
            return Err(SocketError::InvalidMatchState);
        };

        let mut participant = match current_match.participants.get(&data.player.id) {
            Some(participant) => participant.to_owned(),
            None => Participant::from_simple(SimpleParticipant { 
                name: data.player.name.clone(), id: data.player.id.clone(), party_name: Some(data.party_name.clone())
            })
        };
        for participant_listener in self.participant_listeners.iter() {
             participant_listener.on_party_join(&mut self.server, &mut current_match, &mut participant, data.party_name.clone()).await;
        };
        let mut player = participant.get_player(&*self.server.api_state).await;
        for player_listener in self.player_listeners.iter() {
            player_listener.on_party_join(&mut self.server, &mut current_match, &mut player, data.party_name.clone()).await;
        };

        {
            current_match.save_participants(vec![participant.clone()]);
            participant.set_player(&*self.server.api_state, &player).await;
            self.server.api_state.match_cache.set(&self.server.api_state.database, &current_match.id, &current_match, false).await;
        };
        Ok(())
    }

    async fn on_party_leave(&mut self, data: PartyLeaveData) -> Result<(), SocketError> {
        let mut current_match = unwrap_helper::return_default!(self.server.get_match().await, Err(SocketError::InvalidMatchState));
        if current_match.get_state() != MatchState::InProgress {
            return Err(SocketError::InvalidMatchState);
        };

        let mut participant = current_match.participants.get(&data.player.id).unwrap().clone();
        for participant_listener in self.participant_listeners.iter() {
             participant_listener.on_party_leave(&mut self.server, &mut current_match, &mut participant).await;
        };
        let mut player = participant.get_player(&*self.server.api_state).await;
        for player_listener in self.player_listeners.iter() {
            player_listener.on_party_leave(&mut self.server, &mut current_match, &mut player).await;
        };

        {
            current_match.save_participants(vec![participant.clone()]);
            participant.set_player(&*self.server.api_state, &player).await;
            self.server.api_state.match_cache.set(&self.server.api_state.database, &current_match.id, &current_match, false).await;
        };
        Ok(())
    }

    async fn on_destroyable_damage(&mut self, data: DestroyableDamageData) -> Result<(), SocketError> {
        let mut current_match = unwrap_helper::return_default!(self.server.get_match().await, Err(SocketError::InvalidMatchState));
        if current_match.get_state() != MatchState::InProgress {
            return Err(SocketError::InvalidMatchState);
        };

        let mut participant = current_match.participants.get(&data.player_id).unwrap().clone();
        let destroyable = unwrap_helper::return_default!(
            unwrap_helper::return_default!(
                &current_match.level.goals, Ok(())
            ).destroyables.iter().find(|destroyable| destroyable.id == data.destroyable_id), 
            Ok(())
        ).to_owned();
        for participant_listener in self.participant_listeners.iter() {
             participant_listener.on_destroyable_damage(&mut self.server, &mut current_match, &mut participant, &destroyable, data.damage).await;
        };
        let mut player = participant.get_player(&*self.server.api_state).await;
        for player_listener in self.player_listeners.iter() {
            player_listener.on_destroyable_damage(&mut self.server, &mut current_match, &mut player, &destroyable, data.damage).await;
        };

        {
            current_match.save_participants(vec![participant.clone()]);
            participant.set_player(&*self.server.api_state, &player).await;
            self.server.api_state.match_cache.set(&self.server.api_state.database, &current_match.id, &current_match, false).await;
        };
        Ok(())
    }

    async fn on_destroyable_destroy(&mut self, data: DestroyableDestroyData) -> Result<(), SocketError> {
        let mut current_match = unwrap_helper::return_default!(self.server.get_match().await, Err(SocketError::InvalidMatchState));
        for contribution in data.contributions.iter() {
            let mut participant = current_match.participants.get(&contribution.player_id).unwrap().clone();
            for participant_listener in self.participant_listeners.iter() {
                 participant_listener.on_destroyable_destroy(
                     &mut self.server, 
                     &mut current_match, 
                     &mut participant, 
                     contribution.percentage, 
                     contribution.block_count
                ).await;
            };
            let mut player = participant.get_player(&*self.server.api_state).await;
            for player_listener in self.player_listeners.iter() {
                player_listener.on_destroyable_destroy(
                    &mut self.server, 
                    &mut current_match, 
                    &mut player, 
                    contribution.percentage, 
                    contribution.block_count
                ).await;
            };

            {
                current_match.save_participants(vec![participant.clone()]);
                participant.set_player(&*self.server.api_state, &player).await;
            };
        };
        self.server.api_state.match_cache.set(&self.server.api_state.database, &current_match.id, &current_match, false).await;
        Ok(())
    }

    async fn on_core_leak(&mut self, data: CoreLeakData) -> Result<(), SocketError> {
        let mut current_match = unwrap_helper::return_default!(self.server.get_match().await, Err(SocketError::InvalidMatchState));
        for contribution in data.contributions.iter() {
            let mut participant = current_match.participants.get(&contribution.player_id).unwrap().clone();
            for participant_listener in self.participant_listeners.iter() {
                 participant_listener.on_core_leak(
                     &mut self.server, 
                     &mut current_match, 
                     &mut participant, 
                     contribution.percentage, 
                     contribution.block_count
                ).await;
            };
            let mut player = participant.get_player(&*self.server.api_state).await;
            for player_listener in self.player_listeners.iter() {
                player_listener.on_core_leak(
                    &mut self.server, 
                    &mut current_match, 
                    &mut player, 
                    contribution.percentage, 
                    contribution.block_count
                ).await;
            };

            {
                current_match.save_participants(vec![participant.clone()]);
                participant.set_player(&*self.server.api_state, &player).await;
            };
        };
        self.server.api_state.match_cache.set(&self.server.api_state.database, &current_match.id, &current_match, false).await;
        Ok(())
    }

    async fn on_flag_place(&mut self, data: FlagDropData) -> Result<(), SocketError> {
        let mut current_match = unwrap_helper::return_default!(self.server.get_match().await, Err(SocketError::InvalidMatchState));

        let mut participant = current_match.participants.get(&data.player_id).unwrap().clone();
        for participant_listener in self.participant_listeners.iter() {
             participant_listener.on_flag_place(&mut self.server, &mut current_match, &mut participant, data.held_time).await;
        };
        let mut player = participant.get_player(&*self.server.api_state).await;
        for player_listener in self.player_listeners.iter() {
            player_listener.on_flag_place(&mut self.server, &mut current_match, &mut player, data.held_time).await;
        };

        {
            current_match.save_participants(vec![participant.clone()]);
            participant.set_player(&*self.server.api_state, &player).await;
            self.server.api_state.match_cache.set(&self.server.api_state.database, &current_match.id, &current_match, false).await;
        };
        Ok(())
    }

    async fn on_flag_pickup(&mut self, data: FlagEventData) -> Result<(), SocketError> {
        let mut current_match = unwrap_helper::return_default!(self.server.get_match().await, Err(SocketError::InvalidMatchState));
        if current_match.get_state() != MatchState::InProgress {
            return Err(SocketError::InvalidMatchState);
        };

        let mut participant = current_match.participants.get(&data.player_id).unwrap().clone();
        for participant_listener in self.participant_listeners.iter() {
             participant_listener.on_flag_pickup(&mut self.server, &mut current_match, &mut participant).await;
        };
        let mut player = participant.get_player(&*self.server.api_state).await;
        for player_listener in self.player_listeners.iter() {
            player_listener.on_flag_pickup(&mut self.server, &mut current_match, &mut player).await;
        };

        {
            current_match.save_participants(vec![participant.clone()]);
            participant.set_player(&*self.server.api_state, &player).await;
            self.server.api_state.match_cache.set(&self.server.api_state.database, &current_match.id, &current_match, false).await;
        };
        Ok(())
    }

    async fn on_flag_drop(&mut self, data: FlagDropData) -> Result<(), SocketError> {
        let mut current_match = unwrap_helper::return_default!(self.server.get_match().await, Err(SocketError::InvalidMatchState));

        let mut participant = current_match.participants.get(&data.player_id).unwrap().clone();
        for participant_listener in self.participant_listeners.iter() {
             participant_listener.on_flag_drop(&mut self.server, &mut current_match, &mut participant, data.held_time).await;
        };
        let mut player = participant.get_player(&*self.server.api_state).await;
        for player_listener in self.player_listeners.iter() {
            player_listener.on_flag_drop(&mut self.server, &mut current_match, &mut player, data.held_time).await;
        };

        {
            current_match.save_participants(vec![participant.clone()]);
            participant.set_player(&*self.server.api_state, &player).await;
            self.server.api_state.match_cache.set(&self.server.api_state.database, &current_match.id, &current_match, false).await;
        };
        Ok(())
    }

    async fn on_flag_defend(&mut self, data: FlagEventData) -> Result<(), SocketError> {
        let mut current_match = unwrap_helper::return_default!(self.server.get_match().await, Err(SocketError::InvalidMatchState));
        if current_match.get_state() != MatchState::InProgress {
            return Err(SocketError::InvalidMatchState);
        };

        let mut participant = current_match.participants.get(&data.player_id).unwrap().clone();
        for participant_listener in self.participant_listeners.iter() {
             participant_listener.on_flag_defend(&mut self.server, &mut current_match, &mut participant).await;
        };
        let mut player = participant.get_player(&*self.server.api_state).await;
        for player_listener in self.player_listeners.iter() {
            player_listener.on_flag_defend(&mut self.server, &mut current_match, &mut player).await;
        };

        {
            current_match.save_participants(vec![participant.clone()]);
            participant.set_player(&*self.server.api_state, &player).await;
            self.server.api_state.match_cache.set(&self.server.api_state.database, &current_match.id, &current_match, false).await;
        };
        Ok(())
    }

    async fn on_wool_place(&mut self, data: WoolDropData) -> Result<(), SocketError> {
        let mut current_match = unwrap_helper::return_default!(self.server.get_match().await, Err(SocketError::InvalidMatchState));
        if current_match.get_state() != MatchState::InProgress {
            return Err(SocketError::InvalidMatchState);
        };

        let mut participant = current_match.participants.get(&data.player_id).unwrap().clone();
        for participant_listener in self.participant_listeners.iter() {
             participant_listener.on_wool_place(&mut self.server, &mut current_match, &mut participant, data.held_time).await;
        };
        let mut player = participant.get_player(&*self.server.api_state).await;
        for player_listener in self.player_listeners.iter() {
            player_listener.on_wool_place(&mut self.server, &mut current_match, &mut player, data.held_time).await;
        };

        {
            current_match.save_participants(vec![participant.clone()]);
            participant.set_player(&*self.server.api_state, &player).await;
            self.server.api_state.match_cache.set(&self.server.api_state.database, &current_match.id, &current_match, false).await;
        };
        Ok(())
    }

    async fn on_wool_pickup(&mut self, data: WoolEventData) -> Result<(), SocketError> {
        let mut current_match = unwrap_helper::return_default!(self.server.get_match().await, Err(SocketError::InvalidMatchState));
        if current_match.get_state() != MatchState::InProgress {
            return Err(SocketError::InvalidMatchState);
        };

        let mut participant = current_match.participants.get(&data.player_id).unwrap().clone();
        for participant_listener in self.participant_listeners.iter() {
             participant_listener.on_wool_pickup(&mut self.server, &mut current_match, &mut participant).await;
        };
        let mut player = participant.get_player(&*self.server.api_state).await;
        for player_listener in self.player_listeners.iter() {
            player_listener.on_wool_pickup(&mut self.server, &mut current_match, &mut player).await;
        };

        {
            current_match.save_participants(vec![participant.clone()]);
            participant.set_player(&*self.server.api_state, &player).await;
            self.server.api_state.match_cache.set(&self.server.api_state.database, &current_match.id, &current_match, false).await;
        };
        Ok(())
    }

    async fn on_wool_drop(&mut self, data: WoolDropData) -> Result<(), SocketError> {
        let mut current_match = unwrap_helper::return_default!(self.server.get_match().await, Err(SocketError::InvalidMatchState));
        if current_match.get_state() != MatchState::InProgress {
            return Err(SocketError::InvalidMatchState);
        };

        let mut participant = current_match.participants.get(&data.player_id).unwrap().clone();
        for participant_listener in self.participant_listeners.iter() {
             participant_listener.on_wool_drop(&mut self.server, &mut current_match, &mut participant, data.held_time).await;
        };
        let mut player = participant.get_player(&*self.server.api_state).await;
        for player_listener in self.player_listeners.iter() {
            player_listener.on_wool_drop(&mut self.server, &mut current_match, &mut player, data.held_time).await;
        };

        {
            current_match.save_participants(vec![participant.clone()]);
            participant.set_player(&*self.server.api_state, &player).await;
            self.server.api_state.match_cache.set(&self.server.api_state.database, &current_match.id, &current_match, false).await;
        };
        Ok(())
    }

    async fn on_wool_defend(&mut self, data: WoolEventData) -> Result<(), SocketError> {
        let mut current_match = unwrap_helper::return_default!(self.server.get_match().await, Err(SocketError::InvalidMatchState));
        if current_match.get_state() != MatchState::InProgress {
            return Err(SocketError::InvalidMatchState);
        };

        let mut participant = current_match.participants.get(&data.player_id).unwrap().clone();
        for participant_listener in self.participant_listeners.iter() {
             participant_listener.on_wool_defend(&mut self.server, &mut current_match, &mut participant).await;
        };
        let mut player = participant.get_player(&*self.server.api_state).await;
        for player_listener in self.player_listeners.iter() {
            player_listener.on_wool_defend(&mut self.server, &mut current_match, &mut player).await;
        };

        {
            current_match.save_participants(vec![participant.clone()]);
            participant.set_player(&*self.server.api_state, &player).await;
            self.server.api_state.match_cache.set(&self.server.api_state.database, &current_match.id, &current_match, false).await;
        };
        Ok(())
    }

    async fn on_control_point_capture(&mut self, data: ControlPointCaptureData) -> Result<(), SocketError> {
        let mut current_match = unwrap_helper::return_default!(self.server.get_match().await, Err(SocketError::InvalidMatchState));
        if current_match.get_state() != MatchState::InProgress {
            return Err(SocketError::InvalidMatchState);
        };
        for capturer in data.player_ids.iter() {
            let mut participant = current_match.participants.get(capturer).unwrap().clone();
            for participant_listener in self.participant_listeners.iter() {
                 participant_listener.on_control_point_capture(
                     &mut self.server, 
                     &mut current_match, 
                     &mut participant, 
                     data.player_ids.len() as u32, 
                ).await;
            };
            let mut player = participant.get_player(&*self.server.api_state).await;
            for player_listener in self.player_listeners.iter() {
                player_listener.on_control_point_capture(
                    &mut self.server, 
                    &mut current_match, 
                    &mut player, 
                    data.player_ids.len() as u32, 
                ).await;
            };

            {
                current_match.save_participants(vec![participant.clone()]);
                participant.set_player(&*self.server.api_state, &player).await;
            };
        }
        self.server.api_state.match_cache.set(&self.server.api_state.database, &current_match.id, &current_match, false).await;
        Ok(())
    }

    async fn on_achievement_complete(&mut self, data: PlayerAchievementData) -> Result<(), SocketError> {
        let achievement_id = data.achievement_id.as_str();
        let database = &self.server.api_state.database;
        let collection = &database.achievements;

        if let Some(mut achievement) = Database::find_by_id(collection, achievement_id).await {
            if achievement.first_completion.is_none() {
                achievement.first_completion = Some(data.player.id.clone());
                database.save(&achievement).await;
            }
        } else {
            return Ok(());
        }

        let mut player = unwrap_helper::return_default!(
        self.server.api_state.player_cache.get(database, data.player.name.as_str()).await,
        Ok(())
    );

        player.stats.achievements.insert(data.achievement_id.clone(), AchievementData {
            completion_time: data.completion_time
        });

        self.server.api_state.player_cache.set(database, &player.name, &player, true).await;

        Ok(())
    }

    fn parse_data<T: DeserializeOwned>(data: Value) -> T {
        let debug_res = format!("Socket passed malformed data.. {data:?}");
        serde_json::from_value(data).expect(&debug_res)
    }

    async fn get_match_id(&self) -> String {
        let current_match = self.server.get_match().await;
        match current_match {
            Some(actual_match) => actual_match.id,
            None => String::from("null")
        }
    }
}
