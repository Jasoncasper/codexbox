import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ChevronDown,
  ChevronRight,
  Network,
  Plus,
  Save,
  Trash2,
  RefreshCw,
  CheckCircle2,
  XCircle,
  Image,
  Copy,
  FlaskConical,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Badge } from "@/components/ui/badge";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";

// Types
type ProviderProtocol = "responses" | "chat_completions" | "anthropic" | "custom";

interface SmartProvider {
  id: string;
  name: string;
  base_url: string;
  api_key: string;
  protocol: ProviderProtocol;
  enabled: boolean;
  supports_vision: boolean;
  use_full_url: boolean;
}

interface SmartRouterConfig {
  providers: SmartProvider[];
  vision_fallback_model: string;
  fallback: { enabled: boolean; max_retries: number; retry_delay_ms: number };
}

interface RoutingConfigPayload {
  config: SmartRouterConfig;
  config_path: string;
}

type Status = "ok" | "failed" | string;
type CommandResult<T> = T & { status: Status; message: string };

export default function RoutingPage() {
  const [config, setConfig] = useState<SmartRouterConfig | null>(null);
  const [configPath, setConfigPath] = useState("");
  const [activeTab, setActiveTab] = useState("models");
  const [notice, setNotice] = useState<{ type: "ok" | "error"; text: string } | null>(null);

  const [testingIndex, setTestingIndex] = useState<number | null>(null);
  const [savingIndex, setSavingIndex] = useState<number | null>(null);
  const [savingVision, setSavingVision] = useState(false);
  const [expandedIndex, setExpandedIndex] = useState<number | null>(null);

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

  const addProvider = () => {
    if (!config) return;
    const newProvider: SmartProvider = {
      id: "",
      name: "新模型",
      base_url: "",
      api_key: "",
      protocol: "chat_completions",
      enabled: true,
      supports_vision: false,
      use_full_url: false,
    };
    setConfig({ ...config, providers: [...config.providers, newProvider] });
    setExpandedIndex(config.providers.length);
  };

  const saveProvider = async (index: number) => {
    if (!config) return;
    const provider = config.providers[index];
    if (!provider.id.trim()) {
      setNotice({ type: "error", text: "请先填写模型名称" });
      return;
    }
    setSavingIndex(index);
    try {
      const result = await invoke<CommandResult<RoutingConfigPayload>>("upsert_provider", { provider });
      setNotice({ type: result.status === "ok" ? "ok" : "error", text: result.message });
      if (result.status === "ok") {
        setConfig((result as any).config);
        setConfigPath((result as any).config_path);
      }
    } catch (e: any) {
      setNotice({ type: "error", text: String(e) });
    }
    setSavingIndex(null);
  };

  const removeProvider = async (index: number) => {
    if (!config) return;
    const providerId = config.providers[index].id;
    if (providerId) {
      try {
        const result = await invoke<CommandResult<RoutingConfigPayload>>("delete_provider", { providerId });
        if (result.status === "ok") {
          setConfig((result as any).config);
          setNotice({ type: "ok", text: "模型已删除" });
          return;
        }
      } catch (e: any) {
        setNotice({ type: "error", text: String(e) });
      }
    }
    setConfig({ ...config, providers: config.providers.filter((_, i) => i !== index) });
  };

  const updateProvider = (index: number, updates: Partial<SmartProvider>) => {
    if (!config) return;
    const providers = [...config.providers];
    providers[index] = { ...providers[index], ...updates };
    setConfig({ ...config, providers });
  };

  const copyProvider = (index: number) => {
    if (!config) return;
    const source = config.providers[index];
    const copy: SmartProvider = { ...source, id: "", name: `${source.name} (副本)` };
    setConfig({ ...config, providers: [...config.providers, copy] });
  };

  const testProvider = async (index: number) => {
    if (!config) return;
    const provider = config.providers[index];
    setTestingIndex(index);
    try {
      const result = await invoke<CommandResult<any>>("test_smart_provider", { provider });
      setNotice({ type: result.status === "ok" ? "ok" : "error", text: result.message });
    } catch (e: any) {
      setNotice({ type: "error", text: String(e) });
    }
    setTestingIndex(null);
  };

  const saveVisionFallback = async () => {
    if (!config) return;
    setSavingVision(true);
    try {
      const result = await invoke<CommandResult<RoutingConfigPayload>>("save_routing_config", { config });
      setNotice({ type: result.status === "ok" ? "ok" : "error", text: result.message });
    } catch (e: any) {
      setNotice({ type: "error", text: String(e) });
    }
    setSavingVision(false);
  };

  const visionModels = config?.providers.filter((p) => p.supports_vision) ?? [];

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
            配置模型与 API 的映射关系，支持多模态回退路由
          </p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" onClick={loadConfig}>
            <RefreshCw className="h-4 w-4 mr-1" /> 刷新
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
          <TabsTrigger value="models">模型 ({config.providers.length})</TabsTrigger>
          <TabsTrigger value="vision">路由规则</TabsTrigger>
        </TabsList>

        {/* Models Tab */}
        <TabsContent value="models">
          <div className="space-y-4">
            <div className="flex justify-between items-center">
              <h3 className="text-lg font-semibold">模型列表</h3>
              <Button size="sm" onClick={addProvider}>
                <Plus className="h-4 w-4 mr-1" /> 添加模型
              </Button>
            </div>

            {config.providers.map((provider, index) => {
              const isExpanded = expandedIndex === index;
              const isNew = !provider.id;
              return (
              <Card key={index} className={isNew ? "border-blue-300" : ""}>
                <CardHeader
                  className={`pb-3 cursor-pointer hover:bg-muted/30 rounded-t-lg ${isExpanded ? "" : "rounded-b-lg"}`}
                  onClick={() => setExpandedIndex(isExpanded ? null : index)}
                >
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      {isExpanded ? <ChevronDown className="h-4 w-4 text-muted-foreground" /> : <ChevronRight className="h-4 w-4 text-muted-foreground" />}
                      <CardTitle className="text-base">
                        {isExpanded ? (
                          <Input
                            value={provider.name}
                            onChange={(e) => updateProvider(index, { name: e.target.value })}
                            onClick={(e) => e.stopPropagation()}
                            className="h-7 text-base font-semibold border-none p-0 w-auto"
                          />
                        ) : (
                          <span>{provider.name}</span>
                        )}
                      </CardTitle>
                      <span className="text-xs text-muted-foreground">{provider.id}</span>
                      {provider.enabled ? (
                        <Badge variant="default" className="bg-green-100 text-green-800">启用</Badge>
                      ) : (
                        <Badge variant="secondary">禁用</Badge>
                      )}
                      {provider.supports_vision ? (
                        <Badge variant="outline"><Image className="h-3 w-3 inline mr-1" />多模态</Badge>
                      ) : null}
                    </div>
                    <div className="flex gap-1" onClick={(e) => e.stopPropagation()}>
                      <Button
                        variant="ghost"
                        size="sm"
                        title="保存模型"
                        onClick={() => saveProvider(index)}
                        disabled={savingIndex === index}
                      >
                        <Save className={`h-4 w-4 ${savingIndex === index ? "animate-spin" : ""}`} />
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        title="测试连接"
                        onClick={() => testProvider(index)}
                        disabled={testingIndex === index}
                      >
                        <FlaskConical className={`h-4 w-4 ${testingIndex === index ? "animate-spin" : ""}`} />
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        title="复制模型"
                        onClick={() => copyProvider(index)}
                      >
                        <Copy className="h-4 w-4" />
                      </Button>
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
                {isExpanded && (
                <CardContent className="space-y-3">
                  <div className="grid grid-cols-2 gap-3">
                    <div>
                      <Label className="text-xs">模型名称</Label>
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
                  </div>
                  <label className="flex items-center gap-2 text-sm cursor-pointer">
                    <input
                      type="checkbox"
                      checked={provider.supports_vision}
                      onChange={(e) => updateProvider(index, { supports_vision: e.target.checked })}
                    />
                    <Image className="h-4 w-4" /> 支持多模态理解（图片/视觉）
                  </label>
                  <label className="flex items-center gap-2 text-sm cursor-pointer">
                    <input
                      type="checkbox"
                      checked={provider.use_full_url}
                      onChange={(e) => updateProvider(index, { use_full_url: e.target.checked })}
                    />
                    使用完整 URL（不自动拼接 /chat/completions、/v1 等路径）
                  </label>
                </CardContent>
                )}
              </Card>
              );
            })}
          </div>
        </TabsContent>

        {/* Vision Fallback Tab */}
        <TabsContent value="vision">
          <Card>
            <CardHeader>
              <CardTitle className="text-lg flex items-center gap-2">
                <Image className="h-5 w-5" /> 路由规则
              </CardTitle>
              <CardDescription>
                配置图片/视觉消息的路由规则：当选中模型不支持多模态时，自动回退到指定模型
              </CardDescription>
            </CardHeader>
            <CardContent>
              {visionModels.length === 0 ? (
                <p className="text-sm text-muted-foreground">
                  暂无支持多模态的模型。请先在"模型"标签中为模型勾选"支持多模态理解"。
                </p>
              ) : (
                <div className="space-y-2">
                  <Label>回退模型</Label>
                  <select
                    value={config.vision_fallback_model}
                    onChange={(e) => setConfig({ ...config, vision_fallback_model: e.target.value })}
                    className="w-full h-9 text-sm border rounded-md px-2 bg-background"
                  >
                    <option value="">不启用回退</option>
                    {visionModels.map((m) => (
                      <option key={m.id} value={m.id}>
                        {m.name} ({m.id})
                      </option>
                    ))}
                  </select>
                  <Button size="sm" onClick={saveVisionFallback} disabled={savingVision} className="mt-2">
                    <Save className="h-4 w-4 mr-1" /> 保存规则
                  </Button>
                  <p className="text-xs text-muted-foreground mt-2">
                    选择后，当请求包含图片但匹配的模型不支持多模态时，自动改用此模型处理
                  </p>
                </div>
              )}
            </CardContent>
          </Card>
        </TabsContent>

      </Tabs>
    </div>
  );
}
