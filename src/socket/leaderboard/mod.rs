use std::ops::Sub;
use std::sync::Arc;

use chrono::{Datelike, DateTime, Days, FixedOffset, Month, Months, NaiveDate, NaiveTime, TimeDelta, TimeZone, Utc};
use mongodb::{bson::doc, Cursor};
use num_traits::cast::FromPrimitive;
use redis::{aio::Connection, ToRedisArgs};
use rocket::time::macros::date;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter, EnumString};

use crate::{database::{cache::RedisAdapter, Database, models::player::Player}, util::r#macro::unwrap_helper};
use crate::socket::leaderboard::leaderboard_new::LeaderboardV2;

pub mod leaderboard_listener;
pub mod leaderboard_new;

// UTC-4 (EDT, EST during daylight savings)
fn get_offset() -> FixedOffset {
    FixedOffset::west_opt(4 * 3600).expect("FixedOffset::west out of bounds")
}

pub fn get_lb_datetime() -> DateTime<FixedOffset> {
    let naive_utc_time = Utc::now().naive_utc();
    let fixed_offset = get_offset();
    fixed_offset.from_utc_datetime(&naive_utc_time)
}

#[derive(Eq, PartialEq, Clone)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter
}

impl Season {
    pub fn of_northern(month: Month) -> Season {
        match month {
            Month::March | Month::April => Season::Spring,
            Month::May | Month::June | Month::July | Month::August  => Season::Summer,
            Month::September | Month::October => Season::Autumn,
            Month::November | Month::December | Month::January | Month::February => Season::Winter,
        }
    }

    pub fn get_northern_season_start(&self) -> (Month, u32) {
        match &self {
            Season::Spring => {
                (Month::March, 20)
            }
            Season::Summer => {
                (Month::June, 20)
            }
            Season::Autumn => {
                (Month::September, 22)
            }
            Season::Winter => {
                (Month::December, 21)
            }
        }
    }

    pub fn get_season(date: NaiveDate) -> Self {
        let szn_as_date = |szn: &Season| {
            let szn_start = szn.get_northern_season_start();
            NaiveDate::from_ymd_opt(
                date.year(),
                szn_start.0.number_from_month(),
                szn_start.1
            ).expect("Season start date to be valid")
        };
        if date < szn_as_date(&Season::Spring) {
            Season::Winter
        } else if date < szn_as_date(&Season::Summer) {
            Season::Spring
        } else if date < szn_as_date(&Season::Autumn) {
            Season::Summer
        } else if date < szn_as_date(&Season::Winter) {
            Season::Autumn
        } else {
            Season::Winter
        }
    }

    pub fn next(&self) -> Self {
        match &self {
            Season::Spring => Season::Summer,
            Season::Summer => Season::Autumn,
            Season::Autumn => Season::Winter,
            Season::Winter => Season::Spring
        }
    }

    pub fn name(&self) -> &'static str {
        match &self {
            Season::Spring => "spring",
            Season::Summer => "summer",
            Season::Autumn => "autumn",
            Season::Winter => "winter",
        }
    }
}

#[derive(EnumIter, EnumString, Hash, Eq, Clone, PartialEq, strum_macros::Display)]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum LeaderboardPeriod {
    Daily,
    Weekly,
    Monthly,
    Seasonally,
    Yearly,
    AllTime
}

type LeaderboardPeriodDateTimeRange = (Option<DateTime<FixedOffset>>, DateTime<FixedOffset>);

impl LeaderboardPeriod {
    pub fn get_most_granular_period() -> LeaderboardPeriod {
        LeaderboardPeriod::Daily
    }

    pub fn are_datetimes_disjoint(
        &self,
        date1: DateTime<FixedOffset>, date2: DateTime<FixedOffset>
    ) -> bool {
        match &self {
            LeaderboardPeriod::Daily => {
                date1.date_naive() != date2.date_naive()
            }
            LeaderboardPeriod::Weekly => {
                (date1 - Days::new(date1.weekday().num_days_from_sunday() as u64)).date_naive() !=
                    (date2 - Days::new(date2.weekday().num_days_from_sunday() as u64)).date_naive()
            }
            LeaderboardPeriod::Monthly => {
                date1.year() != date2.year() || date1.month() != date2.month()
            }
            LeaderboardPeriod::Seasonally => {
                Season::of_northern(
                    Month::from_u32(date1.month()).expect("Date to be in a valid month")
                ) != Season::of_northern(
                    Month::from_u32(date2.month()).expect("Date to be in a valid month")
                )
            }
            LeaderboardPeriod::Yearly => {
                date1.year() != date2.year()
            }
            LeaderboardPeriod::AllTime => {
                false
            }
        }
    }

