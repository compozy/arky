//! Thread-safe provider registry.

use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        RwLock,
    },
};

use arky_protocol::ProviderId;
use tracing::warn;

use crate::{
    Provider,
    ProviderDescriptor,
    ProviderError,
};

type ProviderMap = BTreeMap<ProviderId, Arc<dyn Provider>>;

/// Thread-safe registry for provider implementations.
#[derive(Clone, Default)]
pub struct ProviderRegistry {
    providers: Arc<RwLock<ProviderMap>>,
}

impl ProviderRegistry {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a provider instance keyed by its descriptor identifier.
    pub fn register<P>(&self, provider: P) -> Result<(), ProviderError>
    where
        P: Provider + 'static,
    {
        self.register_arc(Arc::new(provider))
    }

    /// Registers a provider instance from an existing shared pointer.
    pub fn register_arc(&self, provider: Arc<dyn Provider>) -> Result<(), ProviderError> {
        let descriptor = provider.descriptor().clone();
        let mut providers = write_providers(&self.providers);

        if providers.contains_key(&descriptor.id) {
            return Err(ProviderError::protocol_violation(
                format!("provider `{}` is already registered", descriptor.id),
                None,
            ));
        }

        providers.insert(descriptor.id, provider);
        drop(providers);
        Ok(())
    }

    /// Returns a provider by identifier.
    pub fn get(&self, id: &ProviderId) -> Result<Arc<dyn Provider>, ProviderError> {
        read_providers(&self.providers)
            .get(id)
            .cloned()
            .ok_or_else(|| ProviderError::not_found(id.clone()))
    }

    /// Returns a provider by identifier when present.
    #[must_use]
    pub fn maybe_get(&self, id: &ProviderId) -> Option<Arc<dyn Provider>> {
        read_providers(&self.providers).get(id).cloned()
    }

    /// Lists descriptors for all registered providers in identifier order.
    #[must_use]
    pub fn list(&self) -> Vec<ProviderDescriptor> {
        read_providers(&self.providers)
            .values()
            .map(|provider| provider.descriptor().clone())
            .collect()
    }

    /// Removes a registered provider.
    pub fn remove(&self, id: &ProviderId) -> Option<Arc<dyn Provider>> {
        write_providers(&self.providers).remove(id)
    }

    /// Clears the registry.
    pub fn clear(&self) {
        write_providers(&self.providers).clear();
    }
}

fn read_providers(
    lock: &RwLock<ProviderMap>,
) -> std::sync::RwLockReadGuard<'_, ProviderMap> {
    match lock.read() {
        Ok(guard) => guard,
        Err(poisoned) => {
            warn!("provider registry read lock was poisoned; recovering inner state");
            poisoned.into_inner()
        }
    }
}

fn write_providers(
    lock: &RwLock<ProviderMap>,
) -> std::sync::RwLockWriteGuard<'_, ProviderMap> {
    match lock.write() {
        Ok(guard) => guard,
        Err(poisoned) => {
            warn!("provider registry write lock was poisoned; recovering inner state");
            poisoned.into_inner()
        }
    }
}

#[cfg(test)]
mod tests {
    use futures::stream;
    use pretty_assertions::assert_eq;

    use super::ProviderRegistry;
    use crate::{
        Provider,
        ProviderCapabilities,
        ProviderDescriptor,
        ProviderError,
        ProviderEventStream,
        ProviderFamily,
        ProviderRequest,
    };
    use arky_protocol::ProviderId;

    struct StaticProvider {
        descriptor: ProviderDescriptor,
    }

    #[async_trait::async_trait]
    impl Provider for StaticProvider {
        fn descriptor(&self) -> &ProviderDescriptor {
            &self.descriptor
        }

        async fn stream(
            &self,
            _request: ProviderRequest,
        ) -> Result<ProviderEventStream, ProviderError> {
            Ok(Box::pin(stream::empty()))
        }
    }

    fn provider(id: &str) -> StaticProvider {
        StaticProvider {
            descriptor: ProviderDescriptor::new(
                ProviderId::new(id),
                ProviderFamily::Custom(id.to_owned()),
                ProviderCapabilities::new().with_streaming(true),
            ),
        }
    }

    #[test]
    fn provider_registry_should_register_lookup_and_list_providers() {
        let registry = ProviderRegistry::new();
        registry
            .register(provider("codex"))
            .expect("provider should register");
        registry
            .register(provider("claude-code"))
            .expect("provider should register");

        let listed = registry.list();

        assert_eq!(listed.len(), 2);
        assert_eq!(
            registry
                .get(&ProviderId::new("codex"))
                .expect("provider should resolve")
                .descriptor()
                .id
                .as_str(),
            "codex"
        );
    }

    #[test]
    fn provider_registry_should_return_not_found_for_missing_provider() {
        let registry = ProviderRegistry::new();
        let Err(error) = registry.get(&ProviderId::new("missing")) else {
            panic!("missing provider should fail");
        };

        assert!(matches!(error, ProviderError::NotFound { .. }));
    }

    #[test]
    fn provider_registry_should_reject_duplicate_registration() {
        let registry = ProviderRegistry::new();
        registry
            .register(provider("codex"))
            .expect("provider should register");
        let error = registry
            .register(provider("codex"))
            .expect_err("duplicate should fail");

        assert!(matches!(error, ProviderError::ProtocolViolation { .. }));
    }
}
