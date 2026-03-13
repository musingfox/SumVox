// Google Cloud TTS OAuth2 authentication using service account
// Generates JWT and exchanges for access token

use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::{Result, VoiceError};

const TOKEN_URI: &str = "https://oauth2.googleapis.com/token";
const SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";
const TOKEN_EXPIRY_SECONDS: u64 = 3600;

/// Service account JSON structure
#[derive(Debug, Deserialize)]
struct ServiceAccountKey {
    client_email: String,
    private_key: String,
    token_uri: Option<String>,
}

/// JWT Claims for Google OAuth2
#[derive(Debug, Serialize)]
struct Claims {
    iss: String,
    scope: String,
    aud: String,
    exp: u64,
    iat: u64,
}

/// Token response from OAuth2 endpoint
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(rename = "expires_in")]
    _expires_in: u64,
}

/// OAuth2 token provider with caching
pub struct CloudTtsAuth {
    service_account_json: String,
    cached_token: RwLock<Option<CachedToken>>,
}

#[derive(Debug, Clone)]
struct CachedToken {
    token: String,
    expires_at: SystemTime,
}

impl CloudTtsAuth {
    pub fn new(service_account_json: String) -> Self {
        Self {
            service_account_json,
            cached_token: RwLock::new(None),
        }
    }

    /// Get access token (cached or fresh)
    pub async fn get_token(&self) -> Result<String> {
        // Check if cached token is still valid
        {
            let cached = self.cached_token.read().unwrap();
            if let Some(ref token) = *cached {
                if SystemTime::now() < token.expires_at {
                    tracing::debug!("Using cached OAuth2 token");
                    return Ok(token.token.clone());
                }
            }
        }

        // Fetch new token
        tracing::debug!("Fetching new OAuth2 token");
        let token = self.fetch_token().await?;

        // Cache token
        let expires_at = SystemTime::now() + Duration::from_secs(TOKEN_EXPIRY_SECONDS - 300); // 5 min buffer
        {
            let mut cached = self.cached_token.write().unwrap();
            *cached = Some(CachedToken {
                token: token.clone(),
                expires_at,
            });
        }

        Ok(token)
    }

    /// Fetch new access token from OAuth2 endpoint
    async fn fetch_token(&self) -> Result<String> {
        // Parse service account JSON
        let sa: ServiceAccountKey = serde_json::from_str(&self.service_account_json)
            .map_err(|e| VoiceError::Config(format!("Invalid service account JSON: {}", e)))?;

        // Create JWT
        let jwt = self.create_jwt(&sa)?;

        // Exchange JWT for access token
        let token_uri = sa.token_uri.as_deref().unwrap_or(TOKEN_URI);
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| VoiceError::Voice(format!("Failed to create HTTP client: {}", e)))?;

        let params = [
            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
            ("assertion", &jwt),
        ];

