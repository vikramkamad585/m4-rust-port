# Port GNU m4 1.4.19 to Rust

We are retiring the C build of GNU m4 from a toolchain image and replacing it
with a self-contained Rust binary, `m4rs`. Existing build scripts (autoconf
style macro libraries, code generators, templating) must keep producing
byte-identical output, identical diagnostics and identical exit statuses. Your
job is to implement `m4rs` in `/app/m4rs` so that it behaves exactly like GNU
m4 1.4.19 on the language surface described below.

There is no m4 binary anywhere on this machine and no network. The
specification is the GNU M4 1.4.19 manual in `/app/docs/m4-manual.txt` (plus
the man page `/app/docs/m4.1`) and 494 worked examples whose expected output
was captured from the real GNU m4 1.4.19.

## Time budget

You have **8 hours** of wall-clock time. The system clock does not tell you how
much of it is left; the timer files do:

    cat /app/.timer/remaining_secs     # seconds left
    cat /app/.timer/elapsed_secs
    ls /app/.timer/                    # alert_30min, alert_10min, alert_5min appear near the end

Check `remaining_secs` before any long step and plan your work against it.

## Workspace

    /app/m4rs/Cargo.toml        crate manifest (std only, no dependencies)
    /app/m4rs/src/main.rs       stub entry point: prints an error and exits 70
    /app/docs/m4-manual.txt     GNU M4 1.4.19 manual, plain text: the specification
    /app/docs/m4.1              GNU M4 1.4.19 man page
    /app/examples/cases.jsonl   494 cases with GNU m4's exact stdout, stderr and status
    /app/tools/selfcheck.py     builds like the grader, runs the policy and the cases
    /app/tools/policy.py        the source policy enforced at grading
    /app/README.md              short orientation

Rust 1.83.0 (`cargo`, `rustc`) is installed and works offline. Python 3.11 is
available for tooling. `/app` is a git repository; commit as often as you like.

## The deliverable

A Rust binary crate at `/app/m4rs` whose sources live under
`/app/m4rs/src/` (`main.rs` plus any modules you add, any directory layout).
Grading does **not** use your `Cargo.toml` or `target/`: it copies every
`*.rs` file under `src/` into a fresh crate whose manifest is identical to the
shipped one (package `m4rs`, edition 2021, no dependencies, release profile
`opt-level = 3`) and runs `cargo build --release --offline`. Anything that is
not a `.rs` file under `src/` is ignored. The resulting `target/release/m4rs`
is what gets tested.

### Command line

    m4rs [OPTION]... [FILE]...

Options and operands are processed exactly as GNU m4 processes them,
including interleaving: `-D` and `-U` take effect in command-line order
relative to the files around them, files are read in order, `-` means
standard input, and `--` ends option processing. With no file operands,
standard input is read. Options that must work:

| option | meaning |
|---|---|
| `-D NAME[=VALUE]`, `--define=NAME[=VALUE]` | define NAME (empty when no `=`) |
| `-U NAME`, `--undefine=NAME` | undefine NAME |
| `-I DIR`, `--include=DIR` | append DIR to the include search path |
| `-P`, `--prefix-builtins` | every builtin is named `m4_NAME` instead of `NAME` |
| `-Q`, `--quiet`, `--silent` | suppress the "too few / excess arguments" warnings |
| `-E`, `--fatal-warnings` | a warning makes the final exit status 1 (processing continues) |

Short options cluster (`-PQ`) and take their argument attached or separate
(`-DX=1`, `-D X=1`). Long options accept unambiguous prefixes. An unknown
option prints GNU's two-line usage error and exits 1.

### Behaviour that must match GNU m4 1.4.19

Everything observable on these paths is compared byte for byte:

