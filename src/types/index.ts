export interface Message {
  role: "system" | "user" | "assistant" | "tool";
  content: string | null;
  reasoning_content?: string | null;
  tool_calls?: ToolCall[];
  tool_call_id?: string;
  name?: string;
  prefix?: boolean;
}

export interface ToolCall {
  id: string;
  type: "function";
  function: {
    name: string;
    arguments: string;
  };
}

export interface Conversation {
  id: string;
  title: string;
  model: string;
  created_at: number;
  updated_at: number;
  messages: string;
}

export interface ChatRequest {
  messages: Message[];
  model: string;
  base_url: string;
  api_key: string;
  temperature?: number;
  max_tokens?: number;
  thinking?: { type: "enabled" | "disabled" };
  reasoning_effort?: "high" | "max";
  tools?: Tool[];
  tool_choice?: "auto" | "none" | "required";
  stream?: boolean;
}

export interface AgentChatParams {
  messages: Message[];
  model: string;
  baseUrl: string;
  apiKey: string;
  temperature?: number;
  maxTokens?: number;
  thinking?: { type: "enabled" | "disabled" };
  reasoningEffort?: "high" | "max";
}

export interface ToolExecution {
  id: string;
  name: string;
  arguments: string;
  source: string;
  status: "running" | "done" | "error";
  result?: string;
  error?: string;
}

export interface ApprovalRequest {
  id: string;
  tool_name: string;
  command: string;
  policy: string;
}

export interface AgentUsage {
  prompt_tokens: number;
  completion_tokens: number;
  total_tokens: number;
}

export interface AgentDonePayload {
  messages: Message[];
  reason: "stop" | "max_iterations" | "cancelled" | "finish_reason" | "mcp_error" | "error";
  usage?: AgentUsage;
  mcp_error?: string | null;
}

export interface McpServerConfig {
  id: string;
  name: string;
  command: string;
  args: string[];
  env: Record<string, string>;
  cwd: string | null;
  timeout: number;
  transport: "stdio" | "sse";
  enabled: boolean;
}

export interface Tool {
  type: "function";
  function: {
    name: string;
    description: string;
    parameters: Record<string, unknown>;
    strict?: boolean;
  };
}

export interface StreamChunk {
  id: string;
  choices: Array<{
    index: number;
    delta: {
      role?: string;
      content?: string | null;
      reasoning_content?: string | null;
      tool_calls?: Array<{
        index: number;
        id?: string;
        type?: "function";
        function?: {
          name?: string;
          arguments?: string;
        };
      }>;
    };
    finish_reason: string | null;
  }>;
  created: number;
  model: string;
  object: string;
  usage?: {
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
    prompt_cache_hit_tokens?: number;
    prompt_cache_miss_tokens?: number;
    completion_tokens_details?: {
      reasoning_tokens: number;
    };
  } | null;
}

export type ThemeName = "frost" | "morning-dew" | "aurora" | "dusk" | "deep-ocean";

export const THEME_META: Record<ThemeName, { label: string; desc: string; swatch: string }> = {
  "frost": { label: "霜白", desc: "最浅", swatch: "linear-gradient(135deg,#5068c8,#f2f4f8)" },
  "morning-dew": { label: "晨露", desc: "较浅", swatch: "linear-gradient(135deg,#2d9b8e,#ece7de)" },
  "aurora": { label: "极光", desc: "适中", swatch: "linear-gradient(135deg,#74fcc0,#1a5838)" },
  "dusk": { label: "薄暮", desc: "较深", swatch: "linear-gradient(135deg,#d4745c,#2a2630)" },
  "deep-ocean": { label: "深海", desc: "最深", swatch: "linear-gradient(135deg,#4fc3b4,#080d18)" },
};
