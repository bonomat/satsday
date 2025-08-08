
run:
    cargo run -- --config mutinynet.config.toml start -p 12345


deploy-frontend:
    #!/bin/bash
    set -e  # Exit on any error
    cd frontend/satsday

    echo "🚀 Starting deployment for satsday.xyz..."

    # Build the frontend
    echo "📦 Building frontend..."
    VITE_API_BASE_URL=https://mutinynetapi.satsday.xyz npm run build

    npx wrangler pages deploy dist/ --project-name=satsday-xyz-signet --branch main

deploy:
    #!/bin/bash
    set -e
    
    echo "🔨 Building release binary..."
    cargo build --release --bin satoshi_dice
    
    echo "⏳ Copying binary..."
    cp target/release/satoshi_dice /home/bonomat/satsday/mutinynet/mutinynet-satsday
    
    echo "✅ Deployment complete!"
