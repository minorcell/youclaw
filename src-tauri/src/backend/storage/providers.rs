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
        Ok(parsed.accounts)
    }

    fn write_provider_accounts_unlocked(&self, accounts: &[ProviderAccount]) -> AppResult<()> {
        let body = serde_json::to_vec_pretty(&StoredProviders {
            accounts: accounts.to_vec(),
        })?;
        fs::write(&self.inner.providers_path, body)?;
        Ok(())
    }
}
