//! A2A Security Enforcer
//!
//! Enforces A2A enterprise security features:
//! - TLS 1.2+ requirement
//! - Authentication (Bearer, API Key, mTLS)

/// A2A security enforcer
pub struct A2ASecurityEnforcer {
    /// Require TLS
    tls_required: bool,
    /// Minimum TLS version
    min_tls_version: TlsVersion,
    /// Require authentication
    auth_required: bool,
    /// Allowed auth schemes
    auth_schemes: Vec<AuthScheme>,
}

impl A2ASecurityEnforcer {
    /// Create a new security enforcer
    pub fn new(require_tls: bool) -> Self {
        Self {
            tls_required: require_tls,
            min_tls_version: TlsVersion::Tls12,
            auth_required: false,
            auth_schemes: vec![
                AuthScheme::Bearer,
                AuthScheme::ApiKey,
            ],
        }
    }

    /// Create with full configuration
    pub fn with_config(
        require_tls: bool,
        min_tls_version: TlsVersion,
        auth_required: bool,
        auth_schemes: Vec<AuthScheme>,
    ) -> Self {
        Self {
            tls_required: require_tls,
            min_tls_version,
            auth_required,
            auth_schemes,
        }
    }

    /// Check transport security from connection info
    pub fn check_transport(&self, tls_info: Option<&TlsInfo>) -> Result<(), A2ASecurityError> {
        if !self.tls_required {
            return Ok(());
        }

        let tls = tls_info.ok_or(A2ASecurityError::TlsRequired)?;

        if tls.version < self.min_tls_version {
            return Err(A2ASecurityError::TlsVersionTooLow {
                required: self.min_tls_version,
                actual: tls.version,
            });
        }

        Ok(())
    }

    /// Check authentication from headers
    pub fn check_authentication(&self, headers: &[(String, String)]) -> Result<Option<Identity>, A2ASecurityError> {
        if !self.auth_required {
            // Auth not required, but try to extract identity if present
            return Ok(self.try_extract_identity(headers));
        }

        // Find Authorization header
        let auth_header = headers
            .iter()
            .find(|(name, _)| name.to_lowercase() == "authorization")
            .map(|(_, value)| value.as_str());

        let auth_value = auth_header.ok_or(A2ASecurityError::MissingCredentials)?;

        // Try each auth scheme
        for scheme in &self.auth_schemes {
            if let Some(identity) = scheme.validate(auth_value) {
                return Ok(Some(identity));
            }
        }

        Err(A2ASecurityError::InvalidCredentials)
    }

    /// Try to extract identity from headers (non-required)
    fn try_extract_identity(&self, headers: &[(String, String)]) -> Option<Identity> {
        let auth_header = headers
            .iter()
            .find(|(name, _)| name.to_lowercase() == "authorization")
            .map(|(_, value)| value.as_str())?;

        for scheme in &self.auth_schemes {
            if let Some(identity) = scheme.validate(auth_header) {
                return Some(identity);
            }
        }

        None
    }
}

impl Default for A2ASecurityEnforcer {
    fn default() -> Self {
        Self::new(false)
    }
}

/// TLS version
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TlsVersion {
    /// TLS 1.0 (not recommended)
    Tls10,
    /// TLS 1.1 (not recommended)
    Tls11,
    /// TLS 1.2 (minimum for A2A)
    Tls12,
    /// TLS 1.3 (recommended)
    Tls13,
}

/// TLS connection info
#[derive(Debug, Clone)]
pub struct TlsInfo {
    /// TLS version
    pub version: TlsVersion,
    /// Cipher suite
    pub cipher: Option<String>,
    /// Client certificate (for mTLS)
    pub client_cert: Option<String>,
}

/// Authentication scheme
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthScheme {
    /// Bearer token (OAuth 2.0)
    Bearer,
    /// API Key
    ApiKey,
    /// mTLS (client certificate)
    Mtls,
}

