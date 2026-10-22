# recrs: GNU recutils query tools in Rust

Our data team keeps configuration and inventory data in GNU recutils
"rec" files: plain text, one field per line, records separated by blank
lines, with optional `%rec:` descriptors that declare types, keys and sort
order. Scripts across the company shell out to `recsel`, `recinf`, `recfmt`
and `rec2csv`, and parse what they print. We want to drop the C package and
its libgcrypt/readline dependencies from our images and ship a single static
Rust binary that those scripts can call without noticing any change.

Implement that binary in `/app/recrs`. It has to reproduce GNU recutils 1.9
exactly: the same bytes on standard output, the same messages on standard
error, and the same exit status.

## Budget and pacing

You have **8 hours**. The container clock does not reflect the budget. Read
the countdown files instead:

    cat /app/.timer/remaining_secs
    ls /app/.timer          # alert_30min, alert_10min and alert_5min appear near the end

Look at `remaining_secs` before any large rewrite or long test run.

## What is in /app

| path | contents |
|---|---|
| `recrs/` | the crate: `Cargo.toml` (std only), `src/main.rs` (a stub that exits 70) |
| `reference/recutils-manual.txt` | GNU recutils 1.9 manual, plain text: the specification |
| `reference/man/*.1` | man pages for the four tools |
| `samples/<id>/` | 445 recorded runs of the real tools (format below) |
| `check.py` | builds the crate like the grader, applies `rules.py`, runs the samples |
| `rules.py` | the source rules enforced at grading |
| `NOTES.md` | a short orientation |

Rust 1.83.0 works offline. Python 3.11 is there for tooling. `/app` is a git
repository.

Each sample directory holds `case.json` (`tool`, `args`, `stdin`,
`compare_stderr`, `group`), an `in/` directory whose files are the working
directory for the run, and `expected.stdout`, `expected.stderr` and
`expected.status` as produced by GNU recutils 1.9.

## Interface

One executable, `recrs`, whose first argument selects the tool:

    recrs recsel  [OPTION]... [FILE]...
    recrs recinf  [OPTION]... [FILE]...
    recrs recfmt  [OPTION]... [TEMPLATE]
    recrs rec2csv [OPTION]... [FILE]...

Everything after the tool name is handled exactly as the GNU tool of that
name handles its own argument vector, including GNU `getopt_long`
behaviour: option permutation (operands may come before options), clustered
short options, attached or separate option arguments, unambiguous prefixes
of long options, and GNU's diagnostics for unknown options and missing
arguments. Messages that GNU prefixes with the program name (`recsel: error:
...`, `recsel: invalid option -- 'z'`) use the tool name, not `recrs`. With no
file operands, input is read from standard input, which is reported as
`stdin` in parse errors.

### Required behaviour

**Rec format.** Parsing follows recutils exactly: field names, `name:`
followed by an optional blank, continuation lines starting with `+` (with an
optional following space), backslash-newline joins, comment lines
(`#`) inside and between records, blank-line separation, record descriptors
(`%rec`, `%type`, `%typedef`, `%key`, `%sort`, `%auto`), several record sets
in one file, and the anonymous record set. Malformed input produces GNU's
`FILE: LINE: error: ...` message and exit status 1. Several input files
form one database: a record type repeated across files is an error, and
anonymous records in a later file are appended to the anonymous set.

**recsel.** All of `-t`, `-e`, `-q`, `-n`, `-i`, `-p`, `-P`, `-R`, `-c`, `-C`,
`-d`, `-S`, `-G`, `-U`, `-j` and `--print-sexps`, their long forms, and the
error checks between them (for example `-e` with `-n`, `-c` with `-p`, a join
without `-t`, several record types without `-t`). This includes:

* selection expressions with GNU's operators, precedence and ternary,
  string, integer and real operands with recutils' implicit conversions,
  `#field` counts, `field[n]` subscripts, `~` matching with POSIX extended
  regular expressions, `&` concatenation, `-i` case folding, and the rule that
  a multi-valued field matches if any one of its values satisfies the
  expression;
* field expressions for `-p`/`-P`/`-R`: subscripts `[n]` and ranges
  `[n-m]`, `:alias` renames, and the aggregate calls `Count`, `Sum`, `Avg`,
  `Min` and `Max` (names are case-insensitive) with recutils' number
  formatting, both per group and over the whole record set;
* sorting by declared field types (`int`, `real`, `range`, `bool` and the
  others, also through `%typedef`), `%sort` in the descriptor, `-S`, and
  recutils' exact ordering of ties and of records that lack a sort field;
