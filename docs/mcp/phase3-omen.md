# Bolt MCP Phase 3: OMEN Integration

**AI-Powered Container Intelligence via Universal AI Router**

---

## Overview

Phase 3 integrates [OMEN](https://github.com/ghostkellz/omen) - the Open Model Exchange Network - into Bolt's MCP ecosystem, bringing smart AI routing capabilities to container management.

### What is OMEN?

OMEN is a universal AI API gateway that provides:
- **Smart Routing** across 8+ AI providers (OpenAI, Anthropic, Google, Azure, XAI, Ollama, Bedrock, VertexAI)
- **Cost/Latency/Quality-aware** provider selection
- **Multi-strategy** support (single, race, speculative, parallel_merge)
- **Billing & Rate Limiting** with per-user quotas
- **OpenAI-compatible API** for drop-in replacement

### Integration Benefits

- **AI-Enhanced Containers**: Claude and other assistants can intelligently route AI requests
- **Local-First**: Prefer local Ollama (4090/3070) for code tasks, cloud for reasoning
- **Cost Optimization**: Stay within budgets via intent-based routing
- **Multi-Provider Fallback**: Automatic failover across providers
- **MCP Native**: Expose AI routing as MCP tools for seamless integration

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Bolt MCP Gateway                          │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  Catalog  │  Tool Registry  │  Client Manager          │ │
│  │  Secret Store  │  Interceptors  │  OMEN Adapter       │ │
│  └────────────────┬───────────────────────────────────────┘ │
└───────────────────┼───────────────────────────────────────────┘
                    │
      ┌─────────────┴──────────────┐
      │                            │
┌─────▼─────┐              ┌──────▼──────┐
│   OMEN    │              │  Container  │
│  Router   │              │  MCP Tools  │
└─────┬─────┘              └─────────────┘
      │
┌─────┴────────────────────────┐
│  AI Providers                │
│  • OpenAI (GPT-4, GPT-3.5)   │
│  • Anthropic (Claude 3)      │
│  • Google (Gemini)           │
│  • Azure OpenAI              │
│  • XAI (Grok)                │
│  • Ollama (Local 4090/3070)  │
│  • AWS Bedrock               │
│  • Vertex AI                 │
└──────────────────────────────┘
```

---

## Features

### 1. MCP Tool: `bolt_omen_chat`

Expose OMEN's smart routing as an MCP tool that AI assistants can invoke.

**Capabilities:**
- Intent-based routing (code, tests, analysis, explanation, regex, general)
- Cost/latency/quality optimization
- Budget caps and max latency constraints
- Multi-provider strategies (race, speculative)
- Automatic provider fallback

**Example Usage (via Claude):**

```
User: "Generate a Python function that validates email addresses"

Claude: I'll use the Bolt OMEN router to generate this code efficiently.
[Invokes bolt_omen_chat with intent="code", model="auto"]

OMEN: Routes to Ollama (local 4090) for fast, cost-free code generation
Response: <Python email validation function>
```

### 2. OMEN Provider Adapter

Present each AI provider as a virtual MCP server in the gateway catalog.

**Virtual Servers:**
- `omen-openai` - OpenAI GPT models
- `omen-anthropic` - Anthropic Claude models
- `omen-google` - Google Gemini models
- `omen-azure` - Azure OpenAI
- `omen-xai` - XAI Grok models
- `omen-ollama` - Local Ollama models
- `omen-bedrock` - AWS Bedrock
- `omen-vertexai` - Google Vertex AI (Claude via GCP)

Each virtual server exposes tools like:
- `<provider>_chat` - Chat completion
- `<provider>_embeddings` - Generate embeddings

### 3. CLI Commands

```bash
# Start OMEN router server
bolt mcp omen serve --address 0.0.0.0 --port 8080

# List available providers
bolt mcp omen providers

# Test provider connectivity
bolt mcp omen test openai --prompt "Hello world"

# Show provider health and scores
bolt mcp omen health
```

---

## Quick Start

### 1. Build with OMEN Support

```bash
cd /data/projects/bolt

# Build with OMEN feature enabled
cargo build --features "mcp,omen" --release

# Binary location
./target/x86_64-unknown-linux-gnu/release/bolt
```

### 2. Configure OMEN

Create `~/.config/bolt/omen.toml`:

```toml
[server]
bind = "0.0.0.0:8080"

[routing]
prefer_local_for = ["code", "regex", "tests"]
budget_monthly_usd = 150

[providers.openai]
enabled = true
api_key = "env:OPENAI_API_KEY"

[providers.anthropic]
enabled = true
api_key = "env:ANTHROPIC_API_KEY"

[providers.google]
enabled = true
api_key = "env:GEMINI_API_KEY"

[providers.ollama]
enabled = true
endpoints = ["http://localhost:11434"]
models = ["deepseek-coder:6.7b", "llama3.1:8b-instruct"]
```

### 3. Start OMEN MCP Server

```bash
# Serve via MCP
bolt mcp serve --features omen

# Or start standalone OMEN router
bolt mcp omen serve --config ~/.config/bolt/omen.toml
```

### 4. Use from Claude Desktop

Update `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "bolt-omen": {
      "command": "/data/projects/bolt/target/release/bolt",
      "args": ["mcp", "serve", "--transport", "stdio"],
      "env": {
        "OPENAI_API_KEY": "sk-...",
        "ANTHROPIC_API_KEY": "sk-ant-...",
        "GEMINI_API_KEY": "..."
      }
    }
  }
}
```

**In Claude Desktop:**

> "Use the Bolt OMEN router to generate a function that validates email addresses. Prefer local models for speed."

Claude will invoke `bolt_omen_chat` with `intent="code"` and OMEN will route to Ollama.

---

## Configuration

### MCP Tool Configuration

```toml
# Boltfile.toml
[services.app.mcp]
enabled = true
transport = "websocket"

