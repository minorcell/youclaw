use super::*;

impl StorageService {
    pub fn get_usage_detail_logging_enabled(&self) -> AppResult<bool> {
        let conn = self.open_connection()?;
        let value = conn
            .query_row(
                "SELECT value FROM settings WHERE key = 'usage_detail_logging_enabled'",
                [],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        Ok(match value.as_deref() {
            Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("on") => true,
            Some("0") | Some("false") | Some("FALSE") | Some("no") | Some("off") => false,
            Some(_) => DEFAULT_USAGE_DETAIL_LOGGING_ENABLED,
            None => DEFAULT_USAGE_DETAIL_LOGGING_ENABLED,
        })
    }

    pub fn set_usage_detail_logging_enabled(
        &self,
        detail_logging_enabled: bool,
    ) -> AppResult<UsageSettingsPayload> {
        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('usage_detail_logging_enabled', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [if detail_logging_enabled { "1" } else { "0" }],
        )?;
        Ok(UsageSettingsPayload {
            detail_logging_enabled,
        })
    }

    pub fn usage_settings_payload(&self) -> AppResult<UsageSettingsPayload> {
        Ok(UsageSettingsPayload {
            detail_logging_enabled: self.get_usage_detail_logging_enabled()?,
        })
    }

    pub fn insert_turn_usage_metric_start(
        &self,
        turn: &ChatTurn,
        provider: Option<&ProviderProfile>,
        detail_logged: bool,
    ) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO turn_usage_metrics (
                turn_id, session_id, provider_profile_id, provider_id, provider_name,
                model_id, model_name, model, status, user_message, started_at, step_count, detail_logged
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0, ?12)
             ON CONFLICT(turn_id) DO UPDATE SET
                session_id = excluded.session_id,
                provider_profile_id = excluded.provider_profile_id,
                provider_id = excluded.provider_id,
                provider_name = excluded.provider_name,
                model_id = excluded.model_id,
                model_name = excluded.model_name,
                model = excluded.model,
                status = excluded.status,
                user_message = excluded.user_message,
                started_at = excluded.started_at,
                detail_logged = excluded.detail_logged",
            params![
                turn.id,
                turn.session_id,
                provider.map(|item| item.id.clone()),
                provider.map(|item| item.provider_id.clone()),
                provider.map(|item| item.name.clone()),
                provider.map(|item| item.model.clone()),
                provider.map(|item| item.model_name.clone()),
                provider.map(|item| item.model.clone()),
                turn.status.as_str(),
                turn.user_message,
                turn.created_at,
                if detail_logged { 1i64 } else { 0i64 },
            ],
        )?;
        Ok(())
    }

    pub fn update_turn_usage_metric(
        &self,
        turn: &ChatTurn,
        usage: Option<&aquaregia::Usage>,
        step_count: Option<u32>,
    ) -> AppResult<()> {
        let conn = self.open_connection()?;
        let started_at = conn
            .query_row(
                "SELECT started_at FROM turn_usage_metrics WHERE turn_id = ?1",
                [turn.id.as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        let duration_ms = calculate_duration_ms(started_at.as_deref(), turn.finished_at.as_deref());
        let (
            input_tokens,
            input_no_cache_tokens,
            input_cache_read_tokens,
            input_cache_write_tokens,
            output_tokens,
            output_text_tokens,
            reasoning_tokens,
            total_tokens,
        ) = if let Some(usage) = usage {
            (
                Some(usage.input_tokens as i64),
                Some(usage.input_no_cache_tokens as i64),
                Some(usage.input_cache_read_tokens as i64),
                Some(usage.input_cache_write_tokens as i64),
                Some(usage.output_tokens as i64),
                Some(usage.output_text_tokens as i64),
                Some(usage.reasoning_tokens as i64),
                Some(usage.total_tokens as i64),
            )
        } else {
            (None, None, None, None, None, None, None, None)
        };
        let changed = conn.execute(
            "UPDATE turn_usage_metrics
             SET status = ?2,
                 finished_at = ?3,
                 duration_ms = ?4,
                 input_tokens = COALESCE(?5, input_tokens),
                 input_no_cache_tokens = COALESCE(?6, input_no_cache_tokens),
                 input_cache_read_tokens = COALESCE(?7, input_cache_read_tokens),
                 input_cache_write_tokens = COALESCE(?8, input_cache_write_tokens),
                 output_tokens = COALESCE(?9, output_tokens),
                 output_text_tokens = COALESCE(?10, output_text_tokens),
                 reasoning_tokens = COALESCE(?11, reasoning_tokens),
                 total_tokens = COALESCE(?12, total_tokens),
                 step_count = COALESCE(?13, step_count)
             WHERE turn_id = ?1",
            params![
                turn.id,
                turn.status.as_str(),
                turn.finished_at,
                duration_ms,
                input_tokens,
                input_no_cache_tokens,
                input_cache_read_tokens,
                input_cache_write_tokens,
                output_tokens,
                output_text_tokens,
                reasoning_tokens,
                total_tokens,
                step_count.map(|value| value as i64),
            ],
        )?;

        if changed == 0 {
            let detail_logged = self.get_usage_detail_logging_enabled()?;
            let usage_value = usage.cloned().unwrap_or_default();
            conn.execute(
                "INSERT INTO turn_usage_metrics (
                    turn_id, session_id, status, user_message, started_at,
                    finished_at, duration_ms, step_count, detail_logged,
                    input_tokens, input_no_cache_tokens, input_cache_read_tokens, input_cache_write_tokens,
                    output_tokens, output_text_tokens, reasoning_tokens, total_tokens
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
                params![
                    turn.id,
                    turn.session_id,
                    turn.status.as_str(),
                    turn.user_message,
                    turn.created_at,
                    turn.finished_at,
                    duration_ms,
                    step_count.unwrap_or(0) as i64,
                    if detail_logged { 1i64 } else { 0i64 },
                    usage_value.input_tokens as i64,
                    usage_value.input_no_cache_tokens as i64,
                    usage_value.input_cache_read_tokens as i64,
                    usage_value.input_cache_write_tokens as i64,
                    usage_value.output_tokens as i64,
                    usage_value.output_text_tokens as i64,
                    usage_value.reasoning_tokens as i64,
                    usage_value.total_tokens as i64,
                ],
            )?;
        }
        Ok(())
    }

    pub fn record_turn_tool_metric(
        &self,
        turn_id: &str,
        session_id: &str,
        tool_name: &str,
        tool_action: Option<&str>,
        status: &str,
        duration_ms: Option<u64>,
        is_error: bool,
    ) -> AppResult<()> {
        if !self.is_turn_detail_logging_enabled(turn_id)? {
            return Ok(());
        }

        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO turn_tool_metrics (id, turn_id, session_id, tool_name, tool_action, status, duration_ms, is_error, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                uuid::Uuid::new_v4().to_string(),
                turn_id,
                session_id,
                tool_name,
                tool_action,
                status,
                duration_ms.map(|value| value as i64),
                if is_error { 1i64 } else { 0i64 },
                now_timestamp(),
            ],
        )?;
        Ok(())
    }

    pub fn usage_summary(&self, req: UsageSummaryRequest) -> AppResult<UsageSummaryPayload> {
        let conn = self.open_connection()?;
        let (where_clause, params) = build_turn_usage_where_clause(&req.range, None, None, None)?;
        let sql = format!(
            "SELECT
                COUNT(*) AS total_turns,
                COALESCE(SUM(step_count), 0),
                COALESCE(AVG(step_count), 0),
                COALESCE(SUM(input_tokens), 0),
                COALESCE(SUM(output_tokens), 0),
                COALESCE(SUM(reasoning_tokens), 0),
                COALESCE(SUM(total_tokens), 0),
                COALESCE(SUM(input_cache_read_tokens), 0),
                COALESCE(SUM(input_cache_write_tokens), 0)
             FROM turn_usage_metrics{where_clause}"
        );
        let mut stmt = conn.prepare(&sql)?;
        stmt.query_row(params_from_iter(params), |row| {
            Ok(UsageSummaryPayload {
                range: req.range.clone(),
                total_turns: row.get::<_, i64>(0)? as u64,
                total_steps: row.get::<_, i64>(1)? as u64,
                avg_steps_per_turn: row.get::<_, f64>(2)?,
                input_tokens: row.get::<_, i64>(3)? as u64,
                output_tokens: row.get::<_, i64>(4)? as u64,
                reasoning_tokens: row.get::<_, i64>(5)? as u64,
                total_tokens: row.get::<_, i64>(6)? as u64,
                input_cache_read_tokens: row.get::<_, i64>(7)? as u64,
                input_cache_write_tokens: row.get::<_, i64>(8)? as u64,
            })
        })
        .map_err(Into::into)
    }

    pub fn list_usage_logs(&self, req: UsageLogsListRequest) -> AppResult<UsageLogsPayload> {
        let conn = self.open_connection()?;
        let (page, page_size, offset) = normalize_usage_pagination(req.page, req.page_size);
        let (where_clause, base_params) = build_turn_usage_where_clause(
            &req.range,
            req.provider_profile_id.as_deref(),
            req.status.as_deref(),
            req.detail_logged,
        )?;

        let count_sql = format!("SELECT COUNT(*) FROM turn_usage_metrics{where_clause}");
        let total = conn.query_row(&count_sql, params_from_iter(base_params.clone()), |row| {
            row.get::<_, i64>(0)
        })? as u64;

        let mut list_params = base_params;
        list_params.push(SqlValue::Integer(page_size as i64));
        list_params.push(SqlValue::Integer(offset as i64));
        let list_sql = format!(
            "SELECT
                turn_id, session_id, status, user_message,
                provider_id, provider_name, model_id, model_name, model,
                started_at, finished_at, duration_ms, step_count, detail_logged,
                input_tokens, output_tokens, reasoning_tokens, total_tokens,
                input_cache_read_tokens, input_cache_write_tokens
             FROM turn_usage_metrics{where_clause}
             ORDER BY started_at DESC, turn_id DESC
             LIMIT ? OFFSET ?"
        );
        let mut stmt = conn.prepare(&list_sql)?;
        let rows = stmt.query_map(params_from_iter(list_params), |row| {
            let detail_logged = row.get::<_, i64>(13)? != 0;
            let duration_ms = row.get::<_, Option<i64>>(11)?.and_then(|value| {
                if value >= 0 {
                    Some(value as u64)
                } else {
                    None
                }
            });
            Ok(UsageLogItem {
                turn_id: row.get(0)?,
                session_id: row.get(1)?,
                status: row.get(2)?,
                user_message: row.get(3)?,
                provider_id: row.get(4)?,
                provider_name: row.get(5)?,
                model_id: row.get(6)?,
                model_name: row.get(7)?,
                model: row.get(8)?,
                started_at: row.get(9)?,
                finished_at: row.get(10)?,
                duration_ms,
                step_count: row.get::<_, i64>(12)? as u32,
                detail_logged,
                input_tokens: row.get::<_, i64>(14)? as u64,
                output_tokens: row.get::<_, i64>(15)? as u64,
                reasoning_tokens: row.get::<_, i64>(16)? as u64,
                total_tokens: row.get::<_, i64>(17)? as u64,
                input_cache_read_tokens: row.get::<_, i64>(18)? as u64,
                input_cache_write_tokens: row.get::<_, i64>(19)? as u64,
            })
        })?;
        let items = rows.collect::<Result<Vec<_>, _>>()?;

        let has_more = (offset as u64 + page_size as u64) < total;
        Ok(UsageLogsPayload {
            page: UsagePage {
                page,
                page_size,
                total,
                has_more,
            },
            items,
        })
    }

    pub fn list_usage_provider_stats(
        &self,
        req: UsageStatsListRequest,
    ) -> AppResult<UsageProviderStatsPayload> {
        let conn = self.open_connection()?;
        let (page, page_size, offset) = normalize_usage_pagination(req.page, req.page_size);
        let (where_clause, base_params) =
            build_turn_usage_where_clause(&req.range, None, None, None)?;

        let count_sql = format!(
            "SELECT COUNT(*) FROM (
                SELECT 1 FROM turn_usage_metrics{where_clause}
                GROUP BY provider_id, provider_name
            )"
        );
        let total = conn.query_row(&count_sql, params_from_iter(base_params.clone()), |row| {
            row.get::<_, i64>(0)
        })? as u64;

        let mut list_params = base_params;
        list_params.push(SqlValue::Integer(page_size as i64));
        list_params.push(SqlValue::Integer(offset as i64));
        let list_sql = format!(
            "SELECT
                provider_id, provider_name,
                COUNT(*) AS turn_count,
                SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END) AS completed_count,
                SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) AS failed_count,
                SUM(CASE WHEN status = 'cancelled' THEN 1 ELSE 0 END) AS cancelled_count,
                COALESCE(SUM(input_tokens), 0) AS input_tokens,
                COALESCE(SUM(output_tokens), 0) AS output_tokens,
                COALESCE(SUM(total_tokens), 0) AS total_tokens,
                COALESCE(SUM(input_cache_read_tokens), 0) AS input_cache_read_tokens,
                COALESCE(SUM(input_cache_write_tokens), 0) AS input_cache_write_tokens
             FROM turn_usage_metrics{where_clause}
             GROUP BY provider_id, provider_name
             ORDER BY turn_count DESC, total_tokens DESC
             LIMIT ? OFFSET ?"
        );
        let mut stmt = conn.prepare(&list_sql)?;
        let rows = stmt.query_map(params_from_iter(list_params), |row| {
            Ok(UsageProviderStatsItem {
                provider_id: row.get(0)?,
                provider_name: row.get(1)?,
                turn_count: row.get::<_, i64>(2)? as u64,
                completed_count: row.get::<_, i64>(3)? as u64,
                failed_count: row.get::<_, i64>(4)? as u64,
                cancelled_count: row.get::<_, i64>(5)? as u64,
                input_tokens: row.get::<_, i64>(6)? as u64,
                output_tokens: row.get::<_, i64>(7)? as u64,
                total_tokens: row.get::<_, i64>(8)? as u64,
                input_cache_read_tokens: row.get::<_, i64>(9)? as u64,
                input_cache_write_tokens: row.get::<_, i64>(10)? as u64,
            })
        })?;
        let items = rows.collect::<Result<Vec<_>, _>>()?;
        let has_more = (offset as u64 + page_size as u64) < total;
        Ok(UsageProviderStatsPayload {
            page: UsagePage {
                page,
                page_size,
                total,
                has_more,
            },
            items,
        })
    }

    pub fn list_usage_model_stats(
        &self,
        req: UsageStatsListRequest,
    ) -> AppResult<UsageModelStatsPayload> {
        let conn = self.open_connection()?;
        let (page, page_size, offset) = normalize_usage_pagination(req.page, req.page_size);
        let (where_clause, base_params) =
            build_turn_usage_where_clause(&req.range, None, None, None)?;

        let count_sql = format!(
            "SELECT COUNT(*) FROM (
                SELECT 1 FROM turn_usage_metrics{where_clause}
                GROUP BY model_id, model_name, model, provider_id, provider_name
            )"
        );
        let total = conn.query_row(&count_sql, params_from_iter(base_params.clone()), |row| {
            row.get::<_, i64>(0)
        })? as u64;

        let mut list_params = base_params;
        list_params.push(SqlValue::Integer(page_size as i64));
        list_params.push(SqlValue::Integer(offset as i64));
        let list_sql = format!(
            "SELECT
                model_id, model_name, model, provider_id, provider_name,
                COUNT(*) AS turn_count,
                SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END) AS completed_count,
                SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) AS failed_count,
                SUM(CASE WHEN status = 'cancelled' THEN 1 ELSE 0 END) AS cancelled_count,
                COALESCE(SUM(input_tokens), 0) AS input_tokens,
                COALESCE(SUM(output_tokens), 0) AS output_tokens,
                COALESCE(SUM(total_tokens), 0) AS total_tokens,
                COALESCE(SUM(input_cache_read_tokens), 0) AS input_cache_read_tokens,
                COALESCE(SUM(input_cache_write_tokens), 0) AS input_cache_write_tokens,
                AVG(duration_ms) AS avg_duration_ms
             FROM turn_usage_metrics{where_clause}
             GROUP BY model_id, model_name, model, provider_id, provider_name
             ORDER BY turn_count DESC, total_tokens DESC
             LIMIT ? OFFSET ?"
        );
        let mut stmt = conn.prepare(&list_sql)?;
        let rows = stmt.query_map(params_from_iter(list_params), |row| {
            Ok(UsageModelStatsItem {
                model_id: row.get(0)?,
                model_name: row.get(1)?,
                model: row.get(2)?,
                provider_id: row.get(3)?,
                provider_name: row.get(4)?,
                turn_count: row.get::<_, i64>(5)? as u64,
                completed_count: row.get::<_, i64>(6)? as u64,
                failed_count: row.get::<_, i64>(7)? as u64,
                cancelled_count: row.get::<_, i64>(8)? as u64,
                input_tokens: row.get::<_, i64>(9)? as u64,
                output_tokens: row.get::<_, i64>(10)? as u64,
                total_tokens: row.get::<_, i64>(11)? as u64,
                input_cache_read_tokens: row.get::<_, i64>(12)? as u64,
                input_cache_write_tokens: row.get::<_, i64>(13)? as u64,
                avg_duration_ms: row
                    .get::<_, Option<f64>>(14)?
                    .map(|value| value.max(0.0).round() as u64),
            })
        })?;
        let items = rows.collect::<Result<Vec<_>, _>>()?;
        let has_more = (offset as u64 + page_size as u64) < total;
        Ok(UsageModelStatsPayload {
            page: UsagePage {
                page,
                page_size,
                total,
                has_more,
            },
            items,
        })
    }

    pub fn list_usage_tool_stats(
        &self,
        req: UsageStatsListRequest,
    ) -> AppResult<UsageToolStatsPayload> {
        let conn = self.open_connection()?;
        let (page, page_size, offset) = normalize_usage_pagination(req.page, req.page_size);
        let (where_clause, base_params) =
            build_usage_range_where_clause(&req.range, "r.started_at")?;

        let count_sql = format!(
            "SELECT COUNT(*) FROM (
                SELECT 1
                FROM turn_tool_metrics t
                LEFT JOIN turn_usage_metrics r ON r.turn_id = t.turn_id
                {where_clause}
                GROUP BY t.tool_name, t.tool_action
            )"
        );
        let total = conn.query_row(&count_sql, params_from_iter(base_params.clone()), |row| {
            row.get::<_, i64>(0)
        })? as u64;

        let mut list_params = base_params;
        list_params.push(SqlValue::Integer(page_size as i64));
        list_params.push(SqlValue::Integer(offset as i64));
        let list_sql = format!(
            "SELECT
                t.tool_name,
                t.tool_action,
                COUNT(*) AS call_count,
                SUM(CASE WHEN t.is_error = 0 THEN 1 ELSE 0 END) AS success_count,
                SUM(CASE WHEN t.is_error = 1 THEN 1 ELSE 0 END) AS error_count,
                AVG(t.duration_ms) AS avg_duration_ms
             FROM turn_tool_metrics t
             LEFT JOIN turn_usage_metrics r ON r.turn_id = t.turn_id
             {where_clause}
             GROUP BY t.tool_name, t.tool_action
             ORDER BY call_count DESC, error_count DESC
             LIMIT ? OFFSET ?"
        );
        let mut stmt = conn.prepare(&list_sql)?;
        let rows = stmt.query_map(params_from_iter(list_params), |row| {
            Ok(UsageToolStatsItem {
                tool_name: row.get(0)?,
                tool_action: row.get(1)?,
                call_count: row.get::<_, i64>(2)? as u64,
                success_count: row.get::<_, i64>(3)? as u64,
                error_count: row.get::<_, i64>(4)? as u64,
                avg_duration_ms: row
                    .get::<_, Option<f64>>(5)?
                    .map(|value| value.max(0.0).round() as u64),
            })
        })?;
        let items = rows.collect::<Result<Vec<_>, _>>()?;
        let has_more = (offset as u64 + page_size as u64) < total;
        Ok(UsageToolStatsPayload {
            page: UsagePage {
                page,
                page_size,
                total,
                has_more,
            },
            items,
        })
    }

    pub fn usage_log_detail(&self, req: UsageLogDetailRequest) -> AppResult<UsageLogDetailPayload> {
        let conn = self.open_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, turn_id, session_id, tool_name, tool_action, status, duration_ms, is_error, created_at
             FROM turn_tool_metrics
             WHERE turn_id = ?1
             ORDER BY created_at DESC, id DESC",
        )?;
        let rows = stmt.query_map([req.turn_id.as_str()], |row| {
            let duration_ms = row.get::<_, Option<i64>>(6)?.and_then(|value| {
                if value >= 0 {
                    Some(value as u64)
                } else {
                    None
                }
            });
            Ok(UsageToolLogItem {
                id: row.get(0)?,
                turn_id: row.get(1)?,
                session_id: row.get(2)?,
                tool_name: row.get(3)?,
                tool_action: row.get(4)?,
                status: row.get(5)?,
                duration_ms,
                is_error: row.get::<_, i64>(7)? != 0,
                created_at: row.get(8)?,
            })
        })?;
        let tools = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(UsageLogDetailPayload {
            turn_id: req.turn_id,
            tools,
        })
    }

    fn is_turn_detail_logging_enabled(&self, turn_id: &str) -> AppResult<bool> {
        let conn = self.open_connection()?;
        let value = conn
            .query_row(
                "SELECT detail_logged FROM turn_usage_metrics WHERE turn_id = ?1",
                [turn_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        if let Some(flag) = value {
            return Ok(flag != 0);
        }
        self.get_usage_detail_logging_enabled()
    }
}

fn normalize_usage_pagination(page: Option<u32>, page_size: Option<u32>) -> (u32, u32, u32) {
    let page = page.unwrap_or(1).max(1);
    let page_size = page_size
        .unwrap_or(DEFAULT_USAGE_PAGE_SIZE)
        .clamp(1, MAX_USAGE_PAGE_SIZE);
    let offset = page.saturating_sub(1).saturating_mul(page_size);
    (page, page_size, offset)
}

fn usage_range_start(range: &str) -> AppResult<Option<String>> {
    let now = Utc::now();
    let start = match range {
        USAGE_RANGE_24H => Some(now - ChronoDuration::hours(24)),
        USAGE_RANGE_7D => Some(now - ChronoDuration::days(7)),
        USAGE_RANGE_30D => Some(now - ChronoDuration::days(30)),
        USAGE_RANGE_ALL | "" => None,
        other => {
            return Err(AppError::Validation(format!(
                "unsupported usage range `{other}`"
            )))
        }
    };
    Ok(start.map(|value| value.to_rfc3339_opts(SecondsFormat::Nanos, true)))
}

fn build_turn_usage_where_clause(
    range: &str,
    provider_profile_id: Option<&str>,
    status: Option<&str>,
    detail_logged: Option<bool>,
) -> AppResult<(String, Vec<SqlValue>)> {
    let mut clauses = Vec::<String>::new();
    let mut params = Vec::<SqlValue>::new();

    if let Some(start_at) = usage_range_start(range)? {
        clauses.push("started_at >= ?".to_string());
        params.push(SqlValue::Text(start_at));
    }
    if let Some(profile_id) = provider_profile_id.filter(|value| !value.trim().is_empty()) {
        clauses.push("provider_profile_id = ?".to_string());
        params.push(SqlValue::Text(profile_id.to_string()));
    }
    if let Some(status) = status.filter(|value| !value.trim().is_empty()) {
        clauses.push("status = ?".to_string());
        params.push(SqlValue::Text(status.to_string()));
    }
    if let Some(detail_logged) = detail_logged {
        clauses.push("detail_logged = ?".to_string());
        params.push(SqlValue::Integer(if detail_logged { 1 } else { 0 }));
    }

    if clauses.is_empty() {
        return Ok((String::new(), params));
    }
    Ok((format!(" WHERE {}", clauses.join(" AND ")), params))
}

fn build_usage_range_where_clause(
    range: &str,
    started_at_column: &str,
) -> AppResult<(String, Vec<SqlValue>)> {
    if let Some(start_at) = usage_range_start(range)? {
        return Ok((
            format!(" WHERE {started_at_column} >= ?"),
            vec![SqlValue::Text(start_at)],
        ));
    }
    Ok((String::new(), Vec::new()))
}

fn calculate_duration_ms(started_at: Option<&str>, finished_at: Option<&str>) -> Option<i64> {
    let started_at = started_at?;
    let finished_at = finished_at?;
    let start = chrono::DateTime::parse_from_rfc3339(started_at).ok()?;
    let end = chrono::DateTime::parse_from_rfc3339(finished_at).ok()?;
    let duration = end.signed_duration_since(start).num_milliseconds();
    Some(duration.max(0))
}
