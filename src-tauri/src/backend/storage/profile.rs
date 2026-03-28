use super::*;
use crate::backend::models::domain::{AgentProfile, ProfileTarget};

impl StorageService {
    pub fn list_profiles(&self) -> AppResult<Vec<AgentProfile>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT target, content, created_at, updated_at
             FROM agent_profiles
             ORDER BY CASE target
                WHEN 'user' THEN 0
                WHEN 'soul' THEN 1
                ELSE 9
             END ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let target_raw = row.get::<_, String>(0)?;
            let target = target_raw.parse().map_err(|err: AppError| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::other(err.message())),
                )
            })?;
            Ok(AgentProfile {
                target,
                content: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_profile(&self, target: ProfileTarget) -> AppResult<AgentProfile> {
        let conn = self.open_connection()?;
        conn.query_row(
            "SELECT target, content, created_at, updated_at
             FROM agent_profiles
             WHERE target = ?1",
            [target.as_str()],
            |row| {
                let target_raw = row.get::<_, String>(0)?;
                let parsed_target = target_raw.parse().map_err(|err: AppError| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::other(err.message())),
                    )
                })?;
                Ok(AgentProfile {
                    target: parsed_target,
                    content: row.get(1)?,
                    created_at: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("profile `{}`", target.as_str())))
    }

    pub fn upsert_profile(&self, target: ProfileTarget, content: &str) -> AppResult<AgentProfile> {
        let conn = self.open_connection()?;
        let tx = conn.unchecked_transaction()?;
        let now = now_timestamp();
        let created_at = tx
            .query_row(
                "SELECT created_at FROM agent_profiles WHERE target = ?1",
                [target.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .unwrap_or_else(|| now.clone());
        tx.execute(
            "INSERT INTO agent_profiles (target, content, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(target) DO UPDATE SET
                content = excluded.content,
                updated_at = excluded.updated_at",
            params![target.as_str(), content, created_at, now],
        )?;
        tx.commit()?;

        Ok(AgentProfile {
            target,
            content: content.to_string(),
            created_at,
            updated_at: now,
        })
    }
}
