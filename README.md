# Nginx Sidecar Proxy with Plugin Framework

A robust nginx sidecar proxy with a dynamic plugin framework similar to IBM Context Forge. Plugins are loaded from a Kubernetes ConfigMap and execute in priority order based on MCP protocol hooks.

## Features

- **Dynamic Plugin Loading**: Load plugins from ConfigMap-mounted YAML/JSON configuration
- **MCP Protocol Hooks**: Support for `tool_pre_invoke`, `prompt_pre_fetch`, and other MCP lifecycle hooks
- **Priority-based Execution**: Plugins execute in priority order (lower number = earlier execution)
- **Multiple Execution Modes**: `enforce`, `monitor`, and `warn` modes for flexible policy enforcement
- **Kubernetes Native**: Designed for sidecar deployment with ConfigMap integration

## Architecture

```
Client Request
    ↓
Nginx (njs filter)
    ↓
Hook Dispatcher (identifies MCP hook)
    ↓
Plugin Executor (runs plugins in priority order)
    ↓
Plugins (PII Guard, SQL Sanitizer, etc.)
    ↓
Upstream MCP Server
```

## Installation

### Prerequisites

- Nginx with `ngx_http_js_module` enabled
- Node.js (for NJS plugin development)
- Bash shell (for setup scripts)

### Quick Start

1. **Clone the repository**:
   ```bash
   git clone <repository-url>
   cd plugins_ai-guardrails
   ```

2. **Set up environment variables**:
   
   Create a `.env` file in the project root with your base path:
   ```bash
   echo "BASEPATH=/path/to/your/nginx-dev" > .env
   ```
   
   Or if `.env.example` exists, copy and edit it:
   ```bash
   cp .env.example .env
   # Edit .env and set your BASEPATH
   ```
   
   The `.env` file should contain:
   ```bash
   BASEPATH=/path/to/your/nginx-dev
   ```

3. **Configure nginx paths**:
   
   **Option A: Use the example template** (recommended for new installations):
   ```bash
   cp conf/example.nginx.conf conf/nginx.conf
   ./scripts/devenv.sh
   ```
   
   **Option B: Update existing nginx.conf**:
   ```bash
   ./scripts/devenv.sh
   ```
   
   The `devenv.sh` script will:
   - Read `BASEPATH` from your `.env` file
   - Replace `{{BASEPATH}}` placeholders in `nginx.conf` with your actual path
   - Create a timestamped backup before making changes
   - Show you which paths were updated

4. **Verify configuration**:
   ```bash
   # Test nginx configuration syntax
   nginx -t -c conf/nginx.conf
   ```

5. **Start nginx**:
   ```bash
   nginx -c conf/nginx.conf
   ```

### Configuration Files

- **`conf/example.nginx.conf`**: Template file with `{{BASEPATH}}` placeholders for new installations
- **`conf/nginx.conf`**: Main development configuration (populated from `.env`)
- **`conf/nginx.conf.prod`**: Production configuration with standard Linux paths (`/var/run`, `/etc/nginx`, etc.)

### Environment Setup

The project uses a `.env` file to manage paths dynamically. The `devenv.sh` script reads the `BASEPATH` variable and updates all paths in `nginx.conf`.

**Example `.env` file**:
```bash
# Base path for nginx configuration
# All paths in nginx.conf will be relative to this base path
BASEPATH=/home/user/my-project/nginx-dev
```

**Directory structure** (relative to BASEPATH):
```
nginx-dev/
├── logs/          # nginx.pid, error.log, access.log
├── njs/           # NJS plugin scripts
└── config/        # Plugin configuration (config.json, config.yaml)
```

### Updating Paths

If you need to change your base path:

1. Update `BASEPATH` in `.env`
2. Run `./scripts/devenv.sh` to update all paths in `nginx.conf`

The script is idempotent - it will skip updates if paths already match your `BASEPATH`.

### Production Deployment

For production, use `conf/nginx.conf.prod` which uses standard Linux paths:
- PID: `/var/run/nginx.pid`
- Logs: `/var/log/nginx/`
- NJS: `/etc/nginx/njs/`
- Config: `/etc/nginx/plugins/`

These paths are standard and typically don't need customization. If your deployment differs, edit `nginx.conf.prod` directly.

## Plugin Configuration

Plugins are configured via a YAML or JSON file mounted as a ConfigMap. Example:

```yaml
- name: "SQLSanitizer"
  kind: "plugins.sql_sanitizer"
  hooks: ["tool_pre_invoke"]
  mode: "enforce"
  priority: 40
  config:
    fields: ["sql", "query", "statement"]
    strip_comments: true
    block_delete_without_where: true
    block_update_without_where: true
    require_parameterization: false
    blocked_statements: ["\\bDROP\\b", "\\bTRUNCATE\\b", "\\bALTER\\b"]
    block_on_violation: true

- name: "PIIGuard"
  kind: "plugins.pii"
  hooks: ["tool_pre_invoke", "prompt_pre_fetch"]
  mode: "monitor"
  priority: 10
  config: {}
```

### Configuration Fields

- **name**: Unique plugin name
- **kind**: NJS module path (e.g., `plugins.sql_sanitizer` maps to `plugins/sql_sanitizer.js`)
- **hooks**: Array of hooks this plugin should handle
- **mode**: Execution mode (`enforce`, `monitor`, or `warn`)
- **priority**: Execution priority (lower number = earlier execution)
- **config**: Plugin-specific configuration object

