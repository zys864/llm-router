use std::collections::HashMap;

use axum::http::HeaderMap;

use crate::{config::ProxyKeyConfig, error::AppError};

#[derive(Clone, Debug)]
pub struct AuthenticatedCaller {
    pub id: String,
}

#[derive(Clone, Default)]
pub struct AuthService {
    callers_by_key: HashMap<String, ProxyKeyConfig>,
}

impl AuthService {
    pub fn new(configs: Vec<ProxyKeyConfig>) -> Self {
        Self {
            callers_by_key: configs
                .into_iter()
                .map(|config| (config.api_key.clone(), config))
                .collect(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        !self.callers_by_key.is_empty()
    }

    pub fn authenticate(&self, key: &str) -> Result<Option<AuthenticatedCaller>, AppError> {
        Ok(self
            .callers_by_key
            .get(key)
            .map(|config| AuthenticatedCaller {
                id: config.id.clone(),
            }))
    }

    pub fn authenticate_header(
        &self,
        headers: &HeaderMap,
    ) -> Result<Option<AuthenticatedCaller>, AppError> {
        if !self.is_enabled() {
            return Ok(None);
        }

        let header = headers
            .get(axum::http::header::AUTHORIZATION)
            .ok_or_else(|| AppError::authentication("missing bearer token"))?;
        let header = header
            .to_str()
            .map_err(|_| AppError::authentication("invalid authorization header"))?;
        let token = header
            .strip_prefix("Bearer ")
            .ok_or_else(|| AppError::authentication("expected bearer token"))?;

        self.authenticate(token)?
            .ok_or_else(|| AppError::authentication("invalid proxy api key"))
            .map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_valid_proxy_key() {
        let auth = AuthService::new(vec![ProxyKeyConfig {
            id: "team-alpha".into(),
            api_key: "lr_live_alpha".into(),
            max_requests: 10,
        }]);

        let caller = auth.authenticate("lr_live_alpha").unwrap().unwrap();
        assert_eq!(caller.id, "team-alpha");
    }
}
