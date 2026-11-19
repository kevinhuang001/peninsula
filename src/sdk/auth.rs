//! Credential helpers layered on the extension's standard secret namespace.
use super::secrets::{SecretBackend, SecretStore, SecretsError};

#[derive(Clone, Debug)]
pub struct AuthTokens {
    secrets: SecretStore,
}

impl AuthTokens {
    pub fn new(secrets: SecretStore) -> Self {
        Self { secrets }
    }
    pub fn access_token(&self, account: &str) -> Result<Option<String>, SecretsError> {
        self.secrets.get(&format!("auth.{account}.access"))
    }
    pub fn refresh_token(&self, account: &str) -> Result<Option<String>, SecretsError> {
        self.secrets.get(&format!("auth.{account}.refresh"))
    }
    pub fn set_tokens(
        &self,
        account: &str,
        access: &str,
        refresh: Option<&str>,
    ) -> Result<SecretBackend, SecretsError> {
        let backend = self
            .secrets
            .set(&format!("auth.{account}.access"), access)?;
        if let Some(refresh) = refresh {
            self.secrets
                .set(&format!("auth.{account}.refresh"), refresh)?;
        }
        Ok(backend)
    }
    pub fn remove(&self, account: &str) -> Result<(), SecretsError> {
        self.secrets.delete(&format!("auth.{account}.access"))?;
        self.secrets.delete(&format!("auth.{account}.refresh"))
    }
}
