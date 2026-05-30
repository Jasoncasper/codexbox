//! 路由配置结构定义

use serde::{Deserialize, Serialize, Serializer};

/// 智能路由全局配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartRouterConfig {
    /// 模型列表
    pub providers: Vec<SmartProvider>,
    /// 多模态回退模型名（当选中模型不支持视觉且请求有图片时使用）
    #[serde(default)]
    pub vision_fallback_model: String,
    /// 故障转移配置
    #[serde(default)]
    pub fallback: FallbackConfig,
}

impl Default for SmartRouterConfig {
    fn default() -> Self {
        Self {
            providers: Vec::new(),
            vision_fallback_model: String::new(),
            fallback: FallbackConfig::default(),
        }
    }
}

/// 模型配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartProvider {
    /// 唯一标识
    pub id: String,
    /// 显示名称
    pub name: String,
    /// API 基础地址
    pub base_url: String,
    /// API 密钥（序列化自动脱敏，仅保留首尾各4位）
    #[serde(serialize_with = "mask_api_key")]
    pub api_key: String,
    /// 协议类型
    #[serde(default)]
    pub protocol: ProviderProtocol,
    /// 是否启用
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 是否支持多模态（图片/视觉输入）
    #[serde(default)]
    pub supports_vision: bool,
    /// 使用完整 URL（不自动拼接 /chat/completions 等后缀）
    #[serde(default)]
    pub use_full_url: bool,
    /// 上游模型名（发给 API 的实际 model 值），为空则用 id
    #[serde(default)]
    pub target_model: String,
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

/// 故障转移配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_retry_delay")]
    pub retry_delay_ms: u64,
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
        }
    }
}

/// 请求上下文
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub model: String,
    pub has_image: bool,
    pub has_tools: bool,
}

impl Default for RequestContext {
    fn default() -> Self {
        Self {
            model: String::new(),
            has_image: false,
            has_tools: false,
        }
    }
}

fn mask_api_key<S: Serializer>(key: &str, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&api_key_masked_str(key))
}

pub fn api_key_masked_str(key: &str) -> String {
    if key.is_empty() {
        String::new()
    } else if key.len() <= 8 {
        "****".to_string()
    } else {
        format!("{}...{}", &key[..4], &key[key.len()-4..])
    }
}