* **Lexing and expansion:** words, quoted strings with nesting, comments,
  macro calls with argument collection (leading whitespace skipped,
  parenthesis nesting, commas), rescanning of every expansion, `$0`..`$9`
  and multi-digit `$10`, `$#`, `$*`, `$@`, builtins that are recognised only
  when followed by `(`, builtin tokens produced by `defn` and passed through
  `define`/`pushdef`/`builtin`/`indir`.
* **Builtins:** `define undefine defn pushdef popdef indir builtin ifdef
  ifelse shift dumpdef changequote changecom m4wrap include sinclude divert
  undivert divnum len index regexp substr translit patsubst format incr
  decr eval errprint __file__ __line__ __program__ m4exit dnl`, plus the
  predefined text macros `__gnu__` and `__unix__` (both empty). Also define
  `syscmd esyscmd sysval maketemp mkstemp traceon traceoff debugmode
  debugfile` as builtins with their usual names and argument conventions, so
  that `ifdef`, `defn`, `dumpdef` and `-P` see the complete GNU table; their
  run-time effects (shell commands, temporary files, tracing) are never
  exercised. `__program__` expands to the quoted string `m4`.
* **Quotes and comments:** `changequote` and `changecom` with multi-character
  and empty delimiters, including GNU's rules for missing or empty arguments.
* **Diversions:** positive diversions held until undiverted, negative
  diversions discarding text, `undivert` with numbers and with file names,
  automatic undiversion at end of input, and `m4wrap` text (GNU 1.4.19 order:
  the most recently wrapped text runs first).
* **`eval`:** 32-bit two's-complement arithmetic with GNU's operator set,
  precedence, short-circuit rules, radix and width arguments, number prefixes
  (`0x`, `0b`, `0r`, octal) and every error message.
* **`regexp` / `patsubst`:** GNU Emacs regular-expression syntax as GNU m4
  compiles it (`\(` `\)` `\|` `\1`..`\9` `\w` `\W` `\s` `\S` `\b` `\B` `\<`
  `\>` ``\` `` `\'`, brackets, `*` `+` `?`, context-dependent `^` and `$`
  that also match at embedded newlines, `.` not matching newline), POSIX
  leftmost-longest overall matches, the replacement escapes `\&` `\0`..`\9`,
  and the exact compile-error messages.
* **`format`:** C `printf` semantics in the C locale for `d i o u x X c s e E
  f F g G a A %`, flags `- + space 0 # '`, widths and precisions including
  `*`, the `h`/`hh`/`l` modifiers, and GNU's warnings for rejected
  specifiers and bad numeric arguments.
* **Files:** `include`, `sinclude`, `undivert(file)` and file operands search
  the current directory first, then each `-I` directory in order (the name
  reported by `__file__` is the path as found, e.g. `inc/lib.m4`). The
  `M4PATH` environment variable is never set during grading.
