# Notice, provenance, and scope

## What this repository is

An independent, static binary analysis of publicly distributed Bose QuietComfort 35 and
QuietComfort 35 II firmware, investigating the 2019 claim that firmware update 4.5.2
degraded active noise cancelling. It is documentation and research. See `README.md`,
`REPORT_FOR_AI.md`, `RESEARCH.md`, and `OPEN_FIRMWARE.md`.

## The firmware files

The files under `firmware/` are **Bose's proprietary firmware**. They are not authored here
and no ownership of them is claimed. They were originally distributed publicly by Bose from
`downloads.bose.com` and are mirrored in the community archive
[`bosefirmware/ced`](https://github.com/bosefirmware/ced), which is the immediate source
used here. They are included solely so the analysis is reproducible.

Integrity note: the 4.3.6 and 4.8.1 images were verified byte-for-byte against fresh
downloads from Bose's own servers. Bose has since removed 4.5.2 from its servers; its
provenance and the corroboration for it are documented in `REPORT_FOR_AI.md` section 10.

If Bose requests removal of the firmware binaries, they should be taken down. The analysis,
tools, and findings in this repository stand on their own without them, and the SHA-256
manifest in `findings/SHA256SUMS.txt` lets anyone verify their own copies.

## Trademarks

Bose, QuietComfort, and QC35 are trademarks of Bose Corporation. This project is not
affiliated with, endorsed by, or connected to Bose in any way.

## Scope and safety

This work is static analysis only. No firmware was executed and nothing was flashed to any
device during it. The material on downgrading and on custom firmware is informational.
Flashing firmware, stock or modified, can permanently damage headphones. In particular, on a
QC35 II whose serial number ends in `AZ`, downgrading below 2.1.3 can brick the device.
Anyone acting on this does so at their own risk.

## Licence

The original work here (the analysis text, the scripts under `tools/`, and the generated
output under `findings/`) is released under the MIT License, see `LICENSE`. That license
covers only the original work. It does not apply to the Bose firmware files under
`firmware/`, which remain the property of Bose Corporation.
