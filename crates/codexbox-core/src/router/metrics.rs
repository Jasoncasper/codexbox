//! 指标收集器 — 收集路由和请求指标

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 指标收集器
pub struct MetricsCollector {
    route_counts: Arc<RwLock<HashMap<String, u64>>>,
    provider_counts: Arc<RwLock<HashMap<String, u64>>>,
    total_requests: Arc<RwLock<u64>>,
    total_errors: Arc<RwLock<u64>>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            route_counts: Arc::new(RwLock::new(HashMap::new())),
            provider_counts: Arc::new(RwLock::new(HashMap::new())),
            total_requests: Arc::new(RwLock::new(0)),
            total_errors: Arc::new(RwLock::new(0)),
        }
    }

    /// 记录一次路由决策
    pub async fn record_route(&self, rule_name: &str, provider_id: &str) {
        *self.total_requests.write().await += 1;
        *self
            .route_counts
            .write()
            .await
            .entry(rule_name.to_string())
            .or_insert(0) += 1;
        *self
            .provider_counts
            .write()
            .await
            .entry(provider_id.to_string())
            .or_insert(0) += 1;
    }

    /// 记录一次错误
    pub async fn record_error(&self) {
        *self.total_errors.write().await += 1;
    }

    /// 获取路由统计
    pub async fn get_route_stats(&self) -> HashMap<String, u64> {
        self.route_counts.read().await.clone()
    }

    /// 获取供应商统计
    pub async fn get_provider_stats(&self) -> HashMap<String, u64> {
        self.provider_counts.read().await.clone()
    }

    /// 获取总请求数
    pub async fn get_total_requests(&self) -> u64 {
        *self.total_requests.read().await
    }

    /// 获取总错误数
    pub async fn get_total_errors(&self) -> u64 {
        *self.total_errors.read().await
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}
