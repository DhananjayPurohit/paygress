#!/bin/bash

echo "🔵 Building Paygress NGINX Module"
echo "================================="
echo

# Build the Rust library as a dynamic module
echo "1️⃣ Building Rust dynamic library..."
cargo build --release --features nginx --lib
if [ $? -ne 0 ]; then
    echo "❌ Build failed!"
    exit 1
fi

echo "✅ Rust library built successfully!"
echo

# Check if the .so file was created
SO_FILE="target/release/libpaygress.so"
if [ ! -f "$SO_FILE" ]; then
    echo "❌ Dynamic library not found: $SO_FILE"
    echo "   Make sure Cargo.toml has crate-type = [\"cdylib\", \"rlib\"]"
    exit 1
fi

echo "2️⃣ Library created: $SO_FILE"
echo

# Show next steps
echo "3️⃣ Installation steps:"
echo "   sudo cp $SO_FILE /etc/nginx/modules/ngx_http_paygress_module.so"
echo "   sudo chmod 644 /etc/nginx/modules/ngx_http_paygress_module.so"
echo

echo "4️⃣ NGINX configuration:"
echo "   Add to nginx.conf: load_module modules/ngx_http_paygress_module.so;"
echo "   Use example config: nginx-paygress.conf"
echo

echo "5️⃣ Test NGINX configuration:"
echo "   sudo nginx -t"
echo "   sudo systemctl reload nginx"
echo

echo "6️⃣ Test payment verification:"
echo "   # Free content"
echo "   curl http://localhost/"
echo
echo "   # Premium content (should fail)"
echo "   curl http://localhost/premium"
echo
echo "   # Premium content with token"
echo "   curl -H 'X-Cashu-Token: your-token-here' http://localhost/premium"
echo

# Optional: Install automatically if requested
read -p "Install to NGINX modules directory? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "Installing module..."
    sudo cp "$SO_FILE" /etc/nginx/modules/ngx_http_paygress_module.so
    sudo chmod 644 /etc/nginx/modules/ngx_http_paygress_module.so
    echo "✅ Module installed!"
    echo
    echo "Don't forget to:"
    echo "1. Add 'load_module modules/ngx_http_paygress_module.so;' to nginx.conf"
    echo "2. Configure your server blocks (see nginx-paygress.conf)"
    echo "3. Test with: sudo nginx -t"
    echo "4. Reload with: sudo systemctl reload nginx"
fi

echo
echo "🚀 Build complete! Ready for NGINX integration."