* grouping with `-G`, which merges consecutive equal records;
* joins with `-j` through a `%type: F rec Other` field and the `%key` of
  `Other`;
* the output modes: normal rec output, `-P` values only, `-R` values in a
  row, `-C` collapsed, `-d` with the descriptor, and `--print-sexps`
  s-expressions including recutils' character offsets.

**recinf.** Record counts and type names, `-n`, `-t`, `-d` and `-S`.

**recfmt.** Templates given inline or with `-f FILE`. `{{...}}` slots are
evaluated as selection expressions against each record read from standard
input, with integers printed as `%d` and reals as `%f`, and GNU's errors for
bad slots.

**rec2csv.** Column discovery across records (repeated fields become `Name_2`,
`Name_3`, ...), quoting, `-t`, `-S`, `-d DELIM`, and the same type-aware
sorting as recsel.

**Not exercised:** date/time comparisons (`<<`, `>>`, `==`), `-m` random
selection, encrypted fields and passwords, `--help` and `--version` text, and
real-number subtraction inside selection expressions.

## Grading

Your crate is graded in four steps. The first three are gates: failing any
of them gives a score of 0.

1. **Toolchain integrity.** The Rust toolchain under `/usr/local/rustup` must
   be unmodified, and no cargo configuration may exist at `/.cargo/` or
   `/tmp/.cargo/`.
2. **Source rules.** `python3 /app/rules.py` must report no violations: after
   comments and literals are stripped, no `Command`, `extern` or `unsafe`
   identifiers, no `asm!`, `global_asm!`, `include!`, `include_str!` or
   `include_bytes!`, no `#[link]` or `#[path]` attributes, and
   `src/main.rs` must exist. The binary must do the work itself, without
   child processes or foreign code.
3. **Build.** Every `*.rs` file under `/app/recrs/src` is copied into a fresh
   crate with the shipped manifest (package `recrs`, edition 2021, no
   dependencies, `opt-level = 3`). It is built with
   `cargo build --release --offline`. Your own `Cargo.toml` and `target/`
   are not used.
4. **Behaviour.** 1,795 sealed invocations you have not seen are run against
   the binary. They come from the same groups as the samples: parsing,
   parsing edge cases, selection, printing, sorting, grouping and
   aggregates, quick and index selection, joins, s-expressions and
   descriptors, recinf, rec2csv, recfmt, command-line handling and typed
   sorting. Each runs in a fresh directory containing only its input files,
   as an unprivileged user with `PATH=/usr/bin:/bin` and `LC_ALL=C`. The
   limits are 10 seconds of wall time, 12 seconds of CPU, a 4 GiB address
   space and 8 MiB per output stream. `/app` and the grader's files are
   unreadable during the run. An invocation **passes** when standard output
   and the exit status are identical to GNU recutils 1.9 and, where
   `compare_stderr` is set (about one in eleven: command-line and
   parse-error cases), standard error is identical too. Timeouts and crashes
   fail.

    score = 0                               if a gate fails
    score = passed / 1795                   otherwise

The denominator is fixed. The untouched stub scores 0, and a complete
implementation scores 1.0. Invocations whose expected output would be
produced by a program that prints nothing, or that echoes its input file,
have been left out of the sealed set.

`python3 /app/check.py` runs the same rules check and the same clean build,
then runs every sample with the same pass rule. It prints per-group results
and an estimated score. `--diff ID` shows one sample's differences, and
`-g TEXT` filters by group or id. The samples come from the same
distribution as the sealed set, but they are different invocations, so
implement the behaviour from the manual rather than special-casing sample
inputs.

## Hints from porting experience

* Write the reader first. Almost every sample exercises it, and recutils'
  rules for blanks after the colon, `+` continuations and comments are
  easy to get slightly wrong.
* Selection expressions drive about half of all invocations. Get the lexer
  (longest match: `-1` is an integer token, `a-1` is not a subtraction) and
  the operator precedence right before polishing conversions.
* Several observable behaviours are recutils implementation details rather
  than documented rules: how ties sort, what `Max` prints when no value
  parses, how records that lack the grouping field are kept apart. The samples
  show them. When the manual and a sample disagree, the sample is right.

## How to work

Nobody will answer questions during the run, so decide from the manual and
the samples and keep moving. Commit whenever `check.py` improves, and never
leave the crate in a state that does not build: a failed build scores
nothing, while every passing invocation counts. Work through the groups with
the most failures first, run `check.py` after each change, and back out
changes that make it worse. Check the timer before starting anything large,
and stop adding features with 20 minutes left so there is time to run the
check one last time and commit.
