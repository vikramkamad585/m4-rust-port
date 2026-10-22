# Provenance

## Source

* **Dataset:** NOAA NCEI Global Historical Climatology Network – Daily
  (GHCN-Daily), version **3.35** (update `3.35-upd-2026101917`, built on
  the full reprocessing `3.35-por-2026100923`). The per-station `.dly` files
  were retrieved on 2026-10-22 from
  `https://www.ncei.noaa.gov/pub/data/ghcn/daily/all/`, the station list from
  `https://www.ncei.noaa.gov/pub/data/ghcn/daily/ghcnd-stations.txt`, and the
  format documentation from `https://www.ncei.noaa.gov/pub/data/ghcn/daily/readme.txt`
  (shipped as `environment/docs/ghcnd-readme.txt`).
* **Citation:** Menne, M.J., I. Durre, R.S. Vose, B.E. Gleason, and T.G. Houston,
  2012: An overview of the Global Historical Climatology Network-Daily
  Database. *J. Atmos. Oceanic Technol.*, 29, 897-910,
  doi:10.1175/JTECH-D-11-00103.1. Dataset doi:10.7289/V5D21VHZ.
* **License:** only United States stations (IDs beginning `US`) are used.
  They are works of the US federal government (NOAA) and are in the public
  domain. The non-US data in GHCN-Daily, which may carry WMO Resolution 40
  conditions, was deliberately excluded.

## Why this is real work

Climate archives are mirrored and snapshotted many times over, and
general-purpose compressors leave a lot on the table for fixed-width
scientific text: zlib reaches about 9×, xz about 12×. Designing a
format-aware lossless codec is classic applied research. You parse the
record layout, find which fields predict which (the previous day,
companion variables such as TMAX and TMIN, missing-value and flag runs),
build context models, and validate on data from other networks so the
gains do not just reflect one station type. The shift from training to
evaluation stations mirrors a real deployment: a model fitted on one
observing network has to keep working on archives from others.

## How the task was built

* **Station selection** (`tests/authoring/fetch.py`, seed 20261022):
  US stations were sampled from four pools and files under 2 KB or over
  6 MB were skipped. The pools are COOP stations in the lower 48 states
  (training), first-order airport stations (`USW`), CoCoRaHS gauges
  (`US1`), and COOP or airport stations in AK, HI, PR, VI, GU and the other
  territories. The split (`tests/authoring/split.json`) puts 70 COOP files
  in `environment/data/train`. For each of the other three pools, the first
  22 / 30 / 18 sampled stations are sealed in `tests/hidden` and the next
  3 / 4 / 3 are shipped as `environment/data/dev`. No station appears in
  more than one set. Files are unmodified NOAA files.
* **Starting point:** `environment/codec/codec.cpp` compresses the whole
  file with zlib level 9. It is correct and runs, and it defines the floor:
  9.0512× on the sealed set (5,393,404 bytes for 48,816,810).
* **Reference** (`solution/codec/codec.cpp`, about 520 non-blank lines of
  C++, no external libraries). It parses the fixed-width layout, falling
  back to a generic order-1..4 byte model when a file does not parse. Every
  field is coded with a binary arithmetic coder driven by hashed context
  models, a logistic mixer and an APM stage. The models cover value
  residuals against the previous day, cross-element context from the
  previous line, element succession, missing-value runs, flags and
  zero-padded time values. It reaches 20.9756× on the sealed set
  (2,327,319 bytes), with identical output on x86-64 and arm64, and runs in
  well under a minute. Its integer-only probability arithmetic makes the
  output bit-identical across machines.
* **Grading** (`tests/collect.py`, `tests/compute_reward.py`) builds the
  submission as an unprivileged user and compresses each sealed file as a
  second user. It then removes that user's processes and files, revokes
  write access to pre-existing world-writable files, FIFOs and sockets,
  and decompresses as a third user, with `no_new_privs`, `/app` and
  `/tests` made unreadable, and network and IPC namespaces where available.
  The metric is the aggregate compression ratio. The reward is
  `clamp((ratio − 9.0512)/(20.9756 − 9.0512), 0, 1)`, gated on the build,
  the 16 MiB package limit, an exact round trip of all 70 files and the
  time limits.
