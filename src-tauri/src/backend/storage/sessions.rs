use super::*;

impl StorageService {
    pub fn list_sessions(&self) -> AppResult<Vec<ChatSession>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, provider_profile_id, created_at, updated_at, last_run_at
             FROM chat_sessions
             ORDER BY COALESCE(last_run_at, updated_at) DESC, created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ChatSession {
                id: row.get(0)?,
                title: row.get(1)?,
                provider_profile_id: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                last_run_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_session(&self, session_id: &str) -> AppResult<ChatSession> {
        let conn = self.open_connection()?;
        conn.query_row(
            "SELECT id, title, provider_profile_id, created_at, updated_at, last_run_at
             FROM chat_sessions WHERE id = ?1",
            [session_id],
            |row| {
                Ok(ChatSession {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    provider_profile_id: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    last_run_at: row.get(5)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("session `{session_id}`")))
    }

    pub fn insert_session(&self, session: &ChatSession) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO chat_sessions (id, title, provider_profile_id, created_at, updated_at, last_run_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                session.id,
                session.title,
                session.provider_profile_id,
                session.created_at,
                session.updated_at,
                session.last_run_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_session_provider(
        &self,
        session_id: &str,
        provider_profile_id: &str,
    ) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "UPDATE chat_sessions SET provider_profile_id = ?2, updated_at = ?3 WHERE id = ?1",
            params![session_id, provider_profile_id, now_timestamp()],
        )?;
        Ok(())
    }

    pub fn clear_session_provider_binding(&self, provider_profile_id: &str) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "UPDATE chat_sessions SET provider_profile_id = NULL, updated_at = ?2 WHERE provider_profile_id = ?1",
            params![provider_profile_id, now_timestamp()],
        )?;
        Ok(())
    }

    pub fn touch_session_for_run(&self, session_id: &str, title: Option<&str>) -> AppResult<()> {
        let conn = self.open_connection()?;
        let now = now_timestamp();
        if let Some(title) = title {
            conn.execute(
                "UPDATE chat_sessions SET title = ?2, updated_at = ?3, last_run_at = ?3 WHERE id = ?1",
                params![session_id, title, now],
            )?;
        } else {
            conn.execute(
                "UPDATE chat_sessions SET updated_at = ?2, last_run_at = ?2 WHERE id = ?1",
                params![session_id, now],
            )?;
        }
        Ok(())
    }