[services.app.mcp.tools.omen_router]
enabled = true
default_model = "auto"
default_budget_usd = 0.10
default_max_latency_ms = 5000

[[services.app.mcp.tools.omen_router.providers]]
id = "ollama"
enabled = true
priority = 1  # Highest priority

[[services.app.mcp.tools.omen_router.providers]]
id = "anthropic"
enabled = true
priority = 2

[[services.app.mcp.tools.omen_router.providers]]
id = "openai"
enabled = true
priority = 3
```

### Gateway Catalog with OMEN

```toml
# ~/.config/bolt/mcp-catalog.toml
[metadata]
name = "Bolt + OMEN Catalog"
version = "1.0.0"

# Bolt runtime server
[[servers.bolt-runtime]]
name = "bolt-runtime"
server_type = "embedded"
description = "Bolt container runtime tools"
enabled = true

[[servers.bolt-runtime.tools]]
name = "bolt_gpu_stats"
enabled = true

[[servers.bolt-runtime.tools]]
name = "bolt_filesystem"
enabled = true

# OMEN AI providers (virtual servers)
[[servers.omen-openai]]
name = "omen-openai"
server_type = "omen-provider"
description = "OpenAI GPT models via OMEN"
enabled = true

[[servers.omen-openai.tools]]
name = "openai_chat"
enabled = true

[[servers.omen-anthropic]]
name = "omen-anthropic"
server_type = "omen-provider"
description = "Anthropic Claude models via OMEN"
enabled = true

[[servers.omen-anthropic.tools]]
name = "anthropic_chat"
enabled = true

[[servers.omen-ollama]]
name = "omen-ollama"
server_type = "omen-provider"
description = "Local Ollama models (4090/3070)"
enabled = true

[[servers.omen-ollama.tools]]
name = "ollama_chat"
enabled = true
```

---

## Smart Routing Examples

### Example 1: Code Generation (Local-First)

```json
{
  "messages": [
    {"role": "user", "content": "Write a function to parse CSV files"}
  ],
  "model": "auto",
  "intent": "code",
  "strategy": "single"
}
```

**OMEN Decision:**
- Intent: `code` → prefer local
- Checks Ollama (localhost:11434) health → ✅ healthy
- Routes to `ollama:deepseek-coder:6.7b`
- Latency: ~500ms, Cost: $0.00

### Example 2: Complex Reasoning (Quality-First)

```json
{
  "messages": [
    {"role": "user", "content": "Analyze this architecture diagram and suggest improvements"}
  ],
  "model": "auto",
  "intent": "analysis",
  "strategy": "single"
}
```

**OMEN Decision:**
- Intent: `analysis` → prefer quality
- Quality scores: Claude (0.95) > GPT-4 (0.90) > Gemini (0.85)
- Routes to `anthropic:claude-3-opus`
- Latency: ~1200ms, Cost: $0.015/1k tokens

### Example 3: Race Strategy (Lowest Latency)

```json
{
  "messages": [
    {"role": "user", "content": "Summarize this README"}
  ],
  "model": "auto",
  "intent": "explanation",
  "strategy": "race",
  "providers": ["ollama", "anthropic", "openai"],
  "max_latency_ms": 2000
}
```

**OMEN Decision:**
- Launches 3 parallel requests
- First to respond: Ollama @ 450ms → **Winner**
- Cancels Anthropic (800ms) and OpenAI (1100ms)
- Total cost: Input tokens charged for all, output only for winner

---

## API Reference

### MCP Tool: `bolt_omen_chat`

**Input Schema:**

```json
{
  "messages": [
    {"role": "user", "content": "..."}
  ],
  "model": "auto",
  "temperature": 0.7,
  "max_tokens": 500,
  "intent": "code|tests|analysis|explanation|regex|general",
  "strategy": "single|race|speculate_k|parallel_merge",
  "providers": ["openai", "anthropic", ...],
  "budget_usd": 0.10,
  "max_latency_ms": 5000
}
```

**Output:**

```json
{
  "id": "chatcmpl-...",
  "model": "anthropic:claude-3-opus",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "..."
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 150,
    "completion_tokens": 300,
    "total_tokens": 450
  }
}
```

---

## Performance Characteristics

### Latency Targets by Intent

| Intent | Target Latency | Typical Provider | Strategy |
|--------|----------------|------------------|----------|
| code | < 1s | Ollama (local) | single |
| tests | < 2s | Ollama/GPT-4 | single |
| regex | < 500ms | Ollama | single |
| analysis | < 3s | Claude/GPT-4 | single |
| explanation | < 2s | Any | race |
| general | < 3s | Auto-select | single |

### Cost Optimization

**Monthly Budget Allocation (Example: $150/mo):**
- Anthropic Claude: $70 (47%)
- OpenAI GPT-4: $70 (47%)
- Google Gemini: $10 (6%)
- Ollama (local): $0 (unlimited)

**Smart Routing Reduces Cost:**
- 60% of code/test requests → Ollama (free)
- 30% of requests → Gemini ($0.00125/1k, cheapest cloud)
- 10% of requests → Claude/GPT-4 (quality-critical)

**Estimated Savings:** ~40% vs. always using GPT-4

---

## Integration with Ghost Stack

### Zeke.nvim

```lua
-- ~/.config/nvim/init.lua
require('zeke').setup({
  provider = {
    type = "omen",
    endpoint = "http://localhost:8080/v1",
    model = "auto",
    intent_hints = {
      code_completion = "code",
      code_explanation = "explanation",
      test_generation = "tests",
    }
  }
})
```

### Jarvis (Agent Runtime)

```toml
# jarvis.toml
[ai.provider]
type = "omen"
endpoint = "http://localhost:8080/v1"
routing = "smart"

