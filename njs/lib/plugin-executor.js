import pluginRegistry from './plugin-registry.js';

/**
 * Execute plugins for a hook in priority order
 * Returns: { allow: boolean, modifiedBody: string, error: string, metadata: object }
 */
function executePlugins(hook, context, r) {
    if (!hook || !context) {
        return {
            allow: true,
            modifiedBody: context ? context.body : '',
            error: null,
            metadata: {}
        };
    }
    
    // Get plugins for this hook (already sorted by priority)
    var plugins = pluginRegistry.getPluginsForHook(hook);
    
    if (plugins.length === 0) {
        r.error("ℹ️  No plugins registered for hook: " + hook);
        return {
            allow: true,
            modifiedBody: context.body,
            error: null,
            metadata: {}
        };
    }
    
    r.error("🔌 Executing " + plugins.length + " plugin(s) for hook: " + hook);
    
    var currentBody = context.body;
    var metadata = context.metadata || {};
    var blocked = false;
    var blockError = null;
    var warnings = [];
    
    // Execute plugins in priority order
    for (var i = 0; i < plugins.length; i++) {
        var plugin = plugins[i];
        
        // Skip if already blocked by enforce mode plugin
        if (blocked && plugin.mode === 'enforce') {
            continue;
        }
        
        try {
            r.error("  → Executing plugin: " + plugin.name + " (priority: " + plugin.priority + ", mode: " + plugin.mode + ")");
            
            // Get handler function name
            var handlerName = 'on' + hook.split('_').map(function(part) {
                return part.charAt(0).toUpperCase() + part.slice(1);
            }).join('');
            
            // Call plugin handler
            var handler = plugin[handlerName];
            if (!handler && plugin[hook]) {
                handler = plugin[hook];
            }
            if (!handler && plugin.process) {
                // Fallback to generic process
                handler = function(ctx) {
                    return plugin.process(ctx);
                };
            }
            
            if (!handler) {
                r.error("  ⚠️  Plugin '" + plugin.name + "' has no handler for hook: " + hook);
                continue;
            }
            
            // Update context with current body
            var pluginContext = {
                r: context.r,
                method: context.method,
                params: context.params,
                body: currentBody,
                metadata: metadata
            };
            
            // Execute handler
            var result = handler.call(plugin, pluginContext);
            
            // Normalize result
            var normalizedResult = normalizeResult(result, pluginContext);
            
            // Handle result
            if (normalizedResult.allow === false) {
                if (plugin.mode === 'enforce') {
                    // Block immediately
                    blocked = true;
                    blockError = normalizedResult.error || "Request blocked by plugin: " + plugin.name;
                    r.error("  🚫 Plugin '" + plugin.name + "' blocked request (enforce mode)");
                    r.error("     Reason: " + blockError);
                    break; // Stop execution
                } else if (plugin.mode === 'warn') {
                    warnings.push("Plugin '" + plugin.name + "': " + (normalizedResult.error || "Request would be blocked"));
                    r.error("  ⚠️  Plugin '" + plugin.name + "' would block request (warn mode)");
                    // Continue execution
                } else if (plugin.mode === 'monitor') {
                    r.error("  👁️  Plugin '" + plugin.name + "' detected violation (monitor mode)");
                    // Continue execution
                }
            }
            
            // Update body if modified
            if (normalizedResult.modifiedBody !== undefined && normalizedResult.modifiedBody !== currentBody) {
                currentBody = normalizedResult.modifiedBody;
                r.error("  ✏️  Plugin '" + plugin.name + "' modified request body");
            }
            
            // Merge metadata
            if (normalizedResult.metadata) {
                for (var key in normalizedResult.metadata) {
                    metadata[key] = normalizedResult.metadata[key];
                }
            }
            
        } catch (e) {
            r.error("  ❌ Error executing plugin '" + plugin.name + "': " + e);
            
            // If enforce mode and error, block
            if (plugin.mode === 'enforce') {
                blocked = true;
                blockError = "Plugin execution error: " + e;
                r.error("  🚫 Blocking due to enforce mode plugin error");
                break;
            }
            
            // Otherwise, log and continue
            warnings.push("Plugin '" + plugin.name + "' error: " + e);
        }
    }
    
    // Build final result
    var finalResult = {
        allow: !blocked,
        modifiedBody: currentBody,
        error: blockError,
        warnings: warnings.length > 0 ? warnings : null,
        metadata: metadata
    };
    
    if (blocked) {
        r.error("🚫 Request blocked by plugin framework");
    } else if (warnings.length > 0) {
        r.error("⚠️  " + warnings.length + " warning(s) from plugins");
    } else {
        r.error("✓ All plugins executed successfully");
    }
    
    return finalResult;
}

/**
 * Normalize plugin result to standard format
 */
function normalizeResult(result, context) {
    // If result is a string, treat as modified body
    if (typeof result === 'string') {
        return {
            allow: true,
            modifiedBody: result,
            error: null,
            metadata: {}
        };
    }
    
    // If result is boolean, treat as allow flag
    if (typeof result === 'boolean') {
        return {
            allow: result,
            modifiedBody: context.body,
            error: result ? null : "Request blocked",
            metadata: {}
        };
    }
    
    // If result is object, ensure it has required fields
    if (result && typeof result === 'object') {
        return {
            allow: result.allow !== undefined ? result.allow : true,
            modifiedBody: result.modifiedBody !== undefined ? result.modifiedBody : context.body,
            error: result.error || null,
            metadata: result.metadata || {}
        };
    }
    
    // Default: allow
    return {
        allow: true,
        modifiedBody: context.body,
        error: null,
        metadata: {}
    };
}

export default {
    executePlugins,
    normalizeResult
};

