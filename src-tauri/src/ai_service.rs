//! Provider-backed, safety-constrained AI explanation for a saved daily review.
//!
//! The UI never contacts a model directly. Runtime secrets are read only from environment
//! variables, provider output is structurally validated before persistence, and prohibited
//! investment language is rejected rather than shown to the user.

use std::{
    env,
    error::Error,
    fmt, fs,
    io::Write,
    process::{Command, Stdio},
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    database::service::{AiReview, DatabaseError, DatabaseService, NewAiReview},
    news_service::{NewsArticleView, NewsService, NewsServiceError},
    portfolio_ui_service::{PortfolioHoldingView, PortfolioUiError, PortfolioUiService},
    review_service::{DailyReviewView, ReviewService, ReviewServiceError},
};

const PROMPT_VERSION: &str = "ai-review-v1";

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
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiReviewInput {
    pub prompt_version: String,
    pub daily_review: DailyReviewView,
    pub portfolio: Vec<PortfolioHoldingView>,
    pub holding_news: Vec<NewsArticleView>,
}

pub trait AiProviderAdapter {
    fn model(&self) -> &str;
    fn generate(&self, input: &AiReviewInput) -> Result<AiReviewSections, AiProviderError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiProviderError {
    message: &'static str,
}

impl AiProviderError {
    fn unavailable() -> Self {
        Self {
            message: "AI Provider 暂不可用",
        }
    }

    fn invalid_response() -> Self {
        Self {
            message: "AI Provider 返回格式无效",
        }
    }
}

impl fmt::Display for AiProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl Error for AiProviderError {}

#[derive(Clone)]
struct AiRuntimeConfig {
    api_key: String,
    base_url: String,
    model: String,
}

impl AiRuntimeConfig {
    fn from_environment() -> Option<Self> {
        let api_key = env::var("AI_API_KEY")
            .ok()
            .or_else(|| env::var("OPENAI_API_KEY").ok());
        Self::from_values(
            api_key.as_deref(),
            env::var("AI_BASE_URL").ok().as_deref(),
            env::var("AI_MODEL").ok().as_deref(),
        )
    }

    fn from_values(
        api_key: Option<&str>,
        base_url: Option<&str>,
        model: Option<&str>,
    ) -> Option<Self> {
        let api_key = non_empty(api_key?)?;
        let base_url = non_empty(base_url?)?;
        let model = non_empty(model?)?;
        if !base_url.starts_with("https://")
            || [api_key, base_url, model]
                .iter()
                .any(|value| value.chars().any(char::is_control))
        {
            return None;
        }
        Some(Self {
            api_key: api_key.into(),
            base_url: base_url.into(),
            model: model.into(),
        })
    }
}

fn non_empty(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}

/// OpenAI-compatible Chat Completions adapter. The curl configuration (including the Authorization
/// header) is passed through stdin, so the key never appears in a process argument or diagnostic.
pub struct OpenAiCompatibleAdapter {
    config: AiRuntimeConfig,
}

impl OpenAiCompatibleAdapter {
    fn from_runtime() -> Option<Self> {
        AiRuntimeConfig::from_environment().map(|config| Self { config })
    }
}

impl AiProviderAdapter for OpenAiCompatibleAdapter {
    fn model(&self) -> &str {
        &self.config.model
    }

    fn generate(&self, input: &AiReviewInput) -> Result<AiReviewSections, AiProviderError> {
        let prompt = serde_json::to_string(input).map_err(|_| AiProviderError::unavailable())?;
        let response = curl_ai_request(
            &self.config.base_url,
            &self.config.api_key,
            &json!({
                "model": self.config.model,
                "temperature": 0,
                "response_format": { "type": "json_object" },
                "messages": [
                    {
                        "role": "system",
                        "content": "你是A股个人投研工具的解释助手。仅基于提供的本地结构化数据，返回严格 JSON 对象：{\"facts\":[string],\"inferences\":[string],\"risks\":[string]}。facts 只能陈述输入中的客观事实；inferences 必须明确是基于 facts 的解释；risks 只能陈述风险、数据缺失或需要核验事项。禁止出现买入、卖出、加仓、减仓、建仓、清仓、推荐、目标价、收益预测、收益承诺、保证收益或必涨等内容。不得添加输入未支持的事实。"
                    },
                    { "role": "user", "content": prompt }
                ]
            }),
        )?;
        let content = response
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .ok_or_else(AiProviderError::invalid_response)?;
        parse_and_validate_sections(content).map_err(|_| AiProviderError::invalid_response())
    }
}

#[derive(Debug)]
pub enum AiServiceError {
    Database(DatabaseError),
    Review(ReviewServiceError),
    Portfolio(PortfolioUiError),
    News(NewsServiceError),
    Provider(AiProviderError),
    Serialization(serde_json::Error),
    InvalidOutput(&'static str),
    NotConfigured,
}

impl fmt::Display for AiServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "database error: {error}"),
            Self::Review(error) => write!(formatter, "review error: {error}"),
            Self::Portfolio(error) => write!(formatter, "portfolio error: {error}"),
            Self::News(error) => write!(formatter, "news error: {error}"),
            Self::Provider(error) => write!(formatter, "provider error: {error}"),
            Self::Serialization(error) => {
                write!(formatter, "AI review serialization error: {error}")
            }
            Self::InvalidOutput(message) => formatter.write_str(message),
            Self::NotConfigured => formatter.write_str("AI服务未配置"),
        }
    }
}

