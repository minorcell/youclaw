use super::*;
use crate::backend::models::domain::SessionApprovalMode;
use aquaregia::AgentStep;

impl StorageService {
    pub fn find_latest_empty_session(&self) -> AppResult<Option<ChatSession>> {
        let conn = self.open_connection()?;
        conn.query_row(
            "SELECT s.id, s.title, s.provider_profile_id, s.workspace_path, s.approval_mode, s.created_at, s.updated_at, s.last_turn_at, s.archived_at
             FROM chat_sessions s
             WHERE s.archived_at IS NULL
               AND NOT EXISTS (SELECT 1 FROM chat_messages m WHERE m.session_id = s.id)
               AND NOT EXISTS (SELECT 1 FROM chat_turns t WHERE t.session_id = s.id)
             ORDER BY COALESCE(s.last_turn_at, s.updated_at) DESC, s.created_at DESC
             LIMIT 1",
            [],
            read_chat_session,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn list_sessions(&self) -> AppResult<Vec<ChatSession>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, provider_profile_id, workspace_path, approval_mode, created_at, updated_at, last_turn_at, archived_at
             FROM chat_sessions
             WHERE archived_at IS NULL
             ORDER BY COALESCE(last_turn_at, updated_at) DESC, created_at DESC",
        )?;
        let rows = stmt.query_map([], read_chat_session)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn list_archived_sessions(&self) -> AppResult<Vec<ChatSession>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, provider_profile_id, workspace_path, approval_mode, created_at, updated_at, last_turn_at, archived_at
             FROM chat_sessions
             WHERE archived_at IS NOT NULL
             ORDER BY archived_at DESC, COALESCE(last_turn_at, updated_at) DESC, created_at DESC",
        )?;
        let rows = stmt.query_map([], read_chat_session)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_session(&self, session_id: &str) -> AppResult<ChatSession> {
        let conn = self.open_connection()?;
        conn.query_row(
            "SELECT id, title, provider_profile_id, workspace_path, approval_mode, created_at, updated_at, last_turn_at, archived_at
             FROM chat_sessions
             WHERE id = ?1 AND archived_at IS NULL",
            [session_id],
            read_chat_session,
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("session `{session_id}`")))
    }

    pub fn insert_session(&self, session: &ChatSession) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO chat_sessions (
                id, title, provider_profile_id, workspace_path, approval_mode, created_at, updated_at, last_turn_at, archived_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                session.id,
                session.title,
                session.provider_profile_id,
                session.workspace_path,
                session.approval_mode.as_str(),
                session.created_at,
                session.updated_at,
                session.last_turn_at,
                session.archived_at,
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
        let updated = conn.execute(
            "UPDATE chat_sessions
             SET provider_profile_id = ?2, updated_at = ?3
             WHERE id = ?1 AND archived_at IS NULL",
            params![session_id, provider_profile_id, now_timestamp()],
        )?;
        if updated == 0 {
            return Err(AppError::NotFound(format!("session `{session_id}`")));
        }
        Ok(())
    }

    pub fn update_session_workspace(
        &self,
        session_id: &str,
        workspace_path: &str,
    ) -> AppResult<()> {
        let conn = self.open_connection()?;
        let now = now_timestamp();
        let updated = conn.execute(
            "UPDATE chat_sessions
             SET workspace_path = ?2, updated_at = ?3
             WHERE id = ?1 AND archived_at IS NULL",
            params![session_id, workspace_path, now],
        )?;
        if updated == 0 {
            return Err(AppError::NotFound(format!("session `{session_id}`")));
        }
        conn.execute(
            "INSERT INTO workspace_roots (path, last_used_at) VALUES (?1, ?2)
             ON CONFLICT(path) DO UPDATE SET last_used_at = excluded.last_used_at",
            params![workspace_path, now],
        )?;
        Ok(())
    }

    pub fn update_session_approval_mode(
        &self,
        session_id: &str,
        approval_mode: SessionApprovalMode,
    ) -> AppResult<()> {
        let conn = self.open_connection()?;
        let updated = conn.execute(
            "UPDATE chat_sessions
             SET approval_mode = ?2, updated_at = ?3
             WHERE id = ?1 AND archived_at IS NULL",
            params![session_id, approval_mode.as_str(), now_timestamp()],
        )?;
        if updated == 0 {
            return Err(AppError::NotFound(format!("session `{session_id}`")));
        }
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

    pub fn update_session_title(&self, session_id: &str, title: &str) -> AppResult<()> {
        let conn = self.open_connection()?;
        let updated = conn.execute(
            "UPDATE chat_sessions
             SET title = ?2, updated_at = ?3
             WHERE id = ?1 AND archived_at IS NULL",
            params![session_id, title, now_timestamp()],
        )?;
        if updated == 0 {
            return Err(AppError::NotFound(format!("session `{session_id}`")));
        }
        Ok(())
    }

    pub fn list_recent_workspaces(&self, limit: u32) -> AppResult<Vec<WorkspaceRootInfo>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT path, last_used_at
             FROM workspace_roots
             ORDER BY last_used_at DESC, path ASC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit.clamp(1, 64) as i64], |row| {
            Ok(WorkspaceRootInfo {
                path: row.get(0)?,
                last_used_at: row.get(1)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn touch_session_for_turn(&self, session_id: &str, title: Option<&str>) -> AppResult<()> {
        let conn = self.open_connection()?;
        let now = now_timestamp();
        let updated = if let Some(title) = title {
            conn.execute(
                "UPDATE chat_sessions
                 SET title = ?2, updated_at = ?3, last_turn_at = ?3
                 WHERE id = ?1 AND archived_at IS NULL",
                params![session_id, title, now],
            )?
        } else {
            conn.execute(
                "UPDATE chat_sessions
                 SET updated_at = ?2, last_turn_at = ?2
                 WHERE id = ?1 AND archived_at IS NULL",
                params![session_id, now],
            )?
        };
        if updated == 0 {
            return Err(AppError::NotFound(format!("session `{session_id}`")));
        }
        Ok(())
    }

    pub fn delete_session(&self, session_id: &str) -> AppResult<()> {
        let conn = self.open_connection()?;
        let now = now_timestamp();
        let updated = conn.execute(
            "UPDATE chat_sessions
             SET archived_at = COALESCE(archived_at, ?2), updated_at = ?2
             WHERE id = ?1",
            params![session_id, now],
        )?;
        if updated == 0 {
            return Err(AppError::NotFound(format!("session `{session_id}`")));
        }

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

    pub fn restore_session(&self, session_id: &str) -> AppResult<()> {
        let conn = self.open_connection()?;
        let archived_at = conn
            .query_row(
                "SELECT archived_at FROM chat_sessions WHERE id = ?1",
                [session_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?;
        match archived_at {
            None => {
                return Err(AppError::NotFound(format!(
                    "archived session `{session_id}`"
                )));
            }
            Some(None) => {
                return Err(AppError::Validation("session is not archived".to_string()));
            }
            Some(Some(_)) => {}
        }

        conn.execute(
            "UPDATE chat_sessions
             SET archived_at = NULL, updated_at = ?2
             WHERE id = ?1",
            params![session_id, now_timestamp()],
        )?;
        Ok(())
    }

    pub fn purge_session(&self, session_id: &str) -> AppResult<()> {
        let conn = self.open_connection()?;
        let archived_at = conn
            .query_row(
                "SELECT archived_at FROM chat_sessions WHERE id = ?1",
                [session_id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?;
        match archived_at {
            None => {
                return Err(AppError::NotFound(format!(
                    "archived session `{session_id}`"
                )));
            }
            Some(None) => {
                return Err(AppError::Validation(
                    "session must be archived before purge".to_string(),
                ));
            }
            Some(Some(_)) => {}
        }

        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE settings
             SET value = NULL
             WHERE key = 'last_opened_session_id' AND value = ?1",
            [session_id],
        )?;
        tx.execute(
            "DELETE FROM message_marks
             WHERE message_id IN (
               SELECT id FROM chat_messages WHERE session_id = ?1
             )",
            [session_id],
        )?;
        tx.execute(
            "DELETE FROM turn_tool_metrics WHERE session_id = ?1",
            [session_id],
        )?;
        tx.execute(
            "DELETE FROM turn_usage_metrics WHERE session_id = ?1",
            [session_id],
        )?;
        tx.execute("DELETE FROM chat_steps WHERE session_id = ?1", [session_id])?;
        tx.execute(
            "DELETE FROM tool_approvals WHERE session_id = ?1",
            [session_id],
        )?;
        tx.execute(
            "DELETE FROM file_operations WHERE session_id = ?1",
            [session_id],
        )?;
        tx.execute(
            "DELETE FROM session_memory_state WHERE session_id = ?1",
            [session_id],
        )?;
        tx.execute(
            "DELETE FROM chat_messages WHERE session_id = ?1",
            [session_id],
        )?;
        tx.execute("DELETE FROM chat_turns WHERE session_id = ?1", [session_id])?;
        tx.execute("DELETE FROM chat_sessions WHERE id = ?1", [session_id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn list_messages(&self) -> AppResult<Vec<ChatMessage>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT m.id, m.session_id, m.role, m.parts_json, m.turn_id, m.created_at
             FROM chat_messages m
             INNER JOIN chat_sessions s ON s.id = m.session_id
             WHERE s.archived_at IS NULL
             ORDER BY m.created_at ASC, m.rowid ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let parts_json = row.get::<_, String>(3)?;
            let role_raw = row.get::<_, String>(2)?;
            Ok(ChatMessage {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: parse_message_role_column(&role_raw, 2)?,
                parts_json: serde_json::from_str(&parts_json).unwrap_or(Value::Null),
                turn_id: row.get(4)?,
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
            "SELECT m.id, m.session_id, m.role, m.parts_json, m.turn_id, m.created_at
             FROM chat_messages m
             INNER JOIN chat_sessions s ON s.id = m.session_id
             LEFT JOIN message_marks mm
               ON mm.message_id = m.id AND mm.mark = 'compressed'
             WHERE m.session_id = ?1
               AND s.archived_at IS NULL
               AND mm.message_id IS NULL
             ORDER BY m.created_at ASC, m.rowid ASC",
        )?;
        let rows = stmt.query_map([session_id], |row| {
            let parts_json = row.get::<_, String>(3)?;
            let role_raw = row.get::<_, String>(2)?;
            Ok(ChatMessage {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: parse_message_role_column(&role_raw, 2)?,
                parts_json: serde_json::from_str(&parts_json).unwrap_or(Value::Null),
                turn_id: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
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

    pub fn get_session_context_summary(
        &self,
        session_id: &str,
    ) -> AppResult<SessionContextSummary> {
        let conn = self.open_connection()?;
        let summary_json = conn
            .query_row(
                "SELECT summary_json FROM session_memory_state WHERE session_id = ?1",
                [session_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .unwrap_or_else(|| "{}".to_string());
        Ok(serde_json::from_str::<SessionContextSummary>(&summary_json)?.normalize())
    }

    pub fn upsert_session_context_summary(
        &self,
        session_id: &str,
        summary: &SessionContextSummary,
    ) -> AppResult<()> {
        let conn = self.open_connection()?;
        let summary_json = serde_json::to_string(summary)?;
        conn.execute(
            "INSERT INTO session_memory_state (session_id, summary_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(session_id) DO UPDATE SET
               summary_json = excluded.summary_json,
               updated_at = excluded.updated_at",
            params![session_id, summary_json, now_timestamp()],
        )?;
        Ok(())
    }

    pub fn list_messages_for_turn(&self, turn_id: &str) -> AppResult<Vec<ChatMessage>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT m.id, m.session_id, m.role, m.parts_json, m.turn_id, m.created_at
             FROM chat_messages m
             INNER JOIN chat_sessions s ON s.id = m.session_id
             WHERE m.turn_id = ?1
               AND s.archived_at IS NULL
             ORDER BY m.created_at ASC, m.rowid ASC",
        )?;
        let rows = stmt.query_map([turn_id], |row| {
            let parts_json = row.get::<_, String>(3)?;
            let role_raw = row.get::<_, String>(2)?;
            Ok(ChatMessage {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: parse_message_role_column(&role_raw, 2)?,
                parts_json: serde_json::from_str(&parts_json).unwrap_or(Value::Null),
                turn_id: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn list_turn_steps(&self, turn_id: &str) -> AppResult<Vec<AgentStep>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT
                cs.step,
                cs.output_text,
                cs.reasoning_text,
                cs.reasoning_parts_json,
                cs.finish_reason_json,
                cs.usage_json,
                cs.tool_calls_json,
                cs.tool_results_json
             FROM chat_steps cs
             INNER JOIN chat_turns t ON t.id = cs.turn_id
             INNER JOIN chat_sessions s ON s.id = t.session_id
             WHERE cs.turn_id = ?1
               AND s.archived_at IS NULL
             ORDER BY cs.step ASC",
        )?;
        let rows = stmt.query_map([turn_id], |row| {
            let step_num = row.get::<_, i64>(0)? as u8;
            let output_text = row.get::<_, String>(1)?;
            let reasoning_text = row.get::<_, String>(2)?;
            let reasoning_parts_json = row.get::<_, String>(3)?;
            let finish_reason_json = row.get::<_, String>(4)?;
            let usage_json = row.get::<_, String>(5)?;
            let tool_calls_json = row.get::<_, String>(6)?;
            let tool_results_json = row.get::<_, String>(7)?;
            let reasoning_parts = serde_json::from_str(&reasoning_parts_json).unwrap_or_default();
            let finish_reason = serde_json::from_str(&finish_reason_json)
                .unwrap_or(aquaregia::FinishReason::Unknown("unknown".to_string()));
            let usage = serde_json::from_str(&usage_json).unwrap_or_default();
            let tool_calls = serde_json::from_str(&tool_calls_json).unwrap_or_default();
            let tool_results = serde_json::from_str(&tool_results_json).unwrap_or_default();
            Ok(AgentStep {
                step: step_num,
                output_text,
                reasoning_text,
                reasoning_parts,
                finish_reason,
                usage,
                tool_calls,
                tool_results,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn insert_turn_step(
        &self,
        turn_id: &str,
        session_id: &str,
        step: &AgentStep,
    ) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO chat_steps (
                turn_id, session_id, step, output_text, reasoning_text,
                reasoning_parts_json, finish_reason_json, usage_json,
                tool_calls_json, tool_results_json, created_at
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(turn_id, step) DO UPDATE SET
                session_id = excluded.session_id,
                output_text = excluded.output_text,
                reasoning_text = excluded.reasoning_text,
                reasoning_parts_json = excluded.reasoning_parts_json,
                finish_reason_json = excluded.finish_reason_json,
                usage_json = excluded.usage_json,
                tool_calls_json = excluded.tool_calls_json,
                tool_results_json = excluded.tool_results_json,
                created_at = excluded.created_at",
            params![
                turn_id,
                session_id,
                step.step as i64,
                step.output_text,
                step.reasoning_text,
                serde_json::to_string(&step.reasoning_parts)?,
                serde_json::to_string(&step.finish_reason)?,
                serde_json::to_string(&step.usage)?,
                serde_json::to_string(&step.tool_calls)?,
                serde_json::to_string(&step.tool_results)?,
                now_timestamp(),
            ],
        )?;
        Ok(())
    }

    pub fn insert_messages(&self, messages: &[ChatMessage]) -> AppResult<()> {
        if messages.is_empty() {
            return Ok(());
        }
        let conn = self.open_connection()?;
        let tx = conn.unchecked_transaction()?;
        for message in messages {
            tx.execute(
                "INSERT INTO chat_messages (id, session_id, role, parts_json, turn_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    message.id,
                    message.session_id,
                    message.role.as_str(),
                    serde_json::to_string(&message.parts_json)?,
                    message.turn_id,
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

    pub fn list_active_message_objects_for_session(
        &self,
        session_id: &str,
    ) -> AppResult<Vec<aquaregia::Message>> {
        self.list_active_messages_for_session(session_id)?
            .iter()
            .map(message_from_record)
            .collect()
    }

    pub fn list_turns(&self) -> AppResult<Vec<ChatTurn>> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT
                t.id, t.session_id, t.status, t.user_message, t.output_text,
                t.created_at, t.finished_at, t.error_message
             FROM chat_turns t
             INNER JOIN chat_sessions s ON s.id = t.session_id
             WHERE s.archived_at IS NULL
             ORDER BY t.created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let status_raw = row.get::<_, String>(2)?;
            Ok(ChatTurn {
                id: row.get(0)?,
                session_id: row.get(1)?,
                status: parse_turn_status_column(&status_raw, 2)?,
                user_message: row.get(3)?,
                output_text: row.get(4)?,
                created_at: row.get(5)?,
                finished_at: row.get(6)?,
                error_message: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn insert_turn(&self, turn: &ChatTurn) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO chat_turns (id, session_id, status, user_message, output_text, created_at, finished_at, error_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                turn.id,
                turn.session_id,
                turn.status.as_str(),
                turn.user_message,
                turn.output_text,
                turn.created_at,
                turn.finished_at,
                turn.error_message,
            ],
        )?;
        Ok(())
    }

    pub fn update_turn(
        &self,
        turn_id: &str,
        status: TurnStatus,
        output_text: Option<&str>,
        error_message: Option<&str>,
    ) -> AppResult<ChatTurn> {
        let conn = self.open_connection()?;
        let finished_at = now_timestamp();
        conn.execute(
            "UPDATE chat_turns
             SET status = ?2, output_text = COALESCE(?3, output_text), error_message = ?4, finished_at = ?5
             WHERE id = ?1",
            params![turn_id, status.as_str(), output_text, error_message, finished_at],
        )?;
        conn.query_row(
            "SELECT id, session_id, status, user_message, output_text, created_at, finished_at, error_message
             FROM chat_turns WHERE id = ?1",
            [turn_id],
            |row| {
                let status_raw = row.get::<_, String>(2)?;
                Ok(ChatTurn {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    status: parse_turn_status_column(&status_raw, 2)?,
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
            "SELECT
                a.id, a.session_id, a.turn_id, a.call_id, a.action,
                a.path, a.preview_json, a.status, a.created_at, a.resolved_at
             FROM tool_approvals a
             INNER JOIN chat_sessions s ON s.id = a.session_id
             WHERE s.archived_at IS NULL
             ORDER BY a.created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let preview_json = row.get::<_, String>(6)?;
            Ok(ToolApproval {
                id: row.get(0)?,
                session_id: row.get(1)?,
                turn_id: row.get(2)?,
                call_id: row.get(3)?,
                action: row.get(4)?,
                subject: row.get(5)?,
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
            "INSERT INTO tool_approvals (id, session_id, turn_id, call_id, action, path, preview_json, status, created_at, resolved_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                approval.id,
                approval.session_id,
                approval.turn_id,
                approval.call_id,
                approval.action,
                approval.subject,
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
            "SELECT id, session_id, turn_id, call_id, action, path, preview_json, status, created_at, resolved_at
             FROM tool_approvals WHERE id = ?1",
            [approval_id],
            |row| {
                let preview_json = row.get::<_, String>(6)?;
                Ok(ToolApproval {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    turn_id: row.get(2)?,
                    call_id: row.get(3)?,
                    action: row.get(4)?,
                    subject: row.get(5)?,
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
        turn_id: &str,
        call_id: Option<&str>,
        action: &str,
        path: &str,
        status: &str,
        bytes_written: Option<usize>,
    ) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO file_operations (id, session_id, turn_id, call_id, action, path, status, bytes_written, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                uuid::Uuid::new_v4().to_string(),
                session_id,
                turn_id,
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
        let Some(session_id) = value.flatten() else {
            return Ok(None);
        };
        let exists = conn
            .query_row(
                "SELECT 1 FROM chat_sessions WHERE id = ?1 AND archived_at IS NULL",
                [session_id.as_str()],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        if exists {
            Ok(Some(session_id))
        } else {
            conn.execute(
                "INSERT INTO settings (key, value) VALUES ('last_opened_session_id', NULL)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                [],
            )?;
            Ok(None)
        }
    }

    pub fn sessions_payload(&self) -> AppResult<SessionsChangedPayload> {
        Ok(SessionsChangedPayload {
            sessions: self.list_sessions()?,
            last_opened_session_id: self.get_last_opened_session_id()?,
            recent_workspaces: self.list_recent_workspaces(12)?,
        })
    }

    pub fn archived_sessions_payload(&self) -> AppResult<ArchivedSessionsPayload> {
        Ok(ArchivedSessionsPayload {
            sessions: self.list_archived_sessions()?,
        })
    }
}

fn read_chat_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<ChatSession> {
    let approval_mode_raw = row.get::<_, String>(4)?;
    Ok(ChatSession {
        id: row.get(0)?,
        title: row.get(1)?,
        provider_profile_id: row.get(2)?,
        workspace_path: row.get(3)?,
        approval_mode: parse_session_approval_mode_column(&approval_mode_raw, 4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
        last_turn_at: row.get(7)?,
        archived_at: row.get(8)?,
    })
}

fn parse_message_role_column(value: &str, column: usize) -> rusqlite::Result<MessageRole> {
    value.parse::<MessageRole>().map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid message role `{value}`"),
            )),
        )
    })
}

fn parse_turn_status_column(value: &str, column: usize) -> rusqlite::Result<TurnStatus> {
    value.parse::<TurnStatus>().map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid turn status `{value}`"),
            )),
        )
    })
}

fn parse_session_approval_mode_column(
    value: &str,
    column: usize,
) -> rusqlite::Result<SessionApprovalMode> {
    value.parse::<SessionApprovalMode>().map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid session approval mode `{value}`"),
            )),
        )
    })
}
