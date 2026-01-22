// Cost tracking and budget management

use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::error::{LlmError, LlmResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageData {
    pub date: String,
    pub cost_usd: f64,
    pub calls: u32,
    pub tokens: TokenUsage,
    pub models: HashMap<String, ModelUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input: u32,
    pub output: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUsage {
    pub calls: u32,
    pub cost_usd: f64,
    pub tokens: TokenUsage,
}

pub struct CostTracker {
    usage_file: PathBuf,
}

impl CostTracker {
    pub fn new(usage_file: impl AsRef<Path>) -> Self {
        let usage_file = PathBuf::from(shellexpand::tilde(usage_file.as_ref().to_str().unwrap()).to_string());
        Self { usage_file }
    }

    /// Load usage data from file
    async fn load_usage(&self) -> LlmResult<UsageData> {
        if !self.usage_file.exists() {
            return Ok(self.create_empty_usage());
        }

        let content = fs::read_to_string(&self.usage_file)
            .await
            .map_err(|e| LlmError::Request(format!("Failed to read usage file: {}", e)))?;

        // Handle empty file
        if content.trim().is_empty() {
            return Ok(self.create_empty_usage());
        }

        serde_json::from_str(&content)
            .map_err(|e| LlmError::Request(format!("Failed to parse usage file: {}", e)))
    }

    /// Save usage data to file
    async fn save_usage(&self, usage: &UsageData) -> LlmResult<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.usage_file.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| LlmError::Request(format!("Failed to create usage directory: {}", e)))?;
        }

        let json = serde_json::to_string_pretty(usage)
            .map_err(|e| LlmError::Request(format!("Failed to serialize usage data: {}", e)))?;

        fs::write(&self.usage_file, json)
            .await
            .map_err(|e| LlmError::Request(format!("Failed to write usage file: {}", e)))?;

        Ok(())
    }

    /// Check if daily budget has been exceeded
    pub async fn check_budget(&self, daily_limit_usd: f64) -> LlmResult<bool> {
        let mut usage = self.load_usage().await?;
        let today = Local::now().date_naive().to_string();

        // Reset if new day
        if usage.date != today {
            usage = self.create_empty_usage();
            self.save_usage(&usage).await?;
        }

        Ok(usage.cost_usd < daily_limit_usd)
    }

    /// Record usage for a single API call
    pub async fn record_usage(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
        cost_usd: f64,
    ) -> LlmResult<()> {
        let mut usage = self.load_usage().await?;
        let today = Local::now().date_naive().to_string();

        // Reset if new day
        if usage.date != today {
            usage = self.create_empty_usage();
        }

        // Update totals
        usage.calls += 1;
        usage.cost_usd += cost_usd;
        usage.tokens.input += input_tokens;
        usage.tokens.output += output_tokens;
        usage.tokens.total += input_tokens + output_tokens;

        // Update per-model stats
        usage
            .models
            .entry(model.to_string())
            .and_modify(|m| {
                m.calls += 1;
                m.cost_usd += cost_usd;
                m.tokens.input += input_tokens;
                m.tokens.output += output_tokens;
                m.tokens.total += input_tokens + output_tokens;
            })
            .or_insert(ModelUsage {
                calls: 1,
                cost_usd,
                tokens: TokenUsage {
                    input: input_tokens,
                    output: output_tokens,
                    total: input_tokens + output_tokens,
                },
            });

        self.save_usage(&usage).await?;

        tracing::info!(
            "Recorded usage: {}, cost=${:.6}, tokens={}",
            model,
            cost_usd,
            input_tokens + output_tokens
        );

        Ok(())
    }

    fn create_empty_usage(&self) -> UsageData {
        UsageData {
            date: Local::now().date_naive().to_string(),
            cost_usd: 0.0,
            calls: 0,
            tokens: TokenUsage {
                input: 0,
                output: 0,
                total: 0,
            },
            models: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_create_empty_usage() {
        let temp_file = NamedTempFile::new().unwrap();
        let tracker = CostTracker::new(temp_file.path());

        let usage = tracker.create_empty_usage();
        assert_eq!(usage.cost_usd, 0.0);
        assert_eq!(usage.calls, 0);
        assert_eq!(usage.tokens.total, 0);
    }

    #[tokio::test]
    async fn test_check_budget_under_limit() {
        let temp_file = NamedTempFile::new().unwrap();
        let tracker = CostTracker::new(temp_file.path());

        let under_budget = tracker.check_budget(0.10).await.unwrap();
        assert!(under_budget);
    }

    #[tokio::test]
    async fn test_record_and_check_budget() {
        let temp_file = NamedTempFile::new().unwrap();
        let tracker = CostTracker::new(temp_file.path());

        // Record some usage
        tracker
            .record_usage("test-model", 100, 50, 0.05)
            .await
            .unwrap();

        // Should be under budget
        let under_budget = tracker.check_budget(0.10).await.unwrap();
        assert!(under_budget);

        // Record more usage to exceed budget
        tracker
            .record_usage("test-model", 200, 100, 0.06)
            .await
            .unwrap();

        // Should exceed budget
        let under_budget = tracker.check_budget(0.10).await.unwrap();
        assert!(!under_budget);
    }

    #[tokio::test]
    async fn test_record_multiple_models() {
        let temp_file = NamedTempFile::new().unwrap();
        let tracker = CostTracker::new(temp_file.path());

        tracker
            .record_usage("model-a", 100, 50, 0.02)
            .await
            .unwrap();
        tracker
            .record_usage("model-b", 200, 100, 0.03)
            .await
            .unwrap();
        tracker
            .record_usage("model-a", 150, 75, 0.025)
            .await
            .unwrap();

        let usage = tracker.load_usage().await.unwrap();

        assert_eq!(usage.calls, 3);
        assert_eq!(usage.models.len(), 2);
        assert_eq!(usage.models.get("model-a").unwrap().calls, 2);
        assert_eq!(usage.models.get("model-b").unwrap().calls, 1);
    }

    #[tokio::test]
    async fn test_save_and_load_usage() {
        let temp_file = NamedTempFile::new().unwrap();
        let tracker = CostTracker::new(temp_file.path());

        tracker
            .record_usage("test-model", 100, 50, 0.01)
            .await
            .unwrap();

        let usage = tracker.load_usage().await.unwrap();
        assert_eq!(usage.calls, 1);
        assert_eq!(usage.cost_usd, 0.01);
        assert_eq!(usage.tokens.input, 100);
        assert_eq!(usage.tokens.output, 50);
        assert_eq!(usage.tokens.total, 150);
    }
}
