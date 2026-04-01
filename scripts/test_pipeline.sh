#!/bin/bash
cargo run -q --bin aps -- --pack | cargo run -q --bin aunpack | awk '{print $1}' | tail -n 5 || true
echo ""
cargo run -q --bin aps -- --pack | cargo run -q --bin achunk -- --max 200 | wc -l || true
echo "Tests complete!"
