use super::*;

impl StorageService {
    #[allow(clippy::too_many_arguments)]
    pub fn record_shell_execution(
        &self,
        session_id: &str,
        turn_id: &str,
        call_id: Option<&str>,
        command: &str,
        cwd: &str,
        status: &str,
        exit_code: Option<i32>,
        signal: Option<i32>,
        duration_ms: Option<u64>,
        stdout_bytes: Option<usize>,
        stderr_bytes: Option<usize>,
    ) -> AppResult<()> {
        let conn = self.open_connection()?;
        conn.execute(
            "INSERT INTO shell_executions (
                id, session_id, turn_id, call_id, command, cwd, status, exit_code, signal, duration_ms, stdout_bytes, stderr_bytes, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                uuid::Uuid::new_v4().to_string(),
                session_id,
                turn_id,
                call_id,
                command,
                cwd,
                status,
                exit_code.map(|value| value as i64),
                signal.map(|value| value as i64),
                duration_ms.map(|value| value as i64),
                stdout_bytes.map(|value| value as i64),
                stderr_bytes.map(|value| value as i64),
                now_timestamp(),
            ],
        )?;
        Ok(())
    }
}
