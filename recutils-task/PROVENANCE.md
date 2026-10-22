# Provenance

## Source

* **GNU recutils 1.9** (released 2022-04-16), the release tarball
  `https://ftp.gnu.org/gnu/recutils/recutils-1.9.tar.gz`,
  SHA-256 `6301592b0020c14b456757ef5d434d49f6027b8e5f3a499d13362f205c486e0e`.
  License: GPL-3.0-or-later for the code, GNU FDL 1.3 for the manual
  (`doc/recutils.texi` / `doc/recutils.info`, shipped as
  `environment/reference/recutils-manual.txt`). The man pages
  (`man/recsel.1`, `recinf.1`, `recfmt.1`, `rec2csv.1`) come from the
  same tarball and are distributed under the GPL with the package.
* **Expected behaviour** comes from the Debian bookworm binary package
  `recutils 1.9-2` (built from that release), run inside
  `debian:bookworm-20241016`. Every expected output in
  `environment/samples/` and `tests/cases/` was captured from those
  binaries. They are not present in the task image: the Dockerfile asserts
  that `recsel` is absent.
* Toolchain: the official `rust:1.83.0-slim-bookworm` image, pinned by digest.

## Why this is real work

Rewriting a C command-line tool in Rust without changing its observable
behaviour is common platform work, done to shrink images, drop native
dependencies (recutils links libgcrypt and readline) or harden input
handling. The hard part is fidelity. Scripts depend on the exact text the
tools print: record separators, `+` continuation lines, the order of tied
records, aggregate number formatting, CSV quoting and error messages. So the
engineer works against the real binaries, comparing output and fixing the
next difference. This task reproduces that loop for the four read-only
recutils tools.

## How the task was built

* **Environment.** It ships a stub crate (`recrs`, std only, exits 70), the
  manual and the four man pages. It also ships 445 sample directories, each
  with GNU's stdout, stderr and exit status, and two helpers: `check.py`
  (build, rules check, sample run with the grading pass rule) and `rules.py`.
  No recutils C source ships to the workspace.
* **Cases.** `tests/authoring/gen_rec.py` generates invocations from seeded
  random rec databases: a book catalogue and a staff list with typed fields,
  multi-valued fields, numeric strings in octal, hex and real form,
  multi-line values, comments and descriptors. It covers 15 groups: parsing,
  parsing edge cases, selection expressions, field expressions, typed
  sorting, grouping and aggregates, quick and index selection, joins,
  s-expressions and descriptors, recinf, rec2csv, recfmt templates,
  command-line handling, `%type`/`%typedef` sorting, and diagnostics.
  `tests/authoring/run_rec.py` ran each invocation twice through GNU
  recutils and kept only deterministic, bounded results. The visible set
  (seed 4242) and the sealed set (seed 7331) were de-duplicated against each
  other. Invocations that a program printing nothing (exit 0), or echoing an
  input file, would pass were removed: 273 from the sealed set and 57 from
  the samples. The sealed set has 1,795 invocations.
* **Reference solution** (`solution/src`, about 3,650 non-blank lines of Rust).
  It is a port written against the recutils 1.9 sources: `rec-parser.c`,
  `rec-writer.c`, `rec-db.c` (query, join, field expressions),
  `rec-rset.c` and `rec-mset.c` (typed comparison, gnulib sorted-list
  insertion, grouping), `rec-types.c`, `rec-sex-lex.l` / `rec-sex-tab.y` /
  `rec-sex.c` (selection expressions), `rec-aggregate.c`, `rec-fex.c` and
  the `utils/` front ends. It includes its own POSIX ERE matcher. It
  reproduces GNU's output on all 1,795 sealed invocations and all 445
  samples, so it scores 1.0.
* **Grading.** `tests/collect.py` verifies the Rust toolchain against sealed
  SHA-256 manifests (amd64 and arm64) and applies the source rules. It builds
  only `src/**/*.rs` in a fresh crate with a fixed manifest, then runs every
  sealed invocation as an unprivileged user with `/app`, `/tests`,
  `/solution`, `/logs` and `/root` unreadable. `tests/compute_reward.py`
  applies the gates and scores passed / 1795.
