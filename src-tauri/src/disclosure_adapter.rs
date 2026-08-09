//! Public Eastmoney announcement adapter used only during an explicit manual refresh.
//!
//! It preserves the source's disclosure title, display time and canonical detail link. Failed
//! requests return an adapter error; callers must expose `NO_DATA` instead of manufacturing news.

use std::{error::Error, fmt, process::Command};

use chrono::{DateTime, FixedOffset, SecondsFormat, Utc};
use serde::Deserialize;

use crate::{
    event_service::{
        EventAdapterError, EventDataAdapter, EventInput, EventSource, EventStatus, EventType,
    },
    market_service::MarketSecurity,
    news_service::{
        NewsAdapterError, NewsArticleInput, NewsDataAdapter, NewsDataSource, NewsSourceType,
    },
};

const EASTMONEY_ANNOUNCEMENT_URL: &str = "https://np-anotice-stock.eastmoney.com/api/security/ann";
const EASTMONEY_ANNOUNCEMENT_DETAIL: &str = "https://data.eastmoney.com/notices/detail";

#[derive(Debug, Clone)]
pub struct DisclosureSecurity {
    pub id: i64,
    pub symbol: String,
}

impl From<&MarketSecurity> for DisclosureSecurity {
    fn from(value: &MarketSecurity) -> Self {
        Self {
            id: value.security_id,
            symbol: value.symbol.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisclosureAdapterError {
    pub message: String,
}
impl fmt::Display for DisclosureAdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl Error for DisclosureAdapterError {}

#[derive(Debug, Clone, Default)]
pub struct EastmoneyAnnouncementAdapter;

impl EastmoneyAnnouncementAdapter {
    pub fn fetch_for_securities(
        &self,
        securities: &[DisclosureSecurity],
    ) -> Result<Vec<Disclosure>, DisclosureAdapterError> {
        let fetched_at = Utc::now();
        let mut all = Vec::new();
        for security in securities {
            if !security.symbol.chars().all(|ch| ch.is_ascii_digit()) {
                continue;
            }
            let url = format!("{EASTMONEY_ANNOUNCEMENT_URL}?sr=-1&page_size=20&page_index=1&ann_type=A&stock_list={}", security.symbol);
            let body = curl_json(&url)?;
            let response: EastmoneyResponse =
                serde_json::from_slice(&body).map_err(|_| DisclosureAdapterError {
                    message: "东方财富公告数据格式异常".into(),
                })?;
            if response.success != 1 {
                return Err(DisclosureAdapterError {
                    message: "东方财富公告数据源未返回成功状态".into(),
                });
            }
            for item in response.data.list {
                if !item
                    .codes
                    .iter()
                    .any(|code| code.stock_code == security.symbol)
                {
                    continue;
                }
                let published_at = parse_source_time(&item.display_time).ok_or_else(|| {
                    DisclosureAdapterError {
                        message: "东方财富公告缺少可确认披露时间".into(),
                    }
                })?;
                let title = item.title.trim().to_owned();
                if title.is_empty() || item.art_code.trim().is_empty() {
                    continue;
                }
                all.push(Disclosure {
                    security: security.clone(),
                    title: title.clone(),
                    published_at,
                    fetched_at,
                    url: format!(
                        "{EASTMONEY_ANNOUNCEMENT_DETAIL}/{}/{}.html",
                        security.symbol, item.art_code
                    ),
                });
            }
        }
        Ok(all)
    }
}

impl NewsDataAdapter for EastmoneyAnnouncementAdapter {
    fn source(&self) -> NewsDataSource {
        NewsDataSource {
            name: "东方财富公告".into(),
            source_type: NewsSourceType::Media,
        }
    }
    fn fetch_articles(
        &self,
        securities: &[DisclosureSecurity],
    ) -> Result<Vec<NewsArticleInput>, NewsAdapterError> {
        self.fetch_for_securities(securities)
            .map_err(|error| NewsAdapterError {
                message: error.message,
            })
            .map(|items| items.into_iter().map(Disclosure::as_news).collect())
    }
}

impl EventDataAdapter for EastmoneyAnnouncementAdapter {
    fn source(&self) -> EventSource {
        EventSource {
            name: "东方财富公告".into(),
        }
    }
    fn fetch_events(
        &self,
        securities: &[DisclosureSecurity],
    ) -> Result<Vec<EventInput>, EventAdapterError> {
        self.fetch_for_securities(securities)
            .map_err(|error| EventAdapterError {
                message: error.message,
            })
            .map(|items| items.into_iter().map(Disclosure::as_event).collect())
    }
}

#[derive(Debug, Clone)]
pub struct Disclosure {
    security: DisclosureSecurity,
    title: String,
    published_at: DateTime<Utc>,
    fetched_at: DateTime<Utc>,
    url: String,
}
impl Disclosure {
    fn as_news(self) -> NewsArticleInput {
        NewsArticleInput {
            title: self.title.clone(),
            source: "东方财富公告".into(),
            source_type: NewsSourceType::Media,
            published_at: self
                .published_at
                .to_rfc3339_opts(SecondsFormat::Millis, true),
            fetch_time: self.fetched_at.to_rfc3339_opts(SecondsFormat::Millis, true),
            // The provider list response contains no body abstract. Keep only its verbatim title.
            summary: self.title,
            url: self.url,
            related_security_id: Some(self.security.id),
        }
    }
    fn as_event(self) -> EventInput {
        EventInput {
            event_type: classify_event(&self.title),
            title: self.title,
            security_id: Some(self.security.id),
            event_time: self
                .published_at
                .to_rfc3339_opts(SecondsFormat::Millis, true),
            timezone: "Asia/Shanghai".into(),
            source: "东方财富公告".into(),
            source_url: Some(self.url),
            status: EventStatus::Confirmed,
        }
    }
}

fn classify_event(title: &str) -> EventType {
    if ["年度报告", "半年度报告", "季度报告", "业绩预告", "业绩快报"]
        .iter()
        .any(|keyword| title.contains(keyword))
    {
        EventType::Earnings
    } else if [
        "重大", "重组", "收购", "立案", "诉讼", "处罚", "停牌", "终止", "风险",
    ]
    .iter()
    .any(|keyword| title.contains(keyword))
    {
        EventType::MajorMatter
    } else {
        EventType::CompanyAnnouncement
    }
}

fn parse_source_time(value: &str) -> Option<DateTime<Utc>> {
    let (prefix, milliseconds) = value.trim().rsplit_once(':')?;
    let canonical = format!("{prefix}.{milliseconds}+08:00");
    DateTime::<FixedOffset>::parse_from_rfc3339(&canonical)
        .ok()
        .map(|time| time.with_timezone(&Utc))
}

fn curl_json(url: &str) -> Result<Vec<u8>, DisclosureAdapterError> {
    let output = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--max-time",
            "15",
            "--header",
            "User-Agent: Hero Call/0.9",
            url,
        ])
        .output()
        .map_err(|_| DisclosureAdapterError {
            message: "系统 HTTP 传输不可用".into(),
        })?;
    if !output.status.success() {
        return Err(DisclosureAdapterError {
            message: "东方财富公告请求失败".into(),
        });
    }
    Ok(output.stdout)
}

#[derive(Debug, Deserialize)]
struct EastmoneyResponse {
    success: i64,
    data: EastmoneyData,
}
#[derive(Debug, Deserialize)]
struct EastmoneyData {
    list: Vec<EastmoneyAnnouncement>,
}
#[derive(Debug, Deserialize)]
struct EastmoneyAnnouncement {
    art_code: String,
    codes: Vec<EastmoneyCode>,
    display_time: String,
    title: String,
}
#[derive(Debug, Deserialize)]
struct EastmoneyCode {
    stock_code: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_time_and_event_type_are_preserved_without_inference() {
        assert_eq!(
            parse_source_time("2026-08-07 19:03:06:389")
                .unwrap()
                .to_rfc3339(),
            "2026-08-07T11:03:06.389+00:00"
        );
        assert_eq!(classify_event("公司2026年半年度报告"), EventType::Earnings);
        assert_eq!(
            classify_event("关于重大资产重组的公告"),
            EventType::MajorMatter
        );
        assert_eq!(
            classify_event("董事会决议公告"),
            EventType::CompanyAnnouncement
        );
    }
}
