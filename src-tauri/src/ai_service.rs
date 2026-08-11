//! Safety-constrained DeepSeek explanations for one saved manual market snapshot.
//!
//! The UI never contacts a provider. A successful response is validated before either the
//! immutable context or output is persisted; failed, malformed, or unsafe responses are not kept.

use std::{
    collections::HashSet,
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
        AiReview, DatabaseError, DatabaseService, NewAiResearchReport, NewAiReview,
        NewAiReviewContext, NewAiReviewFailure, NewResearchEvidence, SecurityProfile,
    },
    event_service::{EventService, EventServiceError, EventView},
    intelligence_service::{IntelligenceService, IntelligenceServiceError},
    market_refresh_service::MarketRefreshService,
    news_service::{NewsArticleView, NewsService, NewsServiceError},
    research_service::{
        calculate_technical_snapshot, EastmoneyPriceHistoryAdapter, ResearchService,
    },
    review_service::{DailyReviewView, ReviewService, ReviewServiceError},
    secure_storage::{
        get_ai_provider_key_status, get_deepseek_status, load_ai_provider_api_key_for_adapter,
    },
};

const PROMPT_VERSION: &str = "research-agent-evidence-v2";
const DEEPSEEK_MODEL: &str = "deepseek-chat";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiServiceStatusView {
    pub configured: bool,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderConfigView {
    pub provider: String,
    pub display_name: String,
    pub model: String,
    pub endpoint: String,
    pub configured: bool,
    pub enabled: bool,
    /// The first enabled Provider with a readable Keychain key, using the same priority rule as
    /// `selected_runtime_provider`. At most one entry can be current.
    pub is_current: bool,
    pub priority: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderConnectionTestView {
    pub provider: String,
    pub model: String,
    pub success: bool,
    pub http_status: Option<u16>,
    pub message: String,
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
    #[serde(default, rename = "actions")]
    pub actions: Option<String>,
    #[serde(default, alias = "core_drivers")]
    pub core_drivers: Option<Vec<ResearchDriver>>,
    #[serde(default, alias = "market_thesis")]
    pub market_thesis: Option<MarketThesis>,
    #[serde(default, alias = "bull_bear_analysis")]
    pub bull_bear_analysis: Option<BullBearAnalysis>,
    #[serde(default, alias = "future_catalysts")]
    pub future_catalysts: Option<Vec<FutureCatalyst>>,
    #[serde(default, alias = "risk_factors")]
    pub risk_factors: Option<Vec<RiskFactor>>,
    #[serde(default, alias = "research_score")]
    pub research_score: Option<ResearchScore>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResearchDriver {
    pub title: String,
    pub rationale: String,
    pub impact_level: String,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MarketThesis {
    pub summary: String,
    pub facts: String,
    pub expectations: String,
    pub sentiment: String,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BullBearPoint {
    pub view: String,
    pub basis: String,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BullBearAnalysis {
    #[serde(default)]
    pub bull: Vec<BullBearPoint>,
    #[serde(default)]
    pub bear: Vec<BullBearPoint>,
    pub key_divergence: String,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FutureCatalyst {
    pub time_window: String,
    pub title: String,
    pub source: String,
    pub credibility: String,
    pub time: String,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RiskFactor {
    pub level: String,
    pub title: String,
    pub reason: String,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResearchScore {
    pub fundamental_attention: u8,
    pub technical_state: u8,
    pub market_heat: u8,
    pub sentiment_state: u8,
    pub risk_level: u8,
    pub overall: u8,
    pub explanation: String,
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
    #[serde(default)]
    pub actions: String,
    #[serde(default)]
    pub core_drivers: Vec<ResearchDriver>,
    #[serde(default)]
    pub market_thesis: Option<MarketThesis>,
    #[serde(default)]
    pub bull_bear_analysis: Option<BullBearAnalysis>,
    #[serde(default)]
    pub future_catalysts: Vec<FutureCatalyst>,
    #[serde(default)]
    pub risk_factors: Vec<RiskFactor>,
    #[serde(default)]
    pub research_score: Option<ResearchScore>,
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
    pub security_id: Option<i64>,
    pub security_name: Option<String>,
    pub security_symbol: Option<String>,
    pub created_at: String,
}

/// This is the exact frozen input boundary sent to DeepSeek. News and events are explicit NO_DATA
/// objects in E1; no provider is allowed to silently supplement those fields.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiReviewInput {
    pub prompt_version: String,
    pub manual_refresh_run_id: i64,
    pub research_run_id: Option<i64>,
    #[serde(skip)]
    pub daily_review: DailyReviewView,
    /// Retained for persistence compatibility only. It is never serialized to the model.
    #[serde(skip)]
    pub portfolio: Value,
    pub market: Value,
    pub news: Value,
    pub events: Value,
    pub security: Value,
    pub sector: Value,
    pub intelligence: Value,
    pub research_context: Value,
    pub security_id: i64,
    pub security_name: String,
    pub security_symbol: String,
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
    http_status: Option<u16>,
    raw_response: Option<String>,
}
impl AiProviderError {
    fn unavailable() -> Self {
        Self {
            message: "AI Provider 网络请求失败".into(),
            http_status: None,
            raw_response: None,
        }
    }
    fn invalid_response(reason: impl fmt::Display) -> Self {
        Self {
            message: format!("AI Provider 返回格式错误：{reason}"),
            http_status: None,
            raw_response: None,
        }
    }
    fn invalid_response_with_raw(reason: impl fmt::Display, raw_response: String) -> Self {
        Self {
            message: format!("AI Provider 返回格式错误：{reason}"),
            http_status: None,
            raw_response: Some(raw_response),
        }
    }
    fn http_error(status: u16, body: &str) -> Self {
        let category = match status {
            401 | 403 => "AI Provider API认证失败",
            429 => "AI Provider API请求过于频繁",
            _ => "AI Provider API请求失败",
        };
        let detail = (!body.is_empty()).then(|| format!("：{body}"));
        Self {
            message: format!("{category}（HTTP {status}）{}", detail.unwrap_or_default()),
            http_status: Some(status),
            raw_response: None,
        }
    }
}

impl fmt::Display for AiProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl Error for AiProviderError {}

/// OpenAI-compatible provider boundary shared by the selectable providers. Only the selected
/// provider receives a request; its Keychain key remains in this adapter and never crosses IPC.
struct OpenAiCompatibleProviderAdapter {
    provider: &'static str,
    model: String,
    endpoint: String,
    api_key: String,
}

impl AiProviderAdapter for OpenAiCompatibleProviderAdapter {
    fn model(&self) -> &str {
        &self.model
    }

    fn provider(&self) -> &str {
        self.provider
    }

    fn generate(&self, input: &AiReviewInput) -> Result<AiReviewSections, AiProviderError> {
        let prompt = serde_json::to_string(input).map_err(|_| AiProviderError::unavailable())?;
        let response = curl_ai_request(
            &self.endpoint,
            &self.api_key,
            &json!({
                "model": self.model,
                "stream": false,
                "temperature": 0,
                "response_format": { "type": "json_object" },
                "messages": [
                  { "role": "system", "content": research_report_system_prompt() },
                  { "role": "user", "content": prompt }
                ]
            }),
        )?;
        let content = response
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                let raw_response = sanitize_diagnostic_response(&response.to_string(), &self.api_key);
                ai_diagnostic(format!(
                    "ai_provider_response_invalid provider={} model={} security_id={} error_message=缺少 choices[0].message.content raw_response={raw_response}",
                    self.provider, self.model, input.security_id
                ));
                AiProviderError::invalid_response_with_raw(
                    "缺少 choices[0].message.content",
                    raw_response,
                )
            })?;
        parse_and_normalize_provider_sections(content).map_err(|error| {
            let raw_response = sanitize_diagnostic_response(content, &self.api_key);
            ai_diagnostic(format!(
                "ai_provider_response_invalid provider={} model={} security_id={} error_message={} raw_response={raw_response}",
                self.provider, self.model, input.security_id, error
            ));
            AiProviderError::invalid_response_with_raw(error, raw_response)
        })
    }
}

fn research_report_system_prompt() -> &'static str {
    "你是个人股票研究与复盘助手。仅依据输入 JSON 的 Evidence Context 输出严格 JSON 对象。必须包含既有审计字段 facts:[string]、inferences:[string]、risks:[string]、stock_status:string、market_analysis:string、sector_analysis:string、news_analysis:string、technical_analysis:string、strategy_reference:string、conclusion:string、actions:string，以及 Research Report V2 字段：coreDrivers:[{title,rationale,impactLevel,evidenceIds}]、marketThesis:{summary,facts,expectations,sentiment,evidenceIds}、bullBearAnalysis:{bull:[{view,basis,evidenceIds}],bear:[{view,basis,evidenceIds}],keyDivergence,evidenceIds}、futureCatalysts:[{timeWindow,title,source,credibility,time,evidenceIds}]、riskFactors:[{level,title,reason,evidenceIds}]、researchScore:{fundamentalAttention,technicalState,marketHeat,sentimentState,riskLevel,overall,explanation}。每一个 V2 分析条目必须列出 evidenceIds：只能使用输入 researchContext.allowedEvidenceIds 中的 ID；资料不足时使用 NO_DATA，并写“暂无数据”或“未确认”。今日核心驱动必须解释当日涨跌/波动的已知驱动与当前交易逻辑，而非罗列新闻；impactLevel 仅能为 HIGH、MEDIUM、LOW。marketThesis 必须明确分开 facts（事实）、expectations（市场预期）、sentiment（情绪驱动）。bullBearAnalysis 只分析市场多空观点与最大分歧，严禁给出交易指令。futureCatalysts 仅列输入事件或情报支持的 1D/3D/7D 催化，每项必须带来源、可信度、时间。riskFactors 必须按 HIGH、MEDIUM、LOW 顺序，并说明原因。researchScore 的六项值只能为 0-100，是当前研究状态评分，非涨跌预测、非投资建议，explanation 必须说明依据和局限。FACTS 只能陈述带 source、时间或 evidence 的输入事实；INFERENCES 必须说明基于哪些事实推断；RISKS 只列不确定性、数据缺失或需核验事项。技术面只能基于 technical 字段；profile.status 非 VERIFIED 或资料缺失时，必须写“暂无可验证股票画像”，不得猜测公司、行业、板块或概念。intelligence.verifiedIntelligence 中仅 A/B 级信息可作为事实；C 级仅可作为行业参考；communityOpinion 只能称为社区观点或情绪线索，rumors 只能作为未证实传闻、风险或观察项，绝不可作为 FACTS。不得提及持仓成本、盈亏、是否实际持仓或账户资产。不得输出买入、卖出、加仓、减仓、建仓、清仓、止盈、止损、目标价、收益预测、收益承诺、保证收益、必涨、必跌、稳赚或任何未来确定性/交易建议；actions 只能写“本报告不提供交易建议，建议持续观察的证据条件：…”。输出只能是 JSON 对象，禁止 Markdown、代码块、前后缀说明或额外解释文本；至少固定包含 {\"facts\":[],\"inferences\":[],\"risks\":[],\"strategy_reference\":\"\"}。数据缺失时明确“暂无数据”或“未确认”，绝不补造事实。"
}

fn provider_display_name(provider: &str) -> &'static str {
    match provider {
        "DEEPSEEK" => "DeepSeek",
        "TENCENT_TOKENHUB" => "腾讯混元（TokenHub）",
        "DOUBAO" => "豆包",
        _ => "未确认 Provider",
    }
}

fn provider_static_name(provider: &str) -> Result<&'static str, AiServiceError> {
    match provider {
        "DEEPSEEK" => Ok("DEEPSEEK"),
        "TENCENT_TOKENHUB" => Ok("TENCENT_TOKENHUB"),
        "DOUBAO" => Ok("DOUBAO"),
        _ => Err(AiServiceError::NotConfigured),
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
impl From<IntelligenceServiceError> for AiServiceError {
    fn from(error: IntelligenceServiceError) -> Self {
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

    pub fn provider_configs(
        database: &DatabaseService,
    ) -> Result<Vec<AiProviderConfigView>, AiServiceError> {
        let mut configs = database
            .list_ai_provider_settings()?
            .into_iter()
            .map(|setting| {
                let configured = get_ai_provider_key_status(&setting.provider)
                    .map(|status| status.status == "已配置")
                    .unwrap_or(false);
                Ok(AiProviderConfigView {
                    display_name: provider_display_name(&setting.provider).into(),
                    provider: setting.provider,
                    model: setting.model_id,
                    endpoint: setting.endpoint,
                    configured,
                    enabled: setting.enabled,
                    is_current: false,
                    priority: setting.priority,
                })
            })
            .collect::<Result<Vec<_>, AiServiceError>>()?;
        mark_current_provider(&mut configs);
        Ok(configs)
    }

    pub fn set_provider_enabled(
        database: &DatabaseService,
        provider: &str,
        enabled: bool,
    ) -> Result<Vec<AiProviderConfigView>, AiServiceError> {
        if enabled
            && get_ai_provider_key_status(provider)
                .map_err(|_| AiServiceError::KeychainUnavailable)?
                .status
                != "已配置"
        {
            return Err(AiServiceError::NotConfigured);
        }
        database.set_ai_provider_enabled(provider, enabled)?;
        Self::provider_configs(database)
    }

    pub fn generate_all_from_runtime(
        database: &DatabaseService,
        review_date: &str,
    ) -> Result<Vec<AiReviewView>, AiServiceError> {
        let result = (|| {
            // The planner prepares a trusted user-equivalent snapshot only when no usable
            // snapshot exists or the latest boundary is older than five minutes. This is an
            // action-triggered refresh, never a background polling loop.
            let needs_refresh = database
                .latest_manual_refresh_run()?
                .as_ref()
                .is_none_or(|run| snapshot_is_stale(&run.completed_at));
            if needs_refresh {
                MarketRefreshService::refresh_today_snapshot(database).map_err(|error| {
                    AiServiceError::Context(format!("研究数据准备失败：{error}"))
                })?;
            }
            let _ = ReviewService::generate(database, review_date)?;
            let adapter = Self::selected_runtime_provider(database)?;
            let research_run = database.create_research_run(&Utc::now().to_rfc3339())?;
            let generated = Self::generate_for_all_followed_securities(
                database,
                review_date,
                &adapter,
                Some(research_run.id),
            )?;
            let latest = database.latest_manual_refresh_run()?;
            database.complete_research_run(
                research_run.id,
                &Utc::now().to_rfc3339(),
                latest.and_then(|run| run.indices_snapshot_id),
                "COMPLETED",
            )?;
            Ok(generated)
        })();
        if let Err(error) = &result {
            ai_diagnostic(format!("rust_final_error={error}"));
        }
        result
    }

    fn selected_runtime_provider(
        database: &DatabaseService,
    ) -> Result<OpenAiCompatibleProviderAdapter, AiServiceError> {
        for setting in database.list_ai_provider_settings()? {
            if !setting.enabled {
                continue;
            }
            let Some(api_key) = load_ai_provider_api_key_for_adapter(&setting.provider)
                .map_err(|_| AiServiceError::KeychainUnavailable)?
            else {
                continue;
            };
            return Ok(OpenAiCompatibleProviderAdapter {
                provider: provider_static_name(&setting.provider)?,
                model: setting.model_id,
                endpoint: setting.endpoint,
                api_key,
            });
        }
        Err(AiServiceError::NotConfigured)
    }

    /// Tests the configured Provider's OpenAI-compatible `/models` endpoint without generating
    /// a review or sending any portfolio, market, news, or event data.
    pub fn test_provider_connection(
        database: &DatabaseService,
        provider: &str,
    ) -> Result<AiProviderConnectionTestView, AiServiceError> {
        let setting = database
            .list_ai_provider_settings()?
            .into_iter()
            .find(|setting| setting.provider == provider)
            .ok_or(AiServiceError::NotConfigured)?;
        let Some(api_key) = load_ai_provider_api_key_for_adapter(provider)
            .map_err(|_| AiServiceError::KeychainUnavailable)?
        else {
            return Ok(AiProviderConnectionTestView {
                provider: setting.provider,
                model: setting.model_id,
                success: false,
                http_status: None,
                message: "API Key 未配置".into(),
            });
        };
        match curl_ai_request(
            &setting.endpoint,
            &api_key,
            &json!({
                "model": setting.model_id,
                "stream": false,
                "max_tokens": 1,
                "messages": [{ "role": "user", "content": "ping" }]
            }),
        ) {
            Ok(response) => {
                if connection_response_has_message(&response) {
                    Ok(AiProviderConnectionTestView {
                        provider: setting.provider,
                        model: setting.model_id.clone(),
                        success: true,
                        http_status: Some(200),
                        message: format!("连接成功，模型：{}", setting.model_id),
                    })
                } else {
                    Ok(AiProviderConnectionTestView {
                        provider: setting.provider,
                        model: setting.model_id,
                        success: false,
                        http_status: Some(200),
                        message: "模型响应格式错误：缺少 choices[0].message.content".into(),
                    })
                }
            }
            Err(error) => Ok(AiProviderConnectionTestView {
                provider: setting.provider,
                model: setting.model_id,
                success: false,
                http_status: error.http_status,
                message: error.to_string(),
            }),
        }
    }
    pub fn generate_from_runtime(
        database: &DatabaseService,
        review_date: &str,
    ) -> Result<AiReviewView, AiServiceError> {
        let result = (|| {
            // Keep the legacy single-review command on the exact same Provider-selection path
            // as the per-security UI flow. This prevents a configured-but-not-current DeepSeek
            // key from being called when Settings and Dashboard show another Provider.
            let provider = Self::selected_runtime_provider(database)?;
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
            research_run_id: None,
            daily_review,
            portfolio,
            market,
            news: json!({"status": if news_items.is_empty() { "NO_DATA" } else { "AVAILABLE" }, "items": news_items}),
            events: json!({"status": if event_items.is_empty() { "NO_DATA" } else { "AVAILABLE" }, "items": event_items}),
            security: json!({"name": "历史综合复盘", "status": "LEGACY"}),
            sector: json!({"status": "UNAVAILABLE"}),
            intelligence: json!({"status": "NO_DATA"}),
            research_context: json!({"status": "NO_DATA", "allowedEvidenceIds": ["NO_DATA"]}),
            security_id: 0,
            security_name: "历史综合复盘".into(),
            security_symbol: String::new(),
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
            research_run_id: input.research_run_id,
            portfolio_json: serde_json::to_string(&input.portfolio)?,
            market_json: serde_json::to_string(&input.market)?,
            news_json: serde_json::to_string(&input.news)?,
            events_json: serde_json::to_string(&input.events)?,
            intelligence_json: serde_json::to_string(&input.intelligence)?,
            research_context_json: serde_json::to_string(&input.research_context)?,
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
            security_id: None,
            research_run_id: input.research_run_id,
        })?;
        Self::view_from_record(stored)
    }

    fn generate_for_all_followed_securities<P: AiProviderAdapter>(
        database: &DatabaseService,
        review_date: &str,
        provider: &P,
        research_run_id: Option<i64>,
    ) -> Result<Vec<AiReviewView>, AiServiceError> {
        let daily_review = ReviewService::get(database, review_date)?;
        let run = database
            .latest_manual_refresh_run()?
            .ok_or(AiServiceError::NoManualSnapshot)?;
        let all_holding_quotes =
            database.list_market_quotes_for_snapshot(run.holdings_snapshot_id)?;
        let index_quotes =
            database.list_market_index_quotes_for_snapshot(run.indices_snapshot_id)?;
        let securities = database.list_market_securities_for_holdings()?;
        if securities.is_empty() {
            return Err(AiServiceError::Context("当前没有关注标的".into()));
        }

        let mut generated = Vec::with_capacity(securities.len());
        for security in securities {
            let security_context = json!({
                "symbol": security.symbol,
                "name": security.name,
                "market": security.market,
                "securityType": "STOCK"
            });
            let history = ResearchService::ensure_price_history(
                database,
                &security,
                20,
                &EastmoneyPriceHistoryAdapter,
            )
            .unwrap_or_default();
            let technical = calculate_technical_snapshot(&history);
            let holding_quotes = all_holding_quotes
                .iter()
                .filter(|quote| quote.symbol == security.symbol)
                .collect::<Vec<_>>();
            let news_items = deduplicate_news(NewsService::list_for_run_and_security(
                database,
                run.id,
                security.security_id,
            )?);
            let event_items = deduplicate_events(EventService::list_for_run_and_security(
                database,
                run.id,
                security.security_id,
            )?);
            let intelligence = IntelligenceService::summary_for_run_and_security(
                database,
                run.id,
                security.security_id,
            )?;
            let market = json!({
                "holding": holding_quotes,
                "indices": index_quotes,
                "technical": technical,
                "snapshotCompletedAt": run.completed_at,
            });
            let news = json!({"status": if news_items.is_empty() { "NO_DATA" } else { "AVAILABLE" }, "items": news_items});
            let events = json!({"status": if event_items.is_empty() { "NO_DATA" } else { "AVAILABLE" }, "items": event_items});
            let research_context = build_research_context(
                database,
                security.security_id,
                &market,
                &news,
                &events,
                &intelligence,
            )?;
            let input = AiReviewInput {
                prompt_version: PROMPT_VERSION.into(),
                manual_refresh_run_id: run.id,
                research_run_id,
                daily_review: daily_review.clone(),
                portfolio: Value::Null,
                market,
                news,
                events,
                security: security_context,
                sector: json!({"status": "UNAVAILABLE", "reason": "当前没有可靠板块数据源"}),
                intelligence,
                research_context,
                security_id: security.security_id,
                security_name: security.name,
                security_symbol: security.symbol,
            };
            generated.push(Self::generate_for_security_with_provider(
                database, &input, provider,
            )?);
        }
        Ok(generated)
    }

    fn generate_for_security_with_provider<P: AiProviderAdapter>(
        database: &DatabaseService,
        input: &AiReviewInput,
        provider: &P,
    ) -> Result<AiReviewView, AiServiceError> {
        persist_research_evidence(database, input)?;
        ai_diagnostic(format!(
            "ai_context_generated=true manual_refresh_run_id={} security_id={} holding_quotes={} index_quotes={} news_items={} event_items={}",
            input.manual_refresh_run_id,
            input.security_id,
            input.market["holding"].as_array().map_or(0, Vec::len),
            input.market["indices"].as_array().map_or(0, Vec::len),
            input.news["items"].as_array().map_or(0, Vec::len),
            input.events["items"].as_array().map_or(0, Vec::len),
        ));
        let sections = provider.generate(input).map_err(|error| {
            if let Some(raw_response) = error.raw_response.as_deref() {
                // A parse failure is auditable, but never becomes an `ai_reviews` success row.
                // The response has already been redacted and bounded by the adapter.
                if let Err(store_error) = database.create_ai_review_failure(NewAiReviewFailure {
                    provider: provider.provider().into(),
                    model: provider.model().into(),
                    security_id: input.security_id,
                    raw_response: raw_response.into(),
                    error_message: error.to_string(),
                }) {
                    ai_diagnostic(format!(
                        "ai_review_failure_store_failed provider={} model={} security_id={} error_message={store_error}",
                        provider.provider(),
                        provider.model(),
                        input.security_id,
                    ));
                }
            }
            ai_diagnostic(format!(
                "ai_generation_failed provider={} model={} security_id={} error_message={}",
                provider.provider(),
                provider.model(),
                input.security_id,
                error
            ));
            AiServiceError::Provider(error)
        })?;
        validate_sections(&sections)?;
        let report = report_from_sections(&sections)?;
        validate_report_against_evidence(&report, &input.research_context)?;
        let context_id = database.create_ai_review_context(NewAiReviewContext {
            review_id: input.daily_review.id,
            manual_refresh_run_id: input.manual_refresh_run_id,
            research_run_id: input.research_run_id,
            portfolio_json: serde_json::to_string(&input.portfolio)?,
            market_json: serde_json::to_string(&input.market)?,
            news_json: serde_json::to_string(&input.news)?,
            events_json: serde_json::to_string(&input.events)?,
            intelligence_json: serde_json::to_string(&input.intelligence)?,
            research_context_json: serde_json::to_string(&input.research_context)?,
        })?;
        let stored = database.create_ai_review(NewAiReview {
            review_id: input.daily_review.id,
            model: provider.model().into(),
            prompt_version: input.prompt_version.clone(),
            context_id,
            provider: provider.provider().into(),
            facts: serde_json::to_string(&sections.facts)?,
            inferences: serde_json::to_string(&sections.inferences)?,
            risks: serde_json::to_string(&sections.risks)?,
            report_json: Some(serde_json::to_string(&report)?),
            security_id: Some(input.security_id),
            research_run_id: input.research_run_id,
        })?;
        if report.is_v2() {
            database.create_ai_research_report(NewAiResearchReport {
                ai_review_id: stored.id,
                security_id: input.security_id,
                core_drivers_json: serde_json::to_string(&report.core_drivers)?,
                market_thesis_json: serde_json::to_string(&report.market_thesis)?,
                bull_bear_analysis_json: serde_json::to_string(&report.bull_bear_analysis)?,
                future_catalysts_json: serde_json::to_string(&report.future_catalysts)?,
                risk_factors_json: serde_json::to_string(&report.risk_factors)?,
                research_score_json: serde_json::to_string(&report.research_score)?,
                research_context_json: serde_json::to_string(&input.research_context)?,
            })?;
        }
        Self::view_from_record(stored)
    }

    pub fn list_for_review(
        database: &DatabaseService,
        review_id: i64,
    ) -> Result<Vec<AiReviewView>, AiServiceError> {
        Ok(database
            .list_ai_reviews_for_daily_review(review_id)?
            .into_iter()
            .filter_map(|record| match Self::view_from_record(record.clone()) {
                Ok(view) => Some(view),
                Err(error) => {
                    // A legacy corrupted row must never make all other successful reviews vanish
                    // from the page. The raw stored payload remains untouched for local audit.
                    ai_diagnostic(format!(
                        "stored_ai_review_filtered review_id={} security_id={:?} error_message={}",
                        record.id, record.security_id, error
                    ));
                    None
                }
            })
            .collect())
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
            actions: None,
            core_drivers: None,
            market_thesis: None,
            bull_bear_analysis: None,
            future_catalysts: None,
            risk_factors: None,
            research_score: None,
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
            security_id: record.security_id,
            security_name: record.security_name,
            security_symbol: record.security_symbol,
            created_at: record.created_at,
        })
    }
}

fn persist_research_evidence(
    database: &DatabaseService,
    input: &AiReviewInput,
) -> Result<(), AiServiceError> {
    let Some(research_run_id) = input.research_run_id else {
        return Ok(());
    };
    for (evidence_type, payload) in [
        ("MARKET", &input.market),
        ("NEWS", &input.news),
        ("EVENT", &input.events),
        ("INTELLIGENCE", &input.intelligence),
        ("RESEARCH_CONTEXT", &input.research_context),
    ] {
        database.create_research_evidence(NewResearchEvidence {
            research_run_id,
            security_id: input.security_id,
            evidence_type: evidence_type.into(),
            source: None,
            source_type: None,
            published_at: None,
            source_url: None,
            payload_json: serde_json::to_string(payload)?,
        })?;
    }
    Ok(())
}

fn profile_context(profile: Option<SecurityProfile>) -> Value {
    match profile {
        Some(profile) => json!({
            "status": profile.profile_status,
            "companyDescription": profile.company_description,
            "industry": profile.industry,
            "sector": profile.sector,
            "conceptTags": serde_json::from_str::<Value>(&profile.tags_json).unwrap_or_else(|_| json!([])),
            "businessModel": profile.business_model,
            "historicalCharacteristics": profile.historical_characteristics,
            "source": profile.source,
            "sourceUrl": profile.source_url,
            "fetchedAt": profile.fetched_at,
            "updatedAt": profile.updated_at,
        }),
        None => json!({
            "status": "PENDING",
            "companyDescription": null,
            "industry": null,
            "sector": null,
            "conceptTags": [],
            "businessModel": null,
            "historicalCharacteristics": null,
            "source": null,
            "reason": "正在建立股票画像；暂无可验证股票画像",
        }),
    }
}

fn ids_from_items(value: &Value, prefix: &str) -> Vec<String> {
    value
        .pointer("/items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("id").and_then(Value::as_i64))
        .map(|id| format!("{prefix}:{id}"))
        .collect()
}

fn ids_from_intelligence(value: &Value, path: &str) -> Vec<String> {
    value
        .pointer(path)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("id").and_then(Value::as_i64))
        .map(|id| format!("INTELLIGENCE:{id}"))
        .collect()
}

fn build_research_context(
    database: &DatabaseService,
    security_id: i64,
    market: &Value,
    news: &Value,
    events: &Value,
    intelligence: &Value,
) -> Result<Value, AiServiceError> {
    let profile = profile_context(database.get_security_profile(security_id)?);
    let mut allowed = vec!["NO_DATA".to_owned()];
    if market
        .pointer("/holding")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
        || market
            .pointer("/indices")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
    {
        allowed.push("MARKET".to_owned());
    }
    allowed.extend(ids_from_items(news, "NEWS"));
    allowed.extend(ids_from_items(events, "EVENT"));
    allowed.extend(ids_from_intelligence(intelligence, "/verifiedIntelligence"));
    allowed.extend(ids_from_intelligence(intelligence, "/rumors"));
    if profile["status"] == "VERIFIED" && profile["source"].is_string() {
        allowed.push("PROFILE".to_owned());
    }
    allowed.sort();
    allowed.dedup();
    Ok(json!({
        "profile": profile,
        "allowedEvidenceIds": allowed,
        "evidencePolicy": {
            "A_B": "可作为事实",
            "C": "仅作为行业参考",
            "D": "仅作为市场情绪或社区观点",
            "E": "仅作为未证实传闻、风险或观察项"
        },
        "analysisRequirements": [
            "coreDrivers", "marketThesis", "bullBearAnalysis", "futureCatalysts", "riskFactors", "researchScore"
        ]
    }))
}

fn mark_current_provider(configs: &mut [AiProviderConfigView]) {
    let current = configs
        .iter()
        .filter(|provider| provider.enabled && provider.configured)
        .min_by_key(|provider| provider.priority)
        .map(|provider| provider.provider.clone());
    for provider in configs {
        provider.is_current = current.as_deref() == Some(provider.provider.as_str());
    }
}

fn snapshot_is_stale(completed_at: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(completed_at)
        .map(|timestamp| {
            Utc::now()
                .signed_duration_since(timestamp.with_timezone(&Utc))
                .num_minutes()
                >= 5
        })
        .unwrap_or(true)
}

/// Deduplicate the bounded evidence list by canonical URL, falling back to title + publication
/// time. This keeps announcement copies from inflating the Provider prompt.
fn deduplicate_news(items: Vec<NewsArticleView>) -> Vec<NewsArticleView> {
    let mut seen = HashSet::new();
    items
        .into_iter()
        .filter(|item| {
            let key = if item.url.trim().is_empty() {
                format!("{}|{}", item.title, item.published_at)
            } else {
                item.url.clone()
            };
            seen.insert(key)
        })
        .take(10)
        .collect()
}

fn deduplicate_events(items: Vec<EventView>) -> Vec<EventView> {
    let mut seen = HashSet::new();
    items
        .into_iter()
        .filter(|item| {
            let key = item
                .source_url
                .clone()
                .unwrap_or_else(|| format!("{}|{}", item.title, item.event_time));
            seen.insert(key)
        })
        .take(10)
        .collect()
}

fn parse_and_normalize_provider_sections(value: &str) -> Result<AiReviewSections, AiServiceError> {
    let payload: Value = serde_json::from_str(value)
        .map_err(|_| AiServiceError::InvalidOutput("AI_REVIEW_PARSE_FAILED"))?;

    // First preserve the fully structured V2 response without alteration.
    if let Ok(sections) = serde_json::from_value::<AiReviewSections>(payload.clone()) {
        if validate_sections(&sections).is_ok() && report_from_sections(&sections).is_ok() {
            return Ok(sections);
        }
    }

    // Some OpenAI-compatible models return Chinese display labels or a short summary schema.
    // Convert only explicit fields; missing information remains "暂无数据" rather than invented.
    let sections = normalize_ai_response(&payload)?;
    validate_sections(&sections)?;
    report_from_sections(&sections)?;
    Ok(sections)
}

fn normalize_ai_response(payload: &Value) -> Result<AiReviewSections, AiServiceError> {
    let object = payload
        .as_object()
        .ok_or(AiServiceError::InvalidOutput("AI_REVIEW_PARSE_FAILED"))?;
    let text = |keys: &[&str]| {
        keys.iter()
            .filter_map(|key| object.get(*key))
            .flat_map(response_texts)
            .collect::<Vec<_>>()
    };
    let first = |keys: &[&str]| text(keys).into_iter().next();
    let facts = text(&[
        "facts",
        "当前个股情况",
        "stock_status",
        "current_stock_status",
        "市场环境分析",
        "market_analysis",
        "消息面分析",
        "news_analysis",
        "analysis",
        "分析",
    ]);
    if facts.is_empty() {
        return Err(AiServiceError::InvalidOutput("AI_REVIEW_PARSE_FAILED"));
    }
    let inferences = nonempty_or_no_data(text(&[
        "inferences",
        "技术面分析",
        "technical_analysis",
        "analysis",
        "分析",
    ]));
    let risks = nonempty_or_no_data(text(&[
        "risks",
        "风险提示",
        "风险因素",
        "risk_factors",
        "risk_analysis",
    ]));
    let stock_status = first(&["当前个股情况", "stock_status", "current_stock_status"])
        .unwrap_or_else(|| facts.join("；"));
    let market_analysis =
        first(&["市场环境分析", "market_analysis"]).unwrap_or_else(|| "暂无数据".into());
    let news_analysis =
        first(&["消息面分析", "news_analysis"]).unwrap_or_else(|| "暂无数据".into());
    let technical_analysis =
        first(&["技术面分析", "technical_analysis"]).unwrap_or_else(|| "暂无数据".into());
    let strategy_reference = first(&["策略参考", "strategy_reference", "strategy"])
        .unwrap_or_else(|| "本报告不提供交易建议，建议持续观察的证据条件：暂无数据。".into());
    let conclusion = first(&["结论", "conclusion", "summary", "总结"])
        .or_else(|| first(&["analysis", "分析"]))
        .unwrap_or_else(|| "暂无数据".into());
    Ok(AiReviewSections {
        facts,
        inferences,
        risks,
        stock_status: Some(stock_status),
        market_analysis: Some(market_analysis),
        sector_analysis: Some(
            first(&["所属板块分析", "sector_analysis"]).unwrap_or_else(|| "暂无数据".into()),
        ),
        news_analysis: Some(news_analysis),
        technical_analysis: Some(technical_analysis),
        strategy_reference: Some(strategy_reference),
        conclusion: Some(conclusion),
        actions: Some("本报告不提供交易建议，建议持续观察的证据条件：暂无数据。".into()),
        core_drivers: None,
        market_thesis: None,
        bull_bear_analysis: None,
        future_catalysts: None,
        risk_factors: None,
        research_score: None,
    })
}

fn response_texts(value: &Value) -> Vec<String> {
    match value {
        Value::String(value) if !value.trim().is_empty() => vec![value.trim().to_owned()],
        Value::Array(values) => values.iter().flat_map(response_texts).collect(),
        _ => Vec::new(),
    }
}

fn nonempty_or_no_data(values: Vec<String>) -> Vec<String> {
    if values.is_empty() {
        vec!["暂无数据".into()]
    } else {
        values
    }
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
        actions: required_report_field(&sections.actions)?,
        core_drivers: sections.core_drivers.clone().unwrap_or_default(),
        market_thesis: sections.market_thesis.clone(),
        bull_bear_analysis: sections.bull_bear_analysis.clone(),
        future_catalysts: sections.future_catalysts.clone().unwrap_or_default(),
        risk_factors: sections.risk_factors.clone().unwrap_or_default(),
        research_score: sections.research_score.clone(),
    };
    validate_report(&report)?;
    Ok(report)
}

impl AiResearchReport {
    fn is_v2(&self) -> bool {
        !self.core_drivers.is_empty()
            || self.market_thesis.is_some()
            || self.bull_bear_analysis.is_some()
            || !self.future_catalysts.is_empty()
            || !self.risk_factors.is_empty()
            || self.research_score.is_some()
    }
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
    // Reports saved before V1.1.0 have no ACTIONS field and remain readable. New reports are
    // required to provide it by `report_from_sections` above.
    if !report.actions.trim().is_empty() && contains_prohibited_language(&report.actions) {
        return Err(AiServiceError::InvalidOutput(
            "AI输出包含禁止的投资建议或预测",
        ));
    }
    if report.is_v2() {
        validate_v2_report(report)?;
    }
    Ok(())
}

fn validate_v2_report(report: &AiResearchReport) -> Result<(), AiServiceError> {
    let thesis = report
        .market_thesis
        .as_ref()
        .ok_or(AiServiceError::InvalidOutput("AI研究报告缺少市场交易逻辑"))?;
    let bull_bear = report
        .bull_bear_analysis
        .as_ref()
        .ok_or(AiServiceError::InvalidOutput("AI研究报告缺少多空博弈分析"))?;
    let score = report
        .research_score
        .as_ref()
        .ok_or(AiServiceError::InvalidOutput("AI研究报告缺少研究评分"))?;
    if report.core_drivers.is_empty() || report.risk_factors.is_empty() {
        return Err(AiServiceError::InvalidOutput(
            "AI研究报告缺少核心驱动或风险因素",
        ));
    }
    for driver in &report.core_drivers {
        require_text(&driver.title, "核心驱动标题")?;
        require_text(&driver.rationale, "核心驱动依据")?;
        if !matches!(driver.impact_level.as_str(), "HIGH" | "MEDIUM" | "LOW") {
            return Err(AiServiceError::InvalidOutput("核心驱动影响程度无效"));
        }
        reject_trade_language(&driver.title)?;
        reject_trade_language(&driver.rationale)?;
    }
    for text in [
        &thesis.summary,
        &thesis.facts,
        &thesis.expectations,
        &thesis.sentiment,
        &bull_bear.key_divergence,
    ] {
        require_text(text, "市场交易逻辑")?;
        reject_trade_language(text)?;
    }
    for point in bull_bear.bull.iter().chain(&bull_bear.bear) {
        require_text(&point.view, "多空观点")?;
        require_text(&point.basis, "多空依据")?;
        reject_trade_language(&point.view)?;
        reject_trade_language(&point.basis)?;
    }
    let mut previous_rank = 4;
    for risk in &report.risk_factors {
        let rank = match risk.level.as_str() {
            "HIGH" => 3,
            "MEDIUM" => 2,
            "LOW" => 1,
            _ => return Err(AiServiceError::InvalidOutput("风险等级无效")),
        };
        if rank > previous_rank {
            return Err(AiServiceError::InvalidOutput("风险因素未按高、中、低排序"));
        }
        previous_rank = rank;
        require_text(&risk.title, "风险因素")?;
        require_text(&risk.reason, "风险原因")?;
        reject_trade_language(&risk.title)?;
        reject_trade_language(&risk.reason)?;
    }
    for catalyst in &report.future_catalysts {
        if !matches!(catalyst.time_window.as_str(), "1D" | "3D" | "7D") {
            return Err(AiServiceError::InvalidOutput("未来催化时间窗口无效"));
        }
        if !matches!(
            catalyst.credibility.as_str(),
            "A" | "B" | "C" | "D" | "E" | "NO_DATA"
        ) {
            return Err(AiServiceError::InvalidOutput("未来催化可信度无效"));
        }
        for text in [&catalyst.title, &catalyst.source, &catalyst.time] {
            require_text(text, "未来催化信息")?;
            reject_trade_language(text)?;
        }
    }
    for score in [
        score.fundamental_attention,
        score.technical_state,
        score.market_heat,
        score.sentiment_state,
        score.risk_level,
        score.overall,
    ] {
        if score > 100 {
            return Err(AiServiceError::InvalidOutput("研究评分必须介于 0 至 100"));
        }
    }
    require_text(&score.explanation, "研究评分说明")?;
    reject_trade_language(&score.explanation)?;
    reject_trade_language(&report.actions)?;
    Ok(())
}

fn require_text(value: &str, name: &'static str) -> Result<(), AiServiceError> {
    if value.trim().is_empty() {
        Err(AiServiceError::InvalidOutput(name))
    } else {
        Ok(())
    }
}

fn reject_trade_language(value: &str) -> Result<(), AiServiceError> {
    const DIRECTIVES: &[&str] = &[
        "买入", "卖出", "加仓", "减仓", "建仓", "清仓", "止盈", "止损",
    ];
    if DIRECTIVES.iter().any(|term| value.contains(term)) {
        Err(AiServiceError::InvalidOutput("AI输出包含直接交易指令"))
    } else {
        Ok(())
    }
}

fn validate_report_against_evidence(
    report: &AiResearchReport,
    research_context: &Value,
) -> Result<(), AiServiceError> {
    if !report.is_v2() {
        return Ok(());
    }
    let allowed = research_context
        .get("allowedEvidenceIds")
        .and_then(Value::as_array)
        .ok_or(AiServiceError::InvalidOutput("研究上下文缺少证据目录"))?
        .iter()
        .filter_map(Value::as_str)
        .collect::<HashSet<_>>();
    let mut references = Vec::new();
    for item in &report.core_drivers {
        references.push(&item.evidence_ids);
    }
    if let Some(thesis) = &report.market_thesis {
        references.push(&thesis.evidence_ids);
    }
    if let Some(analysis) = &report.bull_bear_analysis {
        references.push(&analysis.evidence_ids);
        for item in analysis.bull.iter().chain(&analysis.bear) {
            references.push(&item.evidence_ids);
        }
    }
    for item in &report.future_catalysts {
        references.push(&item.evidence_ids);
    }
    for item in &report.risk_factors {
        references.push(&item.evidence_ids);
    }
    for ids in references {
        if ids.is_empty() || ids.iter().any(|id| !allowed.contains(id.as_str())) {
            return Err(AiServiceError::InvalidOutput(
                "AI研究结论未引用当前 Evidence Context",
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
        "目标价",
        "收益预测",
        "收益承诺",
        "保证收益",
        "必涨",
        "必跌",
        "稳赚",
        "一定上涨",
        "一定下跌",
        "price target",
        "guaranteed return",
        "guaranteed profit",
    ];
    let lower = value.to_lowercase();
    PROHIBITED.iter().any(|term| lower.contains(term))
}

fn connection_response_has_message(response: &Value) -> bool {
    response
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .is_some_and(|content| !content.trim().is_empty())
}

// Retained as a low-level OpenAI-compatible helper for future non-generative checks. Runtime
// connection verification uses `curl_ai_request` so it validates the selected model itself.
#[allow(dead_code)]
fn curl_ai_get_request(url: &str, api_key: &str) -> Result<(Value, u16), AiProviderError> {
    let response_path = unique_response_path();
    create_private_empty_file(&response_path)?;
    let result = (|| {
        let mut command = Command::new("curl");
        command
            .args([
                "--config",
                "-",
                "--silent",
                "--show-error",
                "--max-time",
                "15",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command
            .spawn()
            .map_err(|_| AiProviderError::unavailable())?;
        let config = format!(
            "url = \"{}\"\nrequest = \"GET\"\nheader = \"Authorization: Bearer {}\"\noutput = \"{}\"\nwrite-out = \"%{{http_code}}\"\n",
            curl_config_value(url),
            curl_config_value(api_key),
            curl_config_value(&response_path.to_string_lossy()),
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
            ai_diagnostic("ai_provider_connection_http_status=NETWORK_FAILURE");
            return Err(AiProviderError::unavailable());
        }
        let status = std::str::from_utf8(&output.stdout)
            .ok()
            .and_then(|value| value.trim().parse::<u16>().ok())
            .ok_or_else(|| AiProviderError::invalid_response("连接测试缺少HTTP状态码"))?;
        let response_body = fs::read(&response_path).map_err(|_| AiProviderError::unavailable())?;
        if !(200..300).contains(&status) {
            let safe_body = sanitize_api_error_body(&response_body, api_key);
            ai_diagnostic(format!(
                "ai_provider_connection_http_status={status} api_error_body={safe_body}"
            ));
            return Err(AiProviderError::http_error(status, &safe_body));
        }
        ai_diagnostic(format!("ai_provider_connection_http_status={status}"));
        let response = serde_json::from_slice(&response_body).map_err(|error| {
            ai_diagnostic(format!("connection_json_parse_failed=true reason={error}"));
            AiProviderError::invalid_response(error)
        })?;
        Ok((response, status))
    })();
    let _ = fs::remove_file(response_path);
    result
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
            ai_diagnostic("ai_provider_http_status=NETWORK_FAILURE api_error_body=<unavailable>");
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
                "ai_provider_http_status={status} api_error_body={safe_body}"
            ));
            return Err(AiProviderError::http_error(status, &safe_body));
        }
        ai_diagnostic(format!("ai_provider_http_status={status}"));
        serde_json::from_slice(&response_body).map_err(|error| {
            let raw_response = sanitize_api_error_body(&response_body, api_key);
            ai_diagnostic(format!(
                "json_parse_failed=true reason={error} raw_response={}",
                raw_response
            ));
            AiProviderError::invalid_response_with_raw(error, raw_response)
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

/// Diagnostics retain a bounded provider response for local troubleshooting, never a Keychain
/// key. Responses are not persisted in SQLite and are only emitted after redaction.
fn sanitize_diagnostic_response(response: &str, api_key: &str) -> String {
    const MAX_CHARS: usize = 8_000;
    let mut safe = response.replace(api_key, "[REDACTED]");
    if safe.chars().count() > MAX_CHARS {
        safe = format!(
            "{}…<truncated>",
            safe.chars().take(MAX_CHARS).collect::<String>()
        );
    }
    safe.replace(['\n', '\r'], " ")
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
                open_price: None,
                high_price: None,
                low_price: None,
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
                actions: Some("继续观察数据完整性与后续变化。".into()),
                core_drivers: None,
                market_thesis: None,
                bull_bear_analysis: None,
                future_catalysts: None,
                risk_factors: None,
                research_score: None,
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

    struct MalformedProvider;
    impl AiProviderAdapter for MalformedProvider {
        fn model(&self) -> &str {
            "malformed-response-model"
        }
        fn generate(&self, _: &AiReviewInput) -> Result<AiReviewSections, AiProviderError> {
            Err(AiProviderError::invalid_response(
                "invalid type: sequence, expected a string",
            ))
        }
    }

    struct FollowedSecurityProvider;
    impl AiProviderAdapter for FollowedSecurityProvider {
        fn model(&self) -> &str {
            "followed-security-test-model"
        }

        fn generate(&self, input: &AiReviewInput) -> Result<AiReviewSections, AiProviderError> {
            assert_eq!(input.security_symbol, "600519");
            assert_eq!(input.security_name, "贵州茅台");
            assert_eq!(input.news["status"], "NO_DATA");
            assert_eq!(input.events["status"], "NO_DATA");
            let serialized = serde_json::to_string(input).expect("serialize evidence context");
            assert!(!serialized.contains("quantity"));
            assert!(!serialized.contains("averageCost"));
            assert!(!serialized.contains("dailyPnl"));
            assert!(!serialized.contains("totalPnl"));
            assert_eq!(input.sector["status"], "UNAVAILABLE");
            assert!(input.research_context["allowedEvidenceIds"]
                .as_array()
                .is_some_and(|ids| ids.iter().any(|id| id == "MARKET")));
            Ok(AiReviewSections {
                facts: vec!["已保存行情快照仅供事实核验。".into()],
                inferences: vec!["关注标的分析只基于本次已保存数据。".into()],
                risks: vec!["板块与技术指标需要结合后续数据核验。".into()],
                stock_status: Some("当前个股情况以已保存行情快照为准。".into()),
                market_analysis: Some("当前市场环境以已保存指数快照为准。".into()),
                sector_analysis: Some("暂无可核验的板块数据。".into()),
                news_analysis: Some("暂无关联资讯数据。".into()),
                technical_analysis: Some("暂无完整技术指标。".into()),
                strategy_reference: Some("持续观察行情、板块和消息面的后续变化。".into()),
                conclusion: Some("当前结论仅基于本次已保存数据。".into()),
                actions: Some("继续观察行情和后续公告。".into()),
                core_drivers: Some(vec![ResearchDriver {
                    title: "已保存市场快照".into(),
                    rationale: "本次快照包含主要指数。".into(),
                    impact_level: "MEDIUM".into(),
                    evidence_ids: vec!["MARKET".into()],
                }]),
                market_thesis: Some(MarketThesis {
                    summary: "当前仅能基于已保存市场快照研究。".into(),
                    facts: "已保存指数快照可核验。".into(),
                    expectations: "暂无可验证市场预期数据。".into(),
                    sentiment: "暂无可验证情绪数据。".into(),
                    evidence_ids: vec!["MARKET".into()],
                }),
                bull_bear_analysis: Some(BullBearAnalysis {
                    bull: vec![BullBearPoint {
                        view: "指数快照已保存。".into(),
                        basis: "本次市场快照。".into(),
                        evidence_ids: vec!["MARKET".into()],
                    }],
                    bear: vec![BullBearPoint {
                        view: "板块资料未确认。".into(),
                        basis: "研究上下文。".into(),
                        evidence_ids: vec!["NO_DATA".into()],
                    }],
                    key_divergence: "可验证市场快照与缺失板块资料之间的分歧。".into(),
                    evidence_ids: vec!["MARKET".into()],
                }),
                future_catalysts: Some(vec![FutureCatalyst {
                    time_window: "1D".into(),
                    title: "暂无已确认催化事项".into(),
                    source: "暂无数据".into(),
                    credibility: "NO_DATA".into(),
                    time: "暂无数据".into(),
                    evidence_ids: vec!["NO_DATA".into()],
                }]),
                risk_factors: Some(vec![RiskFactor {
                    level: "HIGH".into(),
                    title: "板块资料未确认".into(),
                    reason: "当前没有可靠板块数据源。".into(),
                    evidence_ids: vec!["NO_DATA".into()],
                }]),
                research_score: Some(ResearchScore {
                    fundamental_attention: 0,
                    technical_state: 0,
                    market_heat: 50,
                    sentiment_state: 0,
                    risk_level: 80,
                    overall: 30,
                    explanation: "评分仅反映本次可验证研究数据的完整度，不构成投资建议。".into(),
                }),
            })
        }
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
    fn malformed_provider_response_never_persists_a_successful_review() {
        let database = DatabaseService::open_in_memory().unwrap();
        let review = stored_review_and_snapshot(&database);
        assert!(matches!(
            AiService::generate_with_provider(&database, "2026-08-08", &MalformedProvider),
            Err(AiServiceError::Provider(_))
        ));
        assert!(AiService::latest_for_review(&database, review.id)
            .unwrap()
            .is_none());
        assert_eq!(database.ai_review_context_count().unwrap(), 0);
    }

    #[test]
    fn followed_securities_receive_independent_reviews_and_contexts() {
        let database = DatabaseService::open_in_memory().unwrap();
        let review = stored_review_and_snapshot(&database);
        let security = database
            .create_security(crate::database::service::NewSecurity {
                symbol: "600519".into(),
                name: "贵州茅台".into(),
                market: "SSE".into(),
                exchange: "SSE".into(),
                security_type: "STOCK".into(),
                industry: None,
                concepts_json: "[]".into(),
                trade_rule: "T_PLUS_1".into(),
            })
            .unwrap();
        database.create_watchlist_item(security.id).unwrap();

        let research_run = database
            .create_research_run("2026-08-08T08:01:00Z")
            .expect("create research run");
        let generated = AiService::generate_for_all_followed_securities(
            &database,
            "2026-08-08",
            &FollowedSecurityProvider,
            Some(research_run.id),
        )
        .unwrap();

        assert_eq!(generated.len(), 1);
        assert_eq!(generated[0].security_id, Some(security.id));
        assert_eq!(generated[0].security_symbol.as_deref(), Some("600519"));
        assert_eq!(database.ai_review_context_count().unwrap(), 1);
        assert_eq!(database.ai_research_report_count().unwrap(), 1);
        assert_eq!(
            generated[0]
                .report
                .as_ref()
                .map(|report| report.core_drivers.len()),
            Some(1)
        );
        assert_eq!(
            database
                .get_ai_review(generated[0].id)
                .expect("read generated review")
                .research_run_id,
            Some(research_run.id)
        );
        assert_eq!(
            AiService::list_for_review(&database, review.id)
                .unwrap()
                .first()
                .and_then(|item| item.security_id),
            Some(security.id)
        );
    }
    #[test]
    fn invalid_json_and_prohibited_content_are_rejected() {
        assert!(parse_and_normalize_provider_sections(r#"{"facts":[]}"#).is_err());
        assert!(matches!(
            parse_and_normalize_provider_sections(
                r#"{"facts":["行情暂无数据"],"inferences":["目标价为100元"],"risks":["需要核验来源"]}"#
            ),
            Err(AiServiceError::InvalidOutput(_))
        ));
    }

    #[test]
    fn response_normalizer_maps_explicit_chinese_fields_without_inventing_data() {
        let normalized = parse_and_normalize_provider_sections(
            r#"{
              "当前个股情况":"个股行情来自已保存快照",
              "市场环境分析":"市场指数本次快照已保存",
              "消息面分析":"暂无关联公告",
              "技术面分析":["短期走势暂无完整数据"],
              "风险提示":"需要核验后续数据",
              "策略参考":"本报告不提供交易建议，建议观察公告",
              "summary":"仅基于本次保存数据"
            }"#,
        )
        .expect("normalize compatible response");
        assert_eq!(normalized.facts.len(), 3);
        assert_eq!(normalized.inferences, vec!["短期走势暂无完整数据"]);
        assert_eq!(normalized.risks, vec!["需要核验后续数据"]);
        assert_eq!(
            normalized.strategy_reference.as_deref(),
            Some("本报告不提供交易建议，建议观察公告")
        );
    }

    #[test]
    fn response_normalizer_accepts_summary_style_provider_payload() {
        let normalized = parse_and_normalize_provider_sections(
            r#"{"analysis":"市场分析仅以本次快照为准","summary":"继续关注后续公告"}"#,
        )
        .expect("normalize summary style response");
        assert_eq!(normalized.facts, vec!["市场分析仅以本次快照为准"]);
        assert_eq!(normalized.conclusion.as_deref(), Some("继续关注后续公告"));
    }

    #[test]
    fn response_normalizer_rejects_unmappable_payloads() {
        assert!(matches!(
            parse_and_normalize_provider_sections(r#"["unexpected", "array"]"#),
            Err(AiServiceError::InvalidOutput("AI_REVIEW_PARSE_FAILED"))
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
                intelligence_json: "{}".into(),
                research_context_json: "{}".into(),
                research_run_id: None,
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
                security_id: None,
                research_run_id: None,
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
        assert_eq!(error.http_status, Some(401));
        assert_eq!(
            sanitize_api_error_body(b"{\"error\":\"sk-test-secret\"}", "sk-test-secret"),
            "{\"error\":\"[REDACTED]\"}"
        );
    }

    #[test]
    fn tokenhub_provider_uses_the_hy3_openai_compatible_configuration() {
        assert_eq!(
            provider_display_name("TENCENT_TOKENHUB"),
            "腾讯混元（TokenHub）"
        );
        assert!(connection_response_has_message(
            &json!({"choices": [{"message": {"content": "pong"}}]})
        ));
        assert!(!connection_response_has_message(&json!({"choices": []})));
    }

    #[test]
    fn provider_status_marks_only_the_runtime_priority_winner_as_current() {
        let mut providers = vec![
            AiProviderConfigView {
                provider: "DEEPSEEK".into(),
                display_name: "DeepSeek".into(),
                model: "deepseek-chat".into(),
                endpoint: "https://api.deepseek.com/chat/completions".into(),
                configured: true,
                enabled: true,
                is_current: false,
                priority: 1,
            },
            AiProviderConfigView {
                provider: "TENCENT_TOKENHUB".into(),
                display_name: "腾讯混元（TokenHub）".into(),
                model: "hy3".into(),
                endpoint: "https://tokenhub.tencentmaas.com/v1/chat/completions".into(),
                configured: true,
                enabled: true,
                is_current: false,
                priority: 2,
            },
            AiProviderConfigView {
                provider: "DOUBAO".into(),
                display_name: "豆包".into(),
                model: "doubao-seed-1-6-250615".into(),
                endpoint: "https://ark.cn-beijing.volces.com/api/v3/chat/completions".into(),
                configured: true,
                enabled: false,
                is_current: false,
                priority: 3,
            },
        ];

        mark_current_provider(&mut providers);

        assert!(providers[0].is_current);
        assert!(!providers[1].is_current);
        assert!(!providers[2].is_current);
    }
}
