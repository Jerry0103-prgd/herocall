//! Safety-constrained DeepSeek explanations for one saved manual market snapshot.
//!
//! The UI never contacts a provider. A successful response is validated before either the
//! immutable context or output is persisted; failed, malformed, or unsafe responses are not kept.

use std::{
    error::Error,
    fmt, fs,
    io::Write,
    process::{Command, Stdio},
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    database::service::{
        AiReview, DatabaseError, DatabaseService, NewAiReview, NewAiReviewContext,
    },
    event_service::{EventService, EventServiceError},
    news_service::{NewsService, NewsServiceError},
    review_service::{DailyReviewView, ReviewService, ReviewServiceError},
    secure_storage::{get_deepseek_status, load_deepseek_api_key_for_adapter},
};

const PROMPT_VERSION: &str = "deepseek-research-report-v2";
const DEEPSEEK_CHAT_COMPLETIONS_URL: &str = "https://api.deepseek.com/chat/completions";
const DEEPSEEK_MODEL: &str = "deepseek-chat";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiServiceStatusView {
    pub configured: bool,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiReviewSections {
    pub facts: Vec<String>,
    pub inferences: Vec<String>,
    pub risks: Vec<String>,
    #[serde(default, rename = "stock_status")]
    pub stock_status: Option<String>,
    #[serde(default, rename = "market_analysis")]
    pub market_analysis: Option<String>,
    #[serde(default, rename = "sector_analysis")]
    pub sector_analysis: Option<String>,
    #[serde(default, rename = "news_analysis")]
    pub news_analysis: Option<String>,
    #[serde(default, rename = "technical_analysis")]
    pub technical_analysis: Option<String>,
    #[serde(default, rename = "strategy_reference")]
    pub strategy_reference: Option<String>,
    #[serde(default, rename = "conclusion")]
    pub conclusion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiResearchReport {
    pub stock_status: String,
    pub market_analysis: String,
    pub sector_analysis: String,
    pub news_analysis: String,
    pub technical_analysis: String,
    pub strategy_reference: String,
    pub conclusion: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiReviewView {
    pub id: i64,
    pub review_id: i64,
    pub model: String,
    pub prompt_version: String,
    pub facts: Vec<String>,
    pub inferences: Vec<String>,
    pub risks: Vec<String>,
    pub report: Option<AiResearchReport>,
    pub created_at: String,
}

/// This is the exact frozen input boundary sent to DeepSeek. News and events are explicit NO_DATA
/// objects in E1; no provider is allowed to silently supplement those fields.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiReviewInput {
    pub prompt_version: String,
    pub manual_refresh_run_id: i64,
    pub daily_review: DailyReviewView,
    pub portfolio: Value,
    pub market: Value,
    pub news: Value,
    pub events: Value,
}

pub trait AiProviderAdapter {
    fn model(&self) -> &str;
    fn provider(&self) -> &str {
        "TEST"
    }
    fn generate(&self, input: &AiReviewInput) -> Result<AiReviewSections, AiProviderError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiProviderError {
    message: String,
}
impl AiProviderError {
    fn unavailable() -> Self {
        Self {
            message: "DeepSeek 网络请求失败".into(),
        }
    }
    fn invalid_response(reason: impl fmt::Display) -> Self {
        Self {
            message: format!("DeepSeek 返回格式错误：{reason}"),
        }
    }
    fn http_error(status: u16, body: &str) -> Self {
        let category = match status {
            401 | 403 => "DeepSeek API认证失败",
            429 => "DeepSeek API请求过于频繁",
            _ => "DeepSeek API请求失败",
        };
        let detail = (!body.is_empty()).then(|| format!("：{body}"));
        Self {
            message: format!("{category}（HTTP {status}）{}", detail.unwrap_or_default()),
        }
    }
}
impl fmt::Display for AiProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl Error for AiProviderError {}

/// DeepSeek's OpenAI-compatible non-streaming Chat Completions endpoint. The Keychain value is
/// only held in this Rust adapter and is passed to curl via stdin, never as an argument or log.
pub struct DeepSeekProviderAdapter {
    api_key: String,
}
impl DeepSeekProviderAdapter {
    fn from_keychain() -> Result<Self, AiServiceError> {
        let key = load_deepseek_api_key_for_adapter().map_err(|_| {
            ai_diagnostic("keychain_read=false");
            AiServiceError::KeychainUnavailable
        })?;
        ai_diagnostic(format!("keychain_read={}", key.is_some()));
        Self::from_key(key)
    }
    fn from_key(key: Option<String>) -> Result<Self, AiServiceError> {
        key.filter(|value| !value.trim().is_empty())
            .map(|api_key| Self { api_key })
            .ok_or(AiServiceError::NotConfigured)
    }
}
impl AiProviderAdapter for DeepSeekProviderAdapter {
    fn model(&self) -> &str {
        DEEPSEEK_MODEL
    }
    fn provider(&self) -> &str {
        "DEEPSEEK"
    }
    fn generate(&self, input: &AiReviewInput) -> Result<AiReviewSections, AiProviderError> {
        let prompt = serde_json::to_string(input).map_err(|_| AiProviderError::unavailable())?;
        let response = curl_ai_request(
            DEEPSEEK_CHAT_COMPLETIONS_URL,
            &self.api_key,
            &json!({
                "model": DEEPSEEK_MODEL,
                "stream": false,
                "temperature": 0,
                "response_format": { "type": "json_object" },
                "messages": [
                  { "role": "system", "content": "你是个人A股投研复盘的解释助手。仅依据输入 JSON，输出严格 JSON 对象，必须包含：facts:[string]、inferences:[string]、risks:[string]、stock_status:string、market_analysis:string、sector_analysis:string、news_analysis:string、technical_analysis:string、strategy_reference:string、conclusion:string。FACTS 只陈述有来源和时间的输入事实；INFERENCES 必须是基于这些事实的解释；RISKS 只列不确定性、数据缺失或需核验事项。七项报告依次为：当前个股情况、当日市场整体情绪和板块情况、个股所属板块分析、个股消息面分析、个股技术面分析、策略参考、综合结论。无行业、新闻或技术指标时必须明确“暂无数据/未确认”，不得补造内容。策略参考只能描述持有逻辑、风险关注和后续观察条件；综合结论只能总结当前判断。社区观点不能作为事实。严禁买入、卖出、加仓、减仓、建仓、清仓、推荐、目标价、收益预测、收益承诺、保证收益、必涨，且不得补造事实。" },
                  { "role": "user", "content": prompt }
                ]
            }),
        )?;
        let content = response
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .ok_or_else(|| AiProviderError::invalid_response("缺少 choices[0].message.content"))?;
        parse_and_validate_provider_sections(content).map_err(|error| {
            ai_diagnostic(format!("json_parse_failed=true reason={error}"));
            AiProviderError::invalid_response(error)
        })
    }
}

#[derive(Debug)]
pub enum AiServiceError {
    Database(DatabaseError),
    Review(ReviewServiceError),
    Provider(AiProviderError),
    Serialization(serde_json::Error),
    InvalidOutput(&'static str),
    NotConfigured,
    KeychainUnavailable,
    NoManualSnapshot,
    Context(String),
}
impl fmt::Display for AiServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(e) => write!(f, "database error: {e}"),
            Self::Review(e) => write!(f, "review error: {e}"),
            Self::Provider(e) => write!(f, "provider error: {e}"),
            Self::Serialization(e) => write!(f, "AI review serialization error: {e}"),
            Self::InvalidOutput(m) => f.write_str(m),
            Self::NotConfigured => f.write_str("AI服务未配置"),
            Self::KeychainUnavailable => f.write_str("无法读取系统钥匙串中的 DeepSeek API Key"),
            Self::NoManualSnapshot => f.write_str("请先更新今日市场快照"),
            Self::Context(message) => write!(f, "AI复盘上下文读取失败：{message}"),
        }
    }
}
impl Error for AiServiceError {}
impl From<DatabaseError> for AiServiceError {
    fn from(e: DatabaseError) -> Self {
        Self::Database(e)
    }
}
impl From<ReviewServiceError> for AiServiceError {
    fn from(e: ReviewServiceError) -> Self {
        Self::Review(e)
    }
}
impl From<AiProviderError> for AiServiceError {
    fn from(e: AiProviderError) -> Self {
        Self::Provider(e)
    }
}
impl From<serde_json::Error> for AiServiceError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serialization(e)
    }
}
impl From<NewsServiceError> for AiServiceError {
    fn from(error: NewsServiceError) -> Self {
        Self::Context(error.to_string())
    }
}
impl From<EventServiceError> for AiServiceError {
    fn from(error: EventServiceError) -> Self {
        Self::Context(error.to_string())
    }
}

