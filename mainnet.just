
deploy-frontend:
    #!/bin/bash
    set -e  # Exit on any error
    cd frontend/satsday

    echo "🚀 Starting deployment for satsday.xyz..."

    # Build the frontend
    echo "📦 Building frontend..."
    VITE_API_BASE_URL=https://api.satsday.xyz npm run build

    npx wrangler pages deploy dist/ --project-name=satsday-xyz --branch main

deploy:
    #!/bin/bash
    set -e

    echo "🔨 Building release binary..."
    cargo build --release --bin satoshi_dice

    echo "⏳ Copying binary..."
    cp target/release/satoshi_dice /home/bonomat/satsday/mainnet/mainnet-satsday

    echo "✅ Deployment complete!"
