#!/bin/sh
set -eu
rustfmt --edition 2024 --config skip_children=true /evidence/formatted/src/api/coherent_by_id_tests.rs
rustfmt --edition 2024 --config skip_children=true /evidence/formatted/src/storage/metadata/native_records/reader_tests.rs
rustfmt --edition 2024 --config skip_children=true /evidence/formatted/src/storage/metadata/native_records/wire_tests.rs
