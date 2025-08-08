
run:
    cargo run -- --config mutinynet.config.toml start -p 12345

deploy:
    #!/bin/bash
    set -e
    
    echo "🔨 Building release binary..."
    cargo build --release --bin satoshi_dice
    
    echo "⏳ Copying binary..."
    cp target/release/satoshi_dice /home/bonomat/satsday/mutinynet/mutinynet-satsday
    
    echo "✅ Deployment complete!"
