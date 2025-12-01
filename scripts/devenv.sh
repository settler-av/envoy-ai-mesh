#!/bin/bash
# devenv.sh - Script to populate nginx.conf paths from .env BASEPATH

set -e

# Get the script directory and project root
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
NGINX_CONF="$PROJECT_ROOT/conf/nginx.conf"
ENV_FILE="$PROJECT_ROOT/.env"
ENV_EXAMPLE="$PROJECT_ROOT/.env.example"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored messages
info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if .env file exists
if [ ! -f "$ENV_FILE" ]; then
    error ".env file not found at $ENV_FILE"
    if [ -f "$ENV_EXAMPLE" ]; then
        info "Copying .env.example to .env..."
        cp "$ENV_EXAMPLE" "$ENV_FILE"
        warn "Please edit .env and set your BASEPATH, then run this script again"
        exit 1
    else
        error ".env.example not found. Please create .env file with BASEPATH variable"
        exit 1
    fi
fi

# Source .env file to load BASEPATH
# Remove comments and empty lines, then source
set -a
source <(grep -v '^#' "$ENV_FILE" | grep -v '^$' | sed 's/^/export /')
set +a

# Check if BASEPATH is set
if [ -z "$BASEPATH" ]; then
    error "BASEPATH is not set in .env file"
    exit 1
fi

# Remove trailing slash from BASEPATH if present
BASEPATH="${BASEPATH%/}"

info "Using BASEPATH: $BASEPATH"

# Check if nginx.conf exists
if [ ! -f "$NGINX_CONF" ]; then
    error "nginx.conf not found at $NGINX_CONF"
    exit 1
fi

# Create a backup of the original nginx.conf
BACKUP_FILE="${NGINX_CONF}.backup.$(date +%Y%m%d_%H%M%S)"
info "Creating backup: $BACKUP_FILE"
cp "$NGINX_CONF" "$BACKUP_FILE"

# Check if nginx.conf contains {{BASEPATH}} placeholder
HAS_PLACEHOLDER=$(grep -q "{{BASEPATH}}" "$NGINX_CONF" && echo "yes" || echo "no")

if [ "$HAS_PLACEHOLDER" = "yes" ]; then
    # Replace {{BASEPATH}} placeholders
    info "Found {{BASEPATH}} placeholders, replacing them..."
    ESCAPED_NEW=$(echo "$BASEPATH" | sed 's/[\/&]/\\&/g')
    sed -i.tmp "s|{{BASEPATH}}|$ESCAPED_NEW|g" "$NGINX_CONF"
else
    # Detect the current base path from nginx.conf
    # Look for the first path that contains common subdirectories (logs, njs, config)
    # Extract the base path by finding the common prefix
    OLD_BASE_PATH=""
    if grep -q "logs/nginx.pid\|logs/error.log\|njs/\|config/" "$NGINX_CONF"; then
        # Extract base path from pid or error_log line (first path found)
        FIRST_PATH=$(grep -E "^\s*(pid|error_log|access_log|js_path|alias)" "$NGINX_CONF" | head -1 | grep -oE "/[^[:space:]]+" | head -1)
        if [ -n "$FIRST_PATH" ]; then
            # Extract base path by removing the subdirectory (logs/, njs/, config/)
            OLD_BASE_PATH=$(echo "$FIRST_PATH" | sed -E 's|/(logs|njs|config)/.*||')
            info "Detected current base path: $OLD_BASE_PATH"
        fi
    fi

    # Fallback to default if detection failed
    if [ -z "$OLD_BASE_PATH" ]; then
        OLD_BASE_PATH="/path/to/nginx-dev"
        warn "Could not detect base path from nginx.conf, using default: $OLD_BASE_PATH"
    fi

    # Check if paths already match BASEPATH
    if [ "$OLD_BASE_PATH" = "$BASEPATH" ]; then
        info "Paths in nginx.conf already match BASEPATH ($BASEPATH)"
        info "No changes needed."
        rm -f "$BACKUP_FILE"
        exit 0
    fi

    # Replace paths in nginx.conf
    # We need to escape special characters for sed
    ESCAPED_OLD=$(echo "$OLD_BASE_PATH" | sed 's/[\/&]/\\&/g')
    ESCAPED_NEW=$(echo "$BASEPATH" | sed 's/[\/&]/\\&/g')

    info "Replacing paths in nginx.conf..."

    # Use sed to replace all occurrences
    sed -i.tmp "s|$ESCAPED_OLD|$ESCAPED_NEW|g" "$NGINX_CONF"
fi

# Remove the temporary file created by sed
rm -f "${NGINX_CONF}.tmp"

# Verify the replacement worked by checking if BASEPATH appears in the file
if grep -q "$BASEPATH" "$NGINX_CONF"; then
    info "Successfully updated nginx.conf with paths from BASEPATH"
    info "Backup saved to: $BACKUP_FILE"
else
    warn "Replacement completed, but BASEPATH not found in nginx.conf"
    warn "This might mean the old path pattern didn't match"
fi

# Show what paths were updated
info "Updated paths:"
grep -n "$BASEPATH" "$NGINX_CONF" || warn "No matching paths found"

