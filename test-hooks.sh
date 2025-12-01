#!/bin/bash
# Test script for local hook testing

echo "Testing MCP hooks locally..."
echo ""

# Test 1: tool_pre_invoke hook (tools/call)
echo "=== Test 1: tool_pre_invoke hook (tools/call) ==="
curl -X POST http://localhost:8080/ \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {
      "name": "calculate_sum",
      "arguments": {
        "numbers": [1, 2, 3, 4, 5]
      }
    }
  }'
echo -e "\n"

# Test 2: tool_pre_invoke with PII in body
echo "=== Test 2: tool_pre_invoke with PII (should be redacted) ==="
curl -X POST http://localhost:8080/ \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 2,
    "method": "tools/call",
    "params": {
      "name": "test_tool",
      "arguments": {
        "email": "test@example.com",
        "ssn": "123-45-6789"
      }
    }
  }'
echo -e "\n"

# Test 3: tool_pre_invoke with SQL (should be blocked if enforce mode)
echo "=== Test 3: tool_pre_invoke with dangerous SQL (should be blocked) ==="
curl -X POST http://localhost:8080/ \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 3,
    "method": "tools/call",
    "params": {
      "name": "execute_query",
      "arguments": {
        "sql": "DELETE FROM users"
      }
    }
  }'
echo -e "\n"

# Test 4: prompt_pre_fetch hook
echo "=== Test 4: prompt_pre_fetch hook (prompts/get) ==="
curl -X POST http://localhost:8080/ \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": 4,
    "method": "prompts/get",
    "params": {
      "name": "code_review_prompt"
    }
  }'
echo -e "\n"

# Test 5: Non-MCP request (should pass through)
echo "=== Test 5: Non-MCP request (should pass through) ==="
curl -X POST http://localhost:8080/ \
  -H "Content-Type: application/json" \
  -d '{"test": "data"}'
echo -e "\n"

echo "Done! Check nginx error logs for plugin execution details:"
echo "  tail -f nginx-dev/logs/error.log"