pub struct AiService;
impl AiService {
    pub fn status() -> AiServiceStatusView {
        let configured = get_deepseek_status()
            .map(|view| view.status == "已配置")
            .unwrap_or(false);
        AiServiceStatusView {
            configured,
            model: configured.then(|| DEEPSEEK_MODEL.into()),
        }
    }
    pub fn generate_from_runtime(
        database: &DatabaseService,
        review_date: &str,
    ) -> Result<AiReviewView, AiServiceError> {
        let result = (|| {
            let provider = DeepSeekProviderAdapter::from_keychain()?;
            Self::generate_with_provider(database, review_date, &provider)
        })();
        if let Err(error) = &result {
            ai_diagnostic(format!("rust_final_error={error}"));
        }
        result
    }
    pub fn generate_with_provider<P: AiProviderAdapter>(
        database: &DatabaseService,
        review_date: &str,
        provider: &P,
    ) -> Result<AiReviewView, AiServiceError> {
        let daily_review = ReviewService::get(database, review_date)?;
        let run = database
            .latest_manual_refresh_run()?
            .ok_or(AiServiceError::NoManualSnapshot)?;
        ai_diagnostic(format!("manual_refresh_run_id={}", run.id));
        let portfolio: Value = serde_json::from_str(&run.portfolio_json)?;
        let holding_quotes = database.list_market_quotes_for_snapshot(run.holdings_snapshot_id)?;
        let index_quotes =
            database.list_market_index_quotes_for_snapshot(run.indices_snapshot_id)?;
        let market = json!({
            "holdings": holding_quotes,
            "indices": index_quotes,
            "snapshotCompletedAt": run.completed_at,
        });
        let news_items = NewsService::list_for_manual_refresh_run(database, run.id)?;
        let event_items = EventService::list_for_manual_refresh_run(database, run.id)?;
        let input = AiReviewInput {
            prompt_version: PROMPT_VERSION.into(),
            manual_refresh_run_id: run.id,
            daily_review,
            portfolio,
            market,
            news: json!({"status": if news_items.is_empty() { "NO_DATA" } else { "AVAILABLE" }, "items": news_items}),
            events: json!({"status": if event_items.is_empty() { "NO_DATA" } else { "AVAILABLE" }, "items": event_items}),
        };
        ai_diagnostic(format!(
            "ai_context_generated=true manual_refresh_run_id={} holding_quotes={} index_quotes={} news_items={} event_items={}",
            input.manual_refresh_run_id,
            input.market["holdings"].as_array().map_or(0, Vec::len),
            input.market["indices"].as_array().map_or(0, Vec::len),
            input.news["items"].as_array().map_or(0, Vec::len),
            input.events["items"].as_array().map_or(0, Vec::len),
        ));
        // Provider and safety validation happen before persistence. Therefore every failure leaves
        // neither an AI review nor a context record behind.
        let sections = provider.generate(&input)?;
        validate_sections(&sections)?;
        let report = report_from_sections(&sections)?;
        let context_id = database.create_ai_review_context(NewAiReviewContext {
            review_id: input.daily_review.id,
            manual_refresh_run_id: input.manual_refresh_run_id,
            portfolio_json: serde_json::to_string(&input.portfolio)?,
            market_json: serde_json::to_string(&input.market)?,
            news_json: serde_json::to_string(&input.news)?,
            events_json: serde_json::to_string(&input.events)?,
        })?;
        ai_diagnostic(format!(
            "ai_review_context_written=true context_id={context_id}"
        ));
        let stored = database.create_ai_review(NewAiReview {
            review_id: input.daily_review.id,
            model: provider.model().into(),
            prompt_version: input.prompt_version,
            context_id,
            provider: provider.provider().into(),
            facts: serde_json::to_string(&sections.facts)?,
            inferences: serde_json::to_string(&sections.inferences)?,
            risks: serde_json::to_string(&sections.risks)?,
            report_json: Some(serde_json::to_string(&report)?),
        })?;
        Self::view_from_record(stored)
    }
    pub fn latest_for_review(
        database: &DatabaseService,
        review_id: i64,
    ) -> Result<Option<AiReviewView>, AiServiceError> {
        database
            .latest_ai_review_for_daily_review(review_id)?
            .map(Self::view_from_record)
            .transpose()
    }
    fn view_from_record(record: AiReview) -> Result<AiReviewView, AiServiceError> {
        let sections = AiReviewSections {
            facts: serde_json::from_str(&record.facts)?,
            inferences: serde_json::from_str(&record.inferences)?,
            risks: serde_json::from_str(&record.risks)?,
            stock_status: None,
            market_analysis: None,
            sector_analysis: None,
            news_analysis: None,
            technical_analysis: None,
            strategy_reference: None,
            conclusion: None,
        };
        validate_sections(&sections)?;
        let report = record
            .report_json
            .as_deref()
            .map(serde_json::from_str)
            .transpose()?;
        if let Some(report) = &report {
            validate_report(report)?;
        }
        Ok(AiReviewView {
            id: record.id,
            review_id: record.review_id,
            model: record.model,
            prompt_version: record.prompt_version,
            facts: sections.facts,
            inferences: sections.inferences,
            risks: sections.risks,
            report,
            created_at: record.created_at,
        })
    }
}

