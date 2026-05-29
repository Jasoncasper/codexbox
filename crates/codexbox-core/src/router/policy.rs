//! 策略评估器 — 评估路由规则条件

use serde_json::Value;

use super::config::*;

/// 策略评估器
pub struct PolicyEvaluator;

impl PolicyEvaluator {
    pub fn new() -> Self {
        Self
    }

    /// 评估所有条件（AND 逻辑）
    pub fn evaluate(&self, request: &RequestContext, conditions: &[Condition]) -> bool {
        conditions
            .iter()
            .all(|c| self.evaluate_condition(request, c))
    }

    fn evaluate_condition(&self, request: &RequestContext, condition: &Condition) -> bool {
        let field_value = self.extract_field(request, &condition.field);
        let Some(ref field_val) = field_value else {
            return false;
        };

        match &condition.operator {
            ConditionOperator::Equals => field_val == &condition.value,
            ConditionOperator::NotEquals => field_val != &condition.value,
            ConditionOperator::Contains => {
                if let (Some(s), Some(target)) = (field_val.as_str(), condition.value.as_str()) {
                    s.contains(target)
                } else {
                    false
                }
            }
            ConditionOperator::StartsWith => {
                if let (Some(s), Some(prefix)) = (field_val.as_str(), condition.value.as_str()) {
                    s.starts_with(prefix)
                } else {
                    false
                }
            }
            ConditionOperator::EndsWith => {
                if let (Some(s), Some(suffix)) = (field_val.as_str(), condition.value.as_str()) {
                    s.ends_with(suffix)
                } else {
                    false
                }
            }
            ConditionOperator::In => {
                if let Some(arr) = condition.value.as_array() {
                    arr.contains(field_val)
                } else {
                    false
                }
            }
            ConditionOperator::NotIn => {
                if let Some(arr) = condition.value.as_array() {
                    !arr.contains(field_val)
                } else {
                    true
                }
            }
            ConditionOperator::GreaterThan => {
                match (field_val.as_f64(), condition.value.as_f64()) {
                    (Some(a), Some(b)) => a > b,
                    _ => false,
                }
            }
            ConditionOperator::LessThan => {
                match (field_val.as_f64(), condition.value.as_f64()) {
                    (Some(a), Some(b)) => a < b,
                    _ => false,
                }
            }
            ConditionOperator::Regex => {
                if let (Some(s), Some(pattern)) = (field_val.as_str(), condition.value.as_str()) {
                    regex::Regex::new(pattern).map_or(false, |re| re.is_match(s))
                } else {
                    false
                }
            }
        }
    }

    /// 从请求上下文中提取字段值
    pub fn extract_field(&self, request: &RequestContext, field: &str) -> Option<Value> {
        match field {
            "request.type" => Some(Value::String(format!("{:?}", request.request_type).to_lowercase())),
            "request.model" => Some(Value::String(request.model.clone())),
            "request.has_image" => Some(Value::Bool(request.has_image)),
            "request.has_tools" => Some(Value::Bool(request.has_tools)),
            "request.priority" => Some(Value::String(format!("{:?}", request.priority).to_lowercase())),
            "request.token_count" => request.token_count.map(|t| Value::Number(t.into())),
            _ => None,
        }
    }
}

impl Default for PolicyEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(request: &RequestContext, condition: &Condition) -> bool {
        let evaluator = PolicyEvaluator::new();
        evaluator.evaluate(request, &[condition.clone()])
    }

    #[test]
    fn test_equals_condition() {
        let request = RequestContext {
            model: "gpt-4o".to_string(),
            has_image: true,
            ..Default::default()
        };
        let condition = Condition {
            field: "request.has_image".to_string(),
            operator: ConditionOperator::Equals,
            value: Value::Bool(true),
        };
        assert!(eval(&request, &condition));
    }

    #[test]
    fn test_contains_condition() {
        let request = RequestContext {
            model: "gpt-4o-mini".to_string(),
            ..Default::default()
        };
        let condition = Condition {
            field: "request.model".to_string(),
            operator: ConditionOperator::Contains,
            value: Value::String("gpt-4".to_string()),
        };
        assert!(eval(&request, &condition));
    }

    #[test]
    fn test_starts_with_condition() {
        let request = RequestContext {
            model: "claude-3-opus".to_string(),
            ..Default::default()
        };
        let condition = Condition {
            field: "request.model".to_string(),
            operator: ConditionOperator::StartsWith,
            value: Value::String("claude".to_string()),
        };
        assert!(eval(&request, &condition));
    }

    #[test]
    fn test_in_condition() {
        let request = RequestContext {
            request_type: RequestType::Image,
            ..Default::default()
        };
        let condition = Condition {
            field: "request.type".to_string(),
            operator: ConditionOperator::In,
            value: serde_json::json!(["image", "audio"]),
        };
        assert!(eval(&request, &condition));
    }

    #[test]
    fn test_greater_than_condition() {
        let request = RequestContext {
            token_count: Some(2000),
            ..Default::default()
        };
        let condition = Condition {
            field: "request.token_count".to_string(),
            operator: ConditionOperator::GreaterThan,
            value: serde_json::json!(1000),
        };
        assert!(eval(&request, &condition));
    }
}
