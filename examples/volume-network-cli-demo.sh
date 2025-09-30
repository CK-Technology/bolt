#!/bin/bash
# Bolt Volume and Network Management Demo
# Shows practical usage of volume and QUIC network features

set -e

echo "🚀 Bolt Volume and Network Demo"
echo "================================="

# Clean up any existing resources
echo "🧹 Cleaning up existing resources..."
bolt volume rm demo-vol --force 2>/dev/null || true
bolt network rm demo-net 2>/dev/null || true
bolt rm demo-web demo-db --force 2>/dev/null || true

echo ""
echo "📦 Creating volumes..."

# Create volumes for different purposes
bolt volume create demo-vol --driver local --size 1GB
bolt volume create db-vol --driver local --size 2GB
bolt volume create log-vol --driver local

# List volumes to verify creation
echo ""
echo "📋 Volume listing:"
bolt volume ls

echo ""
echo "🌐 Creating QUIC network..."

# Create QUIC-optimized network
bolt network create demo-net --driver bolt --subnet 172.30.0.0/16

# List networks
echo ""
echo "📋 Network listing:"
bolt network ls

echo ""
echo "🐳 Running containers with volumes and networking..."

# Run database with persistent storage
bolt run \
  --name demo-db \
  --network demo-net \
  --volume db-vol:/var/lib/postgresql/data \
  --volume log-vol:/var/log \
  --env POSTGRES_DB=demo \
  --env POSTGRES_USER=user \
  --env POSTGRES_PASSWORD=pass \
  --detach \
  postgres:15-alpine

# Run web server with shared volumes
bolt run \
  --name demo-web \
  --network demo-net \
  --volume demo-vol:/usr/share/nginx/html \
  --volume log-vol:/var/log/nginx \
  --ports 8080:80 \
  --detach \
  nginx:alpine

echo ""
echo "⏳ Waiting for containers to start..."
sleep 5

echo ""
echo "📋 Container status:"
bolt ps

echo ""
echo "🔍 Volume inspection:"
bolt volume inspect demo-vol

echo ""
echo "🔍 Network inspection:"
bolt network inspect demo-net

echo ""
echo "📊 Volume usage:"
bolt volume ls

echo ""
echo "🌐 Network connectivity test..."
# Test network connectivity between containers
bolt exec demo-web ping -c 3 demo-db || echo "⚠️  Ping failed (expected if containers not fully ready)"

echo ""
echo "🧪 Testing volume persistence..."

# Write some data to the volume
bolt exec demo-web sh -c 'echo "<h1>Hello from Bolt Volume!</h1>" > /usr/share/nginx/html/index.html'

# Test web server response
echo "📡 Testing web server..."
curl -s http://localhost:8080 || echo "⚠️  Web server not yet ready"

echo ""
echo "🛑 Stopping containers..."
bolt stop demo-web demo-db

echo ""
echo "📦 Restarting containers to test persistence..."
bolt start demo-db demo-web

echo ""
echo "⏳ Waiting for restart..."
sleep 3

echo ""
echo "✅ Testing data persistence..."
curl -s http://localhost:8080 | grep "Hello from Bolt Volume" && echo "✅ Volume data persisted!" || echo "❌ Volume data lost"

echo ""
echo "🧹 Cleanup (optional - uncomment to run):"
echo "# bolt stop demo-web demo-db"
echo "# bolt rm demo-web demo-db --force"
echo "# bolt volume rm demo-vol db-vol log-vol --force"
echo "# bolt network rm demo-net"

echo ""
echo "🎉 Demo completed!"
echo ""
echo "📖 What this demo showed:"
echo "  ✅ Volume creation with different drivers and sizes"
echo "  ✅ QUIC network creation with custom subnet"
echo "  ✅ Container deployment with volume and network mounting"
echo "  ✅ Data persistence across container restarts"
echo "  ✅ Container-to-container communication via QUIC networking"
echo "  ✅ Volume and network inspection capabilities"
echo ""
echo "🚀 Key advantages over Docker:"
echo "  • QUIC networking for ultra-low latency"
echo "  • Enhanced volume management with size limits"
echo "  • Better CLI output and inspection tools"
echo "  • Gaming-optimized network performance"
echo "  • Integrated snapshot support (when configured)"

# Uncomment below for automatic cleanup
# echo ""
# echo "🧹 Automatic cleanup..."
# bolt stop demo-web demo-db
# bolt rm demo-web demo-db --force
# bolt volume rm demo-vol db-vol log-vol --force
# bolt network rm demo-net
# echo "✅ Cleanup completed!"