fn parse_and_validate_provider_sections(value: &str) -> Result<AiReviewSections, AiServiceError> {
    let sections = serde_json::from_str(value)?;
    validate_sections(&sections)?;
    report_from_sections(&sections)?;
    Ok(sections)
}

fn report_from_sections(sections: &AiReviewSections) -> Result<AiResearchReport, AiServiceError> {
    let report = AiResearchReport {
        stock_status: required_report_field(&sections.stock_status)?,
        market_analysis: required_report_field(&sections.market_analysis)?,
        sector_analysis: required_report_field(&sections.sector_analysis)?,
        news_analysis: required_report_field(&sections.news_analysis)?,
        technical_analysis: required_report_field(&sections.technical_analysis)?,
        strategy_reference: required_report_field(&sections.strategy_reference)?,
        conclusion: required_report_field(&sections.conclusion)?,
    };
    validate_report(&report)?;
    Ok(report)
}

fn required_report_field(value: &Option<String>) -> Result<String, AiServiceError> {
    value
        .as_ref()
        .map(|entry| entry.trim())
        .filter(|entry| !entry.is_empty())
        .map(str::to_owned)
        .ok_or(AiServiceError::InvalidOutput("AI投研报告缺少必填维度"))
}

fn validate_report(report: &AiResearchReport) -> Result<(), AiServiceError> {
    for entry in [
        &report.stock_status,
        &report.market_analysis,
        &report.sector_analysis,
        &report.news_analysis,
        &report.technical_analysis,
        &report.strategy_reference,
        &report.conclusion,
    ] {
        if entry.trim().is_empty() {
            return Err(AiServiceError::InvalidOutput("AI投研报告包含空内容"));
        }
        if contains_prohibited_language(entry) {
            return Err(AiServiceError::InvalidOutput(
                "AI输出包含禁止的投资建议或预测",
            ));
        }
    }
    Ok(())
}
fn validate_sections(sections: &AiReviewSections) -> Result<(), AiServiceError> {
    for entry in sections
        .facts
        .iter()
        .chain(&sections.inferences)
        .chain(&sections.risks)
    {
        if entry.trim().is_empty() {
            return Err(AiServiceError::InvalidOutput("AI输出包含空内容"));
        }
        if contains_prohibited_language(entry) {
            return Err(AiServiceError::InvalidOutput(
                "AI输出包含禁止的投资建议或预测",
            ));
        }
    }
    Ok(())
}
fn contains_prohibited_language(value: &str) -> bool {
    const PROHIBITED: &[&str] = &[
        "买入",
        "卖出",
        "加仓",
        "减仓",
        "建仓",
        "清仓",
        "推荐",
        "目标价",
        "收益预测",
        "收益承诺",
        "保证收益",
        "必涨",
        "price target",
        "guaranteed return",
        "buy",
        "sell",
    ];
    let lower = value.to_lowercase();
    PROHIBITED.iter().any(|term| lower.contains(term))
}

