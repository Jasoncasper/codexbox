//! 路由引擎 — 核心路由决策逻辑

use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::config::*;
use super::health::HealthChecker;
use super::metrics::MetricsCollector;
use super::policy::PolicyEvaluator;

/// 路由决策结果
#[derive(Debug, Clone)]
pub struct RouteDecision {
    /// 选中的供应商
    pub provider: SmartProvider,
    /// 目标模型名
    pub target_model: String,
    /// 匹配的规则名
    pub rule_name: String,
    /// 使用的策略
    pub strategy_used: RoutingStrategy,
}

/// 路由引擎
pub struct RouterEngine {
    config: Arc<RwLock<SmartRouterConfig>>,
    policy: PolicyEvaluator,
    health: Arc<HealthChecker>,
    metrics: Arc<MetricsCollector>,
    round_robin_index: Arc<RwLock<u64>>,
}

impl RouterEngine {
    /// 创建新的路由引擎
    pub fn new(config: SmartRouterConfig) -> Arc<Self> {
        let config = Arc::new(RwLock::new(config));
        let health = Arc::new(HealthChecker::new());
        let metrics = Arc::new(MetricsCollector::new());

        let engine = Arc::new(Self {
            config: config.clone(),
            policy: PolicyEvaluator::new(),
            health: health.clone(),
            metrics,
            round_robin_index: Arc::new(RwLock::new(0)),
        });

        // 启动健康检查
        let engine_clone = engine.clone();
        tokio::spawn(async move {
            let providers = engine_clone.config.read().await.providers.clone();
            engine_clone.health.update_providers(providers).await;
            engine_clone.health.start().await;
        });

        engine
    }

    /// 从配置文件加载并创建路由引擎
    pub fn load_from_file(path: &Path) -> anyhow::Result<Arc<Self>> {
        let content = std::fs::read_to_string(path)?;
        let config: SmartRouterConfig = toml::from_str(&content)?;
        Ok(Self::new(config))
    }

    /// 获取当前配置
    pub async fn get_config(&self) -> SmartRouterConfig {
        self.config.read().await.clone()
    }

    /// 更新配置
    pub async fn update_config(&self, config: SmartRouterConfig) {
        let mut cfg = self.config.write().await;
        *cfg = config.clone();
        self.health.update_providers(config.providers).await;
    }

    /// 核心路由方法
    pub async fn route(&self, request: &RequestContext) -> anyhow::Result<RouteDecision> {
        let config = self.config.read().await;

        // 1. 按优先级排序规则，找到第一个匹配的
        let mut sorted_rules: Vec<&RoutingRule> =
            config.rules.iter().filter(|r| r.enabled).collect();
        sorted_rules.sort_by_key(|r| r.priority);

        for rule in &sorted_rules {
            if self.policy.evaluate(request, &rule.conditions) {
                match &rule.action {
                    RoutingAction::Route {
                        target_providers,
                        strategy,
                    } => {
                        let providers =
                            self.get_healthy_providers(target_providers, &config.providers).await;

                        if providers.is_empty() {
                            continue;
                        }

                        let providers =
                            self.apply_image_routing(request, &providers);
                        let selected = self.select_provider(strategy, &providers).await?;

                        let target_model = self.vision_model_for(&selected).unwrap_or_else(|| {
                            self.resolve_model(&request.model, &selected, &config.model_mappings)
                        });

                        self.metrics.record_route(&rule.name, &selected.id).await;

                        return Ok(RouteDecision {
                            provider: selected,
                            target_model,
                            rule_name: rule.name.clone(),
                            strategy_used: strategy.clone(),
                        });
                    }
                    RoutingAction::Reject { reason } => {
                        self.metrics.record_error().await;
                        anyhow::bail!("Request rejected by rule '{}': {}", rule.name, reason);
                    }
                    RoutingAction::RewriteModel {
                        target_model,
                        provider_id,
                    } => {
                        if let Some(provider) =
                            config.providers.iter().find(|p| &p.id == provider_id)
                        {
                            self.metrics.record_route(&rule.name, &provider.id).await;
                            return Ok(RouteDecision {
                                provider: provider.clone(),
                                target_model: target_model.clone(),
                                rule_name: rule.name.clone(),
                                strategy_used: RoutingStrategy::Priority,
                            });
                        }
                    }
                }
            }
        }

        // 2. 没有匹配规则，使用默认策略
        let all_providers = self.get_all_healthy_providers(&config.providers).await;
        if all_providers.is_empty() {
            self.metrics.record_error().await;
            anyhow::bail!("No healthy providers available");
        }

        let providers = self.apply_image_routing(request, &all_providers);
        if providers.is_empty() {
            self.metrics.record_error().await;
            anyhow::bail!("No healthy providers available for this request type");
        }
        let selected = self.select_provider(&config.strategy, &providers).await?;

        let target_model = self.vision_model_for(&selected).unwrap_or_else(|| request.model.clone());

        self.metrics.record_route("default", &selected.id).await;

        Ok(RouteDecision {
            provider: selected,
            target_model,
            rule_name: "default".to_string(),
            strategy_used: config.strategy.clone(),
        })
    }