    pub fn get_full_date_range(&self, date_time: DateTime<FixedOffset>) -> LeaderboardPeriodDateTimeRange {
        match &self {
            LeaderboardPeriod::Daily => {
                let midnight = date_time.with_time(NaiveTime::MIN)
                    .single().expect("Should resolve to the single variant");
                let tmrw_midnight = midnight + Days::new(1);
                (Some(midnight), tmrw_midnight)
            }
            LeaderboardPeriod::Weekly => {
                let week_start = (date_time - Days::new(date_time.weekday().num_days_from_sunday() as u64))
                    .with_time(NaiveTime::MIN)
                    .single().expect("Should resolve to the single variant");
                let week_end = week_start + Days::new(7);
                (Some(week_start), week_end)
            }
            LeaderboardPeriod::Monthly => {
                let month_start = date_time.with_day0(0).expect("The first day of the month to exist");
                let month_end = date_time + Months::new(1);
                (Some(month_start), month_end)
            }
            LeaderboardPeriod::Seasonally => {
                let current_season = Season::get_season(date_time.date_naive());
                let szn_to_nd = |szn: &Season| {
                    let (month, day) = szn.get_northern_season_start();
                    let season_year = if month.number_from_month() > date_time.month() ||
                        (month.number_from_month() == date_time.month() && day > date_time.day()) {
                        date_time.year() - 1
                    } else {
                        date_time.year()
                    };
                    NaiveDate::from_ymd_opt(season_year, month.number_from_month(), day).expect("Season start to be a valid date")
                };
                let season_start = szn_to_nd(&current_season);
                let season_end = szn_to_nd(&current_season.next());
                let season_start_datetime =
                    date_time.timezone().from_local_datetime(
                        &season_start.and_time(NaiveTime::MIN)
                    ).single().expect("Resolved season start");
                let season_end_datetime =
                    date_time.timezone().from_local_datetime(
                        &season_end.and_time(NaiveTime::MIN)
                    ).single().expect("Resolved season end");
                (Some(season_start_datetime), season_end_datetime)
            }
            LeaderboardPeriod::Yearly => {
                let begin = date_time.with_month0(0).unwrap().with_day0(0).unwrap()
                    .with_time(NaiveTime::MIN)
                    .single().expect("Should resolve to the single variant");
                let end = begin.with_year(begin.year() + 1).expect("The next year to exist");
                (Some(begin), end)
            }
            LeaderboardPeriod::AllTime => {
                (None, date_time)
            }
        }
    }

    pub fn get_date_range_with_now_as_upperbound(&self) -> LeaderboardPeriodDateTimeRange {
        let today_datetime = get_lb_datetime();
        let past_date = match &self {
            LeaderboardPeriod::Daily => {
                Some(
                    today_datetime.with_time(NaiveTime::MIN)
                        .single()
                        .expect("Should resolve to the single variant")
                )
            }
            LeaderboardPeriod::Weekly => {
                let days_in_the_past = today_datetime.weekday().num_days_from_sunday();
                Some(
                    today_datetime.sub(TimeDelta::days(days_in_the_past as i64)).with_time(NaiveTime::MIN)
                        .single()
                        .expect("Should resolve to the single variant")
                )
            }
            LeaderboardPeriod::Monthly => {
                Some(
                    today_datetime.with_day0(0u32).unwrap()
                        .with_time(NaiveTime::MIN)
                        .single()
                        .expect("Should resolve to the single variant")
                )
            }
            LeaderboardPeriod::Seasonally => {
                let current_month = Month::from_u32(today_datetime.month())
                    .expect("Month to resolve");
                let (season_month, season_day) = Season::of_northern(current_month)
                    .get_northern_season_start();
                // if season begin date is in the future, then the season was from the last year
                let season_year = if season_month.number_from_month() > today_datetime.month() || (season_month.number_from_month() == today_datetime.month() && season_day > today_datetime.day()) {
                    today_datetime.year() - 1
                } else {
                    today_datetime.year()
                };
                let season_date =
                    NaiveDate::from_ymd_opt(season_year, season_month.number_from_month(), season_day)
                        .expect("This should be a valid date");
                let delta = today_datetime.date_naive().signed_duration_since(season_date);
                Some(
                    today_datetime.sub(delta).with_time(NaiveTime::MIN)
                        .single()
                        .expect("Should resolve to the single variant")
                )
            }
            LeaderboardPeriod::Yearly => {
                Some(
                    today_datetime.with_month0(0).unwrap().with_day0(0).unwrap()
                        .with_time(NaiveTime::MIN)
                        .single().expect("Should resolve to the single variant")
                )
            }
            LeaderboardPeriod::AllTime => {
                None
            }
        };
        (past_date, today_datetime)
    }

