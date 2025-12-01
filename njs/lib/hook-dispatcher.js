/**
 * Hook Dispatcher
 * Parses MCP JSON-RPC requests to identify hooks and extract context
 */

// MCP method to hook mapping
var METHOD_TO_HOOK = {
    'tools/call': 'tool_pre_invoke',
    'tools/list': null, // No hook for list operations
    'prompts/get': 'prompt_pre_fetch',
    'prompts/list': null,
    'resources/read': 'resource_pre_fetch',
    'resources/list': null
};

/**
 * Parse JSON-RPC request body
 */
function parseRequest(body) {
    try {
        if (!body || typeof body !== 'string') {
            return null;
        }
        
        var request = JSON.parse(body);
        
        // Validate JSON-RPC structure
        if (!request.jsonrpc || request.jsonrpc !== '2.0') {
            return null;
        }
        
        return request;
    } catch (e) {
        // Not JSON or invalid JSON
        return null;
    }
}

/**
 * Identify hook from MCP method
 */
function identifyHook(method) {
    if (!method || typeof method !== 'string') {
        return null;
    }
    
    // Direct mapping
    if (METHOD_TO_HOOK[method]) {
        return METHOD_TO_HOOK[method];
    }
    
    // Pattern matching for future methods
    if (method.startsWith('tools/') && method !== 'tools/list') {
        return 'tool_pre_invoke';
    }
    
    if (method.startsWith('prompts/') && method !== 'prompts/list') {
        return 'prompt_pre_fetch';
    }
    
    if (method.startsWith('resources/') && method !== 'resources/list') {
        return 'resource_pre_fetch';
    }
    
    return null;
}

/**
 * Extract request context
 */
function extractContext(r, request, body) {
    var context = {
        r: r,
        method: request.method || null,
        params: request.params || {},
        body: body,
        id: request.id || null,
        jsonrpc: request.jsonrpc || '2.0',
        metadata: {}
    };
    
    // Extract additional metadata from request
    if (request.method === 'tools/call' && request.params) {
        context.metadata.toolName = request.params.name || null;
        context.metadata.toolArguments = request.params.arguments || {};
    }
    
    if (request.method === 'prompts/get' && request.params) {
        context.metadata.promptName = request.params.name || null;
    }
    
    if (request.method === 'resources/read' && request.params) {
        context.metadata.resourceUri = request.params.uri || null;
    }
    
    // Extract headers
    context.metadata.headers = {};
    for (var key in r.headersIn) {
        context.metadata.headers[key] = r.headersIn[key];
    }
    
    // Extract URI and method
    context.metadata.uri = r.uri;
    context.metadata.httpMethod = r.method;
    
    return context;
}

/**
 * Dispatch hook from request
 * Returns: { hook: string, context: object } or null
 */
function dispatch(r, body) {
    try {
        // Parse request
        var request = parseRequest(body);
        
        if (!request || !request.method) {
            // Not a JSON-RPC request or no method
            return null;
        }
        
        // Identify hook
        var hook = identifyHook(request.method);
        
        if (!hook) {
            // No hook for this method
            return null;
        }
        
        // Extract context
        var context = extractContext(r, request, body);
        
        return {
            hook: hook,
            context: context,
            request: request
        };
        
    } catch (e) {
        r.error("❌ Error in hook dispatcher: " + e);
        return null;
    }
}

/**
 * Check if request is MCP JSON-RPC
 */
function isMCPRequest(body) {
    try {
        var request = parseRequest(body);
        return request !== null && request.method !== undefined;
    } catch (e) {
        return false;
    }
}

/**
 * Get hook name from method (utility function)
 */
function getHookFromMethod(method) {
    return identifyHook(method);
}

export default {
    dispatch,
    identifyHook,
    extractContext,
    parseRequest,
    isMCPRequest,
    getHookFromMethod,
    METHOD_TO_HOOK
};