        let response = client
            .post(token_uri)
            .form(&params)
            .send()
            .await
            .map_err(|e| VoiceError::Voice(format!("Token request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(VoiceError::Voice(format!(
                "Token request failed ({}): {}",
                status, error_text
            )));
        }

        let token_response: TokenResponse = response
            .json()
            .await
            .map_err(|e| VoiceError::Voice(format!("Failed to parse token response: {}", e)))?;

        Ok(token_response.access_token)
    }

    /// Create JWT assertion for OAuth2
    fn create_jwt(&self, sa: &ServiceAccountKey) -> Result<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = Claims {
            iss: sa.client_email.clone(),
            scope: SCOPE.to_string(),
            aud: TOKEN_URI.to_string(),
            exp: now + TOKEN_EXPIRY_SECONDS,
            iat: now,
        };

        let header = Header::new(Algorithm::RS256);
        let encoding_key = EncodingKey::from_rsa_pem(sa.private_key.as_bytes())
            .map_err(|e| VoiceError::Config(format!("Invalid RSA private key: {}", e)))?;

        encode(&header, &claims, &encoding_key)
            .map_err(|e| VoiceError::Voice(format!("Failed to create JWT: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_service_account_valid() {
        let json = r#"{
            "type": "service_account",
            "client_email": "test@proj.iam.gserviceaccount.com",
            "private_key": "-----BEGIN RSA PRIVATE KEY-----\nMIIEpAIBAAKCAQEA0Z3VS5JJcds3xfn/5wXy3htGrJXPTqq88jP1qnPz5I7AqMsf\nj7SV1rKy4LphQ4zJpqSb0vW5qWdXBGLNqMdKZsJ9LNvr0U/X12jEOBZI9v8Z8wIx\n3nMZeqLJQ3YvBWU6QjqOE6k9lz/P2fD1A3p5ZGVNPT1Q0j9TaB9lNh6qQtA9K8qd\nCxFKZJpV4PqTUDJFLVCKkCQ7v9KqcNgP9j0FXkPKi6vAkYr0jqFQ/tFXVLqE4JLN\nH8gVRYFJ1p2Wj5XuK6yE8vQxY8O8F0xB6bVk0k5N3TQ8RCQR0F9bD3P2x5PCQH8K\n7wH8x0fPQ8Dq9ZVqVKJq0F5Q0P0xD9q0F0QdqwIDAQABAoIBAEZ3KQ7eiP1kQqQN\nf8P5LZ6Kp8uKGQqJ8p0OqQqO1X9KfQ8O0Q8p0Q8p0QO0Q8p0Q8p0Q8p0Q8p0Q8O0\nQ8p0Q8p0Q8p0Q8O0Q8p0Q8p0Q8p0Q8p0Q8O0Q8p0Q8p0Q8p0Q8O0Q8p0Q8p0Q8p0\nQ8O0Q8p0Q8p0Q8p0Q8O0Q8p0Q8p0Q8p0Q8p0Q8O0Q8p0Q8p0Q8p0Q8O0Q8p0Q8p0\nQ8p0Q8O0Q8p0Q8p0Q8p0Q8O0Q8p0Q8p0Q8p0Q8p0Q8O0Q8p0Q8p0Q8p0Q8O0Q8p0\nQ8p0Q8p0Q8O0Q8p0Q8p0Q8p0Q8O0Q8p0Q8p0Q8p0Q8p0Q8O0Q8p0Q8p0Q8p0Q8O0\nQ8p0QOECgYEA7Z3Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q\n8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q\n8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8sEC\ngYEA4Z3Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q\n8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q\n8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8sEC\ngYEA7Z3Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q\n8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q\n8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8sEC\ngYBZ3Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p\n0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p\n0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0Q8p0QsE\nCgYBAOGd0PKdEPKdEPKdEPKdEPKdEPKdEPKdEPKdEPKdEPKdEPKdEPKdEPKdEPKd\nEPKdEPKdEPKdEPKdEPKdEPKdEPKdEPKdEPKdEPKdEPKdEPKdEPKdEPKdEPKdEPKd\nEPKdEPKdEPKdEPKdEPKdEPKdEPKdEPKdEPKdEPKdEPKdEPKdEPKdEPKdEPKdEPKd\nEA==\n-----END RSA PRIVATE KEY-----\n",
            "token_uri": "https://oauth2.googleapis.com/token"
        }"#;

        let sa: std::result::Result<ServiceAccountKey, _> = serde_json::from_str(json);
        assert!(sa.is_ok());
        let sa = sa.unwrap();
        assert_eq!(sa.client_email, "test@proj.iam.gserviceaccount.com");
        assert!(sa.private_key.contains("BEGIN RSA PRIVATE KEY"));
    }

    #[test]
    fn test_invalid_json() {
        let auth = CloudTtsAuth::new("not json".to_string());
        let result = serde_json::from_str::<ServiceAccountKey>(&auth.service_account_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_fields() {
        let json = r#"{"type":"service_account"}"#;
        let result = serde_json::from_str::<ServiceAccountKey>(json);
        assert!(result.is_err());
    }
}
