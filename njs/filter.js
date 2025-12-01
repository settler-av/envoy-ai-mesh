import router from './lib/router.js';
import configLoader from './lib/config-loader.js';
import pluginRegistry from './lib/plugin-registry.js';
import hookDispatcher from './lib/hook-dispatcher.js';
import pluginExecutor from './lib/plugin-executor.js';

// Import plugins and register them
import piiPlugin from './plugins/pii.js';
import sqlSanitizerPlugin from './plugins/sql_sanitizer.js';

// Register plugins in the plugin map
pluginRegistry.registerPluginModule('plugins.pii', piiPlugin);
pluginRegistry.registerPluginModule('plugins.sql_sanitizer', sqlSanitizerPlugin);

// Cache for config (loaded once at startup)
var configLoaded = false;

/**
 * Initialize plugin framework (load config and register plugins)
 */
function initializeFramework(r) {
    if (configLoaded) {
        return; // Already initialized
    }
    
    try {
        r.error("🚀 Initializing plugin framework...");
        
        // Load config
        var config = configLoader.loadConfig(r);
        
        if (config && config.length > 0) {
            // Load plugins from config
            pluginRegistry.loadPlugins(config, r);
            configLoaded = true;
            r.error("✓ Plugin framework initialized");
        } else {
            r.error("⚠️  No plugins configured, using default empty configuration");
            configLoaded = true;
        }
        
    } catch (e) {
        r.error("❌ Error initializing plugin framework: " + e);
        configLoaded = false;
    }
}

function handleRequest(r) {
    try {
        r.error("📩 Request received - Method: " + r.method + ", URI: " + r.uri);
        
        // Initialize framework on first request (if not already done)
        if (!configLoaded) {
            initializeFramework(r);
        }
        
        // Read request body
        var body = router.readBody(r);
        
        // Check if this is an MCP request
        if (!hookDispatcher.isMCPRequest(body)) {
            // Not an MCP request, forward as-is
            r.error("ℹ️  Not an MCP request, forwarding without plugin processing");
            router.forward(r, body);
            return;
        }
        
        // Dispatch hook from request
        var dispatchResult = hookDispatcher.dispatch(r, body);
        
        if (!dispatchResult || !dispatchResult.hook) {
            // No hook identified, forward as-is
            r.error("ℹ️  No hook identified for request, forwarding without plugin processing");
            router.forward(r, body);
            return;
        }
        
        r.error("🎣 Hook identified: " + dispatchResult.hook);
        
        // Execute plugins for this hook
        var executionResult = pluginExecutor.executePlugins(
            dispatchResult.hook,
            dispatchResult.context,
            r
        );
        
        // Handle execution result
        if (!executionResult.allow) {
            // Request blocked
            r.error("🚫 Request blocked: " + (executionResult.error || "Unknown reason"));
            
            // Return error response
            var errorResponse = {
                jsonrpc: "2.0",
                id: dispatchResult.request.id || null,
                error: {
                    code: -32000,
                    message: "Request blocked by plugin framework",
                    data: {
                        reason: executionResult.error,
                        hook: dispatchResult.hook,
                        warnings: executionResult.warnings
                    }
                }
            };
            
            r.return(403, JSON.stringify(errorResponse));
            return;
        }
        
        // Request allowed, forward with potentially modified body
        if (executionResult.warnings && executionResult.warnings.length > 0) {
            r.error("⚠️  Warnings: " + executionResult.warnings.join('; '));
        }
        
        if (executionResult.modifiedBody !== body) {
            r.error("✏️  Body modified by plugins, forwarding modified version");
        } else {
            r.error("✓ No changes by plugins - forwarding original");
        }
        
        router.forward(r, executionResult.modifiedBody);
        
    } catch (e) {
        r.error("❌ ERROR: " + e);
        r.error("Stack: " + (e.stack || "No stack trace"));
        r.return(500, JSON.stringify({
            jsonrpc: "2.0",
            id: null,
            error: {
                code: -32603,
                message: "Internal error",
                data: "NJS Error: " + e
            }
        }));
    }
}

/**
 * Read config file content (called by nginx to populate js_set variable)
 * This function is synchronous and reads the config file at nginx startup
 */
function getConfigContent(r) {
    try {
        // Try to read config.json first (preferred format)
        var configPath = '/home/mnf483/cloud-ex/tinkering/mcp-guard-nginx/nginx-dev/config/config.json';
        
        // For production, use: '/etc/nginx/plugins/config.json'
        // For ConfigMap: the path will be mounted by k8s
        
        // NJS 0.7.0+ supports fs.readFileSync
        // For older versions, we need to rely on nginx serving the file
        // and read it via subrequest callback (async)
        
        // For now, return empty string - config will be served via internal location
        // and read using the alternative approach below
        return '';
        
    } catch (e) {
        r.error("❌ Error reading config file: " + e);
        return '';
    }
}

export default { handleRequest, getConfigContent };
