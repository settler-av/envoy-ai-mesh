// Simple YAML parser for basic structures
// This handles the plugin config format which is relatively simple

function parseYAML(yamlText) {
    if (!yamlText || yamlText.trim().length === 0) {
        return [];
    }
    
    var lines = yamlText.split('\n');
    var result = [];
    var currentItem = null;
    var currentKey = null;
    var indentStack = [];
    var currentIndent = 0;
    var inList = false;
    
    for (var i = 0; i < lines.length; i++) {
        var line = lines[i];
        var trimmed = line.trim();
        
        // Skip empty lines and comments
        if (trimmed.length === 0 || trimmed.startsWith('#')) {
            continue;
        }
        
        // Calculate indentation
        var indent = 0;
        for (var j = 0; j < line.length; j++) {
            if (line[j] === ' ') {
                indent++;
            } else if (line[j] === '\t') {
                indent += 4; // Treat tab as 4 spaces
            } else {
                break;
            }
        }
        
        // List item (starts with -)
        if (trimmed.startsWith('-')) {
            // If we have a previous item, save it
            if (currentItem !== null) {
                result.push(currentItem);
            }
            
            // Start new item
            currentItem = {};
            currentKey = null;
            inList = true;
            currentIndent = indent;
            
            // Parse the item content after the dash
            var itemContent = trimmed.substring(1).trim();
            if (itemContent.length > 0 && itemContent.indexOf(':') > 0) {
                // Key-value pair on same line
                var kv = parseKeyValue(itemContent);
                if (kv.key) {
                    currentItem[kv.key] = kv.value;
                    currentKey = kv.key;
                }
            }
            
            continue;
        }
        
        // Key-value pair
        if (trimmed.indexOf(':') > 0) {
            var kv = parseKeyValue(trimmed);
            
            if (currentItem === null) {
                // Top level, start new item
                currentItem = {};
                result.push(currentItem);
            }
            
            // Check if this is nested (indented more than current)
            if (indent > currentIndent) {
                // Nested - create object if needed
                if (currentKey && currentItem[currentKey] === null) {
                    currentItem[currentKey] = {};
                }
                if (currentKey && typeof currentItem[currentKey] === 'object' && !Array.isArray(currentItem[currentKey])) {
                    currentItem[currentKey][kv.key] = kv.value;
                } else {
                    // Create new nested object
                    if (!currentItem.config) {
                        currentItem.config = {};
                    }
                    currentItem.config[kv.key] = kv.value;
                }
            } else {
                // Same level or less - update current item
                currentItem[kv.key] = kv.value;
                currentKey = kv.key;
                currentIndent = indent;
            }
            
            continue;
        }
    }
    
    // Add last item
    if (currentItem !== null) {
        result.push(currentItem);
    }
    
    return result;
}

function parseKeyValue(line) {
    var colonIndex = line.indexOf(':');
    if (colonIndex < 0) {
        return { key: null, value: null };
    }
    
    var key = line.substring(0, colonIndex).trim();
    var valueStr = line.substring(colonIndex + 1).trim();
    var value = parseValue(valueStr);
    
    return { key: key, value: value };
}

function parseValue(valueStr) {
    if (!valueStr || valueStr.length === 0) {
        return null;
    }
    
    // Remove quotes
    if ((valueStr.startsWith('"') && valueStr.endsWith('"')) ||
        (valueStr.startsWith("'") && valueStr.endsWith("'"))) {
        return valueStr.substring(1, valueStr.length - 1);
    }
    
    // Boolean
    if (valueStr === 'true') return true;
    if (valueStr === 'false') return false;
    
    // Number
    if (/^-?\d+$/.test(valueStr)) {
        return parseInt(valueStr, 10);
    }
    if (/^-?\d+\.\d+$/.test(valueStr)) {
        return parseFloat(valueStr);
    }
    
    // Array (simple format: [item1, item2] or multi-line)
    if (valueStr.startsWith('[') && valueStr.endsWith(']')) {
        var items = valueStr.substring(1, valueStr.length - 1).split(',');
        var result = [];
        for (var i = 0; i < items.length; i++) {
            var item = items[i].trim();
            if (item.length > 0) {
                // Remove quotes from array items
                if ((item.startsWith('"') && item.endsWith('"')) ||
                    (item.startsWith("'") && item.endsWith("'"))) {
                    item = item.substring(1, item.length - 1);
                }
                result.push(item);
            }
        }
        return result;
    }
    
    // String
    return valueStr;
}

export default { parseYAML };
