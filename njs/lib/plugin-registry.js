import configLoader from './config-loader.js';

var pluginRegistry = {
    plugins: [],
    hooks: {}, // Map of hook name -> array of plugins (sorted by priority)
    initialized: false
};

/**
 * Convert kind path to module import path
 * kind: "plugins.sql_sanitizer" -> "plugins/sql_sanitizer.js"
 */
function kindToModulePath(kind) {
    // Replace dots with slashes and add .js extension
    var path = kind.replace(/\./g, '/');
    if (!path.endsWith('.js')) {
        path += '.js';
    }
    return path;
}

/**
 * Dynamically import a plugin module
 */
function importPlugin(kind, r) {
    try {
        var modulePath = kindToModulePath(kind);
        r.error("📦 Attempting to import plugin from: " + modulePath);
        
        // Use dynamic import - njs supports import() but with limitations
        // For njs, we need to use static imports or require-like mechanism
        // Since njs doesn't support dynamic imports, we'll need to use
        // a different approach - pre-register known plugins or use eval
        
        // Alternative: Use a plugin loader that maps kinds to actual imports
        // For now, we'll try to import and handle errors gracefully
        try {
            // NJS doesn't support dynamic import(), so we'll need to
            // use a static import map or require plugins to be pre-loaded
            // This is a limitation we'll work around
            
            // Return a placeholder that will be resolved later
            return {
                kind: kind,
                modulePath: modulePath,
                loaded: false
            };
        } catch (e) {
            r.error("❌ Failed to import plugin module: " + e);
            return null;
        }
    } catch (e) {
        r.error("❌ Error importing plugin: " + e);
        return null;
    }
}

/**
 * Initialize plugin with config
 */
function initializePlugin(pluginModule, pluginConfig, r) {
    try {
        // Plugin module should export a factory function or default object
        var plugin = null;
        
        if (typeof pluginModule === 'function') {
            // Factory function
            plugin = pluginModule(pluginConfig.config || {});
        } else if (pluginModule && typeof pluginModule === 'object') {
            // Plugin object - create instance with config
            plugin = {
                name: pluginConfig.name,
                hooks: pluginConfig.hooks,
                priority: pluginConfig.priority,
                mode: pluginConfig.mode,
                config: pluginConfig.config || {}
            };
            
            // Copy hook handlers from module
            for (var i = 0; i < pluginConfig.hooks.length; i++) {
                var hook = pluginConfig.hooks[i];
                var handlerName = 'on' + hook.split('_').map(function(part) {
                    return part.charAt(0).toUpperCase() + part.slice(1);
                }).join('');
                
                if (pluginModule[handlerName]) {
                    plugin[handlerName] = pluginModule[handlerName].bind(pluginModule);
                } else if (pluginModule[hook]) {
                    plugin[handlerName] = pluginModule[hook].bind(pluginModule);
                } else if (pluginModule.process) {
                    // Fallback to generic process method
                    plugin[handlerName] = function(context) {
                        return pluginModule.process(context);
                    };
                }
            }
        } else {
            r.error("❌ Invalid plugin module format for: " + pluginConfig.name);
            return null;
        }
        
        // Ensure plugin has required properties
        plugin.name = pluginConfig.name;
        plugin.hooks = pluginConfig.hooks;
        plugin.priority = pluginConfig.priority;
        plugin.mode = pluginConfig.mode;
        plugin.config = pluginConfig.config || {};
        
        return plugin;
        
    } catch (e) {
        r.error("❌ Error initializing plugin '" + pluginConfig.name + "': " + e);
        return null;
    }
}

/**
 * Register a plugin for its hooks
 */
function registerPlugin(plugin) {
    if (!plugin || !plugin.hooks) {
        return false;
    }
    
    for (var i = 0; i < plugin.hooks.length; i++) {
        var hook = plugin.hooks[i];
        
        if (!pluginRegistry.hooks[hook]) {
            pluginRegistry.hooks[hook] = [];
        }
        
        pluginRegistry.hooks[hook].push(plugin);
    }
    
    // Sort hooks by priority (lower number = earlier execution)
    for (var hookName in pluginRegistry.hooks) {
        pluginRegistry.hooks[hookName].sort(function(a, b) {
            return a.priority - b.priority;
        });
    }
    
    return true;
}

/**
 * Load and register plugins from config
 * Since njs doesn't support dynamic imports, we use a plugin map
 */
var pluginMap = {}; // Will be populated with actual plugin imports

/**
 * Register a plugin module in the map (called from filter.js or plugin files)
 */
function registerPluginModule(kind, module) {
    pluginMap[kind] = module;
}

/**
 * Load plugins from config
 */
function loadPlugins(config, r) {
    if (!config || !Array.isArray(config)) {
        r.error("⚠️  Invalid config provided to loadPlugins");
        return;
    }
    
    pluginRegistry.plugins = [];
    pluginRegistry.hooks = {};
    
    for (var i = 0; i < config.length; i++) {
        var pluginConfig = config[i];
        
        r.error("🔌 Loading plugin: " + pluginConfig.name + " (kind: " + pluginConfig.kind + ")");
        
        // Get plugin module from map
        var pluginModule = pluginMap[pluginConfig.kind];
        
        if (!pluginModule) {
            r.error("⚠️  Plugin module not found for kind: " + pluginConfig.kind);
            r.error("   Available kinds: " + Object.keys(pluginMap).join(', '));
            continue;
        }
        
        // Initialize plugin
        var plugin = initializePlugin(pluginModule, pluginConfig, r);
        
        if (!plugin) {
            r.error("⚠️  Failed to initialize plugin: " + pluginConfig.name);
            continue;
        }
        
        // Register plugin
        if (registerPlugin(plugin)) {
            pluginRegistry.plugins.push(plugin);
            r.error("✓ Registered plugin: " + plugin.name + " for hooks: " + plugin.hooks.join(', '));
        } else {
            r.error("⚠️  Failed to register plugin: " + pluginConfig.name);
        }
    }
    
    pluginRegistry.initialized = true;
    r.error("✓ Plugin registry initialized with " + pluginRegistry.plugins.length + " plugin(s)");
}

/**
 * Get plugins for a specific hook
 */
function getPluginsForHook(hook) {
    return pluginRegistry.hooks[hook] || [];
}

/**
 * Get all registered plugins
 */
function getAllPlugins() {
    return pluginRegistry.plugins;
}

/**
 * Check if registry is initialized
 */
function isInitialized() {
    return pluginRegistry.initialized;
}

/**
 * Reset registry (for testing or reload)
 */
function reset() {
    pluginRegistry.plugins = [];
    pluginRegistry.hooks = {};
    pluginRegistry.initialized = false;
}

export default {
    loadPlugins,
    getPluginsForHook,
    getAllPlugins,
    isInitialized,
    registerPluginModule,
    reset,
    pluginMap
};

