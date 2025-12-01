function readBody(r) {
    var bodyString = "";
    if (r.requestText) {
        bodyString = r.requestText;
    } else if (r.requestBody) {
        bodyString = typeof r.requestBody === 'string' ? r.requestBody : String(r.requestBody);
    }
    
    if (bodyString.length > 0) {
        // Log truncated body for debugging
        r.error("📦 Body content: '" + bodyString.substring(0, 200) + (bodyString.length > 200 ? "..." : "") + "'");
    }
    
    return bodyString;
}

function forward(r, payload) {
    r.error("🚀 Forwarding " + r.uri + " to upstream via internal redirect");
    
    // Set the variable for proxy_set_body in nginx.conf
    // This requires 'js_var $mcp_payload;' in the http block
    var finalPayload = "";
    if (payload) {
        finalPayload = payload;
    } else if (r.requestBody) {
        // Preserve original body if no changes
        finalPayload = typeof r.requestBody === 'string' ? r.requestBody : String(r.requestBody);
    } else {
        finalPayload = "";
    }
    
    // Set the payload variable
    r.variables.mcp_payload = finalPayload;
    
    // Calculate and set the correct Content-Length for the modified body
    // This is critical - Content-Length must match the actual body size
    var payloadLength = 0;
    if (finalPayload) {
        // For UTF-8, most ASCII characters are 1 byte, but we need accurate byte count
        // NJS doesn't have TextEncoder, so we'll use a simple approximation
        // For JSON (mostly ASCII), string length is usually close to byte length
        // But to be safe, we'll calculate a more accurate byte length
        var byteLength = 0;
        for (var i = 0; i < finalPayload.length; i++) {
            var charCode = finalPayload.charCodeAt(i);
            // UTF-8 encoding: ASCII (0-127) = 1 byte, others = 2-4 bytes
            if (charCode <= 0x7F) {
                byteLength += 1;
            } else if (charCode <= 0x7FF) {
                byteLength += 2;
            } else if (charCode <= 0xFFFF) {
                byteLength += 3;
            } else {
                byteLength += 4;
            }
        }
        payloadLength = byteLength;
    }
    r.variables.mcp_payload_length = String(payloadLength);
    
    r.error("📏 Setting Content-Length to: " + payloadLength + " (original: " + (r.headersIn['Content-Length'] || 'unknown') + ")");
    
    // Use internalRedirect instead of subrequest to support SSE and streaming responses.
    // This hands control back to NGINX's proxy module which handles streaming correctly.
    // The map directive in nginx.conf will route based on URI prefix
    r.internalRedirect('/upstream' + r.uri);
}

export default { readBody, forward };
