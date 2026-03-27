# Build the project
cargo build --release

# Start the daemon (monitors clipboard)
./target/release/clipboard-history daemon

# In another terminal, use these commands:

# List all history entries
./target/release/clipboard-history list

# List only last 5 entries
./target/release/clipboard-history list -c 5

# Get full content of entry at index 2
./target/release/clipboard-history get 2

# Copy entry 3 back to clipboard
./target/release/clipboard-history paste 3

# Clear all history
./target/release/clipboard-history clear
