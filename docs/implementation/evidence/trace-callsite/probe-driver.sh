set -eu
sha256sum --check /source.sha256
trap 'r=$?; sha256sum --check /source.sha256 || exit 99; echo PROBE_EXIT=$r; exit $r' EXIT
rustc --version --verbose
rustc --edition=2024 /work/probe.rs -L dependency=/deps --extern tracing=/deps/libtracing-1e3ebbc28f745e23.rlib --extern tracing_subscriber=/deps/libtracing_subscriber-f6131c1ae22e821e.rlib -o /out/probe
sha256sum /out/probe
/out/probe