impl Error for AiServiceError {}

impl From<DatabaseError> for AiServiceError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

impl From<ReviewServiceError> for AiServiceError {
    fn from(error: ReviewServiceError) -> Self {
        Self::Review(error)
    }
}

impl From<PortfolioUiError> for AiServiceError {
    fn from(error: PortfolioUiError) -> Self {
        Self::Portfolio(error)
    }
}

impl From<NewsServiceError> for AiServiceError {
    fn from(error: NewsServiceError) -> Self {
        Self::News(error)
    }
}

impl From<AiProviderError> for AiServiceError {
    fn from(error: AiProviderError) -> Self {
        Self::Provider(error)
    }
}

impl From<serde_json::Error> for AiServiceError {
    fn from(error: serde_json::Error) -> Self {
        Self::Serialization(error)
    }
}

pub struct AiService;

impl AiService {
    pub fn status() -> AiServiceStatusView {
        Self::status_from_configuration(AiRuntimeConfig::from_environment())
    }

    fn status_from_configuration(configuration: Option<AiRuntimeConfig>) -> AiServiceStatusView {
        AiServiceStatusView {
            configured: configuration.is_some(),
            model: configuration.map(|configuration| configuration.model),
        }
    }

    pub fn generate_from_runtime(
        database: &DatabaseService,
        review_date: &str,
    ) -> Result<AiReviewView, AiServiceError> {
        let provider =
            OpenAiCompatibleAdapter::from_runtime().ok_or(AiServiceError::NotConfigured)?;
        Self::generate_with_provider(database, review_date, &provider)
    }

    pub fn generate_with_provider<P: AiProviderAdapter>(
        database: &DatabaseService,
        review_date: &str,
        provider: &P,
    ) -> Result<AiReviewView, AiServiceError> {
        let input = AiReviewInput {
            prompt_version: PROMPT_VERSION.into(),
            daily_review: ReviewService::get(database, review_date)?,
            portfolio: PortfolioUiService::list(database)?,
            holding_news: NewsService::list_for_holdings(database)?,
        };
        let sections = provider.generate(&input)?;
        validate_sections(&sections)?;
        let stored = database.create_ai_review(NewAiReview {
            review_id: input.daily_review.id,
            model: provider.model().into(),
            prompt_version: input.prompt_version,
            facts: serde_json::to_string(&sections.facts)?,
            inferences: serde_json::to_string(&sections.inferences)?,
            risks: serde_json::to_string(&sections.risks)?,
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
        };
        validate_sections(&sections)?;
        Ok(AiReviewView {
            id: record.id,
            review_id: record.review_id,
            model: record.model,
            prompt_version: record.prompt_version,
            facts: sections.facts,
            inferences: sections.inferences,
            risks: sections.risks,
            created_at: record.created_at,
        })
    }
}

fn parse_and_validate_sections(value: &str) -> Result<AiReviewSections, AiServiceError> {
    let sections: AiReviewSections = serde_json::from_str(value)?;
    validate_sections(&sections)?;
    Ok(sections)
}

fn validate_sections(sections: &AiReviewSections) -> Result<(), AiServiceError> {
    let entries = sections
        .facts
        .iter()
        .chain(&sections.inferences)
        .chain(&sections.risks);
    for entry in entries {
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
    let value = value.to_lowercase();
    PROHIBITED.iter().any(|term| value.contains(term))
}

fn curl_ai_request(base_url: &str, api_key: &str, body: &Value) -> Result<Value, AiProviderError> {
    let body_path = unique_payload_path();
    if let Err(error) = write_payload(&body_path, body) {
        let _ = fs::remove_file(&body_path);
        return Err(error);
    }
    let result = (|| {
        let mut command = Command::new("curl");
        command
            .args([
                "--config",
                "-",
                "--fail",
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
        let config = format!(
            "url = \"{}\"\nrequest = \"POST\"\nheader = \"Authorization: Bearer {}\"\nheader = \"Content-Type: application/json\"\ndata-binary = \"@{}\"\n",
            curl_config_value(base_url),
            curl_config_value(api_key),
            curl_config_value(&body_path.to_string_lossy()),
        );
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
            return Err(AiProviderError::unavailable());
        }
        serde_json::from_slice(&output.stdout).map_err(|_| AiProviderError::invalid_response())
    })();
    let _ = fs::remove_file(body_path);
    result
}

fn unique_payload_path() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "astock-ai-review-request-{}-{}.json",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ))
}

