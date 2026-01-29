# AI-Guard Development Guidelines

## Project Philosophy

AI-Guard implements the **Distributed Interceptor Pattern** for headless AI governance,
following the **A2AS BASIC Security Model** for agentic AI runtime security.

### What is Headless AI Governance?

Traditional centralized AI gateways create "hairpinning" - internal Agent-to-Agent (A2A) traffic must route through a shared ingress and back, adding latency and creating a single point of failure. Headless AI Governance moves governance logic from the gateway directly to the workload using Envoy sidecars and WebAssembly (Wasm).

### Core Principles

1. **Zero-Hop Enforcement** - Governance at the sidecar, no hairpinning to central gateway
2. **Protocol Agnostic** - Support all MCP transports (HTTP, SSE, WebSocket) and A2A bindings (JSONRPC, gRPC, HTTP+JSON)
3. **Defense in Depth** - A2AS BASIC controls at multiple layers
4. **Network Visibility** - Block opaque transports (STDIO), ensure mesh observability
5. **Memory Efficient** - Wasm filters use streaming, not accumulation

---

## Standards Compliance

This implementation follows three key standards:

- **MCP (Model Context Protocol)** - [Specification 2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25)
- **A2A Protocol** - [Agent-to-Agent Protocol](https://a2a-protocol.org/latest/definitions/)
- **A2AS Framework** - [Agentic AI Runtime Security](https://a2as.org/)

---

## A2AS BASIC Security Model

The A2AS framework defines five essential security controls:

| Control | Purpose | Implementation |
|---------|---------|----------------|
| **(B) Behavior Certificates** | Agent capability declaration and enforcement | `wasm-filter/src/a2as/behavior.rs` |
| **(A) Authenticated Prompts** | Context window integrity verification | `wasm-filter/src/a2as/integrity.rs` |
| **(S) Security Boundaries** | Untrusted input isolation | `wasm-filter/src/a2as/boundary.rs` |
| **(I) In-Context Defenses** | Secure model reasoning activation | `wasm-filter/src/a2as/defense.rs` |
| **(C) Codified Policies** | Application-specific rules | `wasm-filter/src/a2as/policy.rs` |

---

## Protocol Support Matrix

| Protocol | Transport | Status | Handler |
|----------|-----------|--------|---------|
| MCP | HTTP | Supported | `wasm-filter/src/protocols/mcp/http.rs` |
| MCP | SSE | Supported | `wasm-filter/src/protocols/mcp/sse.rs` |
| MCP | WebSocket | Supported | `wasm-filter/src/protocols/mcp/websocket.rs` |
| MCP | STDIO | **BLOCKED** | NetworkPolicy + Kyverno |
| A2A | JSONRPC | Supported | `wasm-filter/src/protocols/a2a/validator.rs` |
| A2A | gRPC | Supported | `wasm-filter/src/protocols/a2a/grpc.rs` |
| A2A | HTTP+JSON | Supported | `wasm-filter/src/protocols/a2a/validator.rs` |

---

## Wasm Implementation Guidelines (CRITICAL)

### Memory Management Rules

**Wasm has a linear memory model. These rules MUST be followed:**

1. **Never load entire HTTP body into a String** - Use streaming parser or Ring Buffer
2. **Fixed memory allocation** - Pre-allocate buffers, don't grow dynamically
3. **No file I/O from Wasm** - Load all config from Envoy plugin configuration
4. **Handle UTF-8 boundaries** - Multi-byte characters can split across chunks

### DO NOT:

```rust
// BAD: Accumulating entire body into String
fn on_http_request_body(&mut self, body_size: usize, end_of_stream: bool) -> Action {
    if let Some(body) = self.get_http_request_body(0, body_size) {
        self.body_buffer.extend_from_slice(&body);  // ❌ Memory grows unbounded
    }
    if end_of_stream {
        let body_str = String::from_utf8_lossy(&self.body_buffer);  // ❌ Double memory
        // ... scan ...
    }
}

// BAD: Using regex in Wasm (heavy, can OOM)
let re = Regex::new(r"ignore.*previous").unwrap();  // ❌ Regex is expensive

// BAD: Loading config from file
let config = std::fs::read_to_string("/etc/config.json");  // ❌ No file I/O in Wasm

// BAD: Assuming chunk boundaries align with UTF-8 boundaries
let text = std::str::from_utf8(chunk).unwrap();  // ❌ Will panic on split emoji
```

### DO:

```rust
// GOOD: Stream through ring buffer, constant memory
fn on_http_request_body(&mut self, body_size: usize, end_of_stream: bool) -> Action {
    if let Some(chunk) = self.get_http_request_body(0, body_size) {
        match self.scanner.on_body_chunk(&chunk, end_of_stream) {
            ScanDecision::Block(reason) => {
                self.send_block_response(&reason);
                return Action::Pause;
            }
            ScanDecision::Continue => return Action::Pause,
            ScanDecision::Allow => return Action::Continue,
            ScanDecision::Skip(_) => return Action::Continue,
        }
    }
    Action::Continue
}

// GOOD: Simple FSM pattern matching, O(1) per byte
fn advance(&mut self, byte: u8, pattern: &[u8]) {
    if pattern.get(self.pos) == Some(&byte) {
        self.pos += 1;
    } else {
        self.pos = 0;
    }
}

// GOOD: Load config from Envoy plugin configuration
fn on_configure(&mut self, _: usize) -> bool {
    if let Some(bytes) = self.get_plugin_configuration() {
        self.config = serde_json::from_slice(&bytes).unwrap_or_default();
    }
    true
}

// GOOD: Handle UTF-8 boundaries explicitly
let processed = self.utf8_buffer.process_chunk(chunk);
// Now processed.main is guaranteed valid UTF-8 boundary
```

---

## Kyverno Sidecar Injection

The Headless AI Gateway is automatically injected into pods with the annotation:

```yaml
metadata:
  annotations:
    ai-guard.io/inject: "true"
    ai-guard.io/policy: "default"        # Optional: policy profile
    ai-guard.io/certificate: "agent-v1"  # Optional: behavior certificate
```

**Injection includes:**
1. **Init Container** (`ai-guard-init`) - Configures iptables for transparent interception
2. **Sidecar Container** (`ai-guard-sidecar`) - Envoy proxy with Wasm governance filter
3. **Volumes** - ConfigMaps for Envoy config, Wasm binary, policies, and certificates

---

## Project Structure

```
ai-guard/
├── AGENTS.md                        # This file
├── Makefile                         # ContextForge-style targets
├── README.md                        # Quick start guide
│
├── wasm-filter/                     # Rust Wasm filter
│   ├── src/
│   │   ├── lib.rs                  # Main entry
│   │   ├── config.rs               # Configuration loader (from Envoy)
│   │   ├── streaming/              # Ring buffer, UTF-8 handling
│   │   ├── a2as/                   # A2AS BASIC controls
│   │   ├── protocols/              # MCP & A2A handlers
│   │   └── governance/             # PII, rate limiting, etc.
│
├── envoy/                           # Envoy configurations
├── kubernetes/                      # K8s manifests + Kyverno policies
├── policies/                        # Governance policy examples
├── certificates/                    # Behavior certificate examples
└── demo/                            # Attack vector demos
```

---

## Quick Commands

```bash
# Full build and deploy
make quick-start

# Build Wasm filter only
make build-wasm

# Run tests
make test

# View logs
make logs

# Run demo scenarios
make demo
```

---

## Contributing

1. Follow the Wasm memory guidelines above
2. Use FSM for pattern matching, not regex
3. Load config from Envoy plugin configuration only
4. Handle UTF-8 chunk boundaries with `Utf8Buffer`
5. Test with `make test` before submitting PR

---

## References

- [Envoy Proxy Documentation](https://www.envoyproxy.io/docs/envoy/latest/)
- [proxy-wasm Rust SDK](https://github.com/proxy-wasm/proxy-wasm-rust-sdk)
- [Kyverno Documentation](https://kyverno.io/docs/)
- [MCP Specification](https://modelcontextprotocol.io/specification/2025-11-25)
- [A2A Protocol](https://a2a-protocol.org/)
- [A2AS Framework](https://a2as.org/)
