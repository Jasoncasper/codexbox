import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Image,
  Network,
  Plus,
  Save,
  TestTube,
  Trash2,
  RefreshCw,
  CheckCircle2,
  XCircle,
  Settings,
  Shield,
  Zap,
  DollarSign,
  type LucideIcon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

// Types
type ProviderProtocol = "responses" | "chat_completions" | "anthropic" | "custom";
type RoutingStrategy = "priority" | "round-robin" | "weighted" | "cost-optimized" | "latency-optimized" | "first-healthy";

interface SmartProvider {
  id: string;
  name: string;
  base_url: string;
  api_key: string;
  protocol: ProviderProtocol;
  priority: number;
  weight: number;
  enabled: boolean;
  supports_vision: boolean;
  vision_model: string;
  tags: string[];
  health_check: { enabled: boolean; interval_secs: number; timeout_secs: number; endpoint: string };
  rate_limit: { requests_per_minute: number; tokens_per_minute: number };
  cost: { input_cost_per_1k: number; output_cost_per_1k: number };
}

interface RoutingRule {
  name: string;
  description: string;
  enabled: boolean;
  priority: number;
  conditions: Array<{ field: string; operator: string; value: any }>;
  action: { type: string; target_providers?: string[]; strategy?: string; reason?: string; target_model?: string; provider_id?: string };
}

interface SmartRouterConfig {
  providers: SmartProvider[];
  rules: RoutingRule[];
  strategy: RoutingStrategy;
  fallback: { enabled: boolean; max_retries: number; retry_delay_ms: number };
  model_mappings: Array<{ source_model: string; target_model: string; provider_id: string; fallback_model?: string }>;
}

interface RoutingConfigPayload {
  config: SmartRouterConfig;
  config_path: string;
}

interface RouteTestPayload {
  provider_id: string;
  provider_name: string;
  target_model: string;
  rule_name: string;
  strategy: string;
}

type Status = "ok" | "failed" | string;
type CommandResult<T> = T & { status: Status; message: string };

// Strategy labels
const strategyLabels: Record<RoutingStrategy, string> = {
  "priority": "优先级",
  "round-robin": "轮询",
  "weighted": "加权随机",
  "cost-optimized": "成本最优",
  "latency-optimized": "延迟最低",
  "first-healthy": "首个健康",
};

const strategyIcons: Record<RoutingStrategy, LucideIcon> = {
  "priority": Shield,
  "round-robin": RefreshCw,
  "weighted": Settings,
  "cost-optimized": DollarSign,
  "latency-optimized": Zap,
  "first-healthy": CheckCircle2,
};