* **Diagnostics:** every warning and error goes to standard error in GNU's
  format, `m4:FILE:LINE: message` or `m4: message`. The program-name prefix
  is the literal `m4`, whatever `argv[0]` is. Line numbers follow GNU's rules
  (the line where a macro's name was read; expansion text inherits it).
  `errprint` and `dumpdef` write to standard error too.
* **Exit status:** 0, 1 after errors (missing input file, failed `include`,
  `-E` with warnings, fatal end-of-file errors inside a string, comment or
  argument list), or the `m4exit` value. `m4exit` discards pending
  diversions and wrapped text, exactly as GNU does.

Not exercised: `-s`/synclines, `-G`, `-F`/`-R` frozen files, `-d`/`-t`/`-l`
tracing, `--warn-macro-sequence`, `changeword`, `-L`, NUL bytes in input,
non-ASCII locales, and programs that recurse without bound.

## How you are graded

1. **Integrity gate.** The Rust toolchain under `/usr/local/rustup` must be
   unmodified, and no cargo configuration may exist at `/.cargo/` or
   `/tmp/.cargo/`. Otherwise the score is 0.
2. **Policy gate.** `python3 /app/tools/policy.py` must report no
   violations. After comments and string/char literals are removed, the
   sources may not contain the identifiers `Command`, `extern` or `unsafe`,
   the macros `asm!`, `global_asm!`, `include!`, `include_str!`,
   `include_bytes!`, or the attributes `#[link]` / `#[path]`, and
   `src/main.rs` must exist. (The program must do its own work: no child
   processes, no foreign code, no files pulled in from outside `src/`.)
   Any violation scores 0.
3. **Build gate.** The clean offline build described above must succeed.
   A failed build scores 0.
4. **Behaviour.** 2,175 sealed m4 programs that you have not seen are run,
   each in a fresh empty working directory that contains only that case's
   files. The binary runs as an unprivileged user with the case's arguments,
   its standard input, `PATH=/usr/bin:/bin`, `LC_ALL=C`, a 10-second wall
   clock limit, a 12-second CPU limit, a 4 GiB address-space limit and an
   8 MiB limit per output stream. `/app`, `/root` and the grader's own
   files are not readable to it. A case **passes** when standard output and the exit
   status match GNU m4 exactly and, for the cases marked `check_stderr`
   (about one in five: diagnostics, line numbers, regex and printf corner
   cases), standard error matches byte for byte as well. A timeout, crash
   or signal is a failure.

The cases cover the same categories as the visible examples, in roughly equal
numbers: argument handling, quoting and rescanning, rescanning edge cases,
definitions and stacks, control flow and recursion idioms, quote and comment
delimiters (including multi-character ones), diversions, `m4wrap`, file
inclusion, `eval`, string builtins, `regexp`/`patsubst` (two categories,
one adversarial), `format` (two categories, one adversarial), command-line
handling, diagnostics, line tracking, and realistic macro libraries and
generated programs. They are different programs from the visible ones.

**Reward:**

    reward = 0                                   if any gate fails
    reward = (cases passed) / 2175               otherwise

The denominator is fixed: cases that time out or crash count as failed. The
untouched stub scores 0. A complete port scores 1.0.

**Self-check:** `python3 /app/tools/selfcheck.py` performs the same policy
check and the same clean build, runs all visible cases with the same pass
rule, and prints per-category pass rates and an estimated reward. Use
`--show ID` for a diff of one case and `-k TEXT` to filter by category or id.
The visible cases are a sample of the same distribution as the sealed ones,
so the visible pass rate is a reasonable estimate. Generalise from the
manual; do not special-case example inputs.

## Engineering notes

* The manual's examples (`@result{}` / `@error{}` blocks, rendered as `⇒`
  and `error→` in the text) are precise and cover most diagnostics verbatim.
  When the manual and an example disagree, the example is the real
  behaviour.
* Expansion is a single input stack. Macro output is pushed back and
  rescanned; file inclusion pushes a file; `defn` of a builtin pushes a
  builtin token. Getting this structure right first makes everything else
  (line numbers, `dnl`, multi-character delimiters that partially match and
  must be pushed back) fall out naturally.
* Matching `regexp` and `patsubst` needs a real regex engine with
  leftmost-longest semantics and sub-expression capture. Budget time for it.
  Floating-point `format` output must match the C library digit for digit.
* Deep but finite recursion (a few thousand nested expansions) must not
  overflow the native stack.
* Write output in large buffered chunks; the cases are small, but
  per-case wall time is limited.

## Operating rules

Work on your own until the time runs out; nobody will answer questions, so
make reasonable decisions from the manual and the examples and keep going.
Keep `/app/m4rs` building at every commit: grading can happen at any moment,
and a tree that fails to build scores 0 while a partial port earns credit for
every case it passes. Run `selfcheck.py` after each meaningful change and
revert changes that lower the pass rate. Work on the failing category with
the most cases left before polishing rare corners. Check
`/app/.timer/remaining_secs` before starting a large refactor, and stop
feature work with at least 20 minutes left to run a final self-check and
commit a green build.