### Execution Modes

- **enforce**: Block request if plugin returns `allow: false`
- **monitor**: Log violations but allow request
- **warn**: Log warnings but allow request

## Available Hooks

- `tool_pre_invoke`: Triggered before MCP tool calls
- `tool_post_invoke`: Triggered after MCP tool calls (future)
- `prompt_pre_fetch`: Triggered before MCP prompt fetches
- `prompt_post_fetch`: Triggered after MCP prompt fetches (future)
- `resource_pre_fetch`: Triggered before MCP resource fetches
- `resource_post_fetch`: Triggered after MCP resource fetches (future)

## Built-in Plugins

### PII Guard (`plugins.pii`)

Redacts PII (SSN, email, credit card) from request payloads.

**Configuration**: None required

### SQL Sanitizer (`plugins.sql_sanitizer`)

Validates and sanitizes SQL queries in request payloads.

**Configuration Options**:
- `fields`: Array of field names to scan (null = scan all string args)
- `strip_comments`: Remove SQL comments (default: true)
- `block_delete_without_where`: Block DELETE without WHERE (default: true)
- `block_update_without_where`: Block UPDATE without WHERE (default: true)
- `require_parameterization`: Require parameterized queries (default: false)
- `blocked_statements`: Array of regex patterns for blocked statements
- `block_on_violation`: Block request on violation (default: true)

## Creating Custom Plugins

Plugins must export hook handler functions. Example:

```javascript
// plugins/my_plugin.js
function onToolPreInvoke(context) {
    var r = context.r;
    var body = context.body;
    var config = this.config;
    
    // Process request
    var modifiedBody = processBody(body);
    
    // Return result
    return {
        allow: true,
        modifiedBody: modifiedBody,
        error: null,
        metadata: {}
    };
}

export default {
    onToolPreInvoke
};
```

### Plugin Interface

Plugins receive a `context` object:
- `r`: Nginx request object
- `method`: MCP method name
- `params`: MCP request parameters
- `body`: Request body (string)
- `metadata`: Additional metadata

Plugins return a result object:
- `allow`: Boolean indicating if request should proceed
- `modifiedBody`: Modified request body (optional)
- `error`: Error message if blocking (optional)
- `metadata`: Additional metadata (optional)

## Deployment

### Kubernetes Deployment

1. **Create ConfigMap**:
```bash
kubectl create configmap nginx-plugin-config \
  --from-file=config.yaml=nginx-dev/k8s/configmap.yaml
```

2. **Deploy**:
```bash
kubectl apply -f nginx-dev/k8s/
```

### Docker Build

```bash
cd nginx-dev
docker build -t nginx-sidecar-proxy:latest .
```

### Local Development

1. **Set up your environment** (if not already done):
   ```bash
   # Ensure .env is configured
   cat .env  # Should show BASEPATH=/path/to/your/nginx-dev
   
   # Ensure nginx.conf paths are populated
   ./scripts/devenv.sh
   ```

2. **Start nginx**:
   ```bash
   # Using nginx directly
   nginx -c conf/nginx.conf
   
   # Or using Docker Compose (if available)
   docker-compose up
   ```

3. **Update config**: 
   - Edit `config/config.json` or `config/config.yaml`
   - Reload nginx: `nginx -s reload -c conf/nginx.conf`

4. **View logs**:
   ```bash
   # Error logs
   tail -f $BASEPATH/logs/error.log
   
   # Access logs
   tail -f $BASEPATH/logs/access.log
   ```

## Configuration File Format

The config file can be in YAML or JSON format. JSON is recommended for better njs compatibility.

### Converting YAML to JSON

Use the provided script:
```bash
python3 scripts/yaml-to-json.py k8s/configmap.yaml config.json
```

## GitHub Actions

The repository includes CI/CD workflows:

- **CI** (`.github/workflows/ci.yml`): Builds and validates the Docker image
- **CD** (`.github/workflows/cd.yml`): Builds, pushes, and deploys to Kubernetes

## Troubleshooting

### Path Configuration Issues

- **Paths not updating**: Ensure `.env` file exists and contains `BASEPATH=/your/path`
- **Script fails**: Check that `scripts/devenv.sh` is executable: `chmod +x scripts/devenv.sh`
- **Wrong paths in nginx.conf**: Run `./scripts/devenv.sh` to update paths from `.env`
- **Placeholders still present**: The script replaces `{{BASEPATH}}` - ensure your config uses these placeholders

### Config not loading

- **Local development**: Check that config file exists at `$BASEPATH/config/config.json` or `config.yaml`
- **Kubernetes**: Check that ConfigMap is mounted at `/etc/nginx/plugins/config.yaml`
- Verify nginx can read the file (check permissions)
- Check nginx error logs for parsing errors: `tail -f $BASEPATH/logs/error.log`

### Plugins not executing

- Verify plugin `kind` matches actual module path
- Check that plugins are registered in `filter.js`
- Ensure hooks are correctly identified by hook dispatcher
- Verify plugin files exist in `$BASEPATH/njs/plugins/` directory

### Plugin errors

- Check nginx error logs for detailed error messages: `tail -f $BASEPATH/logs/error.log`
- Verify plugin interface matches expected format
- Test plugin in isolation if possible
- Ensure NJS module is loaded: Check `load_module` directive in nginx.conf

## License

[Your License Here]

