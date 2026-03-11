use super::*;

impl StorageService {
    pub fn list_provider_profiles(&self) -> AppResult<Vec<ProviderProfile>> {
        Ok(flatten_provider_profiles(&self.list_provider_accounts()?))
    }

    pub fn list_provider_accounts(&self) -> AppResult<Vec<ProviderAccount>> {
        let _guard = self
            .inner
            .providers_lock
            .lock()
            .map_err(|_| AppError::Storage("provider lock poisoned".to_string()))?;
        self.load_provider_accounts_unlocked()
    }

    pub fn save_provider_accounts(&self, accounts: &[ProviderAccount]) -> AppResult<()> {
        let _guard = self
            .inner
            .providers_lock
            .lock()
            .map_err(|_| AppError::Storage("provider lock poisoned".to_string()))?;
        self.write_provider_accounts_unlocked(accounts)
    }

    fn load_provider_accounts_unlocked(&self) -> AppResult<Vec<ProviderAccount>> {
        let raw = fs::read_to_string(&self.inner.providers_path)?;
        let parsed = if raw.trim().is_empty() {
            StoredProviders::default()
        } else {
            serde_json::from_str::<StoredProviders>(&raw)?
        };

        let mut should_persist = false;
        let mut accounts = if parsed.accounts.is_empty() && !parsed.profiles.is_empty() {
            should_persist = true;
            migrate_provider_accounts_from_legacy(parsed.profiles)
        } else {
            parsed.accounts
        };
        if normalize_provider_accounts(&mut accounts) {
            should_persist = true;
        }
        if should_persist {
            self.write_provider_accounts_unlocked(&accounts)?;
        }
        Ok(accounts)
    }

    fn write_provider_accounts_unlocked(&self, accounts: &[ProviderAccount]) -> AppResult<()> {
        let body = serde_json::to_vec_pretty(&StoredProviders {
            accounts: accounts.to_vec(),
            profiles: Vec::new(),
        })?;
        fs::write(&self.inner.providers_path, body)?;
        Ok(())
    }
}