    pub fn get_today_id(&self) -> String {
        let date = get_lb_datetime();
        match &self {
            Self::Daily => {
                let day = date.day();
                let month = date.month() - 1; // Java being cringe
                let year = date.year();
                format!("{}:d:{}:{}", year, month, day)
            },
            Self::Weekly => {
                let week = date.iso_week().week();
                let year = date.year();
                format!("{}:w:{}", year, week)
            },
            Self::Monthly => {
                let month = date.month() - 1;
                let year = date.year();
                format!("{}:m:{}", year, month)
            },
            Self::Seasonally => {
                let month = date.month() - 1;
                let season = Season::of_northern(Month::from_u32(month + 1).unwrap_or(Month::January)).name();
                let year = date.year();
                format!("{}:s:{}", year, season)
            },
            Self::Yearly => {
                let year = date.year();
                format!("{}:y", year)
            },
            Self::AllTime => String::from("all"),
        }
    }
}

#[derive(EnumString, Serialize, Deserialize, Clone, Eq, Hash, PartialEq, strum_macros::Display)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]
pub enum ScoreType {
    Kills,
    Deaths,
    FirstBloods,
    Wins,
    Losses,
    Ties,
    Xp,
    MessagesSent,
    MatchesPlayed,
    ServerPlaytime,
    GamePlaytime,
    CoreLeaks,
    CoreBlockDestroys,
    DestroyableDestroys,
    DestroyableBlockDestroys,
    FlagCaptures,
    FlagDrops,
    FlagPickups,
    FlagDefends,
    FlagHoldTime,
    WoolCaptures,
    WoolDrops,
    WoolPickups,
    WoolDefends,
    ControlPointCaptures,
    HighestKillstreak
}

enum ScoreTypeAggregation {
    Sum, Max
}

impl ScoreTypeAggregation {
    // true means the delta is useless
    // false indicates whether it is unknown if the delta is useless
    fn is_delta_useless(&self, delta: u32) -> bool {
        if delta == 0 {
            true
        } else {
            false
        }
    }

    fn requires_sequential_consistency(&self) -> bool {
        match &self {
            ScoreTypeAggregation::Sum => false,
            ScoreTypeAggregation::Max => false
        }
    }

    fn compare(&self, old: u32, new: u32) -> u32 {
        match &self {
            ScoreTypeAggregation::Sum => { old + new }
            ScoreTypeAggregation::Max => { u32::max(old, new) }
        }
    }
}

