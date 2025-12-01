/**
 * PII Guard Plugin
 * Redacts PII (SSN, email, credit card) from request payloads
 */

function redactSSN(text) {
    return text.replace(/\b\d{3}-\d{2}-\d{4}\b/g, "[REDACTED]");
}

function redactEmail(text) {
    return text.replace(/\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b/g, "[REDACTED_EMAIL]");
}

function redactCreditCard(text) {
    return text.replace(/\b\d{4}[\s-]?\d{4}[\s-]?\d{4}[\s-]?\d{4}\b/g, "[REDACTED_CC]");
}

/**
 * Hook handler for tool_pre_invoke
 */
function onToolPreInvoke(context) {
    var r = context.r;
    var payload = context.body;
    
    if (!payload || typeof payload !== 'string') {
        return {
            allow: true,
            modifiedBody: payload,
            error: null
        };
    }
    
    var ssnPattern = /\d{3}-\d{2}-\d{4}/;
    var emailPattern = /[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}/;
    var ccPattern = /\d{4}[\s-]?\d{4}[\s-]?\d{4}[\s-]?\d{4}/;
    
    var hasPII = ssnPattern.test(payload) || 
                 emailPattern.test(payload) || 
                 ccPattern.test(payload);
    
    if (hasPII) {
        r.error("⚠️  PII DETECTED - REDACTING PAYLOAD");
        var sanitized = payload;
        sanitized = redactSSN(sanitized);
        sanitized = redactEmail(sanitized);
        sanitized = redactCreditCard(sanitized);
        r.error("✏️  Redacted: " + sanitized.substring(0, 200));
        
        return {
            allow: true,
            modifiedBody: sanitized,
            error: null,
            metadata: {
                piiDetected: true,
                redacted: true
            }
        };
    }
    
    return {
        allow: true,
        modifiedBody: payload,
        error: null
    };
}

/**
 * Hook handler for prompt_pre_fetch
 */
function onPromptPreFetch(context) {
    // Same logic as tool_pre_invoke
    return onToolPreInvoke(context);
}

/**
 * Generic process method (backward compatibility)
 */
function process(context) {
    // Try to determine hook from context
    if (context.metadata && context.metadata.hook) {
        if (context.metadata.hook === 'tool_pre_invoke') {
            return onToolPreInvoke(context);
        } else if (context.metadata.hook === 'prompt_pre_fetch') {
            return onPromptPreFetch(context);
        }
    }
    
    // Default to tool_pre_invoke behavior
    return onToolPreInvoke(context);
}

export default {
    onToolPreInvoke,
    onPromptPreFetch,
    process
};