[ai.routing]
code_tasks = ["ollama"]
reasoning_tasks = ["anthropic", "openai"]
budget_per_task_usd = 0.05
```

### GhostFlow (Workflow Engine)

```yaml
# workflow.yml
steps:
  - name: generate_code
    type: ai_task
    provider: omen
    intent: code
    model: auto
    prompt: "Generate Rust function for {{feature}}"

  - name: review_code
    type: ai_task
    provider: omen
    intent: analysis
    model: auto
    prompt: "Review this code for issues: {{generated_code}}"
```

---

## Troubleshooting

### OMEN Tool Not Available

**Issue:** `bolt_omen_chat` tool not showing up in MCP server

**Solutions:**
1. Verify OMEN feature is enabled:
   ```bash
   bolt --version  # Should show "omen" in features
   ```

2. Rebuild with OMEN feature:
   ```bash
   cargo build --features "mcp,omen" --release
   ```

3. Check MCP server logs:
   ```bash
   bolt mcp serve --verbose
   ```

### Provider Connectivity Issues

**Issue:** Provider health checks failing

**Solutions:**
1. Test provider directly:
   ```bash
   bolt mcp omen test openai
   ```

2. Check API keys:
   ```bash
   echo $OPENAI_API_KEY
   echo $ANTHROPIC_API_KEY
   ```

3. Verify network access:
   ```bash
   curl -H "Authorization: Bearer $OPENAI_API_KEY" \
        https://api.openai.com/v1/models
   ```

### High Costs

**Issue:** OMEN routing to expensive providers too often

**Solutions:**
1. Increase local preference:
   ```toml
   [routing]
   prefer_local_for = ["code", "tests", "regex", "explanation"]
   ```

2. Set stricter budgets:
   ```toml
   [routing]
   budget_monthly_usd = 50
   soft_limits = { anthropic = 20, openai = 20 }
   ```

3. Enable more local models:
   ```toml
   [providers.ollama]
   models = [
     "deepseek-coder:6.7b",
     "llama3.1:8b-instruct",
     "qwen2.5:7b-instruct",
     "mistral:7b-instruct"
   ]
   ```

---

## Roadmap

### Phase 3.1: Streaming Support
- WebSocket streaming for real-time AI responses
- SSE (Server-Sent Events) for HTTP streaming
- Streaming cancellation for race strategies

### Phase 3.2: Advanced Routing
- Reinforcement learning for routing decisions
- User feedback loop (thumbs up/down affects scores)
- Session-based provider stickiness

### Phase 3.3: Rune Integration
- Zig FFI bindings for ultra-fast MCP operations (>3× Rust baseline)
- Sub-millisecond tool invocation latency
- SIMD-accelerated prompt processing

### Phase 3.4: Multi-Container AI Orchestration
- Federate OMEN across multiple Bolt containers
- Shared provider pool with load balancing
- Distributed rate limiting and billing

---

## Next Steps

- **Phase 4: Rune Integration** - Ultra-fast Zig-powered MCP operations
- **Phase 5: Ghost Stack Full Integration** - Zeke, Jarvis, GhostFlow native support
- **Phase 6: Autonomous Container Management** - AI agents managing container lifecycle

---

**Questions or issues?**
- GitHub: https://github.com/CK-Technology/bolt/issues
- Discord: https://discord.gg/ghoststack
- Email: ghostkellz@proton.me
