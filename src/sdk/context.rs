use crate::{
    extension::{ExtensionContext, ExtensionId, Settings},
    sdk::{
        auth::AuthTokens, cache::ExtensionCache, http::HttpClient, process::ProcessExecutor,
        secrets::SecretStore, settings::SettingsApi, storage::ExtensionStorage, tasks::TaskScope,
    },
};
use std::time::Duration;

/// A typed view of host services for one built-in extension.
#[derive(Clone, Debug)]
pub struct SdkContext {
    context: ExtensionContext,
}

impl SdkContext {
    pub(crate) fn new(context: ExtensionContext) -> Self {
        Self { context }
    }

    pub fn extension_id(&self) -> Option<&ExtensionId> {
        self.context.extension_id()
    }

    pub fn storage(&self) -> ExtensionStorage {
        ExtensionStorage::for_context(&self.context)
    }

    pub fn secrets(&self) -> SecretStore {
        SecretStore::for_context(&self.context)
    }

    pub fn auth(&self) -> AuthTokens {
        AuthTokens::new(self.secrets())
    }

    pub fn cache(&self) -> ExtensionCache {
        ExtensionCache::new(self.storage().cache().clone())
    }

    pub fn http_client(
        &self,
        connect_timeout: Duration,
        timeout: Duration,
        user_agent: &str,
    ) -> Result<HttpClient, reqwest::Error> {
        HttpClient::new(connect_timeout, timeout, user_agent)
    }

    pub fn settings(&self, values: &Settings) -> SettingsApi {
        SettingsApi::from_values(values.clone())
    }

    pub fn process(&self) -> ProcessExecutor {
        ProcessExecutor
    }

    pub fn tasks(&self) -> TaskScope {
        self.context.tasks()
    }

    pub fn log(&self, level: crate::sdk::diagnostics::Level, message: &str) {
        crate::sdk::diagnostics::emit(self.extension_id(), level, message);
    }
}
