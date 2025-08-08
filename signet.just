
deploy-frontend:
    #!/bin/bash
    set -e  # Exit on any error
    cd frontend/satsday

    echo "🚀 Starting deployment for satsday.xyz..."

    # Build the frontend
    echo "📦 Building frontend..."
    VITE_API_BASE_URL=https://mutinynetapi.satsday.xyz npm run build

    npx wrangler pages deploy dist/ --project-name=satsday-xyz-signet --branch main
