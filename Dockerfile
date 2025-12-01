FROM nginx:latest

# Install nginx-module-njs package
RUN apt-get update && \
    apt-get install -y nginx-module-njs && \
    rm -rf /var/lib/apt/lists/*

# Copy nginx configuration (use production config for Docker)
COPY conf/nginx.conf.prod /etc/nginx/nginx.conf

# Copy NJS functions
COPY njs/ /etc/nginx/njs/

# Create logs directory
RUN mkdir -p /var/log/nginx

# Create plugins directory for ConfigMap mount
# ConfigMap will be mounted at /etc/nginx/plugins/config.yaml
RUN mkdir -p /etc/nginx/plugins && \
    chmod 755 /etc/nginx/plugins

# Expose port 8080
EXPOSE 8080

# Start nginx
CMD ["nginx", "-g", "daemon off;"]

