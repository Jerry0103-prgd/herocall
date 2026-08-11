//! Source-backed local investment event calendar service.
//!
//! This module stores only explicit event records from a future Adapter or an explicit import.
//! It never infers dates, securities, or confirmation states.

use std::{error::Error, fmt};

use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};

use crate::database::service::{
    DatabaseError, DatabaseService, EventRecordUpdate, EventWithSecurity, NewEventRecord,
};
use crate::disclosure_adapter::DisclosureSecurity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventType {
    Earnings,
    Dividend,
    ExDividend,
    ShareholderMeeting,
    MacroData,
    FedMeeting,
    CompanyAnnouncement,
    MajorMatter,
}

impl EventType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Earnings => "EARNINGS",
            Self::Dividend => "DIVIDEND",
            Self::ExDividend => "EX_DIVIDEND",
            Self::ShareholderMeeting => "SHAREHOLDER_MEETING",
            Self::MacroData => "MACRO_DATA",
            Self::FedMeeting => "FED_MEETING",
            Self::CompanyAnnouncement => "COMPANY_ANNOUNCEMENT",
            Self::MajorMatter => "MAJOR_MATTER",
        }
    }

    fn parse(value: &str) -> Result<Self, EventServiceError> {
        match value {
            "EARNINGS" => Ok(Self::Earnings),
            "DIVIDEND" => Ok(Self::Dividend),
            "EX_DIVIDEND" => Ok(Self::ExDividend),
            "SHAREHOLDER_MEETING" => Ok(Self::ShareholderMeeting),
            "MACRO_DATA" => Ok(Self::MacroData),
            "FED_MEETING" => Ok(Self::FedMeeting),
            "COMPANY_ANNOUNCEMENT" => Ok(Self::CompanyAnnouncement),
            "MAJOR_MATTER" => Ok(Self::MajorMatter),
            _ => Err(EventServiceError::Validation("事件类型未确认")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventStatus {
    Confirmed,
    Unconfirmed,
    Archived,
}

impl EventStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Confirmed => "CONFIRMED",
            Self::Unconfirmed => "UNCONFIRMED",
            Self::Archived => "ARCHIVED",
        }
    }

    fn parse(value: &str) -> Result<Self, EventServiceError> {
        match value {
            "CONFIRMED" => Ok(Self::Confirmed),
            "UNCONFIRMED" => Ok(Self::Unconfirmed),
            "ARCHIVED" => Ok(Self::Archived),
            _ => Err(EventServiceError::Validation("事件状态未确认")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventSource {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventInput {
    pub event_type: EventType,
    pub title: String,
    pub security_id: Option<i64>,
    pub event_time: String,
    pub timezone: String,
    pub source: String,
    pub source_url: Option<String>,
    pub status: EventStatus,
}

/// Future official, media, or macro-calendar adapters must return complete source-backed inputs.
pub trait EventDataAdapter {
    fn source(&self) -> EventSource;
    fn fetch_events(
        &self,
        securities: &[DisclosureSecurity],
    ) -> Result<Vec<EventInput>, EventAdapterError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventAdapterError {
    pub message: String,
}

impl fmt::Display for EventAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for EventAdapterError {}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EventView {
    pub id: i64,
    pub event_type: String,
    pub title: String,
    pub event_time: String,
    pub timezone: String,
    pub source: String,
    pub source_url: Option<String>,
    pub status: String,
    pub related_security: Option<String>,
    pub holding_related: bool,
}

#[derive(Debug)]
pub enum EventServiceError {
    Database(DatabaseError),
    Validation(&'static str),
    MissingSecurity,
    MissingEvent,
}

impl fmt::Display for EventServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "database error: {error}"),
            Self::Validation(message) => formatter.write_str(message),
            Self::MissingSecurity => formatter.write_str("未找到关联证券"),
            Self::MissingEvent => formatter.write_str("未找到事件"),
        }
    }
}

impl Error for EventServiceError {}

impl From<DatabaseError> for EventServiceError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

pub struct EventService;

impl EventService {
    pub fn list_for_run_and_security(
        database: &DatabaseService,
        run_id: i64,
        security_id: i64,
    ) -> Result<Vec<EventView>, EventServiceError> {
        database
            .list_events_for_run_and_security(run_id, security_id)?
            .into_iter()
            .map(Self::view_from_record)
            .collect()
    }

    pub fn create(
        database: &DatabaseService,
        input: EventInput,
    ) -> Result<EventView, EventServiceError> {
        let event = Self::validate_input(database, input)?;
        let created = database.create_event(event)?;
        Self::list(database, None)?
            .into_iter()
            .find(|event| event.id == created.id)
            .ok_or(EventServiceError::MissingEvent)
    }

    pub fn update(
        database: &DatabaseService,
        id: i64,
        input: EventInput,
    ) -> Result<EventView, EventServiceError> {
        match database.get_event(id) {
            Ok(_) => {}
            Err(DatabaseError::Sqlite(rusqlite::Error::QueryReturnedNoRows)) => {
                return Err(EventServiceError::MissingEvent);
            }
            Err(error) => return Err(EventServiceError::Database(error)),
        }
        let event = Self::validate_input(database, input)?;
        database.update_event(
            id,
            EventRecordUpdate {
                event_type: event.event_type,
                title: event.title,
                security_id: event.security_id,
                event_time: event.event_time,
                timezone: event.timezone,
                source: event.source,
                source_url: event.source_url,
                status: event.status,
            },
        )?;
        Self::list(database, None)?
            .into_iter()
            .find(|event| event.id == id)
            .ok_or(EventServiceError::MissingEvent)
    }

    pub fn delete(database: &DatabaseService, id: i64) -> Result<(), EventServiceError> {
        if database.delete_event(id)? == 0 {
            return Err(EventServiceError::MissingEvent);
        }
        Ok(())
    }

    /// Provider events are idempotent by their source URL when present.
    pub fn ingest(
        database: &DatabaseService,
        input: EventInput,
    ) -> Result<EventView, EventServiceError> {
        let event = Self::validate_input(database, input)?;
        let stored = database.upsert_event_by_source_url(event)?;
        Self::list(database, None)?
            .into_iter()
            .find(|view| view.id == stored.id)
            .ok_or(EventServiceError::MissingEvent)
    }

    pub fn list_for_manual_refresh_run(
        database: &DatabaseService,
        run_id: i64,
    ) -> Result<Vec<EventView>, EventServiceError> {
        database
            .list_events_for_manual_refresh_run(run_id)?
            .into_iter()
            .map(Self::view_from_record)
            .collect()
    }

    pub fn list(
        database: &DatabaseService,
        status_filter: Option<&str>,
    ) -> Result<Vec<EventView>, EventServiceError> {
        let status_filter = status_filter.map(EventStatus::parse).transpose()?;
        let mut events: Vec<_> = database
            .list_events()?
            .into_iter()
            .map(Self::view_from_record)
            .collect::<Result<_, _>>()?;
        if let Some(status_filter) = status_filter {
            events.retain(|event| event.status == status_filter.as_str());
        }
        events.sort_by(|left, right| {
            right
                .holding_related
                .cmp(&left.holding_related)
                .then_with(|| {
                    parse_event_time(&left.event_time).cmp(&parse_event_time(&right.event_time))
                })
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(events)
    }

    fn validate_input(
        database: &DatabaseService,
        input: EventInput,
    ) -> Result<NewEventRecord, EventServiceError> {
        let title = required_text(&input.title, "事件名称不能为空")?;
        let timezone = required_text(&input.timezone, "事件时区不能为空")?;
        let source = required_text(&input.source, "事件来源不能为空")?;
        if timezone.chars().any(char::is_control) {
            return Err(EventServiceError::Validation("事件时区无效"));
        }
        DateTime::parse_from_rfc3339(input.event_time.trim())
            .map_err(|_| EventServiceError::Validation("事件时间必须是带时区的 ISO 8601"))?;
        if let Some(security_id) = input.security_id {
            match database.get_security(security_id) {
                Ok(_) => {}
                Err(DatabaseError::Sqlite(rusqlite::Error::QueryReturnedNoRows)) => {
                    return Err(EventServiceError::MissingSecurity);
                }
                Err(error) => return Err(EventServiceError::Database(error)),
            }
        }
        let source_url = match input.source_url {
            Some(url) if url.trim().is_empty() => None,
            Some(url) if url.starts_with("https://") || url.starts_with("http://") => Some(url),
            Some(_) => {
                return Err(EventServiceError::Validation(
                    "事件原文地址必须是 HTTP(S) 地址",
                ))
            }
            None => None,
        };
        Ok(NewEventRecord {
            event_type: input.event_type.as_str().into(),
            title,
            security_id: input.security_id,
            event_time: input.event_time.trim().into(),
            timezone,
            source,
            source_url,
            status: input.status.as_str().into(),
        })
    }

    fn view_from_record(record: EventWithSecurity) -> Result<EventView, EventServiceError> {
        let event_type = EventType::parse(&record.event.event_type)?;
        let status = EventStatus::parse(&record.event.status)?;
        let related_security = match (record.security_name, record.security_symbol) {
            (Some(name), Some(symbol)) => Some(format!("{name} ({symbol})")),
            _ => None,
        };
        Ok(EventView {
            id: record.event.id,
            event_type: event_type.as_str().into(),
            title: record.event.title,
            event_time: record.event.event_time,
            timezone: record.event.timezone,
            source: record.event.source,
            source_url: record.event.source_url,
            status: status.as_str().into(),
            related_security,
            holding_related: record.holding_related,
        })
    }
}

fn required_text(value: &str, message: &'static str) -> Result<String, EventServiceError> {
    let value = value.trim();
    if value.is_empty() {
        Err(EventServiceError::Validation(message))
    } else {
        Ok(value.into())
    }
}

fn parse_event_time(value: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(value).expect("stored events use validated timestamps")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::service::{NewCashAccount, NewHolding, NewSecurity};

    fn holding_security(database: &DatabaseService) -> i64 {
        let security = database
            .create_security(NewSecurity {
                symbol: "600519".into(),
                name: "测试持仓证券".into(),
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
                name: "事件测试账户".into(),
                currency: "CNY".into(),
                available_to_buy: "0".into(),
                withdrawable_cash: "0".into(),
                pending_settlement: "0".into(),
            })
            .expect("create account");
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
        database
            .create_watchlist_item(security.id)
            .expect("create explicit follow");
        security.id
    }

    fn event_input(
        event_type: EventType,
        title: &str,
        security_id: Option<i64>,
        event_time: &str,
        status: EventStatus,
    ) -> EventInput {
        EventInput {
            event_type,
            title: title.into(),
            security_id,
            event_time: event_time.into(),
            timezone: "Asia/Shanghai".into(),
            source: "Recorded event test source".into(),
            source_url: Some("https://example.invalid/events/test".into()),
            status,
        }
    }

    #[test]
    fn event_crud_sorts_holding_events_and_filters_by_status() {
        let database = DatabaseService::open_in_memory().expect("create database");
        let security_id = holding_security(&database);
        let macro_event = EventService::create(
            &database,
            event_input(
                EventType::MacroData,
                "测试宏观数据",
                None,
                "2026-08-07T09:00:00+08:00",
                EventStatus::Confirmed,
            ),
        )
        .expect("create macro event");
        let earnings = EventService::create(
            &database,
            event_input(
                EventType::Earnings,
                "测试业绩公告",
                Some(security_id),
                "2026-08-09T09:00:00+08:00",
                EventStatus::Confirmed,
            ),
        )
        .expect("create earnings event");
        let dividend = EventService::create(
            &database,
            event_input(
                EventType::Dividend,
                "测试分红公告",
                Some(security_id),
                "2026-08-10T09:00:00+08:00",
                EventStatus::Unconfirmed,
            ),
        )
        .expect("create dividend event");

        let events = EventService::list(&database, None).expect("list events");
        assert_eq!(
            events.iter().map(|event| event.id).collect::<Vec<_>>(),
            vec![earnings.id, dividend.id, macro_event.id]
        );
        assert!(events[0].holding_related);
        assert_eq!(
            events[0].related_security.as_deref(),
            Some("测试持仓证券 (600519)")
        );

        let confirmed = EventService::list(&database, Some("CONFIRMED")).expect("filter confirmed");
        assert_eq!(confirmed.len(), 2);
        assert!(confirmed.iter().all(|event| event.status == "CONFIRMED"));

        let updated = EventService::update(
            &database,
            dividend.id,
            event_input(
                EventType::ExDividend,
                "更新后的测试除权除息",
                Some(security_id),
                "2026-08-10T09:00:00+08:00",
                EventStatus::Archived,
            ),
        )
        .expect("update event");
        assert_eq!(updated.event_type, "EX_DIVIDEND");
        assert_eq!(updated.status, "ARCHIVED");

        EventService::delete(&database, macro_event.id).expect("delete macro event");
        assert_eq!(
            EventService::list(&database, None)
                .expect("list after delete")
                .len(),
            2
        );
    }

    #[test]
    fn event_adapter_port_requires_explicit_records_without_network_access() {
        struct RecordedAdapter;
        impl EventDataAdapter for RecordedAdapter {
            fn source(&self) -> EventSource {
                EventSource {
                    name: "Recorded event adapter".into(),
                }
            }

            fn fetch_events(
                &self,
                _securities: &[DisclosureSecurity],
            ) -> Result<Vec<EventInput>, EventAdapterError> {
                Ok(Vec::new())
            }
        }

        let adapter = RecordedAdapter;
        assert_eq!(adapter.source().name, "Recorded event adapter");
        assert!(adapter
            .fetch_events(&[])
            .expect("recorded adapter")
            .is_empty());
    }
}