    pub fn delete_session(&self, session_id: &str) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "DELETE FROM message_marks
             WHERE message_id IN (SELECT id FROM chat_messages WHERE session_id = ?1)",
            [session_id],
        )?;
        conn.execute(
            "DELETE FROM session_memory_state WHERE session_id = ?1",
            [session_id],
        )?;
        conn.execute(
            "DELETE FROM chat_messages WHERE session_id = ?1",
            [session_id],
        )?;
        conn.execute(
            "DELETE FROM tool_approvals WHERE session_id = ?1",
            [session_id],
        )?;
        conn.execute("DELETE FROM chat_runs WHERE session_id = ?1", [session_id])?;
        conn.execute(
            "DELETE FROM run_usage_metrics WHERE session_id = ?1",
            [session_id],
        )?;
        conn.execute(
            "DELETE FROM run_tool_metrics WHERE session_id = ?1",
            [session_id],
        )?;
        conn.execute(
            "DELETE FROM file_operations WHERE session_id = ?1",
            [session_id],
        )?;
        conn.execute("DELETE FROM chat_sessions WHERE id = ?1", [session_id])?;

        if self.get_last_opened_session_id()?.as_deref() == Some(session_id) {
            let next = self
                .list_sessions()?
                .into_iter()
                .next()
                .map(|session| session.id);
            self.set_last_opened_session_id(next.as_deref())?;
        }
        Ok(())
    }

    pub fn list_messages(&self) -> AppResult<Vec<ChatMessage>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, session_id, role, parts_json, run_id, created_at
             FROM chat_messages
             ORDER BY created_at ASC, rowid ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let parts_json = row.get::<_, String>(3)?;
            Ok(ChatMessage {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                parts_json: serde_json::from_str(&parts_json).unwrap_or(Value::Null),
                run_id: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn list_messages_for_session(&self, session_id: &str) -> AppResult<Vec<ChatMessage>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, session_id, role, parts_json, run_id, created_at
             FROM chat_messages
             WHERE session_id = ?1
             ORDER BY created_at ASC, rowid ASC",
        )?;
        let rows = stmt.query_map([session_id], |row| {
            let parts_json = row.get::<_, String>(3)?;
            Ok(ChatMessage {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                parts_json: serde_json::from_str(&parts_json).unwrap_or(Value::Null),
                run_id: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn list_active_messages_for_session(
        &self,
        session_id: &str,
    ) -> AppResult<Vec<ChatMessage>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT m.id, m.session_id, m.role, m.parts_json, m.run_id, m.created_at
             FROM chat_messages m
             LEFT JOIN message_marks mm
               ON mm.message_id = m.id AND mm.mark = 'compressed'
             WHERE m.session_id = ?1
               AND mm.message_id IS NULL
             ORDER BY m.created_at ASC, m.rowid ASC",
        )?;
        let rows = stmt.query_map([session_id], |row| {
            let parts_json = row.get::<_, String>(3)?;
            Ok(ChatMessage {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                parts_json: serde_json::from_str(&parts_json).unwrap_or(Value::Null),
                run_id: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn clear_session_messages(&self, session_id: &str) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "DELETE FROM message_marks
             WHERE message_id IN (SELECT id FROM chat_messages WHERE session_id = ?1)",
            [session_id],
        )?;
        conn.execute(
            "DELETE FROM chat_messages WHERE session_id = ?1",
            [session_id],
        )?;
        conn.execute(
            "DELETE FROM session_memory_state WHERE session_id = ?1",
            [session_id],
        )?;
        Ok(())
    }

    pub fn mark_messages(&self, message_ids: &[String], mark: &str) -> AppResult<u32> {
        if message_ids.is_empty() {
            return Ok(0);
        }
        let conn = self.open_connection()?;
        let tx = conn.unchecked_transaction()?;
        let mut changed = 0u32;
        for message_id in message_ids {
            changed += tx.execute(
                "INSERT OR IGNORE INTO message_marks (message_id, mark, created_at)
                 VALUES (?1, ?2, ?3)",
                params![message_id, mark, now_timestamp()],
            )? as u32;
        }
        tx.commit()?;
        Ok(changed)
    }

    pub fn count_marked_messages(&self, session_id: &str, mark: &str) -> AppResult<u32> {
        let conn = self.open_connection()?;
        let count = conn.query_row(
            "SELECT COUNT(*)
             FROM message_marks mm
             JOIN chat_messages m ON m.id = mm.message_id
             WHERE m.session_id = ?1 AND mm.mark = ?2",
            params![session_id, mark],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(count.max(0) as u32)
    }

    pub fn get_session_compressed_summary(&self, session_id: &str) -> AppResult<String> {
        let conn = self.open_connection()?;
        let summary = conn
            .query_row(
                "SELECT compressed_summary FROM session_memory_state WHERE session_id = ?1",
                [session_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .unwrap_or_default();
        Ok(summary)
    }

    pub fn upsert_session_compressed_summary(
        &self,
        session_id: &str,
        compressed_summary: &str,
    ) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO session_memory_state (session_id, compressed_summary, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(session_id) DO UPDATE SET
               compressed_summary = excluded.compressed_summary,
               updated_at = excluded.updated_at",
            params![session_id, compressed_summary, now_timestamp()],
        )?;
        Ok(())
    }

    pub fn list_messages_for_run(&self, run_id: &str) -> AppResult<Vec<ChatMessage>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, session_id, role, parts_json, run_id, created_at
             FROM chat_messages
             WHERE run_id = ?1
             ORDER BY created_at ASC, rowid ASC",
        )?;
        let rows = stmt.query_map([run_id], |row| {
            let parts_json = row.get::<_, String>(3)?;
            Ok(ChatMessage {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                parts_json: serde_json::from_str(&parts_json).unwrap_or(Value::Null),
                run_id: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn insert_messages(&self, messages: &[ChatMessage]) -> AppResult<()> {
        if messages.is_empty() {
            return Ok(());
        }
        let conn = self.open_connection()?;
        let tx = conn.unchecked_transaction()?;
        for message in messages {
            tx.execute(
                "INSERT INTO chat_messages (id, session_id, role, parts_json, run_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    message.id,
                    message.session_id,
                    message.role,
                    serde_json::to_string(&message.parts_json)?,
                    message.run_id,
                    message.created_at,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn insert_message(&self, message: &ChatMessage) -> AppResult<()> {
        self.insert_messages(std::slice::from_ref(message))
    }

    pub fn list_message_objects_for_session(
        &self,
        session_id: &str,
    ) -> AppResult<Vec<aquaregia::Message>> {
        self.list_messages_for_session(session_id)?
            .iter()
            .map(message_from_record)
            .collect()
    }

    pub fn list_active_message_objects_for_session(
        &self,
        session_id: &str,
    ) -> AppResult<Vec<aquaregia::Message>> {
        self.list_active_messages_for_session(session_id)?
            .iter()
            .map(message_from_record)
            .collect()
    }

    pub fn list_runs(&self) -> AppResult<Vec<ChatRun>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, session_id, status, user_message, output_text, created_at, finished_at, error_message
             FROM chat_runs
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ChatRun {
                id: row.get(0)?,
                session_id: row.get(1)?,
                status: row.get(2)?,
                user_message: row.get(3)?,
                output_text: row.get(4)?,
                created_at: row.get(5)?,
                finished_at: row.get(6)?,
                error_message: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn insert_run(&self, run: &ChatRun) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO chat_runs (id, session_id, status, user_message, output_text, created_at, finished_at, error_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                run.id,
                run.session_id,
                run.status,
                run.user_message,
                run.output_text,
                run.created_at,
                run.finished_at,
                run.error_message,
            ],
        )?;
        Ok(())
    }

    pub fn update_run(
        &self,
        run_id: &str,
        status: &str,
        output_text: Option<&str>,
        error_message: Option<&str>,
    ) -> AppResult<ChatRun> {
        let conn = self.open_connection()?;
        let finished_at = now_timestamp();
        conn.execute(
            "UPDATE chat_runs
             SET status = ?2, output_text = COALESCE(?3, output_text), error_message = ?4, finished_at = ?5
             WHERE id = ?1",
            params![run_id, status, output_text, error_message, finished_at],
        )?;
        conn.query_row(
            "SELECT id, session_id, status, user_message, output_text, created_at, finished_at, error_message
             FROM chat_runs WHERE id = ?1",
            [run_id],
            |row| {
                Ok(ChatRun {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    status: row.get(2)?,
                    user_message: row.get(3)?,
                    output_text: row.get(4)?,
                    created_at: row.get(5)?,
                    finished_at: row.get(6)?,
                    error_message: row.get(7)?,
                })
            },
        )
        .map_err(Into::into)
    }
    pub fn list_approvals(&self) -> AppResult<Vec<ToolApproval>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, session_id, run_id, call_id, action, path, preview_json, status, created_at, resolved_at
             FROM tool_approvals
             ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let preview_json = row.get::<_, String>(6)?;
            Ok(ToolApproval {
                id: row.get(0)?,
                session_id: row.get(1)?,
                run_id: row.get(2)?,
                call_id: row.get(3)?,
                action: row.get(4)?,
                path: row.get(5)?,
                preview_json: serde_json::from_str(&preview_json).unwrap_or(Value::Null),
                status: row.get(7)?,
                created_at: row.get(8)?,
                resolved_at: row.get(9)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn insert_approval(&self, approval: &ToolApproval) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO tool_approvals (id, session_id, run_id, call_id, action, path, preview_json, status, created_at, resolved_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                approval.id,
                approval.session_id,
                approval.run_id,
                approval.call_id,
                approval.action,
                approval.path,
                serde_json::to_string(&approval.preview_json)?,
                approval.status,
                approval.created_at,
                approval.resolved_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_approval_status(
        &self,
        approval_id: &str,
        status: &str,
    ) -> AppResult<ToolApproval> {
        let conn = self.open_connection()?;
        let resolved_at = if status == "pending" {
            None
        } else {
            Some(now_timestamp())
        };
        conn.execute(
            "UPDATE tool_approvals SET status = ?2, resolved_at = ?3 WHERE id = ?1",
            params![approval_id, status, resolved_at],
        )?;
        conn.query_row(
            "SELECT id, session_id, run_id, call_id, action, path, preview_json, status, created_at, resolved_at
             FROM tool_approvals WHERE id = ?1",
            [approval_id],
            |row| {
                let preview_json = row.get::<_, String>(6)?;
                Ok(ToolApproval {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    run_id: row.get(2)?,
                    call_id: row.get(3)?,
                    action: row.get(4)?,
                    path: row.get(5)?,
                    preview_json: serde_json::from_str(&preview_json).unwrap_or(Value::Null),
                    status: row.get(7)?,
                    created_at: row.get(8)?,
                    resolved_at: row.get(9)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("approval `{approval_id}`")))
    }

    pub fn record_file_operation(
        &self,
        session_id: &str,
        run_id: &str,
        call_id: Option<&str>,
        action: &str,
        path: &str,
        status: &str,
        bytes_written: Option<usize>,
    ) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO file_operations (id, session_id, run_id, call_id, action, path, status, bytes_written, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                uuid::Uuid::new_v4().to_string(),
                session_id,
                run_id,
                call_id,
                action,
                path,
                status,
                bytes_written.map(|value| value as i64),
                now_timestamp(),
            ],
        )?;
        Ok(())
    }

    pub fn set_last_opened_session_id(&self, session_id: Option<&str>) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('last_opened_session_id', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [session_id],
        )?;
        Ok(())
    }

    pub fn get_last_opened_session_id(&self) -> AppResult<Option<String>> {
        let conn = self.open_connection()?;
        let value = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'last_opened_session_id'",
                [],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?;
        Ok(value.flatten())
    }

    pub fn sessions_payload(&self) -> AppResult<SessionsChangedPayload> {
        Ok(SessionsChangedPayload {
            sessions: self.list_sessions()?,
            last_opened_session_id: self.get_last_opened_session_id()?,
        })
    }
}