export default function RoutingPage() {
  const [config, setConfig] = useState<SmartRouterConfig | null>(null);
  const [configPath, setConfigPath] = useState("");
  const [activeTab, setActiveTab] = useState("providers");
  const [notice, setNotice] = useState<{ type: "ok" | "error"; text: string } | null>(null);
  const [loading, setLoading] = useState(false);

  // Test state
  const [testModel, setTestModel] = useState("gpt-4o");
  const [testHasImage, setTestHasImage] = useState(false);
  const [testResult, setTestResult] = useState<RouteTestPayload | null>(null);

  const loadConfig = useCallback(async () => {
    try {
      const result = await invoke<CommandResult<RoutingConfigPayload>>("load_routing_config");
      if (result.status === "ok") {
        setConfig((result as any).config);
        setConfigPath((result as any).config_path);
      }
    } catch (e) {
      console.error("Failed to load routing config:", e);
    }
  }, []);

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  const saveConfig = async () => {
    if (!config) return;
    setLoading(true);
    try {
      const result = await invoke<CommandResult<RoutingConfigPayload>>("save_routing_config", { config });
      setNotice({ type: result.status === "ok" ? "ok" : "error", text: result.message });
    } catch (e: any) {
      setNotice({ type: "error", text: String(e) });
    }
    setLoading(false);
  };

  const testRouting = async () => {
    if (!config) return;
    setLoading(true);
    try {
      const result = await invoke<CommandResult<RouteTestPayload>>("test_routing_decision", {
        config,
        model: testModel,
        hasImage: testHasImage,
      });
      setTestResult(result);
      setNotice({ type: result.status === "ok" ? "ok" : "error", text: result.message });
    } catch (e: any) {
      setNotice({ type: "error", text: String(e) });
    }
    setLoading(false);
  };

  const addProvider = () => {
    if (!config) return;
    const newProvider: SmartProvider = {
      id: `provider-${Date.now()}`,
      name: "新供应商",
      base_url: "",
      api_key: "",
      protocol: "chat_completions",
      priority: 100,
      weight: 100,
      enabled: true,
      supports_vision: false,
      vision_model: "",
      tags: [],
      health_check: { enabled: true, interval_secs: 60, timeout_secs: 5, endpoint: "/v1/models" },
      rate_limit: { requests_per_minute: 60, tokens_per_minute: 100000 },
      cost: { input_cost_per_1k: 0, output_cost_per_1k: 0 },
    };
    setConfig({ ...config, providers: [...config.providers, newProvider] });
  };

  const removeProvider = (index: number) => {
    if (!config) return;
    setConfig({ ...config, providers: config.providers.filter((_, i) => i !== index) });
  };

  const updateProvider = (index: number, updates: Partial<SmartProvider>) => {
    if (!config) return;
    const providers = [...config.providers];
    providers[index] = { ...providers[index], ...updates };
    setConfig({ ...config, providers });
  };

  const addRule = () => {
    if (!config) return;
    const newRule: RoutingRule = {
      name: `rule-${Date.now()}`,
      description: "",
      enabled: true,
      priority: 50,
      conditions: [{ field: "request.model", operator: "contains", value: "" }],
      action: { type: "route", target_providers: [], strategy: "priority" },
    };
    setConfig({ ...config, rules: [...config.rules, newRule] });
  };

  const removeRule = (index: number) => {
    if (!config) return;
    setConfig({ ...config, rules: config.rules.filter((_, i) => i !== index) });
  };

  if (!config) {
    return <div className="flex items-center justify-center h-64 text-muted-foreground">加载中...</div>;
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold flex items-center gap-2">
            <Network className="h-6 w-6" />
            智能路由
          </h2>
          <p className="text-sm text-muted-foreground mt-1">
            配置多供应商路由策略，支持按请求类型、模型、优先级智能分发
          </p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" onClick={loadConfig} disabled={loading}>
            <RefreshCw className="h-4 w-4 mr-1" /> 刷新
          </Button>
          <Button onClick={saveConfig} disabled={loading}>
            <Save className="h-4 w-4 mr-1" /> 保存配置
          </Button>
        </div>
      </div>

      {/* Notice */}
      {notice && (
        <div className={`p-3 rounded-md text-sm ${notice.type === "ok" ? "bg-green-50 text-green-800 border border-green-200" : "bg-red-50 text-red-800 border border-red-200"}`}>
          {notice.text}
          <button className="float-right" onClick={() => setNotice(null)}>×</button>
        </div>
      )}

      {/* Config path */}
      <div className="text-xs text-muted-foreground">配置文件: {configPath}</div>

      {/* Tabs */}
      <Tabs value={activeTab} onValueChange={setActiveTab}>
        <TabsList>
          <TabsTrigger value="providers">供应商 ({config.providers.length})</TabsTrigger>
          <TabsTrigger value="rules">规则 ({config.rules.length})</TabsTrigger>
          <TabsTrigger value="test">路由测试</TabsTrigger>
        </TabsList>

        {/* Providers Tab */}
        <TabsContent value="providers">
          <div className="space-y-4">
            <div className="flex justify-between items-center">
              <h3 className="text-lg font-semibold">供应商列表</h3>
              <Button size="sm" onClick={addProvider}>
                <Plus className="h-4 w-4 mr-1" /> 添加供应商
              </Button>
            </div>

            {config.providers.map((provider, index) => (
              <Card key={provider.id}>
                <CardHeader className="pb-3">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <CardTitle className="text-base">
                        <Input
                          value={provider.name}
                          onChange={(e) => updateProvider(index, { name: e.target.value })}
                          className="h-7 text-base font-semibold border-none p-0 w-auto"
                        />
                      </CardTitle>
                      {provider.enabled ? (
                        <Badge variant="default" className="bg-green-100 text-green-800">启用</Badge>
                      ) : (
                        <Badge variant="secondary">禁用</Badge>
                      )}
                      {provider.tags.map((tag) => (
                        <Badge key={tag} variant="outline">{tag}</Badge>
                      ))}
                    </div>
                    <div className="flex gap-1">
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => updateProvider(index, { enabled: !provider.enabled })}
                      >
                        {provider.enabled ? <XCircle className="h-4 w-4" /> : <CheckCircle2 className="h-4 w-4" />}
                      </Button>
                      <Button variant="ghost" size="sm" onClick={() => removeProvider(index)}>
                        <Trash2 className="h-4 w-4 text-destructive" />
                      </Button>
                    </div>
                  </div>
                </CardHeader>
                <CardContent className="space-y-3">
                  <div className="grid grid-cols-2 gap-3">
                    <div>
                      <Label className="text-xs">ID</Label>
                      <Input
                        value={provider.id}
                        onChange={(e) => updateProvider(index, { id: e.target.value })}
                        className="h-8 text-sm"
                      />
                    </div>
                    <div>
                      <Label className="text-xs">协议</Label>
                      <select
                        value={provider.protocol}
                        onChange={(e) => updateProvider(index, { protocol: e.target.value as ProviderProtocol })}
                        className="w-full h-8 text-sm border rounded-md px-2 bg-background"
                      >
                        <option value="chat_completions">Chat Completions</option>
                        <option value="responses">Responses</option>
                        <option value="anthropic">Anthropic</option>
                      </select>
                    </div>
                    <div className="col-span-2">
                      <Label className="text-xs">Base URL</Label>
                      <Input
                        value={provider.base_url}
                        onChange={(e) => updateProvider(index, { base_url: e.target.value })}
                        placeholder="https://api.openai.com/v1"
                        className="h-8 text-sm"
                      />
                    </div>
                    <div className="col-span-2">
                      <Label className="text-xs">API Key</Label>
                      <Input
                        type="password"
                        value={provider.api_key}
                        onChange={(e) => updateProvider(index, { api_key: e.target.value })}
                        placeholder="sk-..."
                        className="h-8 text-sm"
                      />
                    </div>
                    <div>
                      <Label className="text-xs">优先级 (越小越优先)</Label>
                      <Input
                        type="number"
                        value={provider.priority}
                        onChange={(e) => updateProvider(index, { priority: Number(e.target.value) })}
                        className="h-8 text-sm"
                      />
                    </div>
                    <div>
                      <Label className="text-xs">权重 (加权路由)</Label>
                      <Input
                        type="number"
                        value={provider.weight}
                        onChange={(e) => updateProvider(index, { weight: Number(e.target.value) })}
                        className="h-8 text-sm"
                      />
                    </div>
                  </div>
                  <div className="flex items-center gap-3">
                    <label className="flex items-center gap-2 text-sm cursor-pointer">
                      <input
                        type="checkbox"
                        checked={provider.supports_vision}
                        onChange={(e) => updateProvider(index, { supports_vision: e.target.checked })}
                      />
                      <Image className="h-4 w-4" /> 支持图片/视觉
                    </label>
                    {provider.supports_vision ? (
                      <div className="flex-1">
                        <Label className="text-xs">图片专用模型 (可选)</Label>
                        <Input
                          value={provider.vision_model}
                          onChange={(e) => updateProvider(index, { vision_model: e.target.value })}
                          placeholder="留空则用原模型，例如 gpt-4o"
                          className="h-8 text-sm"
                        />
                      </div>
                    ) : null}
                  </div>
                </CardContent>
              </Card>
            ))}
          </div>
        </TabsContent>

        {/* Rules Tab */}
        <TabsContent value="rules">
          <div className="space-y-4">
            <div className="flex justify-between items-center">
              <h3 className="text-lg font-semibold">路由规则</h3>
              <div className="flex gap-2">
                <Button size="sm" onClick={addRule}>
                  <Plus className="h-4 w-4 mr-1" /> 添加规则
                </Button>
              </div>
            </div>

            {/* Default strategy */}
            <Card>
              <CardHeader className="pb-3">
                <CardTitle className="text-sm">默认策略</CardTitle>
              </CardHeader>
              <CardContent>
                <div className="flex gap-2">
                  {(Object.keys(strategyLabels) as RoutingStrategy[]).map((strategy) => {
                    const Icon = strategyIcons[strategy];
                    return (
                      <button
                        key={strategy}
                        className={`flex items-center gap-1 px-3 py-1.5 text-xs rounded-md border transition-colors ${
                          config.strategy === strategy ? "bg-primary text-primary-foreground border-primary" : "hover:bg-muted"
                        }`}
                        onClick={() => setConfig({ ...config, strategy })}
                      >
                        <Icon className="h-3 w-3" />
                        {strategyLabels[strategy]}
                      </button>
                    );
                  })}
                </div>
              </CardContent>
            </Card>

            {/* Rules list */}
            {config.rules.map((rule, index) => (
              <Card key={rule.name}>
                <CardHeader className="pb-3">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <CardTitle className="text-sm">
                        <Input
                          value={rule.name}
                          onChange={(e) => {
                            const rules = [...config.rules];
                            rules[index] = { ...rules[index], name: e.target.value };
                            setConfig({ ...config, rules });
                          }}
                          className="h-6 text-sm font-semibold border-none p-0 w-auto"
                        />
                      </CardTitle>
                      {rule.enabled ? (
                        <Badge variant="default" className="bg-green-100 text-green-800">启用</Badge>
                      ) : (
                        <Badge variant="secondary">禁用</Badge>
                      )}
                      <Badge variant="outline">优先级: {rule.priority}</Badge>
                    </div>
                    <div className="flex gap-1">
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => {
                          const rules = [...config.rules];
                          rules[index] = { ...rules[index], enabled: !rules[index].enabled };
                          setConfig({ ...config, rules });
                        }}
                      >
                        {rule.enabled ? <XCircle className="h-4 w-4" /> : <CheckCircle2 className="h-4 w-4" />}
                      </Button>
                      <Button variant="ghost" size="sm" onClick={() => removeRule(index)}>
                        <Trash2 className="h-4 w-4 text-destructive" />
                      </Button>
                    </div>
                  </div>
                  <CardDescription>{rule.description || "无描述"}</CardDescription>
                </CardHeader>
                <CardContent className="space-y-2">
                  <div className="text-xs text-muted-foreground">
                    条件: {rule.conditions.map((c) => `${c.field} ${c.operator} ${JSON.stringify(c.value)}`).join(" AND ")}
                  </div>
                  <div className="text-xs text-muted-foreground">
                    动作: {rule.action.type === "route" 
                      ? `路由到 [${rule.action.target_providers?.join(", ")}] 策略: ${rule.action.strategy}`
                      : rule.action.type === "reject"
                      ? `拒绝: ${rule.action.reason}`
                      : `重写模型: ${rule.action.target_model} -> ${rule.action.provider_id}`}
                  </div>
                </CardContent>
              </Card>
            ))}
          </div>
        </TabsContent>

        {/* Test Tab */}
        <TabsContent value="test">
          <div className="space-y-4">
            <Card>
              <CardHeader>
                <CardTitle className="text-lg flex items-center gap-2">
                  <TestTube className="h-5 w-5" /> 路由测试
                </CardTitle>
                <CardDescription>测试路由决策，验证规则是否按预期工作</CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <Label>模型名</Label>
                    <Input
                      value={testModel}
                      onChange={(e) => setTestModel(e.target.value)}
                      placeholder="gpt-4o"
                    />
                  </div>
                  <div className="flex items-end">
                    <label className="flex items-center gap-2">
                      <input
                        type="checkbox"
                        checked={testHasImage}
                        onChange={(e) => setTestHasImage(e.target.checked)}
                      />
                      <span className="text-sm">包含图片</span>
                    </label>
                  </div>
                </div>
                <Button onClick={testRouting} disabled={loading}>
                  <TestTube className="h-4 w-4 mr-1" /> 测试路由
                </Button>

                {testResult && (
                  <div className="p-4 bg-muted rounded-lg space-y-2">
                    <div className="text-sm"><strong>选中供应商:</strong> {testResult.provider_name} ({testResult.provider_id})</div>
                    <div className="text-sm"><strong>目标模型:</strong> {testResult.target_model}</div>
                    <div className="text-sm"><strong>匹配规则:</strong> {testResult.rule_name}</div>
                    <div className="text-sm"><strong>使用策略:</strong> {testResult.strategy}</div>
                  </div>
                )}
              </CardContent>
            </Card>
          </div>
        </TabsContent>
      </Tabs>
    </div>
  );
}