fn write_payload(path: &std::path::Path, body: &Value) -> Result<(), AiProviderError> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|_| AiProviderError::unavailable())?;
    serde_json::to_writer(&mut file, body).map_err(|_| AiProviderError::unavailable())?;
    file.flush().map_err(|_| AiProviderError::unavailable())
}

fn curl_config_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        database::service::NewDailyReview,
        review_service::{
            ReviewHoldingSummary, ReviewMarketSummary, ReviewPortfolioSummary, ReviewRiskSummary,
        },
    };

    fn stored_review(database: &DatabaseService) -> DailyReviewView {
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
                .expect("serialize portfolio"),
                market_summary: serde_json::to_string(&ReviewMarketSummary {
                    snapshot: None,
                    major_indices: Vec::new(),
                })
                .expect("serialize market"),
                holding_summary: serde_json::to_string(&ReviewHoldingSummary {
                    contributions: Vec::new(),
                })
                .expect("serialize holding"),
                risk_summary: serde_json::to_string(&ReviewRiskSummary {
                    facts: vec!["暂无当日市场快照。".into()],
                    related_news_count: 0,
                })
                .expect("serialize risk"),
            })
            .expect("store daily review");
        ReviewService::get(database, "2026-08-08").expect("read daily review")
    }

    struct RecordedProvider;

    impl AiProviderAdapter for RecordedProvider {
        fn model(&self) -> &str {
            "recorded-safe-model"
        }

        fn generate(&self, input: &AiReviewInput) -> Result<AiReviewSections, AiProviderError> {
            assert_eq!(input.daily_review.review_date, "2026-08-08");
            assert_eq!(input.prompt_version, PROMPT_VERSION);
            Ok(AiReviewSections {
                facts: vec!["当日市场快照暂无数据。".into()],
                inferences: vec!["基于已保存复盘，账户汇总数据尚未完整。".into()],
                risks: vec!["需要核验当日市场快照和账户汇总数据。".into()],
            })
        }
    }

    #[test]
    fn missing_runtime_values_reports_unconfigured_without_exposing_a_key() {
        assert!(AiRuntimeConfig::from_values(None, None, None).is_none());
        assert!(AiRuntimeConfig::from_values(Some("key"), None, Some("model")).is_none());
        assert!(
            AiRuntimeConfig::from_values(Some("key"), Some("http://insecure"), Some("model"))
                .is_none()
        );
        let configured = AiRuntimeConfig::from_values(
            Some("test-value"),
            Some("https://provider.example.invalid/v1/chat/completions"),
            Some("test-model"),
        )
        .expect("configured test values");
        assert_eq!(configured.model, "test-model");
        assert_eq!(
            AiService::status_from_configuration(None),
            AiServiceStatusView {
                configured: false,
                model: None,
            }
        );
    }

    #[test]
    fn provider_interface_persists_valid_structured_sections() {
        let database = DatabaseService::open_in_memory().expect("create database");
        let daily_review = stored_review(&database);
        let generated =
            AiService::generate_with_provider(&database, "2026-08-08", &RecordedProvider)
                .expect("generate with recorded provider");
        assert_eq!(generated.review_id, daily_review.id);
        assert_eq!(generated.model, "recorded-safe-model");
        assert_eq!(generated.facts.len(), 1);
        let latest = AiService::latest_for_review(&database, daily_review.id)
            .expect("load latest AI review")
            .expect("AI review exists");
        assert_eq!(latest, generated);
    }

    #[test]
    fn output_structure_and_prohibited_language_are_checked() {
        let valid = parse_and_validate_sections(
            r#"{"facts":["行情状态为暂无数据。"],"inferences":["基于已保存数据，无法确认市场表现。"],"risks":["需要核验行情来源。"]}"#,
        )
        .expect("validate structured output");
        assert_eq!(valid.facts.len(), 1);

        let invalid = parse_and_validate_sections(
            r#"{"facts":["行情状态为暂无数据。"],"inferences":["建议买入该证券。"],"risks":["需要核验行情来源。"]}"#,
        );
        assert!(matches!(invalid, Err(AiServiceError::InvalidOutput(_))));
        assert!(parse_and_validate_sections(r#"{"facts":[]}"#).is_err());
    }
}
