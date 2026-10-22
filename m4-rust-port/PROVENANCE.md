# Provenance

## Source

* **GNU M4 1.4.19** (released 2021-05-28), the release tarball
  `https://ftp.gnu.org/gnu/m4/m4-1.4.19.tar.xz`,
  SHA-256 `63aede5c6d33b6d9b13511cd0be2cac046f2e70fd0a07aa9573a04a82783af96`.
  Its manual (`doc/m4.texi`, prebuilt `doc/m4.info*`, GNU FDL 1.3) and man page
  (`doc/m4.1`) are the specification shipped to the workspace.
* **Expected behaviour** comes from the Debian bookworm binary package
  `m4 1.4.19-3` (built from that release, glibc 2.36 regex), run inside
  `debian:bookworm-20241016`. That binary produced every expected
  output in `environment/examples/` and `tests/cases/`. It is never installed in
  the task image (the Dockerfile asserts `! command -v m4`).
* Toolchain: the official `rust:1.83.0-slim-bookworm` image, pinned by digest.

## Why this is real work

Replacing a C system tool with a memory-safe, dependency-free reimplementation
that stays byte-compatible with existing scripts is routine infrastructure work.
Autoconf, sendmail configuration, and many in-house code generators rely on
m4, and they depend on its exact behaviour: rescanning order, quote and
comment handling, diversion order, diagnostic text and line numbers, GNU
regex semantics in `patsubst`, and C `printf` output in `format`. A port that is
"mostly right" breaks generated files silently. So the engineer works against
the real binary's behaviour, case by case. The task reproduces that loop:
read the manual, implement, diff against captured GNU output, repeat.

## How the task was built

* **Environment.** It ships a stub crate (`m4rs`, std only, exits 70), the
  plain-text manual (the `.info` files with node separators stripped) and the
  man page. It also ships 494 visible cases with GNU m4's stdout, stderr and
  exit status, and two helper scripts: `selfcheck.py`, which builds and runs
  the visible cases with the grading pass rule, and `policy.py`, the source
  rules. Nothing from the GNU C sources ships to the workspace.
* **Cases.** `tests/authoring/gen_cases.py` (sealed with the grader, alongside the `run_cases.py` freezer) generates m4 programs from
  seeded randomized templates in 21 categories: quoting and rescanning,
  arguments, definitions and stacks, control idioms (the manual's
  `forloop`/`foreach`/`join`/`reverse`), delimiter changes including
  multi-character ones, diversions, `m4wrap`, includes with `-I`, random
  `eval` expression trees, string builtins, `regexp`/`patsubst` (plain and an
  adversarial grammar of nested groups, backrefs, brackets and anchors),
  `format` (plain and adversarial: rounding, `%g`/`%a`, length modifiers,
  rejected specifiers), command lines, diagnostics, line tracking, and
  realistic macro libraries (enum/struct code generators, HTML templating,
  config headers, insertion sort, the manual's string utilities). Each program
  was run twice through GNU m4. Cases were dropped when the two runs differed,
  ran longer than 5 s, produced more than 64 KiB, contained NUL bytes, or hit
  GNU's own resource failures ("memory exhausted", stack overflow). Cases that
  a do-nothing or echo-the-input program would pass were also dropped (none
  survived). The visible set (seed 4242) and the hidden set (seed 7331) were
  de-duplicated against each other.
* **Reference solution** (`solution/src`, about 3,850 non-blank lines of Rust).
  It is a from-scratch port written against the 1.4.19 C sources (`input.c`,
  `macro.c`, `builtin.c`, `eval.c`, `format.c`, `symtab.c`, `output.c`,
  `path.c`) and glibc's `regcomp.c` parsing rules for `RE_SYNTAX_EMACS`. It
  includes its own backtracking regex engine with leftmost-longest overall
  matching and a printf implementation. It matched GNU m4 on 2,175 of the
  2,175 frozen hidden cases and 494 of 494 visible ones. About 0.3% of
  candidate cases had been dropped earlier, during development, for known
  glibc sub-match quirks (e.g. captures inside an optional group that holds a
  nested star), which no reasonable port would reproduce. The hidden set is
  exactly the frozen cases the reference passes, so the reference scores 1.0.
* **Grading.** `tests/collect.py` verifies the Rust toolchain against sealed
  SHA-256 manifests (amd64 and arm64) and applies the source policy. It
  builds only `src/**/*.rs` in a fresh crate with a fixed manifest, then runs
  every hidden case as an unprivileged user with `/app`, `/tests`,
  `/solution`, `/logs` and `/root` made unreadable to it. `compute_reward.py`
  applies the gates and scores passed / 2175.
