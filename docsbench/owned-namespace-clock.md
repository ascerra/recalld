# Owned namespace timestamps

[Exact report and evidence](../benchmarks/owned-namespace-clock/REPORT.md).

Legacy MCP/REST namespace adapters use the same core Clock when constructed by
the application. Default constructors retain SystemClock. The sampled time is
shared by response, canonical namespace and receipt; logical hashes retain it.
The PR120 independent canonical replay gate must consume this separate minimal
PR104 child before stronger startup/replay claims.
