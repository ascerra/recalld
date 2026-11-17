# Actual owned get versus missing daemon consumer

Source a2374ddf0bf0d1369b72ca1ce9bf9ea8e5735a39, actual parents e1451717 (41ba core +200a R1). Actual configured owner get returned200, then a real temporary daemon returned `unknown method: get_contract_v1`. Canonical control state unchanged. One test FAIL,0PASS,0ignored in0.13s; build passed. This is the missing private route, not ordinary API1 get/startup proof.

The provisional raw request used responseLimit262144 and originalContractVersion2. The correction replaces that provisional constant with the newly defined get-specific era allowance; that test-input change is explicit, not an unchanged-request GREEN claim. The BEFORE fails method lookup before inspecting that allowance.

Network-none isolated Docker,2CPU/4GiB/jobs1, private cache/target; previous893 executable hash verified before own-package clean for source-root switch, new executable independently hashed. No personal data, credentials or socket mount. Source unchanged during execution. No completed PR/raw release yet.
