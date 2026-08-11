//! Source-attributed market-intelligence orchestration.
//!
//! Providers only collect explicit, traceable records. This service performs deterministic
//! relevance, deduplication, credibility labelling and topic grouping; it never promotes a
//! community post or rumour into an objective fact.

#![allow(dead_code)] // Provider variants are intentional extension points; not every optional source ships by default.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    error::Error,
    fmt,
    process::Command,
};

use chrono::{DateTime, FixedOffset, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    database::service::{
        DatabaseError, DatabaseService, IntelligenceItem, IntelligenceItemWithSecurity,
        NewIntelligenceItem,
    },
    disclosure_adapter::DisclosureSecurity,
    event_service::EventView,
    news_service::{NewsArticleInput, NewsSourceType},
};

const EASTMONEY_FLASH_URL: &str = "https://np-listapi.eastmoney.com/comm/web/getNewsByColumns?client=web&biz=web_news_col&column=348&order=1&needInteract=0&pageIndex=1&pageSize=80&req_trace=hero-call&fields=code,showTime,title,summary";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IntelligenceSourceType {
    Official,
    News,
    Industry,
    Community,
    Social,
    Rumor,
}

impl IntelligenceSourceType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Official => "OFFICIAL",
            Self::News => "NEWS",
            Self::Industry => "INDUSTRY",
            Self::Community => "COMMUNITY",
            Self::Social => "SOCIAL",
            Self::Rumor => "RUMOR",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CredibilityLevel {
    A,
    B,
    C,
    D,
    E,
}