    /// 图片场景过滤：当请求有图片时，只保留支持视觉的供应商；
    /// 无图片时正常返回全部。
    fn apply_image_routing(
        &self,
        request: &RequestContext,
        providers: &[SmartProvider],
    ) -> Vec<SmartProvider> {
        if !request.has_image {
            return providers.to_vec();
        }
        let vision_providers: Vec<SmartProvider> = providers
            .iter()
            .filter(|p| p.supports_vision)
            .cloned()
            .collect();
        if vision_providers.is_empty() {
            return providers.to_vec();
        }
        vision_providers
    }

    /// 如果供应商配了 vision_model 且当前请求有图片，返回它
    fn vision_model_for(&self, provider: &SmartProvider) -> Option<String> {
        if !provider.supports_vision || provider.vision_model.is_empty() {
            return None;
        }
        Some(provider.vision_model.clone())
    }

    /// 选择供应商（根据策略）
    async fn select_provider(
        &self,
        strategy: &RoutingStrategy,
        providers: &[SmartProvider],
    ) -> anyhow::Result<SmartProvider> {
        if providers.is_empty() {
            anyhow::bail!("No providers to select from");
        }

        match strategy {
            RoutingStrategy::Priority => {
                Ok(providers.iter().min_by_key(|p| p.priority).unwrap().clone())
            }
            RoutingStrategy::Weighted => self.weighted_selection(providers),
            RoutingStrategy::RoundRobin => self.round_robin_selection(providers).await,
            RoutingStrategy::CostOptimized => self.cost_optimized_selection(providers),
            RoutingStrategy::LatencyOptimized => Ok(self
                .latency_optimized_selection(providers)
                .await),
            RoutingStrategy::FirstHealthy => Ok(providers[0].clone()),
        }
    }

    /// 加权随机选择
    fn weighted_selection(&self, providers: &[SmartProvider]) -> anyhow::Result<SmartProvider> {
        use rand::Rng;
        let total_weight: u32 = providers.iter().map(|p| p.weight).sum();
        if total_weight == 0 {
            return Ok(providers[0].clone());
        }

        let mut rng = rand::rng();
        let mut random_value = rng.random_range(0..total_weight);
        for provider in providers {
            if random_value < provider.weight {
                return Ok(provider.clone());
            }
            random_value -= provider.weight;
        }

        Ok(providers.last().unwrap().clone())
    }

    /// 轮询选择
    async fn round_robin_selection(
        &self,
        providers: &[SmartProvider],
    ) -> anyhow::Result<SmartProvider> {
        let mut index = self.round_robin_index.write().await;
        let idx = (*index as usize) % providers.len();
        *index += 1;
        Ok(providers[idx].clone())
    }

    /// 成本最优选择
    fn cost_optimized_selection(&self, providers: &[SmartProvider]) -> anyhow::Result<SmartProvider> {
        Ok(providers
            .iter()
            .min_by(|a, b| {
                a.cost
                    .input_cost_per_1k
                    .partial_cmp(&b.cost.input_cost_per_1k)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap()
            .clone())
    }

    /// 延迟最优选择
    async fn latency_optimized_selection(&self, providers: &[SmartProvider]) -> SmartProvider {
        let health_data = self.health.get_all_status().await;

        providers
            .iter()
            .min_by_key(|p| {
                health_data
                    .get(&p.id)
                    .map(|h| h.latency_ms)
                    .unwrap_or(u64::MAX)
            })
            .unwrap()
            .clone()
    }

    /// 获取健康的供应商（从指定列表）
    async fn get_healthy_providers(
        &self,
        provider_ids: &[String],
        all_providers: &[SmartProvider],
    ) -> Vec<SmartProvider> {
        all_providers
            .iter()
            .filter(|p| provider_ids.contains(&p.id) && p.enabled)
            .cloned()
            .collect()
    }

    /// 获取所有健康供应商
    async fn get_all_healthy_providers(
        &self,
        all_providers: &[SmartProvider],
    ) -> Vec<SmartProvider> {
        all_providers
            .iter()
            .filter(|p| p.enabled)
            .cloned()
            .collect()
    }

    /// 解析模型映射
    fn resolve_model(
        &self,
        model: &str,
        provider: &SmartProvider,
        mappings: &[ModelMapping],
    ) -> String {
        for mapping in mappings {
            if mapping.source_model == model && mapping.provider_id == provider.id {
                return mapping.target_model.clone();
            }
        }
        model.to_string()
    }

    /// 获取健康检查器引用
    pub fn health_checker(&self) -> &HealthChecker {
        &self.health
    }

    /// 获取指标收集器引用
    pub fn metrics(&self) -> &MetricsCollector {
        &self.metrics
    }
}