fn curl_ai_request(url: &str, api_key: &str, body: &Value) -> Result<Value, AiProviderError> {
    let body_path = unique_payload_path();
    let response_path = unique_response_path();
    if let Err(error) = write_payload(&body_path, body) {
        let _ = fs::remove_file(&body_path);
        return Err(error);
    }
    if let Err(error) = create_private_empty_file(&response_path) {
        let _ = fs::remove_file(&body_path);
        return Err(error);
    }
    let result = (|| {
        let mut command = Command::new("curl");
        command
            .args([
                "--config",
                "-",
                "--silent",
                "--show-error",
                "--max-time",
                "30",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command
            .spawn()
            .map_err(|_| AiProviderError::unavailable())?;
        let config = format!("url = \"{}\"\nrequest = \"POST\"\nheader = \"Authorization: Bearer {}\"\nheader = \"Content-Type: application/json\"\ndata-binary = \"@{}\"\noutput = \"{}\"\nwrite-out = \"%{{http_code}}\"\n", curl_config_value(url), curl_config_value(api_key), curl_config_value(&body_path.to_string_lossy()), curl_config_value(&response_path.to_string_lossy()));
        child
            .stdin
            .as_mut()
            .ok_or_else(AiProviderError::unavailable)?
            .write_all(config.as_bytes())
            .map_err(|_| AiProviderError::unavailable())?;
        let output = child
            .wait_with_output()
            .map_err(|_| AiProviderError::unavailable())?;
        if !output.status.success() {
            ai_diagnostic("deepseek_http_status=NETWORK_FAILURE api_error_body=<unavailable>");
            return Err(AiProviderError::unavailable());
        }
        let status = std::str::from_utf8(&output.stdout)
            .ok()
            .and_then(|value| value.trim().parse::<u16>().ok())
            .ok_or_else(|| AiProviderError::invalid_response("缺少HTTP状态码"))?;
        let response_body = fs::read(&response_path).map_err(|_| AiProviderError::unavailable())?;
        if !(200..300).contains(&status) {
            let safe_body = sanitize_api_error_body(&response_body, api_key);
            ai_diagnostic(format!(
                "deepseek_http_status={status} api_error_body={safe_body}"
            ));
            return Err(AiProviderError::http_error(status, &safe_body));
        }
        ai_diagnostic(format!("deepseek_http_status={status}"));
        serde_json::from_slice(&response_body).map_err(|error| {
            ai_diagnostic(format!("json_parse_failed=true reason={error}"));
            AiProviderError::invalid_response(error)
        })
    })();
    let _ = fs::remove_file(body_path);
    let _ = fs::remove_file(response_path);
    result
}
fn unique_payload_path() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "astock-ai-review-request-{}-{}.json",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ))
}
fn unique_response_path() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "hero-call-ai-response-{}-{}.json",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ))
}
fn write_payload(path: &std::path::Path, body: &Value) -> Result<(), AiProviderError> {
    let mut file = create_private_empty_file(path)?;
    serde_json::to_writer(&mut file, body).map_err(|_| AiProviderError::unavailable())?;
    file.flush().map_err(|_| AiProviderError::unavailable())
}
fn create_private_empty_file(path: &std::path::Path) -> Result<fs::File, AiProviderError> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .map_err(|_| AiProviderError::unavailable())
}
fn curl_config_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
fn sanitize_api_error_body(body: &[u8], api_key: &str) -> String {
    let value = String::from_utf8_lossy(body).replace(api_key, "[REDACTED]");
    let value = value.replace("Authorization: Bearer", "Authorization: [REDACTED]");
    let mut safe = value.chars().take(800).collect::<String>();
    if value.chars().count() > 800 {
        safe.push('…');
    }
    safe.replace(['\n', '\r'], " ")
}
fn ai_diagnostic(message: impl fmt::Display) {
    // Deliberately contains only status, IDs, counts and server-returned text with the Key redacted.
    eprintln!("[hero-call][ai-diagnostic] {message}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        database::service::{
            NewAiReview, NewAiReviewContext, NewDailyReview, NewEventRecord, NewManualRefreshRun,
            NewNewsArticle,
        },
        market_service::{
            DataSourcePriority, MarketDataSource, MarketQuote, MarketSnapshot, MarketStatus,
            SourceClass,
        },
        review_service::{
            ReviewHoldingSummary, ReviewMarketSummary, ReviewPortfolioSummary, ReviewRiskSummary,
        },
    };
    use chrono::{TimeZone, Utc};
    use rust_decimal::Decimal;

    fn stored_review_and_snapshot(database: &DatabaseService) -> DailyReviewView {
        let market_timestamp = Utc.with_ymd_and_hms(2026, 8, 8, 8, 0, 0).unwrap();
        let fetched_at = Utc.with_ymd_and_hms(2026, 8, 8, 8, 1, 0).unwrap();
        let source = MarketDataSource {
            name: "Recorded index source".into(),
            base_url: "https://test.invalid/indices".into(),
            priority: DataSourcePriority::PublicQuote,
            source_class: SourceClass::PublicQuote,
        };
        let index_snapshot = MarketSnapshot {
            source: source.clone(),
            market_timestamp: Some(market_timestamp),
            fetched_at,
            delay_status: MarketStatus::Delayed,
            quotes: vec![MarketQuote {
                security_id: -1,
                symbol: "000001.SH".into(),
                name: "上证指数".into(),
                market: "SSE".into(),
                current_price: Decimal::new(350_000, 2),
                previous_close: Decimal::new(349_000, 2),
                price_change: Decimal::new(1_000, 2),
                change_percent: Decimal::new(29, 2),
                volume: Decimal::ZERO,
                volume_unit: "UNKNOWN".into(),
                turnover_amount: Decimal::ZERO,
                turnover_unit: "UNKNOWN".into(),
                market_timestamp,
                fetched_at,
                source: source.name.clone(),
                delay_status: MarketStatus::Delayed,
            }],
            unavailable_reason: None,
        };
        let indices_snapshot_id = database
            .save_market_index_snapshot_with_id(&index_snapshot)
            .unwrap()
            .expect("index snapshot has a market timestamp");
        let run = database
            .create_manual_refresh_run(NewManualRefreshRun {
                started_at: "2026-08-08T08:00:00Z".into(),
                completed_at: "2026-08-08T08:01:00Z".into(),
                holdings_snapshot_id: None,
                indices_snapshot_id: Some(indices_snapshot_id),
                portfolio_json: "[]".into(),
                status: "NO_DATA".into(),
            })
            .unwrap();
        let article = database
            .create_news_article(NewNewsArticle {
                title: "已保存公告".into(),
                source: "测试公开公告源".into(),
                source_type: "MEDIA".into(),
                published_at: "2026-08-08T08:00:00.000Z".into(),
                fetch_time: "2026-08-08T08:01:00.000Z".into(),
                summary: "公告摘要测试夹具".into(),
                url: "https://example.invalid/announcement/1".into(),
                related_security_id: None,
            })
            .unwrap();
        let event = database
            .create_event(NewEventRecord {
                event_type: "COMPANY_ANNOUNCEMENT".into(),
                title: "已保存公司公告事件".into(),
                security_id: None,
                event_time: "2026-08-08T08:00:00.000Z".into(),
                timezone: "Asia/Shanghai".into(),
                source: "测试公开公告源".into(),
                source_url: Some("https://example.invalid/announcement/1/event".into()),
                status: "CONFIRMED".into(),
            })
            .unwrap();
        database
            .link_news_articles_to_manual_refresh_run(run.id, &[article.id])
            .unwrap();
        database
            .link_events_to_manual_refresh_run(run.id, &[event.id])
            .unwrap();
        database
            .upsert_daily_review(NewDailyReview {
                review_date: "2026-08-08".into(),
                snapshot_id: None,
                portfolio_summary: serde_json::to_string(&ReviewPortfolioSummary {
                    total_assets: None,
                    daily_pnl: None,
                    return_rate: None,
                    holding_count: 0,
                })
                .unwrap(),
                market_summary: serde_json::to_string(&ReviewMarketSummary {
                    snapshot: None,
                    major_indices: Vec::new(),
                })
                .unwrap(),
                holding_summary: serde_json::to_string(&ReviewHoldingSummary {
                    contributions: Vec::new(),
                })
                .unwrap(),
                risk_summary: serde_json::to_string(&ReviewRiskSummary {
                    facts: vec!["暂无当日市场快照。".into()],
                    related_news_count: 0,
                })
                .unwrap(),
            })
            .unwrap();
        assert!(run.id > 0);
        ReviewService::get(database, "2026-08-08").unwrap()
    }
    struct RecordedProvider;
    impl AiProviderAdapter for RecordedProvider {
        fn model(&self) -> &str {
            "recorded-safe-model"
        }
        fn generate(&self, input: &AiReviewInput) -> Result<AiReviewSections, AiProviderError> {
            assert_eq!(input.manual_refresh_run_id, 1);
            assert_eq!(input.news["status"], "AVAILABLE");
            assert_eq!(input.events["status"], "AVAILABLE");
            assert_eq!(input.news["items"][0]["source"], "测试公开公告源");
            assert_eq!(
                input.events["items"][0]["eventType"],
                "COMPANY_ANNOUNCEMENT"
            );
            assert_eq!(input.market["indices"].as_array().map(Vec::len), Some(1));
            assert_eq!(input.market["indices"][0]["symbol"], "000001.SH");
            assert_eq!(input.market["indices"][0]["changePercent"], "0.29");
            Ok(AiReviewSections {
                facts: vec!["当日市场快照暂无数据。".into()],
                inferences: vec!["基于已保存复盘，账户汇总数据尚未完整。".into()],
                risks: vec!["需要核验当日市场快照和账户汇总数据。".into()],
                stock_status: Some("当前个股行情仅以已保存快照为准。".into()),
                market_analysis: Some("市场整体信息以已保存指数快照为准。".into()),
                sector_analysis: Some("暂无可核验的板块数据。".into()),
                news_analysis: Some("已保存公告仅作为事实来源。".into()),
                technical_analysis: Some("暂无完整技术指标，需结合后续快照观察。".into()),
                strategy_reference: Some("保留原有关注逻辑，并持续观察数据完整性。".into()),
                conclusion: Some("当前数据有限，结论仅供后续观察。".into()),
            })
        }
    }
    struct FailingProvider;
    impl AiProviderAdapter for FailingProvider {
        fn model(&self) -> &str {
            "failure"
        }
        fn generate(&self, _: &AiReviewInput) -> Result<AiReviewSections, AiProviderError> {
            Err(AiProviderError::unavailable())
        }
    }
    #[test]
    fn no_key_is_unconfigured() {
        assert!(matches!(
            DeepSeekProviderAdapter::from_key(None),
            Err(AiServiceError::NotConfigured)
        ));
    }
    #[test]
    fn provider_success_persists_only_valid_context_bound_sections() {
        let database = DatabaseService::open_in_memory().unwrap();
        let review = stored_review_and_snapshot(&database);
        let generated =
            AiService::generate_with_provider(&database, "2026-08-08", &RecordedProvider).unwrap();
        assert_eq!(generated.review_id, review.id);
        assert_eq!(generated.facts.len(), 1);
        assert_eq!(
            generated
                .report
                .as_ref()
                .map(|report| report.conclusion.as_str()),
            Some("当前数据有限，结论仅供后续观察。")
        );
        assert_eq!(
            AiService::latest_for_review(&database, review.id).unwrap(),
            Some(generated)
        );
    }
    #[test]
    fn network_failure_does_not_persist_an_ai_review_or_context() {
        let database = DatabaseService::open_in_memory().unwrap();
        let review = stored_review_and_snapshot(&database);
        assert!(matches!(
            AiService::generate_with_provider(&database, "2026-08-08", &FailingProvider),
            Err(AiServiceError::Provider(_))
        ));
        assert!(AiService::latest_for_review(&database, review.id)
            .unwrap()
            .is_none());
        assert_eq!(database.ai_review_context_count().unwrap(), 0);
    }
    #[test]
    fn invalid_json_and_prohibited_content_are_rejected() {
        assert!(parse_and_validate_provider_sections(r#"{"facts":[]}"#).is_err());
        assert!(matches!(
            parse_and_validate_provider_sections(
                r#"{"facts":["行情暂无数据"],"inferences":["建议买入"],"risks":["需要核验来源"]}"#
            ),
            Err(AiServiceError::InvalidOutput(_))
        ));
    }

    #[test]
    fn historical_review_without_report_remains_readable() {
        let database = DatabaseService::open_in_memory().unwrap();
        let review = stored_review_and_snapshot(&database);
        let run = database.latest_manual_refresh_run().unwrap().unwrap();
        let context_id = database
            .create_ai_review_context(NewAiReviewContext {
                review_id: review.id,
                manual_refresh_run_id: run.id,
                portfolio_json: "[]".into(),
                market_json: "{}".into(),
                news_json: "{}".into(),
                events_json: "{}".into(),
            })
            .unwrap();
        database
            .create_ai_review(NewAiReview {
                review_id: review.id,
                model: "legacy-model".into(),
                prompt_version: "deepseek-review-v1".into(),
                context_id,
                provider: "DEEPSEEK".into(),
                facts: serde_json::to_string(&vec!["已保存事实".to_string()]).unwrap(),
                inferences: serde_json::to_string(&vec!["已保存分析".to_string()]).unwrap(),
                risks: serde_json::to_string(&vec!["已保存风险".to_string()]).unwrap(),
                report_json: None,
            })
            .unwrap();

        let restored = AiService::latest_for_review(&database, review.id)
            .unwrap()
            .expect("legacy review remains visible");
        assert!(restored.report.is_none());
        assert_eq!(restored.facts, vec!["已保存事实"]);
    }

    #[test]
    fn api_errors_keep_status_but_redact_the_key() {
        let error = AiProviderError::http_error(401, "invalid api key: [REDACTED]");
        assert!(error.to_string().contains("API认证失败（HTTP 401）"));
        assert_eq!(
            sanitize_api_error_body(b"{\"error\":\"sk-test-secret\"}", "sk-test-secret"),
            "{\"error\":\"[REDACTED]\"}"
        );
    }
}
