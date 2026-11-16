# Native producer allocation consumer

See [the frozen proof](../benchmarks/owned-native-ids/REPORT.md).

This minimal actual PR104+PR118 join verifies the existing owned MetadataStore
allocation source through single/batch producer commits, replay, close/reopen
and post-commit repair. Public native activation, CAS, and native snapshot/reset
remain separate joins. Default constructors are unchanged.
