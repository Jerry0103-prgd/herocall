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
            let Some(stock_code) = eastmoney_stock_code(&security.symbol) else {
                news_diagnostic(format!(
                    "security_id={} stored_symbol={} request_skipped=unsupported_symbol_format",
                    security.id, security.symbol
                ));
                continue;
            };
            let url = format!("{EASTMONEY_ANNOUNCEMENT_URL}?sr=-1&page_size=20&page_index=1&ann_type=A&stock_list={stock_code}");
            news_diagnostic(format!(
                "security_id={} stored_symbol={} stock_code={stock_code} request_url={url}",
                security.id, security.symbol
            ));
            let response = curl_json(&url)?;
            let response: EastmoneyResponse =
                serde_json::from_slice(&response.body).map_err(|error| {
                    news_diagnostic(format!(
                        "security_id={} request_stock_list={stock_code} json_parse_error={error}",
                        security.id
                    ));
                    DisclosureAdapterError {
                        message: "东方财富公告数据格式异常".into(),
                    }
                })?;
            if response.success != 1 {
                news_diagnostic(format!(
                    "security_id={} request_stock_list={stock_code} provider_success={}",
                    security.id, response.success
                ));
                return Err(DisclosureAdapterError {
                    message: "东方财富公告数据源未返回成功状态".into(),
                });
            }
            let source_count = response.data.list.len();
            let before_matched = all.len();
            for item in response.data.list {
                if !item.codes.iter().any(|code| code.stock_code == stock_code) {
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
            news_diagnostic(format!(
                "security_id={} stock_code={stock_code} returned_count={source_count} parsed_count={}",
                security.id,
                all.len() - before_matched
            ));
        }
        news_diagnostic(format!(
            "requested_securities={} returned_disclosures={}",
            securities.len(),
            all.len()
        ));
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

struct HttpJsonResponse {
    body: Vec<u8>,
}

fn curl_json(url: &str) -> Result<HttpJsonResponse, DisclosureAdapterError> {
    let output = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--max-time",
            "15",
            "--header",
            "User-Agent: Hero Call/0.9",
            "--write-out",
            "\n__HERO_CALL_HTTP_STATUS__:%{http_code}",
            url,
        ])
        .output()
        .map_err(|_| DisclosureAdapterError {
            message: "系统 HTTP 传输不可用".into(),
        })?;
    if !output.status.success() {
        let transport_error = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        news_diagnostic(format!("eastmoney_transport_error={transport_error}"));
        return Err(DisclosureAdapterError {
            message: "东方财富公告请求失败".into(),
        });
    }
    let marker = b"\n__HERO_CALL_HTTP_STATUS__:";
    let Some(marker_position) = output
        .stdout
        .windows(marker.len())
        .rposition(|window| window == marker)
    else {
        news_diagnostic("eastmoney_http_status=unavailable".into());
        return Err(DisclosureAdapterError {
            message: "东方财富公告请求未返回 HTTP 状态".into(),
        });
    };
    let status = std::str::from_utf8(&output.stdout[marker_position + marker.len()..])
        .ok()
        .and_then(|value| value.trim().parse::<u16>().ok());
    let Some(status) = status else {
        news_diagnostic("eastmoney_http_status=unparseable".into());
        return Err(DisclosureAdapterError {
            message: "东方财富公告请求 HTTP 状态异常".into(),
        });
    };
    news_diagnostic(format!("http_status={status}"));
    if !(200..300).contains(&status) {
        return Err(DisclosureAdapterError {
            message: format!("东方财富公告请求失败（HTTP {status}）"),
        });
    }
    Ok(HttpJsonResponse {
        body: output.stdout[..marker_position].to_vec(),
    })
}

fn eastmoney_stock_code(symbol: &str) -> Option<&str> {
    let code = symbol
        .trim()
        .split_once('.')
        .map_or(symbol.trim(), |(code, _)| code);
    (code.len() == 6 && code.chars().all(|character| character.is_ascii_digit())).then_some(code)
}

fn news_diagnostic(message: String) {
    eprintln!("[hero-call][news-diagnostic] {message}");
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

    #[test]
    fn eastmoney_stock_code_accepts_plain_and_exchange_qualified_a_share_symbols() {
        assert_eq!(eastmoney_stock_code("300209"), Some("300209"));
        assert_eq!(eastmoney_stock_code("300209.SZ"), Some("300209"));
        assert_eq!(eastmoney_stock_code("600330.SH"), Some("600330"));
        assert_eq!(eastmoney_stock_code("HK.00700"), None);
        assert_eq!(eastmoney_stock_code("30020"), None);
    }
}
