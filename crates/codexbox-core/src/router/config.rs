//! 路由配置结构定义

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 智能路由全局配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartRouterConfig {
    /// 供应商列表
    pub providers: Vec<SmartProvider>,
    /// 路由规则列表
    pub rules: Vec<RoutingRule>,
    /// 默认路由策略
    pub strategy: RoutingStrategy,
    /// 故障转移配置
    pub fallback: FallbackConfig,
    /// 模型映射表
    #[serde(default)]
    pub model_mappings: Vec<ModelMapping>,
}

impl Default for SmartRouterConfig {
    fn default() -> Self {
        Self {
            providers: Vec::new(),
            rules: Vec::new(),
            strategy: RoutingStrategy::Priority,
            fallback: FallbackConfig::default(),
            model_mappings: Vec::new(),
        }
    }
}

/// 增强供应商配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartProvider {
    /// 唯一标识
    pub id: String,
    /// 显示名称
    pub name: String,
    /// API 基础地址
    pub base_url: String,
    /// API 密钥
    pub api_key: String,
    /// 协议类型
    pub protocol: ProviderProtocol,
    /// 优先级（越小越优先）
    #[serde(default = "default_priority")]
    pub priority: u32,
    /// 权重（用于加权路由）
    #[serde(default = "default_weight")]
    pub weight: u32,
    /// 是否启用
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 标签列表
    #[serde(default)]
    pub tags: Vec<String>,
    /// 是否支持图片/视觉输入
    #[serde(default)]
    pub supports_vision: bool,
    /// 图片场景专用模型（为空则用原 model）
    #[serde(default)]
    pub vision_model: String,
    /// 健康检查配置
    #[serde(default)]
    pub health_check: HealthCheckConfig,
    /// 速率限制配置
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
    /// 成本配置
    #[serde(default)]
    pub cost: CostConfig,
}

fn default_priority() -> u32 {
    100
}
fn default_weight() -> u32 {
    100
}
fn default_true() -> bool {
    true
}

/// 供应商协议类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderProtocol {
    /// OpenAI Responses API (Codex 原生)
    Responses,
    /// OpenAI Chat Completions API
    ChatCompletions,
    /// Anthropic Messages API
    Anthropic,
    /// 自定义协议
    Custom,
}

impl Default for ProviderProtocol {
    fn default() -> Self {
        Self::Responses
    }
}

/// 健康检查配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    pub enabled: bool,
    #[serde(default = "default_health_interval")]
    pub interval_secs: u64,
    #[serde(default = "default_health_timeout")]
    pub timeout_secs: u64,
    #[serde(default = "default_health_endpoint")]
    pub endpoint: String,
}

fn default_health_interval() -> u64 {
    60
}
fn default_health_timeout() -> u64 {
    5
}
fn default_health_endpoint() -> String {
    "/v1/models".to_string()
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_secs: default_health_interval(),
            timeout_secs: default_health_timeout(),
            endpoint: default_health_endpoint(),
        }
    }
}

/// 速率限制配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    #[serde(default = "default_rpm")]
    pub requests_per_minute: u32,
    #[serde(default = "default_tpm")]
    pub tokens_per_minute: u64,
}

fn default_rpm() -> u32 {
    60
}
fn default_tpm() -> u64 {
    100_000
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: default_rpm(),
            tokens_per_minute: default_tpm(),
        }
    }
}

/// 成本配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostConfig {
    /// 每 1000 输入 token 成本（美元）
    #[serde(default)]
    pub input_cost_per_1k: f64,
    /// 每 1000 输出 token 成本（美元）
    #[serde(default)]
    pub output_cost_per_1k: f64,
}

impl Default for CostConfig {
    fn default() -> Self {
        Self {
            input_cost_per_1k: 0.0,
            output_cost_per_1k: 0.0,
        }
    }
}

/// 路由规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRule {
    /// 规则名称
    pub name: String,
    /// 规则描述
    #[serde(default)]
    pub description: String,
    /// 是否启用
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 规则优先级（越小越先匹配）
    #[serde(default = "default_priority")]
    pub priority: u32,
    /// 条件列表（AND 逻辑）
    pub conditions: Vec<Condition>,
    /// 匹配后的动作
    pub action: RoutingAction,
}

/// 条件定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    /// 字段路径（如 "request.type", "request.model"）
    pub field: String,
    /// 操作符
    pub operator: ConditionOperator,
    /// 比较值
    pub value: Value,
}

/// 条件操作符
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionOperator {
    Equals,
    NotEquals,
    Contains,
    In,
    NotIn,
    GreaterThan,
    LessThan,
    Regex,
    StartsWith,
    EndsWith,
}

/// 路由动作
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RoutingAction {
    /// 路由到指定供应商
    Route {
        target_providers: Vec<String>,
        strategy: RoutingStrategy,
    },
    /// 拒绝请求
    Reject { reason: String },
    /// 重写模型名并路由
    RewriteModel {
        target_model: String,
        provider_id: String,
    },
}

/// 路由策略
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum RoutingStrategy {
    /// 按优先级
    Priority,
    /// 轮询
    RoundRobin,
    /// 按权重随机
    Weighted,
    /// 成本最优
    CostOptimized,
    /// 延迟最低
    LatencyOptimized,
    /// 第一个健康的
    FirstHealthy,
}

impl Default for RoutingStrategy {
    fn default() -> Self {
        Self::Priority
    }
}

/// 故障转移配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackConfig {
    pub enabled: bool,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_retry_delay")]
    pub retry_delay_ms: u64,
    #[serde(default)]
    pub circuit_breaker: CircuitBreakerConfig,
}

fn default_max_retries() -> u32 {
    3
}
fn default_retry_delay() -> u64 {
    1000
}

impl Default for FallbackConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_retries: default_max_retries(),
            retry_delay_ms: default_retry_delay(),
            circuit_breaker: CircuitBreakerConfig::default(),
        }
    }
}

/// 熔断器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub enabled: bool,
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: u32,
    #[serde(default = "default_recovery_timeout")]
    pub recovery_timeout_secs: u64,
}

fn default_failure_threshold() -> u32 {
    5
}
fn default_recovery_timeout() -> u64 {
    30
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            failure_threshold: default_failure_threshold(),
            recovery_timeout_secs: default_recovery_timeout(),
        }
    }
}

/// 模型映射
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMapping {
    /// 源模型名（Codex 请求的模型）
    pub source_model: String,
    /// 目标模型名（发送给供应商的模型）
    pub target_model: String,
    /// 目标供应商 ID
    pub provider_id: String,
    /// 备选模型（当目标供应商不可用时）
    #[serde(default)]
    pub fallback_model: Option<String>,
}

/// 请求上下文（用于策略评估）
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub request_type: RequestType,
    pub model: String,
    pub has_image: bool,
    pub has_tools: bool,
    pub token_count: Option<u64>,
    pub priority: RequestPriority,
}

/// 请求类型
#[derive(Debug, Clone, PartialEq)]
pub enum RequestType {
    Chat,
    Image,
    Code,
    Embedding,
    Audio,
}

/// 请求优先级
#[derive(Debug, Clone, PartialEq)]
pub enum RequestPriority {
    Low,
    Normal,
    High,
}

impl Default for RequestContext {
    fn default() -> Self {
        Self {
            request_type: RequestType::Chat,
            model: String::new(),
            has_image: false,
            has_tools: false,
            token_count: None,
            priority: RequestPriority::Normal,
        }
    }
}
