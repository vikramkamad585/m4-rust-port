# A better codec for GHCN-Daily station archives

Our climate data service mirrors NOAA's Global Historical Climatology
Network Daily (GHCN-Daily) archive: one fixed-width text file per weather
station (`.dly`), each line holding one month of one variable for one
station. We keep many snapshots of these files, and right now they are
stored with zlib. Build a compressor that exploits the structure of this
format and stores the files in as few bytes as possible, without losing a
single byte.

The catch is the distribution. Your training data is from cooperative
observer stations (COOP, IDs `USC…`) in the lower 48 states. The files you
are scored on come from **different station networks and regions**:
first-order airport stations (`USW…`, many more variables, including
time-of-day elements), CoCoRaHS volunteer gauges (`US1…`, short
precipitation-only records), and stations in Alaska, Hawaii and the US
territories. A codec tuned only to the training stations will not
generalise; one that models the format will.

## Budget

You have **8 hours**. The container clock is not the budget. Read the
countdown:

    cat /app/.timer/remaining_secs      # alert_30min / alert_10min / alert_5min files appear near the end

## Workspace

| path | contents |
|---|---|
| `/app/codec/` | the codec. Today: `codec.cpp` (whole file through zlib level 9) and a `Makefile` |
| `/app/data/train/` | 70 COOP station files, 48 MB |
| `/app/data/dev/` | 10 files from the scored networks (airport, CoCoRaHS, remote), 4 MB. These stations are not in the scored set |
| `/app/docs/ghcnd-readme.txt` | NOAA's format documentation (section III describes `.dly` lines) |
| `/app/docs/stations.txt` | station metadata for the shipped files |
| `/app/tools/evaluate.py` | builds the codec the way grading does and reports ratio, timing and an estimated score |

Toolchain: g++ 12 and make, with headers and libraries for zlib, liblzma, libzstd
and libbz2, plus Python 3.11 (standard library only). There is no network.
`/app` is a git repository.

## The contract

`make` run in a copy of `/app/codec/` must produce an executable `codec` in
that directory. It is used as:

    codec compress   IN OUT
    codec decompress IN OUT

* `compress` reads one `.dly` file and writes one compressed file.
  `decompress` reads that file and must write back the original
  **byte for byte**.
* Each invocation is a separate process. The decompressor only receives the
  compressed file: it runs as a different user, after everything the
  compressor wrote outside its output file has been deleted. Neither step
  has network access.
* The program may read data it ships with: anything `make` leaves under
  `/app/codec/model/` is copied next to the executable, so the code can find
  it relative to its own path (`/proc/self/exe`). The executable plus
  `model/` must total at most **16 MiB**. Learning priors from the
  training data is allowed and encouraged, within that size.
* Any language the image can build and run works (C, C++, Python 3), as
  long as `make` produces `codec`. It is built with plain `make` as an
  unprivileged user, so it must not need root, network or files outside
  the copied directory.
* Input that does not follow the documented layout must still round-trip.
  Sealed files are genuine GHCN-Daily files, but the format has corners
  (for example zero-padded `HHMM` time values), so keep a safe fallback.

## Scoring

Grading builds your codec, compresses each of the **70 sealed station
files** (about 49 MB in total) and decompresses them as described above.

**Gates.** Any failure scores 0:

1. `make` succeeds and produces `codec`: self-check with `evaluate.py`.
2. Executable plus `model/` is 16 MiB or less: `evaluate.py` prints the size.
3. Every sealed file round-trips exactly. One mismatch, crash or missing
   output fails the gate: `evaluate.py` reports round-trip failures on
   dev or train.
4. Time: each invocation finishes within 120 s, and all compress calls,
   and separately all decompress calls, total at most 1,200 s. Each
   invocation is also limited to 120 s of CPU time and 4 GiB of address
   space, and up to four files are processed in parallel. Stay well inside these limits, since
   the grading machine may be slower than yours.

**Metric.** The compression ratio over the whole sealed set:

    ratio = (total original bytes) / (total compressed bytes)

**Score.** The ratio placed between the shipped zlib codec and our
reference codec:

    score = clamp((ratio - 9.0512) / (20.9756 - 9.0512), 0, 1)

The untouched workspace scores 0 (zlib reaches 9.0512× on the sealed set).
Our reference, a format-aware context-mixing coder, reaches 20.9756× and
scores 1.0. The score is capped at 1.0, but the raw ratio is reported with
every run, so improvements beyond the reference are visible.

`python3 /app/tools/evaluate.py` gives the local estimate. On the dev
stations the same two codecs reach 7.2697× and 15.8913×, and the script
prints `clamp((dev_ratio - 7.2697) / (15.8913 - 7.2697), 0, 1)`. Use
`--set train` for the training stations or `--files ...` for anything
else. Dev is the better guide to the sealed score, because train is a
different network. Don't over-fit to the 10 dev files either: they are a
small sample.

## Directions that pay off

* Parse the layout (station id, year, month, element, then 31 day slots of a
  5-character value and three 1-character flags) and model each field
  separately instead of compressing text.
* Values are highly predictable from the previous day, from the same day in
  the line just before (TMAX then TMIN for the same month), and from the
  same element in earlier months. Missing values (`-9999`) and flags cluster.
* Arithmetic coding with adaptive context models usually beats any
  transform followed by a general-purpose compressor, but the transform
  route is a solid first milestone.

## Working rules

Nobody will answer questions, so make your own decisions and keep going.
Keep `/app/codec` building and round-tripping at every commit: a codec that
fails a single file scores 0, while a modest gain that always round-trips
scores. Measure with `evaluate.py` after each idea, and revert ideas that
don't move the dev ratio. Check `/app/.timer/remaining_secs` before large
changes, and finish with at least 20 minutes to spare: rerun `evaluate.py`
on both dev and train, confirm zero round-trip failures, and commit.