impl ScoreType {
    pub fn to_leaderboard<'a>(&self, lbs: &'a MarsLeaderboards) -> &'a LeaderboardV2 {
        match self {
            ScoreType::Kills => &lbs.kills,
            ScoreType::Deaths => &lbs.deaths,
            ScoreType::FirstBloods => &lbs.first_bloods,
            ScoreType::Wins => &lbs.wins,
            ScoreType::Losses => &lbs.losses,
            ScoreType::Ties => &lbs.ties,
            ScoreType::Xp => &lbs.xp,
            ScoreType::MessagesSent => &lbs.messages_sent,
            ScoreType::MatchesPlayed => &lbs.matches_played,
            ScoreType::ServerPlaytime => &lbs.server_playtime,
            ScoreType::GamePlaytime => &lbs.game_playtime,
            ScoreType::CoreLeaks => &lbs.core_leaks,
            ScoreType::CoreBlockDestroys => &lbs.core_block_destroys,
            ScoreType::DestroyableDestroys => &lbs.destroyable_destroys,
            ScoreType::DestroyableBlockDestroys => &lbs.destroyable_block_destroys,
            ScoreType::FlagCaptures => &lbs.flag_captures,
            ScoreType::FlagDrops => &lbs.flag_drops,
            ScoreType::FlagPickups => &lbs.flag_pickups,
            ScoreType::FlagDefends => &lbs.flag_defends,
            ScoreType::FlagHoldTime => &lbs.flag_hold_time,
            ScoreType::WoolCaptures => &lbs.wool_captures,
            ScoreType::WoolDrops => &lbs.wool_drops,
            ScoreType::WoolPickups => &lbs.wool_pickups,
            ScoreType::WoolDefends => &lbs.wool_defends,
            ScoreType::ControlPointCaptures => &lbs.control_point_captures,
            ScoreType::HighestKillstreak => &lbs.highest_killstreak,
        }
    }

    pub fn get_aggregation_type(&self) -> ScoreTypeAggregation {
        match &self {
            ScoreType::Kills | ScoreType::Deaths | ScoreType::FirstBloods | ScoreType::Wins |
            ScoreType::Losses | ScoreType::Ties | ScoreType::Xp | ScoreType::MessagesSent |
            ScoreType::MatchesPlayed | ScoreType::ServerPlaytime | ScoreType::GamePlaytime |
            ScoreType::CoreLeaks | ScoreType::CoreBlockDestroys | ScoreType::DestroyableDestroys |
            ScoreType::DestroyableBlockDestroys | ScoreType::FlagCaptures | ScoreType::FlagDrops |
            ScoreType::FlagPickups | ScoreType::FlagDefends | ScoreType::FlagHoldTime |
            ScoreType::WoolCaptures | ScoreType::WoolDrops | ScoreType::WoolPickups |
            ScoreType::WoolDefends | ScoreType::ControlPointCaptures => {
                ScoreTypeAggregation::Sum
            }
            ScoreType::HighestKillstreak => {
                ScoreTypeAggregation::Max
            }
        }
    }
}

pub struct Leaderboard {
    pub score_type: ScoreType,
    pub database: Arc<Database>,
    pub cache: Arc<RedisAdapter>
}


impl Leaderboard {
    async fn zadd_entries<T: ToRedisArgs, K: ToRedisArgs, V: ToRedisArgs>(&self, key: &T, items: &Vec<(K, V)>) {
        let _ = self.cache.submit(|mut conn| async move {
            let _ = redis::cmd("ZADD")
                .arg(key)
                .arg(items)
                .query_async::<Connection, ()>(&mut conn).await;
        }).await;
    }

    pub async fn populate_all_time(&self) {
        let cursor : Cursor<Player> = match self.database.players.find(doc! {}, None).await {
            Ok(player_cursor) => player_cursor,
            Err(_) => return
        };
        let players = {
            let mut players = Database::consume_cursor_into_owning_vec(cursor).await;
            players.sort_by(|a, b| {
                b.stats.get_score(&self.score_type).cmp(&a.stats.get_score(&self.score_type))
            });
            players
        };
        let members = {
            let mut members : Vec<(String, u64)> = Vec::new();
            for player in players.iter() {
                members.push((player.id_name(), player.stats.get_score(&self.score_type) as u64));
            };
            members
        };
        self.zadd_entries(&self.get_id(&LeaderboardPeriod::AllTime), &members).await;
    }

    pub async fn set(&self, id: &String, score: u32) {
        let u64_score = score as u64;
        let _ = self.cache.submit(|mut conn| async move {
            for period in LeaderboardPeriod::iter() {
                let _ = redis::cmd("ZADD").arg(&self.get_id(&period)).arg(u64_score).arg(id).query_async::<Connection, ()>(&mut conn).await;
            };
        }).await;
    }

    pub async fn increment(&self, id: &String, incr: Option<u32>) {
        let u64_incr = incr.unwrap_or(1) as u64;
        let _ = self.cache.submit(|mut conn| async move {
            for period in LeaderboardPeriod::iter() {
                let _ = redis::cmd("ZINCRBY").arg(&self.get_id(&period)).arg(u64_incr).arg(id).query_async::<Connection, ()>(&mut conn).await;
            };
        }).await;
    }

