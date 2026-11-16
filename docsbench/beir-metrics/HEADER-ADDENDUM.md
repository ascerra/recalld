# Exact Python development prerequisite after dependency001

The first approved offline reference build failed at `src/pytrec_eval.cpp:1`: `Python.h` absent. Exit1 occurred before extension import or any metric probe. C compilation had begun; this is a build prerequisite failure, not a scorer/runtime failure. Original001 logs/command/image/driver/input hashes are retained; no scoring dependency changed.

The unchanged image `sha256:daefdc9cbe9b0f39b1fc29d975148b3b10adc1bd4309f9ca1be8abd36ed811ef` contains Debian arm64 CPython **3.11.2-6+deb12u6**, interpreter SHA256 `9a070c8a36e16c152dae21b66aeb707f4d21b506c875e9b15edf4387fca7dcf8`, SOABI `cpython-311-aarch64-linux-gnu`, extension suffix `.cpython-311-aarch64-linux-gnu.so`, multiarch `aarch64-linux-gnu`, no debug ABI flag. GCC/G++12.2.0, glibc2.36. Runtime, minimal and stdlib package versions agree. The normal Debian pool has newer updates; none was substituted.

Official Debian Snapshot exact-version metadata selected:

| Package | Exact artifact | SHA256 |
| --- | --- | --- |
|libpython3.11-dev|3.11.2-6+deb12u6 arm64,4,389,544bytes|355437c28e946ab6faeb241560c8abe693f4433ef535cabd02b690b8b29313fb|
|libpython3.11|3.11.2-6+deb12u6 arm64,1,841,228bytes|17df0aac143b71865574d10cecd525511b4636e83c21c2761ff87ccffa5a5fef|

Official [dev metadata](https://snapshot.debian.org/mr/binary/libpython3.11-dev/3.11.2-6%2Bdeb12u6/binfiles) and [library metadata](https://snapshot.debian.org/mr/binary/libpython3.11/3.11.2-6%2Bdeb12u6/binfiles) bind architecture/version to Snapshot file identities. Downloaded bytes match those identities; SHA256s and archive/control metadata are recorded in `debian-prerequisites.json`. Packages were extracted solely to new private scratch for inspection with `dpkg-deb -x`; no install, maintainer script, apt resolution or host/global mutation.

The dev package declares exact stdlib/library versions plus libexpat1-dev/zlib1g-dev; libc6-dev is recommended. Existing image has matching stdlib, libexpat1-dev2.5.0-1+deb12u2, zlib1g-dev1:1.2.13.dfsg-1 and libc6-dev2.36-9+deb12u13. Its runtime counterparts satisfy the library package's declared lower bounds. libpython3.11 shared runtime is not installed; `/usr/bin/python3` does not link it. The prefetched matching library is inventory only, **not a requested runtime replacement**.

Extracted headers identify3.11.2 and agree with runtime pointer/size_t8, wchar_t4, pymalloc1 and non-debug build. Preserve the actual Debian distinction: runtime sysconfig reports Py_ENABLE_SHARED1, while installed dev header leaves that macro undefined and the actual interpreter has no libpython dependency. Do not edit the header or forge sysconfig equality. Successful real extension link/import and runtime checks remain the compatibility gate.

## Proposed bounded retry

Use a new private output directory, retain001. Verify/extract the exact dev archive offline beneath that directory; add only its `usr/include/python3.11` and `usr/include` paths via explicit compiler `CPATH`. Keep the existing interpreter, stdlib, compiler and sysconfig unchanged. Do not install the package, replace `/usr`, modify LD_LIBRARY_PATH, link the unused static Python library, or load the prefetched libpython runtime. Original pytrec_eval setup links only m/stdc++; capture actual compile/link commands and extension `ldd` output to verify this boundary.

Keep all approved scorer/dependency pins and network-none2CPU4GiB unchanged. Record dev archive and extracted header hashes, interpreter pre/post hash, compiled extension/site roster and imported module paths. Run mandatory known-empty/known-zero reference checks first; missing output keys stop the population proof, never insert dummy hits. Only then run the other hand-derived/tie/missing-query characterizations. No Rust suite or actual public121→100 claim yet. Shared-runner implementation remains held for Halley's frozen handoff.

This addendum proposes the exact prerequisite correction; no second build has run at its creation.
