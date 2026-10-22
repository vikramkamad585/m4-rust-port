#!/usr/bin/env python3
"""Source rules for the recrs crate (identical rules are enforced at grading).

Usage: rules.py [SRC_DIR]   (default /app/recrs/src)
Prints one line per violation; exit status 0 when the tree is clean.

Rules (applied after removing comments, string, byte-string and char
literals):
  * src/main.rs must exist; only *.rs files under src/ are compiled.
  * forbidden identifiers: Command, extern, unsafe
  * forbidden macros: asm!, global_asm!, include!, include_str!, include_bytes!
  * forbidden attributes: #[link ...], #[path ...]
"""
import os
import re
import sys

BANNED_IDENTS = ("Command", "extern", "unsafe")
BANNED_MACROS = ("asm", "global_asm", "include", "include_str", "include_bytes")
BANNED_ATTRS = ("link", "path")


def strip_rust(src):
    """Remove comments and literals, keeping newlines for line numbers."""
    out = []
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if src.startswith("//", i):
            j = src.find("\n", i)
            i = n if j < 0 else j
            continue
        if src.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if src.startswith("/*", j):
                    depth += 1
                    j += 2
                elif src.startswith("*/", j):
                    depth -= 1
                    j += 2
                else:
                    j += 1
            out.append("\n" * src.count("\n", i, j))
            i = j
            continue
        m = re.match(r'b?r(#*)"', src[i:])
        if m and (i == 0 or not (src[i - 1].isalnum() or src[i - 1] == "_")):
            end = '"' + m.group(1)
            j = src.find(end, i + len(m.group(0)))
            j = n if j < 0 else j + len(end)
            out.append(' ""' + "\n" * src.count("\n", i, j))
            i = j
            continue
        if c == '"' or (c == "b" and src.startswith('b"', i) and not (i and (src[i - 1].isalnum() or src[i - 1] == "_"))):
            j = i + (2 if c == "b" else 1)
            while j < n and src[j] != '"':
                j += 2 if src[j] == "\\" else 1
            j += 1
            out.append(' ""' + "\n" * src.count("\n", i, j))
            i = j
            continue
        m = re.match(r"b?'(\\(x[0-9a-fA-F]{2}|u\{[0-9a-fA-F]+\}|.)|[^\\'\n])'", src[i:])
        if m and (c == "'" or (c == "b" and not (i and (src[i - 1].isalnum() or src[i - 1] == "_")))):
            out.append(" ' ' ")
            i += len(m.group(0))
            continue
        out.append(c)
        i += 1
    return "".join(out)


def check_tree(src_dir):
    problems = []
    if not os.path.isfile(os.path.join(src_dir, "main.rs")):
        problems.append("%s/main.rs is missing" % src_dir)
    for root, dirs, files in os.walk(src_dir):
        dirs.sort()
        for f in sorted(files):
            if not f.endswith(".rs"):
                continue
            p = os.path.join(root, f)
            try:
                text = open(p, encoding="utf-8").read()
            except (OSError, UnicodeDecodeError) as e:
                problems.append("%s: unreadable (%s)" % (p, e))
                continue
            code = strip_rust(text)
            for ln, line in enumerate(code.split("\n"), 1):
                for w in BANNED_IDENTS:
                    if re.search(r"(?<![A-Za-z0-9_])%s(?![A-Za-z0-9_])" % w, line):
                        problems.append("%s:%d: forbidden identifier `%s`" % (p, ln, w))
                for mac in BANNED_MACROS:
                    if re.search(r"(?<![A-Za-z0-9_])%s\s*!" % mac, line):
                        problems.append("%s:%d: forbidden macro `%s!`" % (p, ln, mac))
            for a in BANNED_ATTRS:
                for m in re.finditer(r"#\s*!?\s*\[\s*%s(?![A-Za-z0-9_])" % a, code):
                    ln = code.count("\n", 0, m.start()) + 1
                    problems.append("%s:%d: forbidden attribute `#[%s]`" % (p, ln, a))
    return problems


if __name__ == "__main__":
    d = sys.argv[1] if len(sys.argv) > 1 else "/app/recrs/src"
    probs = check_tree(d)
    for p in probs:
        print(p)
    print("rules: %s (%d violation%s)" % ("OK" if not probs else "FAIL", len(probs), "" if len(probs) == 1 else "s"))
    sys.exit(1 if probs else 0)