    fn strings_as_leaderboard_entries(raw: Vec<String>) -> Vec<LeaderboardLine> {
        let mut entries : Vec<LeaderboardLine> = Vec::new();
        if raw.len() <= 1 || raw.len() % 2 == 1 {
            return entries;
        };
        for i in (0..=(raw.len() - 2)).step_by(2) {
            let id_name = raw[i].clone();
            let score = raw[i + 1].parse::<u32>().unwrap_or(0);
            let (id, name) = {
                let mut parts = id_name.split("/");
                let id = unwrap_helper::continue_default!(parts.next());
                let name = unwrap_helper::continue_default!(parts.next());
                (id, name)
            };
            entries.push(LeaderboardLine { id: id.to_owned(), name: name.to_owned(), score });
        }
        entries
    }

    pub async fn fetch_top(&self, period: &LeaderboardPeriod, limit: u32) -> Vec<LeaderboardLine> {
        let lb_top = self.cache.submit(|mut conn| async move {
            let top : Option<Vec<String>> = match redis::cmd("ZRANGE").arg(&self.get_id(period)).arg(0u32).arg(limit - 1).arg("REV").arg("WITHSCORES").query_async::<Connection, Vec<String>>(&mut conn).await {
                Ok(res) => Some(res),
                Err(_) => None
            };
            top.unwrap_or(Vec::new())
        }).await.unwrap_or(Vec::new());
        Self::strings_as_leaderboard_entries(lb_top)
    }

    pub async fn set_if_higher(&self, id: &String, new: u32) {
        let _ = self.cache.submit(|mut conn| async move {
            for period in LeaderboardPeriod::iter() {
                let current = match redis::cmd("ZSCORE").arg(&self.get_id(&period)).arg(id).query_async::<Connection, String>(&mut conn).await {
                    Ok(res) => { res.parse::<u32>().unwrap() },
                    Err(_) => { 0u32 }
                };
                if new > current {
                    redis::cmd("ZADD").arg(&self.get_id(&period)).arg(new as f64).arg(id).query_async::<Connection, ()>(&mut conn).await;
                };
            };
        }).await;
    }

    pub async fn get_position(&self, id: &String, period: &LeaderboardPeriod) -> Option<u64> {
        self.cache.submit(|mut conn| async move {
            let rank : Option<u64> = match redis::cmd("ZREVRANK").arg(&self.get_id(period)).arg(id).query_async::<Connection, u64>(&mut conn).await {
                Ok(res) => Some(res),
                Err(_) => None // this error occurs when redis encounters an issue executing the query
            };
            rank
        }).await.unwrap_or(None) // this unwrap occurs if a connection can't be obtained
    }

    fn get_id(&self, period: &LeaderboardPeriod) -> String {
        format!("lb:{}:{}", self.score_type, period.get_today_id())
    }
}

pub struct MarsLeaderboards {
    pub kills: LeaderboardV2,
    pub deaths: LeaderboardV2,
    pub first_bloods: LeaderboardV2,
    pub wins: LeaderboardV2,
    pub losses: LeaderboardV2,
    pub ties: LeaderboardV2,
    pub xp: LeaderboardV2,
    pub messages_sent: LeaderboardV2,
    pub matches_played: LeaderboardV2,
    pub server_playtime: LeaderboardV2,
    pub game_playtime: LeaderboardV2,
    pub core_leaks: LeaderboardV2,
    pub core_block_destroys: LeaderboardV2,
    pub destroyable_destroys: LeaderboardV2,
    pub destroyable_block_destroys: LeaderboardV2,
    pub flag_captures: LeaderboardV2,
    pub flag_drops: LeaderboardV2,
    pub flag_pickups: LeaderboardV2,
    pub flag_defends: LeaderboardV2,
    pub flag_hold_time: LeaderboardV2,
    pub wool_captures: LeaderboardV2,
    pub wool_drops: LeaderboardV2,
    pub wool_pickups: LeaderboardV2,
    pub wool_defends: LeaderboardV2,
    pub control_point_captures: LeaderboardV2,
    pub highest_killstreak: LeaderboardV2
}

