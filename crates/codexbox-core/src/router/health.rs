//! 健康检查器 — 定期检查供应商可用性

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time;

use super::config::SmartProvider;

/// 供应商健康状态
#[derive(Debug, Clone)]
pub struct ProviderHealthStatus {
    pub provider_id: String,
    pub is_healthy: bool,
    pub latency_ms: u64,
    pub last_check: Instant,
    pub error_message: Option<String>,
    pub consecutive_failures: u32,
}

/// 健康检查器
pub struct HealthChecker {
    status: Arc<RwLock<HashMap<String, ProviderHealthStatus>>>,
    providers: Arc<RwLock<Vec<SmartProvider>>>,
}

impl HealthChecker {
    pub fn new() -> Self {
        Self {
            status: Arc::new(RwLock::new(HashMap::new())),
            providers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 更新供应商列表
    pub async fn update_providers(&self, providers: Vec<SmartProvider>) {
        let mut p = self.providers.write().await;
        *p = providers;
    }

    /// 启动健康检查循环
    pub async fn start(&self) {
        let providers = self.providers.clone();
        let status = self.status.clone();

        tokio::spawn(async move {
            loop {
                let snapshot = providers.read().await.clone();
                let min_interval = Self::check_all(&snapshot, &status).await;

                time::sleep(Duration::from_secs(min_interval.max(10))).await;
            }
        });
    }

    /// 检查所有供应商
    async fn check_all(
        providers: &[SmartProvider],
        status: &Arc<RwLock<HashMap<String, ProviderHealthStatus>>>,
    ) -> u64 {
        let mut min_interval = u64::MAX;

        for provider in providers {
            if !provider.enabled || !provider.health_check.enabled {
                continue;
            }

            min_interval = min_interval.min(provider.health_check.interval_secs);

            let health_status = Self::check_single(provider).await;
            status
                .write()
                .await
                .insert(provider.id.clone(), health_status);
        }

        min_interval
    }

    /// 检查单个供应商
    async fn check_single(provider: &SmartProvider) -> ProviderHealthStatus {
        let client = reqwest::Client::new();
        let start = Instant::now();

        let url = if provider.base_url.contains("/v1") {
            format!("{}{}", provider.base_url.trim_end_matches('/'), provider.health_check.endpoint.trim_start_matches('/').replacen("v1/", "", 1))
        } else {
            format!(
                "{}/{}",
                provider.base_url.trim_end_matches('/'),
                provider.health_check.endpoint.trim_start_matches('/')
            )
        };

        let timeout = Duration::from_secs(provider.health_check.timeout_secs);

        let result = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", provider.api_key))
            .timeout(timeout)
            .send()
            .await;

        let latency = start.elapsed().as_millis() as u64;

        match result {
            Ok(response) => {
                let is_healthy = response.status().is_success();
                ProviderHealthStatus {
                    provider_id: provider.id.clone(),
                    is_healthy,
                    latency_ms: latency,
                    last_check: Instant::now(),
                    error_message: if is_healthy {
                        None
                    } else {
                        Some(format!("HTTP {}", response.status()))
                    },
                    consecutive_failures: if is_healthy { 0 } else { 1 },
                }
            }
            Err(e) => ProviderHealthStatus {
                provider_id: provider.id.clone(),
                is_healthy: false,
                latency_ms: latency,
                last_check: Instant::now(),
                error_message: Some(e.to_string()),
                consecutive_failures: 1,
            },
        }
    }

    /// 获取所有供应商健康状态
    pub async fn get_all_status(&self) -> HashMap<String, ProviderHealthStatus> {
        self.status.read().await.clone()
    }

    /// 获取单个供应商健康状态
    pub async fn get_status(&self, provider_id: &str) -> Option<ProviderHealthStatus> {
        self.status.read().await.get(provider_id).cloned()
    }

    /// 检查供应商是否健康（默认健康）
    pub async fn is_healthy(&self, provider_id: &str) -> bool {
        self.status
            .read()
            .await
            .get(provider_id)
            .map(|s| s.is_healthy)
            .unwrap_or(true)
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}
