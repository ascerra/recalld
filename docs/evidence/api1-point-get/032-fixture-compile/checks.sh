#!/bin/sh
set -eu
rustfmt --edition 2024 --config skip_children=true /evidence/formatted/src/api/coherent_by_id_tests.rs
rustfmt --edition 2024 --config skip_children=true /evidence/formatted/src/daemon/api1_get.rs
rustfmt --edition 2024 --config skip_children=true /evidence/formatted/src/daemon/api1_get_tests.rs
rustfmt --edition 2024 --config skip_children=true /evidence/formatted/src/daemon/observed_join_tests.rs
rustfmt --edition 2024 --config skip_children=true /evidence/formatted/src/daemon/read_only_tests.rs
rustfmt --edition 2024 --config skip_children=true /evidence/formatted/src/storage/metadata/native_records/public_matrix_tests.rs
rustfmt --edition 2024 --config skip_children=true /evidence/formatted/src/storage/metadata/native_records/reader_tests.rs
rustfmt --edition 2024 --config skip_children=true /evidence/formatted/src/storage/metadata/native_records/wire_tests.rs