impl MarsLeaderboards {
    pub fn new(redis: Arc<RedisAdapter>, database: Arc<Database>) -> Self {
        MarsLeaderboards {
            kills: LeaderboardV2::new(ScoreType::Kills, redis.clone(), database.clone()),
            deaths: LeaderboardV2::new(ScoreType::Deaths, redis.clone(), database.clone()),
            first_bloods: LeaderboardV2::new(ScoreType::FirstBloods, redis.clone(), database.clone()),
            wins: LeaderboardV2::new(ScoreType::Wins, redis.clone(), database.clone()),
            losses: LeaderboardV2::new(ScoreType::Losses, redis.clone(), database.clone()),
            ties: LeaderboardV2::new(ScoreType::Ties, redis.clone(), database.clone()),
            xp: LeaderboardV2::new(ScoreType::Xp, redis.clone(), database.clone()),
            messages_sent: LeaderboardV2::new(ScoreType::MessagesSent, redis.clone(), database.clone()),
            matches_played: LeaderboardV2::new(ScoreType::MatchesPlayed, redis.clone(), database.clone()),
            server_playtime: LeaderboardV2::new(ScoreType::ServerPlaytime, redis.clone(), database.clone()),
            game_playtime: LeaderboardV2::new(ScoreType::GamePlaytime, redis.clone(), database.clone()),
            core_leaks: LeaderboardV2::new(ScoreType::CoreLeaks, redis.clone(), database.clone()),
            core_block_destroys: LeaderboardV2::new(ScoreType::CoreBlockDestroys, redis.clone(), database.clone()),
            destroyable_destroys: LeaderboardV2::new(ScoreType::DestroyableDestroys, redis.clone(), database.clone()),
            destroyable_block_destroys: LeaderboardV2::new(ScoreType::DestroyableBlockDestroys, redis.clone(), database.clone()),
            flag_captures: LeaderboardV2::new(ScoreType::FlagCaptures, redis.clone(), database.clone()),
            flag_drops: LeaderboardV2::new(ScoreType::FlagDrops, redis.clone(), database.clone()),
            flag_pickups: LeaderboardV2::new(ScoreType::FlagPickups, redis.clone(), database.clone()),
            flag_defends: LeaderboardV2::new(ScoreType::FlagDefends, redis.clone(), database.clone()),
            flag_hold_time: LeaderboardV2::new(ScoreType::FlagHoldTime, redis.clone(), database.clone()),
            wool_captures: LeaderboardV2::new(ScoreType::WoolCaptures, redis.clone(), database.clone()),
            wool_drops: LeaderboardV2::new(ScoreType::WoolDrops, redis.clone(), database.clone()),
            wool_pickups: LeaderboardV2::new(ScoreType::WoolPickups, redis.clone(), database.clone()),
            wool_defends: LeaderboardV2::new(ScoreType::WoolDefends, redis.clone(), database.clone()),
            control_point_captures: LeaderboardV2::new(ScoreType::ControlPointCaptures, redis.clone(), database.clone()),
            highest_killstreak: LeaderboardV2::new(ScoreType::HighestKillstreak, redis.clone(), database.clone())
        }
    }

    pub fn from_score_type(&self, score_type: ScoreType) -> &LeaderboardV2 {
        match score_type {
            ScoreType::Kills => &self.kills,
            ScoreType::Deaths => &self.deaths,
            ScoreType::FirstBloods => &self.first_bloods,
            ScoreType::Wins => &self.wins,
            ScoreType::Losses => &self.losses,
            ScoreType::Ties => &self.ties,
            ScoreType::Xp => &self.xp,
            ScoreType::MessagesSent => &self.messages_sent,
            ScoreType::MatchesPlayed => &self.matches_played,
            ScoreType::ServerPlaytime => &self.server_playtime,
            ScoreType::GamePlaytime => &self.game_playtime,
            ScoreType::CoreLeaks => &self.core_leaks,
            ScoreType::CoreBlockDestroys => &self.core_block_destroys,
            ScoreType::DestroyableDestroys => &self.destroyable_destroys,
            ScoreType::DestroyableBlockDestroys => &self.destroyable_block_destroys,
            ScoreType::FlagCaptures => &self.flag_captures,
            ScoreType::FlagDrops => &self.flag_drops,
            ScoreType::FlagPickups => &self.flag_pickups,
            ScoreType::FlagDefends => &self.flag_defends,
            ScoreType::FlagHoldTime => &self.flag_hold_time,
            ScoreType::WoolCaptures => &self.wool_captures,
            ScoreType::WoolDrops => &self.wool_drops,
            ScoreType::WoolPickups => &self.wool_pickups,
            ScoreType::WoolDefends => &self.wool_defends,
            ScoreType::ControlPointCaptures => &self.control_point_captures,
            ScoreType::HighestKillstreak => &self.highest_killstreak
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardLine {
    pub id: String,
    pub name: String,
    pub score: u32
}
