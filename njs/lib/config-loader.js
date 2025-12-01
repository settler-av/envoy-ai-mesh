import yamlParser from './yaml-parser.js';
// Import fs module for file reading (njs 0.7.0+)
import fs from 'fs';

var CONFIG_PATH = '/etc/nginx/plugins/config.yaml';
var CONFIG_PATH_JSON = '/etc/nginx/plugins/config.json';
var cachedConfig = null;
var configLoadError = null;

// Known hooks
var VALID_HOOKS = [
    'tool_pre_invoke',
    'tool_post_invoke',
    'prompt_pre_fetch',
    'prompt_post_fetch',
    'resource_pre_fetch',
    'resource_post_fetch'
];

// Valid modes
var VALID_MODES = ['enforce', 'monitor', 'warn'];

/**
 * Read file content (simplified - njs doesn't have fs module)
 * This will need to be implemented using nginx's file reading capabilities
 * For now, we'll use a workaround with subrequest or expect the file to be
 * pre-loaded into a variable
 */
function readFile(path) {
    // NJS doesn't have direct file I/O
    // We'll need to use nginx's file reading or load via subrequest
    // For now, return null and handle in loadConfig
    return null;
}

/**
 * Load and parse config from file
 * Uses fs.readFileSync for synchronous file reading (njs 0.7.0+)
 */
function loadConfig(r) {
    try {
        var configText = null;
        
        // For local dev, use absolute path
        var devConfigPathJSON = '/home/mnf483/cloud-ex/tinkering/mcp-guard-nginx/nginx-dev/config/config.json';
        var devConfigPathYAML = '/home/mnf483/cloud-ex/tinkering/mcp-guard-nginx/nginx-dev/config/config.yaml';
        
        // Try reading JSON config first (preferred format)
        var configPaths = [
            devConfigPathJSON,      // Local dev
            CONFIG_PATH_JSON,       // Production/k8s
            devConfigPathYAML,      // Local dev YAML
            CONFIG_PATH             // Production/k8s YAML
        ];
        
        for (var i = 0; i < configPaths.length; i++) {
            try {
                configText = fs.readFileSync(configPaths[i], 'utf8');
                if (configText) {
                    r.error("✓ Config loaded from: " + configPaths[i] + " (" + configText.length + " bytes)");
                    break;
                }
            } catch (e) {
                // File not found or read error, try next path
                if (i === configPaths.length - 1) {
                    // Last attempt failed
                    r.error("⚠️  Could not read config from any path: " + e);
                }
            }
        }
        
        if (!configText) {
            r.error("⚠️  Config file not accessible, using default empty config");
            r.error("   Tried paths: " + configPaths.join(', '));
            return [];
        }
        
        // Try to parse as JSON first (faster and more reliable)
        var config = null;
        try {
            config = JSON.parse(configText);
            r.error("✓ Config parsed as JSON");
        } catch (e) {
            // Not JSON, try YAML
            try {
                config = yamlParser.parseYAML(configText);
                r.error("✓ Config parsed as YAML");
            } catch (e2) {
                r.error("❌ Failed to parse config as JSON or YAML: " + e2);
                configLoadError = "Config parsing failed: " + e2;
                cachedConfig = [];
                return [];
            }
        }
        
        // Validate and cache
        var validated = validateConfig(config, r);
        cachedConfig = validated;
        
        return validated;
        
    } catch (e) {
        r.error("❌ Error loading config: " + e);
        configLoadError = e;
        cachedConfig = [];
        return [];
    }
}

/**
 * Load config from string (for testing or direct injection)
 */
function loadConfigFromString(configText, r) {
    try {
        // Try JSON first, then YAML
        var config = null;
        try {
            config = JSON.parse(configText);
        } catch (e) {
            config = yamlParser.parseYAML(configText);
        }
        return validateConfig(config, r);
    } catch (e) {
        if (r) {
            r.error("❌ Error parsing config: " + e);
        }
        configLoadError = e;
        return [];
    }
}

/**
 * Validate plugin configuration
 */
function validateConfig(config, r) {
    if (!Array.isArray(config)) {
        var error = "Config must be an array of plugins";
        if (r) r.error("❌ " + error);
        configLoadError = error;
        return [];
    }
    
    var validated = [];
    var errors = [];
    
    for (var i = 0; i < config.length; i++) {
        var plugin = config[i];
        
        // Validate required fields
        if (!plugin.name) {
            errors.push("Plugin at index " + i + " missing 'name' field");
            continue;
        }
        
        if (!plugin.kind) {
            errors.push("Plugin '" + plugin.name + "' missing 'kind' field");
            continue;
        }
        
        if (!plugin.hooks || !Array.isArray(plugin.hooks) || plugin.hooks.length === 0) {
            errors.push("Plugin '" + plugin.name + "' missing or invalid 'hooks' field");
            continue;
        }
        
        if (plugin.mode === undefined || !VALID_MODES.includes(plugin.mode)) {
            errors.push("Plugin '" + plugin.name + "' has invalid 'mode'. Must be one of: " + VALID_MODES.join(', '));
            continue;
        }
        
        if (plugin.priority === undefined || typeof plugin.priority !== 'number') {
            errors.push("Plugin '" + plugin.name + "' missing or invalid 'priority' field (must be a number)");
            continue;
        }
        
        // Validate hooks
        var invalidHooks = [];
        for (var j = 0; j < plugin.hooks.length; j++) {
            if (!VALID_HOOKS.includes(plugin.hooks[j])) {
                invalidHooks.push(plugin.hooks[j]);
            }
        }
        if (invalidHooks.length > 0) {
            errors.push("Plugin '" + plugin.name + "' has invalid hooks: " + invalidHooks.join(', ') + ". Valid hooks: " + VALID_HOOKS.join(', '));
            continue;
        }
        
        // Config field is optional but should be an object if present
        if (plugin.config !== undefined && typeof plugin.config !== 'object') {
            errors.push("Plugin '" + plugin.name + "' has invalid 'config' field (must be an object)");
            continue;
        }
        
        // All validations passed
        validated.push(plugin);
    }
    
    if (errors.length > 0) {
        var errorMsg = "Config validation errors:\n" + errors.join('\n');
        if (r) {
            r.error("⚠️  " + errorMsg);
        }
        configLoadError = errorMsg;
    } else {
        configLoadError = null;
        if (r) {
            r.error("✓ Loaded " + validated.length + " plugin(s) from config");
        }
    }
    
    return validated;
}

/**
 * Get cached config (if available)
 */
function getCachedConfig() {
    return cachedConfig;
}

/**
 * Get last config load error
 */
function getConfigError() {
    return configLoadError;
}

export default {
    loadConfig,
    loadConfigFromString,
    validateConfig,
    getCachedConfig,
    getConfigError,
    CONFIG_PATH,
    VALID_HOOKS,
    VALID_MODES
};

