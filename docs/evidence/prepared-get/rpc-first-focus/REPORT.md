# First O1 RPC consumer focus
Source d76451c9d502a28ba3a65edec778b077834ec28b:28PASS/1FAIL/1historical ignored in48.37s. Build passed. Actual native origin1|2 forwarding/current projection, full receipt, publication failure, postcommit unknown/reopen, real daemon route, malformed evidence and injected postcore final-overflow tests passed. No full-suite/READY claim.

The failure was old-peer test lifetime: its last client Arc was dropped before the no-request timeout, so the server saw EOF instead of timeout. Test-only36e retains the client through that assertion; its focused control passed1/0 in0.16s. No admission runtime was weakened. Original failure retained here.

Get F at this source used `(MAX-error_overhead)/3`; later audit adds explicit fixed numeric expansion reserve because GetReply MCP serializes f32 through Value. This source is not certified for the final outer bound. Injected overflow enlarges payload after the real core returns; it is not claimed reachable under normal core admission.
