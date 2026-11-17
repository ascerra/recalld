# Same-connection get capability downgrade BEFORE
Source36e4915eeabd55ad6d692f0883d9312d1558b93b, runtime d764 unchanged. Real DaemonClient negotiates get=true, sends one get, then explicit ping returns get=false. Next request incorrectly sent one get instead of zero.1FAIL/0PASS/0ignored in0.01s. This is a private-protocol peer regression; it does not fabricate a canonical observation.

Separate corrected old-peer lifetime control:1PASS/0FAIL/0ignored in0.16s, no request sent. Runtime fix is limited to refreshing get capability under existing ping mutex; sibling capability behavior is not changed. No final validation claim.
