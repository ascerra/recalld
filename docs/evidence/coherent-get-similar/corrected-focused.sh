#!/bin/bash
set -eu
cargo fmt --all
cargo test --offline --all-features --lib coherent_by_id -- --nocapture
while IFS= read -r test_name; do
  cargo test --offline --all-features --lib "$test_name" -- --exact --nocapture
done < .validation/failed-cases.txt