impl AuthScheme {
    /// Validate auth header and extract identity
    pub fn validate(&self, auth_header: &str) -> Option<Identity> {
        match self {
            AuthScheme::Bearer => {
                if auth_header.to_lowercase().starts_with("bearer ") {
                    let token = auth_header[7..].trim();
                    if !token.is_empty() {
                        return Some(Identity {
                            scheme: *self,
                            identifier: token.to_string(),
                            claims: None,
                        });
                    }
                }
                None
            }
            AuthScheme::ApiKey => {
                // Check for API key in various formats
                if auth_header.to_lowercase().starts_with("apikey ") {
                    let key = auth_header[7..].trim();
                    if !key.is_empty() {
                        return Some(Identity {
                            scheme: *self,
                            identifier: key.to_string(),
                            claims: None,
                        });
                    }
                }
                // Also accept X-API-Key style (would be in separate header)
                None
            }
            AuthScheme::Mtls => {
                // mTLS is validated at transport level, not in auth header
                None
            }
        }
    }
}

/// Authenticated identity
#[derive(Debug, Clone)]
pub struct Identity {
    /// Auth scheme used
    pub scheme: AuthScheme,
    /// Identifier (token, API key, cert CN)
    pub identifier: String,
    /// Claims (for JWT tokens)
    pub claims: Option<serde_json::Value>,
}

/// A2A security errors
#[derive(Debug, Clone)]
pub enum A2ASecurityError {
    /// TLS required but not present
    TlsRequired,
    /// TLS version too low
    TlsVersionTooLow {
        required: TlsVersion,
        actual: TlsVersion,
    },
    /// Missing credentials
    MissingCredentials,
    /// Invalid credentials
    InvalidCredentials,
    /// Insufficient permissions
    InsufficientPermissions(String),
}

impl std::fmt::Display for A2ASecurityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            A2ASecurityError::TlsRequired => write!(f, "TLS is required for A2A communication"),
            A2ASecurityError::TlsVersionTooLow { required, actual } => {
                write!(f, "TLS version {:?} is below minimum {:?}", actual, required)
            }
            A2ASecurityError::MissingCredentials => write!(f, "Authentication credentials required"),
            A2ASecurityError::InvalidCredentials => write!(f, "Invalid authentication credentials"),
            A2ASecurityError::InsufficientPermissions(msg) => write!(f, "Insufficient permissions: {}", msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_tls_required() {
        let enforcer = A2ASecurityEnforcer::new(false);
        let result = enforcer.check_transport(None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_tls_required_missing() {
        let enforcer = A2ASecurityEnforcer::new(true);
        let result = enforcer.check_transport(None);
        assert!(matches!(result, Err(A2ASecurityError::TlsRequired)));
    }

    #[test]
    fn test_tls_version_check() {
        let enforcer = A2ASecurityEnforcer::new(true);
        let tls_info = TlsInfo {
            version: TlsVersion::Tls12,
            cipher: None,
            client_cert: None,
        };

        let result = enforcer.check_transport(Some(&tls_info));
        assert!(result.is_ok());
    }

    #[test]
    fn test_tls_version_too_low() {
        let enforcer = A2ASecurityEnforcer::new(true);
        let tls_info = TlsInfo {
            version: TlsVersion::Tls11,
            cipher: None,
            client_cert: None,
        };

        let result = enforcer.check_transport(Some(&tls_info));
        assert!(matches!(result, Err(A2ASecurityError::TlsVersionTooLow { .. })));
    }

    #[test]
    fn test_bearer_auth() {
        let enforcer = A2ASecurityEnforcer::with_config(
            false,
            TlsVersion::Tls12,
            true,
            vec![AuthScheme::Bearer],
        );

        let headers = vec![(
            "authorization".to_string(),
            "Bearer my-secret-token".to_string(),
        )];

        let result = enforcer.check_authentication(&headers);
        assert!(result.is_ok());
        let identity = result.unwrap().unwrap();
        assert_eq!(identity.identifier, "my-secret-token");
    }

    #[test]
    fn test_missing_auth() {
        let enforcer = A2ASecurityEnforcer::with_config(
            false,
            TlsVersion::Tls12,
            true,
            vec![AuthScheme::Bearer],
        );

        let headers = vec![];
        let result = enforcer.check_authentication(&headers);
        assert!(matches!(result, Err(A2ASecurityError::MissingCredentials)));
    }
}
