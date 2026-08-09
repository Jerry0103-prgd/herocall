//! Traceable local news application service.
//!
//! Provider-specific transports belong behind `NewsDataAdapter`. This module never fabricates
//! articles: it stores only complete records supplied by a future Adapter or explicit import.

use std::{error::Error, fmt};

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

use crate::database::service::{
    DatabaseError, DatabaseService, NewNewsArticle, NewsArticleUpdate, NewsArticleWithSecurity,
};
use crate::disclosure_adapter::DisclosureSecurity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NewsSourceType {
    Official,
    Media,
    Community,
}

impl NewsSourceType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Official => "OFFICIAL",
            Self::Media => "MEDIA",
            Self::Community => "COMMUNITY",
        }
    }

    fn parse(value: &str) -> Result<Self, NewsServiceError> {
        match value {
            "OFFICIAL" => Ok(Self::Official),
            "MEDIA" => Ok(Self::Media),
            "COMMUNITY" => Ok(Self::Community),
            _ => Err(NewsServiceError::Validation("资讯来源类型未确认")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsDataSource {
    pub name: String,
    pub source_type: NewsSourceType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsArticleInput {
    pub title: String,
    pub source: String,
    pub source_type: NewsSourceType,
    pub published_at: String,
    pub fetch_time: String,
    pub summary: String,
    pub url: String,
    pub related_security_id: Option<i64>,
}

/// Port reserved for official announcements, professional media and community sources.
/// An Adapter must return the source metadata with every article and may not create fallback news.
pub trait NewsDataAdapter {
    fn source(&self) -> NewsDataSource;
    fn fetch_articles(
        &self,
        securities: &[DisclosureSecurity],
    ) -> Result<Vec<NewsArticleInput>, NewsAdapterError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsAdapterError {
    pub message: String,
}

impl fmt::Display for NewsAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for NewsAdapterError {}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NewsArticleView {
    pub id: i64,
    pub title: String,
    pub source: String,
    pub source_type: String,
    pub published_at: String,
    pub fetch_time: String,
    pub summary: String,
    pub url: String,
    pub related_security: Option<String>,
}

#[derive(Debug)]
pub enum NewsServiceError {
    Database(DatabaseError),
    Validation(&'static str),
    MissingRelatedSecurity,
    MissingArticle,
}

impl fmt::Display for NewsServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "database error: {error}"),
            Self::Validation(message) => formatter.write_str(message),
            Self::MissingRelatedSecurity => formatter.write_str("未找到关联证券"),
            Self::MissingArticle => formatter.write_str("未找到资讯"),
        }
    }
}

impl Error for NewsServiceError {}

impl From<DatabaseError> for NewsServiceError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

pub struct NewsService;

impl NewsService {
    pub fn create(
        database: &DatabaseService,
        input: NewsArticleInput,
    ) -> Result<NewsArticleView, NewsServiceError> {
        let article = Self::validate_input(database, input)?;
        let created = database.create_news_article(article)?;
        Self::list(database)?
            .into_iter()
            .find(|view| view.id == created.id)
            .ok_or(NewsServiceError::MissingArticle)
    }

    pub fn update(
        database: &DatabaseService,
        id: i64,
        input: NewsArticleInput,
    ) -> Result<NewsArticleView, NewsServiceError> {
        match database.get_news_article(id) {
            Ok(_) => {}
            Err(DatabaseError::Sqlite(rusqlite::Error::QueryReturnedNoRows)) => {
                return Err(NewsServiceError::MissingArticle);
            }
            Err(error) => return Err(NewsServiceError::Database(error)),
        }
        let article = Self::validate_input(database, input)?;
        database.update_news_article(
            id,
            NewsArticleUpdate {
                title: article.title,
                source: article.source,
                source_type: article.source_type,
                published_at: article.published_at,
                fetch_time: article.fetch_time,
                summary: article.summary,
                url: article.url,
                related_security_id: article.related_security_id,
            },
        )?;
        Self::list(database)?
            .into_iter()
            .find(|view| view.id == id)
            .ok_or(NewsServiceError::MissingArticle)
    }

    pub fn delete(database: &DatabaseService, id: i64) -> Result<(), NewsServiceError> {
        if database.delete_news_article(id)? == 0 {
            return Err(NewsServiceError::MissingArticle);
        }
        Ok(())
    }

    /// External ingestion is idempotent by the provider's canonical article URL.
    pub fn ingest(
        database: &DatabaseService,
        input: NewsArticleInput,
    ) -> Result<NewsArticleView, NewsServiceError> {
        let article = Self::validate_input(database, input)?;
        let stored = database.upsert_news_article(article)?;
        Self::list(database)?
            .into_iter()
            .find(|view| view.id == stored.id)
            .ok_or(NewsServiceError::MissingArticle)
    }

    pub fn list_for_manual_refresh_run(
        database: &DatabaseService,
        run_id: i64,
    ) -> Result<Vec<NewsArticleView>, NewsServiceError> {
        database
            .list_news_articles_for_manual_refresh_run(run_id)?
            .into_iter()
            .map(Self::view_from_record)
            .collect()
    }

    pub fn list(database: &DatabaseService) -> Result<Vec<NewsArticleView>, NewsServiceError> {
        database
            .list_news_articles()?
            .into_iter()
            .map(Self::view_from_record)
            .collect()
    }

    pub fn list_for_holdings(
        database: &DatabaseService,
    ) -> Result<Vec<NewsArticleView>, NewsServiceError> {
        database
            .list_news_articles_for_holdings()?
            .into_iter()
            .map(Self::view_from_record)
            .collect()
    }

    fn validate_input(
        database: &DatabaseService,
        input: NewsArticleInput,
    ) -> Result<NewNewsArticle, NewsServiceError> {
        let title = required_text(&input.title, "资讯标题不能为空")?;
        let source = required_text(&input.source, "资讯来源不能为空")?;
        let summary = required_text(&input.summary, "资讯摘要不能为空")?;
        let url = input.url.trim();
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err(NewsServiceError::Validation("原文地址必须是 HTTP(S) 地址"));
        }
        if let Some(security_id) = input.related_security_id {
            match database.get_security(security_id) {
                Ok(_) => {}
                Err(DatabaseError::Sqlite(rusqlite::Error::QueryReturnedNoRows)) => {
                    return Err(NewsServiceError::MissingRelatedSecurity);
                }
                Err(error) => return Err(NewsServiceError::Database(error)),
            }
        }
        Ok(NewNewsArticle {
            title,
            source,
            source_type: input.source_type.as_str().into(),
            published_at: normalized_timestamp(&input.published_at)?,
            fetch_time: normalized_timestamp(&input.fetch_time)?,
            summary,
            url: url.into(),
            related_security_id: input.related_security_id,
        })
    }

    fn view_from_record(
        record: NewsArticleWithSecurity,
    ) -> Result<NewsArticleView, NewsServiceError> {
        let source_type = NewsSourceType::parse(&record.article.source_type)?;
        let related_security = match (record.related_security_name, record.related_security_symbol)
        {
            (Some(name), Some(symbol)) => Some(format!("{name} ({symbol})")),
            _ => None,
        };
        Ok(NewsArticleView {
            id: record.article.id,
            title: record.article.title,
            source: record.article.source,
            source_type: source_type.as_str().into(),
            published_at: record.article.published_at,
            fetch_time: record.article.fetch_time,
            summary: record.article.summary,
            url: record.article.url,
            related_security,
        })
    }
}

fn required_text(value: &str, message: &'static str) -> Result<String, NewsServiceError> {
    let value = value.trim();
    if value.is_empty() {
        Err(NewsServiceError::Validation(message))
    } else {
        Ok(value.into())
    }
}

fn normalized_timestamp(value: &str) -> Result<String, NewsServiceError> {
    DateTime::parse_from_rfc3339(value.trim())
        .map(|time| {
            time.with_timezone(&Utc)
                .to_rfc3339_opts(SecondsFormat::Millis, true)
        })
        .map_err(|_| NewsServiceError::Validation("资讯时间必须是带时区的 ISO 8601"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::service::{NewCashAccount, NewHolding, NewSecurity};

    fn related_security(database: &DatabaseService) -> i64 {
        let security = database
            .create_security(NewSecurity {
                symbol: "600519".into(),
                name: "贵州茅台".into(),
                market: "SSE".into(),
                exchange: "SSE".into(),
                security_type: "STOCK".into(),
                industry: None,
                concepts_json: "[]".into(),
                trade_rule: "T_PLUS_1".into(),
            })
            .expect("create security");
        let account = database
            .create_cash_account(NewCashAccount {
                name: "新闻测试账户".into(),
                currency: "CNY".into(),
                available_to_buy: "0".into(),
                withdrawable_cash: "0".into(),
                pending_settlement: "0".into(),
            })
            .expect("create cash account");
        database
            .create_holding(NewHolding {
                cash_account_id: account.id,
                security_id: security.id,
                quantity: 100,
                available_quantity: 100,
                average_cost: "1".into(),
                cost_amount: "100".into(),
                position_source: "MANUAL".into(),
                as_of_date: None,
            })
            .expect("create holding");
        security.id
    }

    fn article_input(
        source_type: NewsSourceType,
        related_security_id: Option<i64>,
    ) -> NewsArticleInput {
        NewsArticleInput {
            title: "测试资讯标题".into(),
            source: "测试来源".into(),
            source_type,
            published_at: "2026-08-08T09:30:00+08:00".into(),
            fetch_time: "2026-08-08T10:00:00+08:00".into(),
            summary: "仅用于 Rust 服务测试的可追溯测试夹具。".into(),
            url: "https://example.invalid/news/test".into(),
            related_security_id,
        }
    }

    #[test]
    fn news_crud_keeps_traceable_fields_and_holding_association() {
        let database = DatabaseService::open_in_memory().expect("create database");
        let security_id = related_security(&database);
        let created = NewsService::create(
            &database,
            article_input(NewsSourceType::Official, Some(security_id)),
        )
        .expect("create official news");
        assert_eq!(created.source_type, "OFFICIAL");
        assert_eq!(
            created.related_security.as_deref(),
            Some("贵州茅台 (600519)")
        );
        assert_eq!(created.published_at, "2026-08-08T01:30:00.000Z");
        assert_eq!(created.fetch_time, "2026-08-08T02:00:00.000Z");

        let community = NewsService::create(
            &database,
            NewsArticleInput {
                title: "社区测试观点".into(),
                url: "https://example.invalid/news/community".into(),
                ..article_input(NewsSourceType::Community, None)
            },
        )
        .expect("create community news");
        assert_eq!(community.source_type, "COMMUNITY");

        let updated = NewsService::update(
            &database,
            created.id,
            NewsArticleInput {
                title: "更新后的测试资讯标题".into(),
                ..article_input(NewsSourceType::Media, Some(security_id))
            },
        )
        .expect("update news");
        assert_eq!(updated.title, "更新后的测试资讯标题");
        assert_eq!(updated.source_type, "MEDIA");

        assert_eq!(NewsService::list(&database).expect("list news").len(), 2);
        let holding_news = NewsService::list_for_holdings(&database).expect("list holding news");
        assert_eq!(holding_news, vec![updated]);

        NewsService::delete(&database, community.id).expect("delete community news");
        assert_eq!(
            NewsService::list(&database)
                .expect("list after delete")
                .len(),
            1
        );
    }

    #[test]
    fn news_adapter_port_preserves_source_category_without_network_access() {
        struct RecordedAdapter;
        impl NewsDataAdapter for RecordedAdapter {
            fn source(&self) -> NewsDataSource {
                NewsDataSource {
                    name: "Recorded official adapter".into(),
                    source_type: NewsSourceType::Official,
                }
            }

            fn fetch_articles(
                &self,
                _securities: &[DisclosureSecurity],
            ) -> Result<Vec<NewsArticleInput>, NewsAdapterError> {
                Ok(Vec::new())
            }
        }

        let adapter = RecordedAdapter;
        assert_eq!(adapter.source().source_type, NewsSourceType::Official);
        assert!(adapter
            .fetch_articles(&[])
            .expect("recorded adapter")
            .is_empty());
    }
}
