# Dockerfile for Mock AI Agent
#
# A simple Python Flask application that simulates an AI agent
# for testing AI-Guard governance.
#
FROM python:3.11-slim

WORKDIR /app

# Install dependencies
# - curl is used by docker-compose healthchecks
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates && rm -rf /var/lib/apt/lists/* && \
    pip install --no-cache-dir flask requests

# Copy application
COPY mock_agent.py .

# Create health endpoint script
RUN echo 'import sys; sys.exit(0)' > health_check.py

# Expose port
EXPOSE 8080

# Run the application
CMD ["python", "mock_agent.py"]
