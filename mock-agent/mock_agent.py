#!/usr/bin/env python3
"""
Mock AI Agent - Simple HTTP Echo Server

This is a mock AI agent service that echoes back received JSON requests.
It simulates an AI agent endpoint for testing the AI Guardrail Mesh.

Features:
- HTTP server on port 8080
- JSON echo endpoint at POST /
- Health check endpoint at GET /health
- Simulated AI response endpoint at POST /chat

Usage:
    python mock_agent.py
    
    # Test endpoints:
    curl -X POST http://localhost:8080/ -H "Content-Type: application/json" -d '{"message": "hello"}'
    curl http://localhost:8080/health
"""

import json
import logging
import os
import sys
from http.server import HTTPServer, BaseHTTPRequestHandler
from datetime import datetime
from typing import Dict, Any

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
    stream=sys.stdout
)
logger = logging.getLogger('mock-ai-agent')

# Configuration from environment
PORT = int(os.getenv('PORT', '8080'))
HOST = os.getenv('HOST', '0.0.0.0')


class MockAIAgentHandler(BaseHTTPRequestHandler):
    """HTTP request handler for the mock AI agent."""
    
    def _set_headers(self, status_code: int = 200, content_type: str = 'application/json'):
        """Set standard response headers."""
        self.send_response(status_code)
        self.send_header('Content-Type', content_type)
        self.send_header('X-Agent-Version', '1.0.0')
        self.send_header('X-Processed-By', 'mock-ai-agent')
        self.end_headers()
    
    def _send_json_response(self, data: Dict[str, Any], status_code: int = 200):
        """Send a JSON response."""
        self._set_headers(status_code)
        response = json.dumps(data, indent=2)
        self.wfile.write(response.encode('utf-8'))
    
    def _read_request_body(self) -> bytes:
        """Read the request body."""
        content_length = int(self.headers.get('Content-Length', 0))
        return self.rfile.read(content_length)
    
    def log_message(self, format: str, *args):
        """Override to use our logger."""
        logger.info(f"{self.address_string()} - {format % args}")
    
    def do_GET(self):
        """Handle GET requests."""
        logger.info(f"GET {self.path}")
        
        if self.path == '/health' or self.path == '/healthz':
            # Health check endpoint
            self._send_json_response({
                'status': 'healthy',
                'timestamp': datetime.utcnow().isoformat(),
                'service': 'mock-ai-agent'
            })
        elif self.path == '/ready':
            # Readiness check
            self._send_json_response({
                'ready': True,
                'timestamp': datetime.utcnow().isoformat()
            })
        elif self.path == '/':
            # Root endpoint
            self._send_json_response({
                'service': 'Mock AI Agent',
                'version': '1.0.0',
                'endpoints': {
                    'POST /': 'Echo JSON request',
                    'POST /chat': 'Simulated AI chat response',
                    'GET /health': 'Health check',
                    'GET /ready': 'Readiness check'
                }
            })
        else:
            self._send_json_response({
                'error': 'Not Found',
                'path': self.path
            }, 404)
    
    def do_POST(self):
        """Handle POST requests."""
        logger.info(f"POST {self.path}")
        
        try:
            # Read and parse request body
            body = self._read_request_body()
            
            # Log received body (truncated for large payloads)
            body_preview = body[:500].decode('utf-8', errors='replace')
            if len(body) > 500:
                body_preview += '... (truncated)'
            logger.info(f"Received body: {body_preview}")
            
            # Check X-Guardrail-Inspected header
            guardrail_inspected = self.headers.get('X-Guardrail-Inspected', 'false')
            logger.info(f"X-Guardrail-Inspected: {guardrail_inspected}")
            
            # Parse JSON if possible
            try:
                request_data = json.loads(body) if body else {}
            except json.JSONDecodeError:
                request_data = {'raw_body': body.decode('utf-8', errors='replace')}
            
            if self.path == '/chat':
                # Simulated AI chat response
                user_message = request_data.get('message', request_data.get('prompt', ''))
                self._send_json_response({
                    'response': f"This is a simulated AI response to: {user_message}",
                    'model': 'mock-gpt-4',
                    'usage': {
                        'prompt_tokens': len(user_message.split()),
                        'completion_tokens': 15,
                        'total_tokens': len(user_message.split()) + 15
                    },
                    'timestamp': datetime.utcnow().isoformat(),
                    'guardrail_inspected': guardrail_inspected
                })
            elif self.path == '/' or self.path == '/echo':
                # Echo endpoint - return the received data
                self._send_json_response({
                    'echo': request_data,
                    'received_at': datetime.utcnow().isoformat(),
                    'content_length': len(body),
                    'content_type': self.headers.get('Content-Type', 'unknown'),
                    'guardrail_inspected': guardrail_inspected,
                    'headers': dict(self.headers)
                })
            else:
                self._send_json_response({
                    'error': 'Not Found',
                    'path': self.path
                }, 404)
                
        except Exception as e:
            logger.exception(f"Error processing request: {e}")
            self._send_json_response({
                'error': 'Internal Server Error',
                'message': str(e)
            }, 500)


def run_server():
    """Start the HTTP server."""
    server_address = (HOST, PORT)
    httpd = HTTPServer(server_address, MockAIAgentHandler)
    
    logger.info(f"=" * 60)
    logger.info(f"Mock AI Agent starting on {HOST}:{PORT}")
    logger.info(f"=" * 60)
    logger.info(f"Endpoints:")
    logger.info(f"  POST /      - Echo JSON request")
    logger.info(f"  POST /chat  - Simulated AI chat")
    logger.info(f"  GET /health - Health check")
    logger.info(f"  GET /ready  - Readiness check")
    logger.info(f"=" * 60)
    
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        logger.info("Shutting down server...")
        httpd.shutdown()


if __name__ == '__main__':
    run_server()