impl CredibilityLevel {
    fn as_str(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::E => "E",
        }
    }
    fn score(self) -> i64 {
        match self {
            Self::A => 500,
            Self::B => 400,
            Self::C => 300,
            Self::D => 200,
            Self::E => 100,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IntelligenceInput {
    pub title: String,
    pub summary: String,
    pub source: String,
    pub source_type: IntelligenceSourceType,
    pub source_url: Option<String>,
    pub published_at: String,
    pub fetched_at: String,
    pub credibility_level: CredibilityLevel,
    pub intelligence_type: String,
    pub security_ids: Vec<i64>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntelligenceProviderError(pub String);
impl fmt::Display for IntelligenceProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl Error for IntelligenceProviderError {}

/// External source boundary. Adding a source must implement this interface rather than changing
/// aggregation, credibility or UI logic.
pub trait IntelligenceProvider {
    fn name(&self) -> &'static str;
    fn source_type(&self) -> IntelligenceSourceType;
    fn fetch(
        &self,
        securities: &[DisclosureSecurity],
    ) -> Result<Vec<IntelligenceInput>, IntelligenceProviderError>;
}

/// Public, no-cookie Eastmoney finance flash feed. Only entries explicitly mentioning a followed
/// stock code or name are retained; unrelated market headlines are discarded.
pub struct EastmoneyFlashNewsProvider;
impl IntelligenceProvider for EastmoneyFlashNewsProvider {
    fn name(&self) -> &'static str {
        "东方财富财经快讯"
    }
    fn source_type(&self) -> IntelligenceSourceType {
        IntelligenceSourceType::News
    }
    fn fetch(
        &self,
        securities: &[DisclosureSecurity],
    ) -> Result<Vec<IntelligenceInput>, IntelligenceProviderError> {
        if securities.is_empty() {
            return Ok(Vec::new());
        }
        let output = Command::new("curl")
            .args([
                "--fail",
                "--silent",
                "--show-error",
                "--max-time",
                "15",
                "--header",
                "User-Agent: Hero Call/1.1",
                EASTMONEY_FLASH_URL,
            ])
            .output()
            .map_err(|_| IntelligenceProviderError("财经快讯网络请求失败".into()))?;
        if !output.status.success() {
            return Err(IntelligenceProviderError("东方财富财经快讯暂不可用".into()));
        }
        let response: EastmoneyFlashResponse = serde_json::from_slice(&output.stdout)
            .map_err(|_| IntelligenceProviderError("东方财富财经快讯返回格式异常".into()))?;
        if response.code.as_deref() != Some("1") {
            return Err(IntelligenceProviderError(
                "东方财富财经快讯未返回成功状态".into(),
            ));
        }
        let fetched_at = Utc::now().to_rfc3339();
        let mut records = Vec::new();
        for item in response.data.and_then(|data| data.list).unwrap_or_default() {
            let title = item.title.unwrap_or_default().trim().to_owned();
            if title.is_empty() {
                continue;
            }
            let summary = item.summary.unwrap_or_default().trim().to_owned();
            let related = securities
                .iter()
                .filter(|security| {
                    title.contains(&security.symbol)
                        || summary.contains(&security.symbol)
                        || (!security.name.trim().is_empty()
                            && (title.contains(&security.name) || summary.contains(&security.name)))
                })
                .map(|security| security.id)
                .collect::<Vec<_>>();
            if related.is_empty() {
                continue;
            }
            let Some(published_at) = item.show_time.as_deref().and_then(parse_eastmoney_time)
            else {
                continue;
            };
            let code = item.code.unwrap_or_default();
            let url =
                (!code.is_empty()).then(|| format!("https://finance.eastmoney.com/a/{code}.html"));
            records.push(IntelligenceInput {
                title: title.clone(),
                summary: if summary.is_empty() { title } else { summary },
                source: self.name().into(),
                source_type: IntelligenceSourceType::News,
                source_url: url,
                published_at,
                fetched_at: fetched_at.clone(),
                credibility_level: CredibilityLevel::B,
                intelligence_type: "MARKET_FLASH".into(),
                security_ids: related,
                status: "ACTIVE".into(),
            });
        }
        Ok(records)
    }
}

/// Community collection is deliberately optional. It has no default network implementation
/// because Hero Call must not require Cookie scraping or store account identities.
pub struct OptionalCommunityProvider;
impl IntelligenceProvider for OptionalCommunityProvider {
    fn name(&self) -> &'static str {
        "可选社区舆情"
    }
    fn source_type(&self) -> IntelligenceSourceType {
        IntelligenceSourceType::Community
    }
    fn fetch(
        &self,
        _: &[DisclosureSecurity],
    ) -> Result<Vec<IntelligenceInput>, IntelligenceProviderError> {
        Err(IntelligenceProviderError(
            "社区舆情 Provider 未配置；未执行高风险 Cookie 抓取".into(),
        ))
    }
}

#[derive(Debug, Deserialize)]
struct EastmoneyFlashResponse {
    code: Option<String>,
    data: Option<EastmoneyFlashData>,
}
#[derive(Debug, Deserialize)]
struct EastmoneyFlashData {
    list: Option<Vec<EastmoneyFlashItem>>,
}
#[derive(Debug, Deserialize)]
struct EastmoneyFlashItem {
    code: Option<String>,
    #[serde(rename = "showTime")]
    show_time: Option<String>,
    title: Option<String>,
    summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceItemView {
    pub id: i64,
    pub title: String,
    pub summary: String,
    pub source: String,
    pub source_type: String,
    pub source_url: Option<String>,
    pub published_at: String,
    pub credibility_level: String,
    pub intelligence_type: String,
    pub topic_key: String,
    pub importance_score: i64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceTopicView {
    pub title: String,
    pub source_count: usize,
    pub source_types: Vec<String>,
    pub credibility_levels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SecurityIntelligenceView {
    pub security_id: i64,
    pub security_name: String,
    pub security_symbol: String,
    pub discussion_heat: String,
    pub sentiment: String,
    pub topics: Vec<IntelligenceTopicView>,
    pub important_items: Vec<IntelligenceItemView>,
    pub community_opinions: Vec<String>,
    pub rumors: Vec<IntelligenceItemView>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MarketIntelligenceView {
    pub securities: Vec<SecurityIntelligenceView>,
    pub partial_unavailable_sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RadarEventView {
    pub id: i64,
    pub event_type: String,
    pub title: String,
    pub event_time: String,
    pub timezone: String,
    pub source: String,
    pub source_url: Option<String>,
    pub related_security: Option<String>,
    pub credibility_level: String,
    pub potential_impact: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MarketRadarView {
    pub next_24_hours: Vec<RadarEventView>,
    pub next_3_days: Vec<RadarEventView>,
    pub next_7_days: Vec<RadarEventView>,
}

#[derive(Debug)]
pub enum IntelligenceServiceError {
    Database(DatabaseError),
    Validation(&'static str),
}
impl fmt::Display for IntelligenceServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(e) => write!(f, "database error: {e}"),
            Self::Validation(m) => f.write_str(m),
        }
    }
}
impl Error for IntelligenceServiceError {}
impl From<DatabaseError> for IntelligenceServiceError {
    fn from(value: DatabaseError) -> Self {
        Self::Database(value)
    }
}

pub struct IntelligenceService;
impl IntelligenceService {
    pub fn ingest(
        database: &DatabaseService,
        input: IntelligenceInput,
    ) -> Result<IntelligenceItem, IntelligenceServiceError> {
        if input.title.trim().is_empty()
            || input.summary.trim().is_empty()
            || input.security_ids.is_empty()
        {
            return Err(IntelligenceServiceError::Validation(
                "情报缺少标题、摘要或关联证券",
            ));
        }
        let dedup_key = dedup_key(&input);
        let topic_key = topic_key(&input.title);
        let source_type = input.source_type;
        let credibility_level = input.credibility_level;
        let security_ids = input.security_ids.clone();
        let item = database.upsert_intelligence_item(
            NewIntelligenceItem {
                title: input.title,
                summary: input.summary,
                source: input.source,
                source_type: input.source_type.as_str().into(),
                source_url: input.source_url,
                published_at: input.published_at,
                fetched_at: input.fetched_at,
                credibility_level: input.credibility_level.as_str().into(),
                intelligence_type: input.intelligence_type,
                dedup_key,
                topic_key,
                importance_score: input.credibility_level.score(),
                heat_score: 1,
                status: input.status,
            },
            &security_ids,
        )?;
        if matches!(
            source_type,
            IntelligenceSourceType::Official | IntelligenceSourceType::News
        ) && matches!(credibility_level, CredibilityLevel::A | CredibilityLevel::B)
        {
            database.mark_related_rumors_partially_confirmed(&item.topic_key, &security_ids)?;
        }
        Ok(item)
    }

    pub fn from_news_input(input: &NewsArticleInput) -> Option<IntelligenceInput> {
        let security_id = input.related_security_id?;
        let (source_type, credibility) = match input.source_type {
            NewsSourceType::Official => (IntelligenceSourceType::Official, CredibilityLevel::A),
            NewsSourceType::Media => (IntelligenceSourceType::News, CredibilityLevel::B),
            NewsSourceType::Community => (IntelligenceSourceType::Community, CredibilityLevel::D),
        };
        Some(IntelligenceInput {
            title: input.title.clone(),
            summary: input.summary.clone(),
            source: input.source.clone(),
            source_type,
            source_url: Some(input.url.clone()),
            published_at: input.published_at.clone(),
            fetched_at: input.fetch_time.clone(),
            credibility_level: credibility,
            intelligence_type: "DISCLOSURE".into(),
            security_ids: vec![security_id],
            status: "ACTIVE".into(),
        })
    }

    pub fn list_for_followed_securities(
        database: &DatabaseService,
    ) -> Result<MarketIntelligenceView, IntelligenceServiceError> {
        let records = database.list_intelligence_for_followed_securities()?;
        let mut by_security: HashMap<i64, Vec<IntelligenceItemWithSecurity>> = HashMap::new();
        for record in records {
            by_security
                .entry(record.security_id)
                .or_default()
                .push(record);
        }
        // Preserve the watchlist order and surface a transparent empty state for every followed
        // security. A lack of an item must not make the followed security itself disappear.
        let securities = database
            .list_watchlist_item_data()?
            .into_iter()
            .map(|watch| {
                let records = by_security.remove(&watch.security_id).unwrap_or_default();
                if records.is_empty() {
                    Self::empty_security_view(watch.security_id, watch.name, watch.symbol)
                } else {
                    Self::build_security_view(records)
                }
            })
            .collect();
        Ok(MarketIntelligenceView {
            securities,
            partial_unavailable_sources: vec!["社区舆情 Provider 未配置".into()],
        })
    }

    pub fn summary_for_run_and_security(
        database: &DatabaseService,
        run_id: i64,
        security_id: i64,
    ) -> Result<Value, IntelligenceServiceError> {
        let records = database.list_intelligence_for_run_and_security(run_id, security_id)?;
        let view = Self::build_security_view(records);
        let verified_facts = view
            .important_items
            .iter()
            .filter(|item| matches!(item.credibility_level.as_str(), "A" | "B"))
            .take(5)
            .cloned()
            .collect::<Vec<_>>();
        Ok(json!({
            "security": {"symbol": view.security_symbol, "name": view.security_name},
            "coreTopics": view.topics,
            "marketSentiment": view.sentiment,
            "discussionHeat": view.discussion_heat,
            "verifiedIntelligence": verified_facts,
            "communityOpinion": view.community_opinions,
            "rumors": view.rumors,
            "rules": {"communityIsNotFact": true, "rumorIsNotFact": true}
        }))
    }

    pub fn market_radar(events: Vec<EventView>) -> MarketRadarView {
        let now = Utc::now();
        let mut day1 = Vec::new();
        let mut day3 = Vec::new();
        let mut day7 = Vec::new();
        for event in events {
            let Ok(time) = DateTime::parse_from_rfc3339(&event.event_time) else {
                continue;
            };
            let hours = time
                .with_timezone(&Utc)
                .signed_duration_since(now)
                .num_hours();
            if !(0..=24 * 7).contains(&hours) {
                continue;
            }
            let credibility_level = if event.source.contains("公告")
                || event.source.contains("交易所")
                || event.source.contains("监管")
            {
                "A".into()
            } else {
                "C".into()
            };
            let view = RadarEventView {
                id: event.id,
                event_type: event.event_type,
                title: event.title,
                event_time: event.event_time,
                timezone: event.timezone,
                source: event.source,
                source_url: event.source_url,
                related_security: event.related_security,
                // Announcement/exchange sources have first-party provenance. Other imported or
                // user-supplied calendar sources remain traceable but must not be shown as A.
                credibility_level,
                potential_impact: "UNCERTAIN".into(),
            };
            if hours <= 24 {
                day1.push(view.clone());
            }
            if hours <= 24 * 3 {
                day3.push(view.clone());
            }
            day7.push(view);
        }
        MarketRadarView {
            next_24_hours: day1,
            next_3_days: day3,
            next_7_days: day7,
        }
    }

    fn empty_security_view(
        security_id: i64,
        security_name: String,
        security_symbol: String,
    ) -> SecurityIntelligenceView {
        SecurityIntelligenceView {
            security_id,
            security_name,
            security_symbol,
            discussion_heat: "NO_DATA".into(),
            sentiment: "暂无趋势判断".into(),
            topics: Vec::new(),
            important_items: Vec::new(),
            community_opinions: Vec::new(),
            rumors: Vec::new(),
            summary: "当前暂无值得关注的新情报。".into(),
        }
    }

    fn build_security_view(records: Vec<IntelligenceItemWithSecurity>) -> SecurityIntelligenceView {
        let first = records.first();
        let security_id = first.map(|item| item.security_id).unwrap_or_default();
        let security_name = first
            .map(|item| item.security_name.clone())
            .unwrap_or_else(|| "未确认标的".into());
        let security_symbol = first
            .map(|item| item.security_symbol.clone())
            .unwrap_or_default();
        // A source-level upsert already removes duplicate collection from the same provider.
        // Preserve independent sources here: collapsing an A confirmation and an E rumour into
        // one visible item would hide the rumour's verification status from the user.
        let mut deduped: HashMap<String, IntelligenceItem> = HashMap::new();
        for record in &records {
            deduped
                .entry(format!("{}|{}", record.item.dedup_key, record.item.source))
                .and_modify(|current| {
                    if record.item.importance_score > current.importance_score {
                        *current = record.item.clone();
                    }
                })
                .or_insert_with(|| record.item.clone());
        }
        let mut items = deduped.into_values().collect::<Vec<_>>();
        items.sort_by(|left, right| {
            right
                .importance_score
                .cmp(&left.importance_score)
                .then_with(|| right.published_at.cmp(&left.published_at))
        });
        let topics = build_topics(&records);
        let community_opinions = records
            .iter()
            .filter(|record| matches!(record.item.source_type.as_str(), "COMMUNITY" | "SOCIAL"))
            .take(3)
            .map(|record| record.item.summary.clone())
            .collect::<Vec<_>>();
        let rumors = items
            .iter()
            .filter(|item| item.credibility_level == "E" || item.source_type == "RUMOR")
            .map(item_view)
            .collect::<Vec<_>>();
        let direction = sentiment(&records);
        let discussion_heat = heat(records.len());
        let topic_names = topics
            .iter()
            .take(3)
            .map(|topic| topic.title.as_str())
            .collect::<Vec<_>>()
            .join("、");
        let summary = if records.is_empty() {
            "当前暂无值得关注的新情报。".into()
        } else {
            format!(
                "当前已聚合 {} 条可追溯情报，核心主题为{}；舆情方向为{}。",
                records.len(),
                if topic_names.is_empty() {
                    "暂无主题"
                } else {
                    &topic_names
                },
                direction
            )
        };
        SecurityIntelligenceView {
            security_id,
            security_name,
            security_symbol,
            discussion_heat,
            sentiment: direction,
            topics: topics.into_iter().take(5).collect(),
            important_items: items.iter().take(8).map(item_view).collect(),
            community_opinions,
            rumors,
            summary,
        }
    }
}

fn item_view(item: &IntelligenceItem) -> IntelligenceItemView {
    IntelligenceItemView {
        id: item.id,
        title: item.title.clone(),
        summary: item.summary.clone(),
        source: item.source.clone(),
        source_type: item.source_type.clone(),
        source_url: item.source_url.clone(),
        published_at: item.published_at.clone(),
        credibility_level: item.credibility_level.clone(),
        intelligence_type: item.intelligence_type.clone(),
        topic_key: item.topic_key.clone(),
        importance_score: item.importance_score,
        status: item.status.clone(),
    }
}

fn build_topics(records: &[IntelligenceItemWithSecurity]) -> Vec<IntelligenceTopicView> {
    let mut topics: BTreeMap<String, Vec<&IntelligenceItem>> = BTreeMap::new();
    for record in records {
        topics
            .entry(record.item.topic_key.clone())
            .or_default()
            .push(&record.item);
    }
    let mut result = topics
        .into_iter()
        .map(|(title, values)| {
            let sources = values
                .iter()
                .map(|item| item.source.clone())
                .collect::<HashSet<_>>();
            let mut source_types = values
                .iter()
                .map(|item| item.source_type.clone())
                .collect::<HashSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            source_types.sort();
            let mut credibility_levels = values
                .iter()
                .map(|item| item.credibility_level.clone())
                .collect::<HashSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            credibility_levels.sort();
            IntelligenceTopicView {
                title,
                source_count: sources.len(),
                source_types,
                credibility_levels,
            }
        })
        .collect::<Vec<_>>();
    result.sort_by(|a, b| {
        b.source_count
            .cmp(&a.source_count)
            .then_with(|| a.title.cmp(&b.title))
    });
    result
}

fn heat(count: usize) -> String {
    match count {
        0 => "NO_DATA",
        1 => "LOW",
        2..=3 => "NORMAL",
        4..=6 => "HIGH",
        _ => "SURGING",
    }
    .into()
}
fn sentiment(records: &[IntelligenceItemWithSecurity]) -> String {
    let community = records
        .iter()
        .filter(|record| matches!(record.item.source_type.as_str(), "COMMUNITY" | "SOCIAL"))
        .collect::<Vec<_>>();
    if community.is_empty() {
        return "暂无趋势判断".into();
    }
    let positive = ["上涨", "增长", "中标", "改善", "利好", "突破"]
        .iter()
        .filter(|word| {
            community
                .iter()
                .any(|item| item.item.title.contains(**word) || item.item.summary.contains(**word))
        })
        .count();
    let negative = ["下跌", "下滑", "风险", "处罚", "亏损", "减持"]
        .iter()
        .filter(|word| {
            community
                .iter()
                .any(|item| item.item.title.contains(**word) || item.item.summary.contains(**word))
        })
        .count();
    match positive.cmp(&negative) {
        std::cmp::Ordering::Greater => "偏积极",
        std::cmp::Ordering::Less => "偏谨慎",
        std::cmp::Ordering::Equal if positive > 0 => "分歧",
        _ => "中性",
    }
    .into()
}

fn dedup_key(input: &IntelligenceInput) -> String {
    input
        .source_url
        .clone()
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| {
            format!(
                "{}|{}",
                normalize(&input.title),
                input.published_at.get(..10).unwrap_or(&input.published_at)
            )
        })
}
fn topic_key(title: &str) -> String {
    normalize(title).chars().take(24).collect()
}
fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|character| {
            character.is_alphanumeric() || ('\u{4e00}'..='\u{9fff}').contains(character)
        })
        .collect::<String>()
        .to_lowercase()
}
fn parse_eastmoney_time(value: &str) -> Option<String> {
    let canonical = format!("{}+08:00", value.trim().replace(' ', "T"));
    DateTime::<FixedOffset>::parse_from_rfc3339(&canonical)
        .ok()
        .map(|time| time.with_timezone(&Utc).to_rfc3339())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::service::NewSecurity;
    use chrono::Duration;

    fn input(
        title: &str,
        source: &str,
        kind: IntelligenceSourceType,
        credibility: CredibilityLevel,
        security_id: i64,
    ) -> IntelligenceInput {
        IntelligenceInput {
            title: title.into(),
            summary: title.into(),
            source: source.into(),
            source_type: kind,
            source_url: None,
            published_at: "2026-08-11T08:00:00Z".into(),
            fetched_at: "2026-08-11T08:01:00Z".into(),
            credibility_level: credibility,
            intelligence_type: "TEST".into(),
            security_ids: vec![security_id],
            status: if kind == IntelligenceSourceType::Rumor {
                "UNVERIFIED".into()
            } else {
                "ACTIVE".into()
            },
        }
    }
    #[test]
    fn deduplicates_same_source_item_and_keeps_community_out_of_verified_facts() {
        let db = DatabaseService::open_in_memory().unwrap();
        let security = db
            .create_security(NewSecurity {
                symbol: "300209".into(),
                name: "测试科技".into(),
                market: "SZSE".into(),
                exchange: "SZSE".into(),
                security_type: "STOCK".into(),
                industry: None,
                concepts_json: "[]".into(),
                trade_rule: "UNKNOWN".into(),
            })
            .unwrap();
        db.create_watchlist_item(security.id).unwrap();
        let first = IntelligenceService::ingest(
            &db,
            input(
                "官方公告",
                "交易所",
                IntelligenceSourceType::Official,
                CredibilityLevel::A,
                security.id,
            ),
        )
        .unwrap();
        let second = IntelligenceService::ingest(
            &db,
            input(
                "官方公告",
                "交易所",
                IntelligenceSourceType::Official,
                CredibilityLevel::A,
                security.id,
            ),
        )
        .unwrap();
        assert_eq!(first.id, second.id);
        IntelligenceService::ingest(
            &db,
            input(
                "社区传闻",
                "社区",
                IntelligenceSourceType::Community,
                CredibilityLevel::D,
                security.id,
            ),
        )
        .unwrap();
        let view = IntelligenceService::list_for_followed_securities(&db).unwrap();
        assert_eq!(view.securities[0].important_items.len(), 2);
        let run = db
            .create_manual_refresh_run(crate::database::service::NewManualRefreshRun {
                started_at: "2026-08-11T08:00:00Z".into(),
                completed_at: "2026-08-11T08:01:00Z".into(),
                holdings_snapshot_id: None,
                indices_snapshot_id: None,
                portfolio_json: "[]".into(),
                status: "NO_DATA".into(),
            })
            .unwrap();
        db.link_intelligence_items_to_manual_refresh_run(run.id, &[first.id, second.id])
            .unwrap();
        let summary =
            IntelligenceService::summary_for_run_and_security(&db, run.id, security.id).unwrap();
        assert_eq!(summary["verifiedIntelligence"].as_array().unwrap().len(), 1);
    }
    #[test]
    fn rumor_is_explicitly_unverified_and_radar_groups_future_windows() {
        let rumor = input(
            "市场传闻",
            "传闻",
            IntelligenceSourceType::Rumor,
            CredibilityLevel::E,
            1,
        );
        assert_eq!(rumor.status, "UNVERIFIED");
        let event = EventView {
            id: 1,
            event_type: "EARNINGS".into(),
            title: "财报披露".into(),
            event_time: (Utc::now() + Duration::hours(20)).to_rfc3339(),
            timezone: "Asia/Shanghai".into(),
            source: "交易所".into(),
            source_url: None,
            status: "CONFIRMED".into(),
            related_security: Some("测试科技 (300209)".into()),
            holding_related: true,
        };
        let radar = IntelligenceService::market_radar(vec![event]);
        assert_eq!(radar.next_24_hours.len(), 1);
        assert_eq!(radar.next_3_days.len(), 1);
        assert_eq!(radar.next_7_days.len(), 1);
    }

    #[test]
    fn official_confirmation_marks_only_the_matching_rumor_as_partially_confirmed() {
        let db = DatabaseService::open_in_memory().unwrap();
        let security = db
            .create_security(NewSecurity {
                symbol: "300209".into(),
                name: "测试科技".into(),
                market: "SZSE".into(),
                exchange: "SZSE".into(),
                security_type: "STOCK".into(),
                industry: None,
                concepts_json: "[]".into(),
                trade_rule: "UNKNOWN".into(),
            })
            .unwrap();
        db.create_watchlist_item(security.id).unwrap();
        let rumor = IntelligenceService::ingest(
            &db,
            input(
                "同一事项",
                "社区",
                IntelligenceSourceType::Rumor,
                CredibilityLevel::E,
                security.id,
            ),
        )
        .unwrap();
        IntelligenceService::ingest(
            &db,
            input(
                "同一事项",
                "交易所",
                IntelligenceSourceType::Official,
                CredibilityLevel::A,
                security.id,
            ),
        )
        .unwrap();
        let view = IntelligenceService::list_for_followed_securities(&db).unwrap();
        let status = view.securities[0]
            .rumors
            .iter()
            .find(|item| item.id == rumor.id)
            .expect("rumor remains auditable")
            .status
            .as_str();
        assert_eq!(status, "PARTIALLY_CONFIRMED");
    }

    #[test]
    fn followed_security_without_intelligence_has_an_explicit_empty_state() {
        let db = DatabaseService::open_in_memory().unwrap();
        let security = db
            .create_security(NewSecurity {
                symbol: "300209".into(),
                name: "测试科技".into(),
                market: "SZSE".into(),
                exchange: "SZSE".into(),
                security_type: "STOCK".into(),
                industry: None,
                concepts_json: "[]".into(),
                trade_rule: "UNKNOWN".into(),
            })
            .unwrap();
        db.create_watchlist_item(security.id).unwrap();
        let view = IntelligenceService::list_for_followed_securities(&db).unwrap();
        assert_eq!(view.securities.len(), 1);
        assert_eq!(view.securities[0].discussion_heat, "NO_DATA");
        assert!(view.securities[0].important_items.is_empty());
    }
}
