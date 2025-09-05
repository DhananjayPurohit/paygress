#!/bin/bash

echo "🧪 Testing Paygress NGINX Module"
echo "================================"
echo

# Check if module was built
SO_FILE="target/release/libpaygress.so"
if [ ! -f "$SO_FILE" ]; then
    echo "❌ Module not found: $SO_FILE"
    echo "Run: ./build-nginx-module.sh first"
    exit 1
fi

echo "✅ Module found: $SO_FILE"
echo

# Check module symbols
echo "📋 Module exports:"
if command -v nm >/dev/null 2>&1; then
    nm -D "$SO_FILE" | grep -E "(ngx_http_paygress|paygress)" | head -10
    echo
fi

# Check file info
echo "📊 Module info:"
file "$SO_FILE"
ls -lh "$SO_FILE"
echo

# Test the demo binary
echo "🔧 Testing plugin functionality:"
if [ -f "target/release/nginx-plugin" ]; then
    ./target/release/nginx-plugin
else
    echo "Build with: cargo build --release --features nginx --bin nginx-plugin"
fi

echo
echo "🚀 Next steps:"
echo "1. Copy module: sudo cp $SO_FILE /etc/nginx/modules/ngx_http_paygress_module.so"
echo "2. Configure NGINX: Add load_module directive"
echo "3. Use example config: nginx-paygress.conf"
echo "4. Test with real requests"
