# Third-party code in spps-gpu

| What | Where | Version | Licence | Source |
|---|---|---|---|---|
| TinyXML-2 (config.xml parser) | `third_party/tinyxml2/` (`tinyxml2.h`, `tinyxml2.cpp`, `LICENSE.txt`) | 10.0.0, unmodified | zlib (`third_party/tinyxml2/LICENSE.txt`) | https://github.com/leethomason/tinyxml2/tree/10.0.0, fetched 2026-10-06 |

sha256 of the vendored files: `tinyxml2.h` 02f2389feb67fbb3efe42aaa6942034bc7d54695f357c5aac3cbd2f78a06ca4a,
`tinyxml2.cpp` d5a7fd2ad255d716c4d2a64d5686f90c7e27a8285c49c59a5d105cb400bdf703,
`LICENSE.txt` 9332252e9b9e46db8285d4a3f0bf25f139bf1dca6781b956d57f2302efca6432.

The walk and the file writers are ports of upstream I-Simpa's SPPS (GPL-3.0, `Universite-Gustave-Eiffel/I-Simpa`,
tag `v1.4.0_snapshot_14_01_2026`), with receipts in the source; Philox-4x32-10 is written from Salmon et al.,
"Parallel random numbers: as easy as 1, 2, 3" (SC11), the algorithm Random123 and cuRAND implement.
