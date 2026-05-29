//! 智能路由引擎模块
//!
//! 支持多供应商配置、策略路由、健康检查、故障转移等功能。

pub mod config;
pub mod engine;
pub mod health;
pub mod metrics;
pub mod policy;

pub use config::*;
pub use engine::{RouteDecision, RouterEngine};
pub use health::{HealthChecker, ProviderHealthStatus};
pub use metrics::MetricsCollector;
pub use policy::PolicyEvaluator;
