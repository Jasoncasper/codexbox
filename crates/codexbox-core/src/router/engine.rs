//! 路由引擎 — 核心路由决策逻辑

use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::config::*;

/// 路由决策结果
#[derive(Debug, Clone)]
pub struct RouteDecision {
    /// 选中的模型配置
    pub provider: SmartProvider,
    /// 目标模型名
    pub target_model: String,
    /// 决策说明
    pub rule_name: String,
}

/// 路由引擎
pub struct RouterEngine {
    config: Arc<RwLock<SmartRouterConfig>>,
}

impl RouterEngine {
    /// 创建新的路由引擎
    pub fn new(config: SmartRouterConfig) -> Arc<Self> {
        Arc::new(Self {
            config: Arc::new(RwLock::new(config)),
        })
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
        *cfg = config;
    }

    /// 核心路由方法：按模型名查找配置
    pub async fn route(&self, request: &RequestContext) -> anyhow::Result<RouteDecision> {
        let config = self.config.read().await;

        // 辅助：解析实际发给上游的模型名
        fn resolve_target(provider: &SmartProvider, fallback: &str) -> String {
            if !provider.target_model.trim().is_empty() {
                provider.target_model.trim().to_string()
            } else {
                fallback.to_string()
            }
        }

        // 1. 按模型名精确匹配
        let provider = config
            .providers
            .iter()
            .find(|p| p.enabled && p.id == request.model);

        match provider {
            Some(p) => {
                // 2. 图片请求但模型不支持多模态 → 回退
                if request.has_image && !p.supports_vision && !config.vision_fallback_model.is_empty() {
                    if let Some(fallback) = config
                        .providers
                        .iter()
                        .find(|fp| fp.enabled && fp.id == config.vision_fallback_model)
                    {
                        return Ok(RouteDecision {
                            target_model: resolve_target(fallback, &config.vision_fallback_model),
                            provider: fallback.clone(),
                            rule_name: "vision-fallback".to_string(),
                        });
                    }
                }
                Ok(RouteDecision {
                    target_model: resolve_target(p, &request.model),
                    provider: p.clone(),
                    rule_name: "model-match".to_string(),
                })
            }
            None => {
                // 3. 无匹配模型，用第一个启用的
                let first = config
                    .providers
                    .iter()
                    .find(|p| p.enabled)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("No enabled models"))?;
                let target = resolve_target(&first, &first.id);
                Ok(RouteDecision {
                    target_model: target,
                    provider: first,
                    rule_name: "default".to_string(),
                })
            }
        }
    }
}
