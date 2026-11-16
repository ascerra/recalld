# Native physical-sweep consumer invocation

Use the exact source/build/executable identities recorded in this directory's
report and validation manifest. The new internal diagnostic track is
`serialized-native-sweep-synthetic-diagnostic-v2`; its checkpoint track is
`isolated-native-sweep-checkpoint-v2`. These names do not activate public API2.

Build the existing `owned_evaluator`, `native_prefix_fixture`, and
`score_mcp_episode` examples with `--features bench`, then use the actual runner:

```sh
python3 scripts/public-mcp-evaluation.py prepare \
  --track serialized-native-sweep-synthetic-diagnostic-v2 \
  --binary /target/debug/examples/owned_evaluator \
  --scorer /target/debug/examples/score_mcp_episode \
  --build /output/build.json --out /output/inputs
python3 scripts/public-mcp-evaluation.py run \
  --binary /target/debug/examples/owned_evaluator \
  --scorer /target/debug/examples/score_mcp_episode \
  --source-root /work/source --manifest /output/inputs/manifest.json \
  --out /output/episode
```

The build manifest supplies the actual frozen source commit, file digests and
container image; `prepare` hashes the executable and generated artifacts. Do
not substitute invented pins. The supervisor builds a genuine migrated prefix
using fixtureVersion2, global multiplier1 and a real foreign-namespace numeric
override0. It owns the private controls; the actor receives only the current
public false-read action. The actor cannot invoke sweep or write operations.

The existing published #149 v1 artifacts remain unchanged. The new runner
rejects that old manifest profile even with current valid executable/source
pins and unchanged verified artifact digests. Restore separately rejects an
otherwise valid same-binary checkpoint whose sole profile change is the old
track name, with the checkpoint digest recomputed correctly. Neither path
emulates the prior no-compaction behavior or performs a mutating fallback.

The validation drivers retained beside the results pin an offline read-only
source/image, 2 CPUs, `-j1`, and one private Cargo/target cache. The first 4 GiB
library link was OOM-killed before tests. A single unchanged-source/flags/profile
retry uses 6 GiB and captures cgroup peak/events; full correctness validation
uses that same cap. These runs are not performance comparisons.

The suffix checks real phase and compaction receipts, all four derived
generations, pending work zero, surviving foreign full text, namespace-confined
public search, three exact fresh-owner restores, sparse scorer labels, actor
canaries and fail-closed negatives. Independent native genesis, ordinary native
serving and public native writers remain separate gates.
