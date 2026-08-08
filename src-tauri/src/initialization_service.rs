//! First-run state boundary for the optional local setup wizard.
//!
//! This service stores only a completion flag. Cash, holdings and provider configuration remain
//! owned by their existing application services; provider keys never enter SQLite.

use std::{error::Error, fmt};

use serde::Serialize;

use crate::database::service::{DatabaseError, DatabaseService};

const INITIALIZATION_COMPLETED_KEY: &str = "initialization_completed";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InitializationStatusView {
    pub completed: bool,
}

#[derive(Debug)]
pub enum InitializationError {
    Database(DatabaseError),
}

impl fmt::Display for InitializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "database error: {error}"),
        }
    }
}

impl Error for InitializationError {}

impl From<DatabaseError> for InitializationError {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

pub struct InitializationService;

impl InitializationService {
    pub fn status(
        database: &DatabaseService,
    ) -> Result<InitializationStatusView, InitializationError> {
        Ok(InitializationStatusView {
            completed: database
                .get_app_setting(INITIALIZATION_COMPLETED_KEY)?
                .as_deref()
                == Some("true"),
        })
    }

    pub fn complete(
        database: &DatabaseService,
    ) -> Result<InitializationStatusView, InitializationError> {
        database.set_app_setting(INITIALIZATION_COMPLETED_KEY, "true")?;
        Self::status(database)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::InitializationService;
    use crate::database::service::DatabaseService;

    #[test]
    fn first_run_is_incomplete_then_completion_is_saved() {
        let database = DatabaseService::open_in_memory().expect("open in-memory database");

        assert!(
            !InitializationService::status(&database)
                .expect("read first-run status")
                .completed
        );
        assert!(
            InitializationService::complete(&database)
                .expect("save completion")
                .completed
        );
        assert!(
            InitializationService::status(&database)
                .expect("read persisted status")
                .completed
        );
    }

    #[test]
    fn completion_survives_database_reopen() {
        let path = std::env::temp_dir().join(format!(
            "astock-ai-workbench-first-run-{}-{}.sqlite3",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().expect("timestamp")
        ));

        {
            let database = DatabaseService::open(&path).expect("open first-run database");
            assert!(
                !InitializationService::status(&database)
                    .expect("read new database status")
                    .completed
            );
            InitializationService::complete(&database).expect("save completion");
        }
        let reopened = DatabaseService::open(&path).expect("reopen persisted database");
        assert!(
            InitializationService::status(&reopened)
                .expect("read restored status")
                .completed
        );
        drop(reopened);
        fs::remove_file(path).expect("remove temporary database");
    }
}
