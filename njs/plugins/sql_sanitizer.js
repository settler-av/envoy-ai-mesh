/**
 * SQL Sanitizer Plugin
 * Sanitizes and validates SQL queries in request payloads
 */

/**
 * Strip SQL comments
 */
function stripComments(sql) {
    // Remove single-line comments (-- comment)
    sql = sql.replace(/--[^\r\n]*/g, '');
    
    // Remove multi-line comments (/* comment */)
    sql = sql.replace(/\/\*[\s\S]*?\*\//g, '');
    
    return sql;
}

/**
 * Check if SQL has WHERE clause
 */
function hasWhereClause(sql) {
    // Case-insensitive check for WHERE keyword
    var wherePattern = /\bWHERE\b/i;
    return wherePattern.test(sql);
}

/**
 * Check if SQL is parameterized (has placeholders)
 */
function isParameterized(sql) {
    // Check for common parameterization patterns
    var paramPatterns = [
        /\$\d+/,           // $1, $2, etc.
        /\?/,              // ?
        /:\w+/,            // :param
        /@\w+/             // @param
    ];
    
    for (var i = 0; i < paramPatterns.length; i++) {
        if (paramPatterns[i].test(sql)) {
            return true;
        }
    }
    
    return false;
}

/**
 * Check if SQL matches blocked statements
 */
function matchesBlockedStatement(sql, blockedStatements) {
    if (!blockedStatements || !Array.isArray(blockedStatements)) {
        return false;
    }
    
    for (var i = 0; i < blockedStatements.length; i++) {
        var pattern = blockedStatements[i];
        try {
            var regex = new RegExp(pattern, 'i');
            if (regex.test(sql)) {
                return true;
            }
        } catch (e) {
            // Invalid regex, skip
        }
    }
    
    return false;
}

/**
 * Extract SQL from request body
 */
function extractSQL(body, fields) {
    if (!body || typeof body !== 'string') {
        return null;
    }
    
    try {
        var json = JSON.parse(body);
        
        // If fields specified, check those fields
        if (fields && Array.isArray(fields) && fields.length > 0) {
            var sqlStatements = [];
            for (var i = 0; i < fields.length; i++) {
                var field = fields[i];
                if (json[field] && typeof json[field] === 'string') {
                    sqlStatements.push({
                        field: field,
                        sql: json[field]
                    });
                }
            }
            return sqlStatements;
        }
        
        // Otherwise, scan all string values
        var allSQL = [];
        for (var key in json) {
            if (typeof json[key] === 'string') {
                // Check if it looks like SQL
                var value = json[key];
                if (/\b(SELECT|INSERT|UPDATE|DELETE|DROP|TRUNCATE|ALTER|CREATE)\b/i.test(value)) {
                    allSQL.push({
                        field: key,
                        sql: value
                    });
                }
            }
        }
        
        return allSQL.length > 0 ? allSQL : null;
        
    } catch (e) {
        // Not JSON, check if entire body is SQL
        if (/\b(SELECT|INSERT|UPDATE|DELETE|DROP|TRUNCATE|ALTER|CREATE)\b/i.test(body)) {
            return [{
                field: 'body',
                sql: body
            }];
        }
        
        return null;
    }
}

/**
 * Hook handler for tool_pre_invoke
 */
function onToolPreInvoke(context) {
    var r = context.r;
    var payload = context.body;
    var config = this.config || {};
    
    // Extract configuration
    var fields = config.fields || null; // null = scan all string args
    var stripComments = config.strip_comments !== false; // default true
    var blockDeleteWithoutWhere = config.block_delete_without_where !== false; // default true
    var blockUpdateWithoutWhere = config.block_update_without_where !== false; // default true
    var requireParameterization = config.require_parameterization === true; // default false
    var blockedStatements = config.blocked_statements || [];
    var blockOnViolation = config.block_on_violation !== false; // default true
    
    // Extract SQL from payload
    var sqlStatements = extractSQL(payload, fields);
    
    if (!sqlStatements || sqlStatements.length === 0) {
        // No SQL found, allow
        return {
            allow: true,
            modifiedBody: payload,
            error: null
        };
    }
    
    r.error("🔍 SQL Sanitizer: Found " + sqlStatements.length + " SQL statement(s)");
    
    var violations = [];
    var modifiedPayload = payload;
    
    // Process each SQL statement
    for (var i = 0; i < sqlStatements.length; i++) {
        var stmt = sqlStatements[i];
        var sql = stmt.sql;
        var originalSQL = sql;
        
        // Strip comments if configured
        if (stripComments) {
            sql = stripComments(sql);
            if (sql !== originalSQL) {
                r.error("  ✂️  Stripped comments from SQL in field: " + stmt.field);
                // Update payload if we modified SQL
                modifiedPayload = modifiedPayload.replace(originalSQL, sql);
            }
        }
        
        // Check blocked statements
        if (matchesBlockedStatement(sql, blockedStatements)) {
            violations.push("Blocked statement pattern matched in field: " + stmt.field);
            r.error("  🚫 Blocked statement detected in field: " + stmt.field);
        }
        
        // Check DELETE without WHERE
        if (blockDeleteWithoutWhere && /\bDELETE\s+FROM\b/i.test(sql) && !hasWhereClause(sql)) {
            violations.push("DELETE statement without WHERE clause in field: " + stmt.field);
            r.error("  🚫 DELETE without WHERE detected in field: " + stmt.field);
        }
        
        // Check UPDATE without WHERE
        if (blockUpdateWithoutWhere && /\bUPDATE\b/i.test(sql) && !hasWhereClause(sql)) {
            violations.push("UPDATE statement without WHERE clause in field: " + stmt.field);
            r.error("  🚫 UPDATE without WHERE detected in field: " + stmt.field);
        }
        
        // Check parameterization
        if (requireParameterization && !isParameterized(sql)) {
            violations.push("SQL not parameterized in field: " + stmt.field);
            r.error("  🚫 SQL not parameterized in field: " + stmt.field);
        }
    }
    
    // Handle violations
    if (violations.length > 0) {
        var errorMsg = "SQL Sanitizer violations: " + violations.join('; ');
        
        if (blockOnViolation) {
            return {
                allow: false,
                modifiedBody: modifiedPayload,
                error: errorMsg,
                metadata: {
                    violations: violations,
                    sqlStatements: sqlStatements.length
                }
            };
        } else {
            // Warn but allow
            r.error("  ⚠️  SQL violations detected but not blocking: " + errorMsg);
            return {
                allow: true,
                modifiedBody: modifiedPayload,
                error: null,
                warnings: violations,
                metadata: {
                    violations: violations,
                    sqlStatements: sqlStatements.length
                }
            };
        }
    }
    
    // No violations
    r.error("  ✓ SQL statements validated successfully");
    return {
        allow: true,
        modifiedBody: modifiedPayload,
        error: null,
        metadata: {
            sqlStatements: sqlStatements.length,
            validated: true
        }
    };
}

/**
 * Generic process method (backward compatibility)
 */
function process(context) {
    return onToolPreInvoke.call(this, context);
}

export default {
    onToolPreInvoke,
    process
};

