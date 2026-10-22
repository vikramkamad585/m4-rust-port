#!/usr/bin/env python3
"""Generate m4 test programs (inputs only) for differential testing.

Usage: gen_cases.py SEED COUNT_PER_CATEGORY OUT.jsonl [--prefix ID]

Each line: {"id", "category", "args": [...], "files": {name: text},
            "stdin": text, "check_stderr": bool}
Expected outputs are filled in later by running GNU m4 1.4.19.
"""
import json
import random
import sys

WORDS = ["alpha", "beta", "gamma", "delta", "foo", "bar", "baz", "qux", "x", "y",
         "hello", "world", "Item", "NAME", "value", "list", "node", "a1", "b_2", "_t"]
TEXT_WORDS = ["the", "quick", "brown", "fox", "jumps", "over", "lazy", "dog", "m4", "is",
              "a", "macro", "processor", "with", "quotes", "and", "comments", "1", "42", "x-y"]
BUILTINS = ["define", "undefine", "defn", "pushdef", "popdef", "indir", "builtin", "ifdef",
            "ifelse", "shift", "changequote", "changecom", "divert", "undivert", "divnum",
            "len", "index", "substr", "translit", "regexp", "patsubst", "format", "incr",
            "decr", "eval", "dnl", "m4wrap", "errprint", "__file__", "__line__", "include",
            "sinclude", "dumpdef", "m4exit", "__program__"]


class G:
    def __init__(self, rnd):
        self.r = rnd

    def c(self, seq):
        return self.r.choice(seq)

    def n(self, a, b):
        return self.r.randint(a, b)

    def p(self, prob):
        return self.r.random() < prob

    def word(self):
        return self.c(TEXT_WORDS)

    def words(self, a=1, b=5, sep=" "):
        return sep.join(self.word() for _ in range(self.n(a, b)))

    def name(self):
        return self.c(WORDS) + (str(self.n(0, 9)) if self.p(0.3) else "")

    def text_line(self):
        parts = [self.word() for _ in range(self.n(1, 6))]
        return " ".join(parts)


# --------------------------------------------------------------------------
# Category generators. Each returns (files, args, stdin, check_stderr).

def single(prog, args=None, stdin="", check=False):
    return {"input.m4": prog}, (args if args is not None else ["input.m4"]), stdin, check


def cat_quote_rescan(g):
    lines = []
    names = [g.name() for _ in range(g.n(2, 4))]
    for i, nm in enumerate(names):
        kind = g.n(0, 7)
        if kind == 0:
            lines.append("define(`%s', `%s')dnl" % (nm, g.words()))
        elif kind == 1:
            lines.append("define(`%s', ``%s'')dnl" % (nm, g.words()))
        elif kind == 2 and i > 0:
            lines.append("define(`%s', `<%s>')dnl" % (nm, names[i - 1]))
        elif kind == 3 and i > 0:
            lines.append("define(`%s', `%s(%s)')dnl" % (nm, names[i - 1], g.word()))
        elif kind == 4:
            lines.append("define(`%s', `[$1|$2]')dnl" % nm)
        elif kind == 5:
            lines.append("define(`%s', `$0:$#:$*')dnl" % nm)
        elif kind == 6:
            lines.append("define(`%s', `$@')dnl" % nm)
        else:
            lines.append("define(`%s', `%s')dnl" % (nm, "`" + g.word() + "'" + g.word()))
    for _ in range(g.n(3, 7)):
        nm = g.c(names)
        form = g.n(0, 10)
        if form == 0:
            lines.append("%s %s" % (nm, g.word()))
        elif form == 1:
            lines.append("`%s' is quoted, %s is not" % (nm, nm))
        elif form == 2:
            lines.append("%s(%s, %s)" % (nm, g.word(), g.word()))
        elif form == 3:
            lines.append("%s(`%s', `%s(x)')" % (nm, g.word(), nm))
        elif form == 4:
            lines.append("``%s''" % nm)
        elif form == 5:
            lines.append("# comment %s, not expanded (%s)" % (nm, nm))
        elif form == 6:
            lines.append("%s%s %s_%s" % (nm, nm, nm, g.word()))
        elif form == 7:
            lines.append("%s( %s , `%s' )" % (nm, g.word(), g.words(1, 3)))
        elif form == 8:
            lines.append("%s((a,b),c)" % nm)
        elif form == 9:
            lines.append("defn(`%s') / defn(`%s', `%s')" % (nm, nm, g.c(names)))
        else:
            lines.append("%s(`a,b', (c,d), `(')" % nm)
    if g.p(0.3):
        lines.append("define(`q', `$1')q(`q(`nested')')")
    if g.p(0.3):
        lines.append("define(`dq', ``$1'')dq(`x')|dq(dq(`y'))")
    return single("\n".join(lines) + "\n")


def cat_args(g):
    lines = []
    lines.append("define(`count', `$#')dnl")
    lines.append("define(`all', `[$*]')dnl")
    lines.append("define(`allq', `[$@]')dnl")
    lines.append("define(`first', `$1')dnl")
    lines.append("define(`second', `$2')dnl")
    lines.append("define(`last', `ifelse(`$#', `1', `$1', `last(shift($@))')')dnl")
    lines.append("define(`nine', `$9$10$11')dnl")
    calls = []
    for _ in range(g.n(4, 9)):
        args = []
        for _ in range(g.n(0, 6)):
            k = g.n(0, 6)
            if k == 0:
                args.append(g.word())
            elif k == 1:
                args.append("`%s'" % g.words(1, 3, ", "))
            elif k == 2:
                args.append(" " * g.n(1, 3) + g.word())
            elif k == 3:
                args.append("(%s,%s)" % (g.word(), g.word()))
            elif k == 4:
                args.append("")
            elif k == 5:
                args.append("first(%s)" % g.word())
            else:
                args.append(g.word() + " ")
        m = g.c(["count", "all", "allq", "first", "second", "last", "nine", "shift"])
        if m == "last" and not args:
            args = ["z"]
        if m == "shift" and not args:
            args = ["a", "b"]
        if m == "nine":
            args = [str(i) for i in range(1, g.n(8, 12))]
        call = "%s(%s)" % (m, ",".join(args))
        if g.p(0.15):
            call = m
        calls.append(call)
    lines.extend(calls)
    if g.p(0.4):
        lines.append("define(`rev', `ifelse(`$#', `0', , `$#', `1', ``$1'', `rev(shift($@)),`$1'')')dnl")
        lines.append("rev(%s)" % ",".join(g.word() for _ in range(g.n(1, 6))))
    if g.p(0.3):
        lines.append("define(`args', `$0($*)')dnl")
        lines.append("`args'(1,2) count() count(,) count(`')")
    return single("\n".join(lines) + "\n")


def cat_defs(g):
    lines = []
    nm = g.name()
    other = g.name()
    steps = g.n(4, 9)
    lines.append("define(`%s', `one')dnl" % nm)
    for _ in range(steps):
        k = g.n(0, 13)
        if k == 0:
            lines.append("pushdef(`%s', `%s')%s" % (nm, g.word(), nm))
        elif k == 1:
            lines.append("popdef(`%s')%s" % (nm, nm))
        elif k == 2:
            lines.append("define(`%s', defn(`%s'))%s" % (other, nm, other))
        elif k == 3:
            lines.append("undefine(`%s')`%s'=%s" % (nm, nm, nm))
        elif k == 4:
            lines.append("ifdef(`%s', `yes', `no') ifdef(`%s', `Y')" % (nm, other))
        elif k == 5:
            lines.append("indir(`%s', `a', `b')" % nm)
        elif k == 6:
            lines.append("define(`%s', `%s')dnl" % (nm, g.words()))
        elif k == 7:
            lines.append("define(`def', defn(`define'))def(`%s', `via def')%s" % (other, other))
        elif k == 8:
            lines.append("builtin(`define', `%s', `bi')%s builtin(`len', `abcd')" % (other, other))
        elif k == 9:
            lines.append("define(`%s', `$1-%s')%s(%s)" % (other, nm, other, g.word()))
        elif k == 10:
            lines.append("define (`%s') define" % nm)
        elif k == 11:
            lines.append("pushdef(`%s', `p1')pushdef(`%s', `p2')%s popdef(`%s')%s undefine(`%s')`'%s" % (nm, nm, nm, nm, nm, nm, nm))
        elif k == 12:
            lines.append("define(`%s', `before')%s(define(`%s', `after'))%s" % (nm, nm, nm, nm))
        else:
            lines.append("define(`mac', defn(`%s', `%s'))`'mac" % (nm, other))
    if g.p(0.3):
        lines.append("define(`len', `mylen')len(`abc') builtin(`len', `abc')")
    if g.p(0.3):
        lines.append("indir(`define', `ind', `indirect $1')indir(`ind', `call') ind(`direct')")
    if g.p(0.2):
        lines.append("define(`a b', `odd')indir(`a b') defn(`a b')")
    return single("\n".join(lines) + "\n")


FORLOOP = """divert(`-1')
# forloop(var, from, to, stmt)
define(`forloop', `pushdef(`$1', `$2')_forloop($@)popdef(`$1')')
define(`_forloop',
       `$4`'ifelse($1, `$3', `', `define(`$1', incr($1))$0($@)')')
divert`'dnl
"""

FOREACH = """divert(`-1')
# foreach(x, (item_1, item_2, ..., item_n), stmt)
define(`foreach', `pushdef(`$1')_foreach($@)popdef(`$1')')
define(`_arg1', `$1')
define(`_foreach', `ifelse(`$2', `()', `',
  `define(`$1', _arg1$2)$3`'$0(`$1', (shift$2), `$3')')')
divert`'dnl
"""

JOIN = """divert(`-1')
define(`join', `ifelse(`$#', `2', ``$2'',
  `ifelse(`$2', `', `', ``$2'_')$0(`$1', shift(shift($@)))')')
define(`quote', `ifelse(`$#', `0', `', ``$*'')')
define(`dquote', ``$@'')
define(`reverse', `ifelse(`$#', `0', , `$#', `1', ``$1'',
                          `reverse(shift($@)), `$1'')')
divert`'dnl
"""


def cat_control(g):
    parts = []
    k = g.n(0, 6)
    if k == 0:
        a, b = g.n(1, 5), g.n(1, 12)
        parts.append(FORLOOP)
        parts.append("forloop(`i', %d, %d, `i ')\n" % (a, max(a, b)))
        parts.append("forloop(`j', 1, %d, `forloop(`k', 1, j, `k')`'ifelse(j, %d, , `,')')\n" % (g.n(1, 4), 0))
    elif k == 1:
        items = ", ".join(g.word() for _ in range(g.n(1, 6)))
        parts.append(FOREACH)
        parts.append("foreach(`x', (%s), `[x]')\n" % items)
        parts.append("foreach(`x', `(%s)', `x-')\n" % items)
    elif k == 2:
        parts.append(JOIN)
        items = ", ".join("`%s'" % g.word() for _ in range(g.n(1, 6)))
        parts.append("join(`-', %s)\njoin(`, ', %s)\n" % (items, items))
        parts.append("reverse(%s)\nquote(%s)\n" % (items, items))
    elif k == 3:
        n = g.n(1, 10)
        parts.append("define(`fact', `ifelse(`$1', `0', `1', `eval($1 * fact(decr($1)))')')dnl\n")
        parts.append("fact(%d) fact(%d)\n" % (n, g.n(0, 6)))
        parts.append("define(`fib', `ifelse(eval($1 < 2), 1, $1, `eval(fib(decr($1)) + fib(eval($1 - 2)))')')dnl\nfib(%d)\n" % g.n(1, 12))
    elif k == 4:
        vals = [g.word() for _ in range(g.n(3, 7))]
        conds = []
        for i in range(len(vals) - 1):
            conds.append("`%s', `%s'" % (vals[i], "r%d" % i))
        parts.append("define(`sw', `ifelse(`$1', %s, `default')')dnl\n" % ", `$1', ".join(conds))
        for _ in range(g.n(3, 6)):
            parts.append("sw(`%s') " % g.c(vals + ["zzz"]))
        parts.append("\nifelse(`a', `b', `1', `a', `a', `2') ifelse(`x') ifelse(`a', `a') ifelse(`a',`b',`c')\n")
    elif k == 5:
        parts.append("define(`cnt', 0)define(`bump', `define(`cnt', incr(cnt))')dnl\n")
        for _ in range(g.n(2, 6)):
            parts.append("bump`'cnt ")
        parts.append("\ndefine(`stack', `pushdef(`stk', `$1')')define(`pop', `stk`'popdef(`stk')')dnl\n")
        items = [g.word() for _ in range(g.n(1, 5))]
        parts.append("".join("stack(`%s')" % w for w in items) + "\n")
        parts.append("pop pop pop\n")
    else:
        n = g.n(2, 8)
        parts.append("define(`repeat', `ifelse(eval(`$1' > 0), 1, `$2`'repeat(decr(`$1'), `$2')')')dnl\n")
        parts.append("repeat(%d, `ab')|repeat(0, `x')|repeat(%d, `[*]')\n" % (n, g.n(1, 4)))
        parts.append("define(`upcase', `translit(`$*', `a-z', `A-Z')')upcase(`%s')\n" % g.words())
    return single("".join(parts))


def cat_quotes_comments(g):
    lines = []
    qsets = [("[", "]"), ("<<", ">>"), ("{", "}"), ("(*", "*)"), ("`", "'"), ("|", "|"),
             ("<!", "!>"), ("$", "$"), ("[[", "]]"), ("'", "`")]
    csets = [("/*", "*/"), ("%", "\n"), ("@@", "@@"), ("//", "\n"), ("<!--", "-->"), (";", "\n")]
    lines.append("define(`w', `W')define(`v', `w-w')dnl")
    for _ in range(g.n(3, 6)):
        k = g.n(0, 9)
        if k == 0:
            lq, rq = g.c(qsets)
            lines.append("changequote(`%s', `%s')dnl" % (lq, rq) if not lines[-1].startswith("changequote(") else "dnl")
            lines.append("%sw%s w %s%sw%s%s v" % (lq, rq, lq, lq, rq, rq))
            lines.append("define(%sv%s, %snew w%s)v" % (lq, rq, lq, rq))
            lines.append("changequote%s%s" % ("" if g.p(0.5) else "()", "dnl"))
        elif k == 1:
            bc, ec = g.c(csets)
            ecs = ec if ec != "\n" else "\n"
            if ecs == "\n":
                lines.append("changecom(`%s')dnl" % bc)
            else:
                lines.append("changecom(`%s', `%s')dnl" % (bc, ec))
            lines.append("w %s w comment v %s w" % (bc, ec if ec != "\n" else ""))
            lines.append("# w is not a comment now")
            lines.append("changecom`'dnl")
            lines.append("# v")
            lines.append("changecom(`#')dnl")
        elif k == 2:
            lines.append("changequote(`', `')dnl")
            lines.append("`w' w define(w,x)w")
            lines.append("changequote`'dnl")
        elif k == 3:
            lines.append("# comment with `unbalanced quote and w")
            lines.append("`quoted # not comment' w")
        elif k == 4:
            lines.append("changequote([, ])dnl")
            lines.append("define([sq], [[$1]])sq([a, b]) [w] [[w]] `w'")
            lines.append("changequote(`, ')dnl")
        elif k == 5:
            lines.append("changecom(`', `')dnl")
            lines.append("# w v")
            lines.append("changecom(`#', `')dnl")
            lines.append("# w")
            lines.append("w")
        elif k == 6:
            lines.append("changequote(`<', `>')dnl")
            lines.append("define(<x>, <<w>>)x <x> <<x>>")
            lines.append("changequote(`<')dnl")
            lines.append("<w' `w'")
            lines.append("changequote`'dnl")
        elif k == 7:
            lines.append("changecom(`w')dnl")
            lines.append("v w here")
            lines.append("changecom`'changecom(`#')dnl")
        elif k == 8:
            lines.append("changequote(`aa', `bb')dnl")
            lines.append("aawbb aaaawbbbb a w b")
            lines.append("changequote`'dnl")
        else:
            lines.append("define(`cq', `changequote($@)')cq(`{', `}'){w} w cq`'`w'")
    return single("\n".join(lines) + "\n")


def cat_diversions(g):
    lines = []
    used = []
    for _ in range(g.n(4, 9)):
        k = g.n(0, 9)
        if k <= 2:
            d = g.c([1, 2, 3, 5, 9, 10, 100])
            used.append(d)
            lines.append("divert(%d)%s" % (d, g.text_line()))
        elif k == 3:
            lines.append("divert(-1)hidden %s" % g.text_line())
        elif k == 4:
            lines.append("divert`'divnum: %s" % g.text_line())
        elif k == 5 and used:
            lines.append("divert(0)undivert(%d) after" % g.c(used))
        elif k == 6:
            lines.append("divert(0)undivert`'all")
        elif k == 7 and used:
            lines.append("divert(%d)undivert(%d)into" % (g.c([2, 7]), g.c(used)))
        elif k == 8:
            lines.append("divert(0)divnum undivert(divnum) undivert(-3)")
        else:
            lines.append("divert(%d)define(`x', divnum)x" % g.c([-2, 4, 0]))
    lines.append("divert(%d)end" % g.c([0, 0, 1, 2]))
    if g.p(0.3):
        lines.append("divert(-1)m4wrap(`divert(1)wrapped divnum')")
    return single("\n".join(lines) + "\n")


def cat_wrap(g):
    lines = []
    for i in range(g.n(2, 5)):
        k = g.n(0, 4)
        if k == 0:
            lines.append("m4wrap(`w%d ')" % i)
        elif k == 1:
            lines.append("m4wrap(`m4wrap(`inner%d ')outer%d ')" % (i, i))
        elif k == 2:
            lines.append("m4wrap(`a%d', `b%d', `c')" % (i, i))
        elif k == 3:
            lines.append("define(`w%d', `W%d')m4wrap(`w%d')" % (i, i, i))
        else:
            lines.append("m4wrap(`divert(1)d%d divert(0)n%d')" % (i, i))
    lines.append(g.text_line())
    if g.p(0.3):
        lines.append("divert(2)two divert")
    if g.p(0.2):
        lines.append("m4wrap(`__line__ __file__')")
    return single("\n".join(lines) + "\n")


def cat_include(g):
    files = {}
    inc_name = g.c(["lib.m4", "defs.m4", "part.txt"])
    files["inc/" + inc_name] = "define(`libmac', `from lib: $1')dnl\nlib line __file__:__line__\n"
    files["other.m4"] = "other __file__ __line__\nlibmac(`o')\n"
    files["raw.txt"] = "raw `text' with libmac and # comment\n"
    lines = []
    args = []
    if g.p(0.6):
        args = ["-I", "inc"] if g.p(0.5) else ["-Iinc"]
    has_inc = bool(args)
    for _ in range(g.n(3, 6)):
        k = g.n(0, 8)
        if k == 0:
            lines.append("include(`%s')" % (inc_name if has_inc else "inc/" + inc_name))
        elif k == 1:
            lines.append("sinclude(`missing.m4')[s]")
        elif k == 2:
            lines.append("include(`other.m4')x")
        elif k == 3:
            lines.append("undivert(`raw.txt')")
        elif k == 4:
            lines.append("libmac(`arg') __file__ __line__")
        elif k == 5:
            lines.append("define(`f', `other.m4')include(f)")
        elif k == 6:
            lines.append("before include(`raw.txt') after")
        elif k == 7:
            lines.append("ifdef(`libmac', `defined', `undef')")
        else:
            lines.append("__line__ dnl\n__line__")
    files["input.m4"] = "\n".join(lines) + "\n"
    return files, args + ["input.m4"], "", False


def rand_expr(g, depth=0):
    if depth > 3 or g.p(0.3):
        k = g.n(0, 9)
        if k == 0:
            return "0x%x" % g.n(0, 255)
        if k == 1:
            return "0b%s" % bin(g.n(0, 31))[2:]
        if k == 2:
            return "0%o" % g.n(0, 63)
        if k == 3:
            return "0r%d:%s" % (g.c([3, 7, 36]), g.c(["12", "zz", "1a", "6"]))
        if k == 4:
            return str(g.c([2147483647, 65536, 1000000]))
        return str(g.n(0, 99))
    k = g.n(0, 6)
    if k == 0:
        return "(%s)" % rand_expr(g, depth + 1)
    if k == 1:
        return "%s%s" % (g.c(["-", "~", "!", "+"]), rand_expr(g, depth + 1))
    op = g.c(["+", "-", "*", "/", "%", "**", "<<", ">>", "<", "<=", ">", ">=", "==", "!=",
              "&", "|", "^", "&&", "||"])
    a = rand_expr(g, depth + 1)
    b = rand_expr(g, depth + 1)
    if op == "**":
        b = str(g.n(0, 5))
    if op in ("/", "%"):
        b = str(g.n(1, 9)) if g.p(0.9) else b
    return "%s %s %s" % (a, op, b)


def cat_eval(g):
    lines = []
    for _ in range(g.n(4, 8)):
        k = g.n(0, 9)
        if k <= 4:
            lines.append("eval(`%s')" % rand_expr(g))
        elif k == 5:
            lines.append("eval(`%s', %d)" % (rand_expr(g), g.c([2, 8, 16, 36, 1, 3])))
        elif k == 6:
            lines.append("eval(`%s', %d, %d)" % (rand_expr(g), g.c([10, 16, 2, 1]), g.n(0, 12)))
        elif k == 7:
            lines.append("eval(%s)" % g.c(["-2147483648 / -1", "-2147483648 % -1", "1 << 31",
                                         "-1 >> 1", "-8 >> 1", "2 ** 31", "0 || 1/0",
                                         "1 && 0", "1 || 1/0", "0 && 1%0", "-(-2147483648)",
                                         "7 % -3", "-7 / 2", "-7 % 2", "~0", "!0 + !5",
                                         "1 < 2 < 3", "3 == 3 != 0", "(1, 2)", "0x7fffffff + 1"]))
        elif k == 8:
            lines.append("define(`n', %d)eval(n * n - n)eval(`n + 1' > 3)" % g.n(0, 50))
        else:
            lines.append("eval(%s, `%s')" % (g.n(-100, 100000), g.c(["16", "2", "36", "10"])))
    return single("\n".join(lines) + "\n")


def cat_strings(g):
    lines = []
    for _ in range(g.n(5, 9)):
        s = g.words(1, 4)
        k = g.n(0, 9)
        if k == 0:
            lines.append("len(`%s') len(`') len(%s)" % (s, s.split()[0]))
        elif k == 1:
            sub = g.c(s.split() + ["", "zz", s[:2]])
            lines.append("index(`%s', `%s')" % (s, sub))
        elif k == 2:
            lines.append("substr(`%s', %d)" % (s, g.n(0, len(s) + 2)))
        elif k == 3:
            lines.append("substr(`%s', %d, %d)" % (s, g.n(0, len(s)), g.n(-1, len(s) + 3)))
        elif k == 4:
            frm = g.c(["a-z", "aeiou", "abc", "z-a", "a-", "-a", "a-c-e", "lo", "o", "A-Za-z", "0-9"])
            to = g.c(["A-Z", "", "xyz", "_", "1-3", "zyx", "*", "a-", "Z-A"])
            lines.append("translit(`%s', `%s', `%s')" % (s, frm, to))
        elif k == 5:
            lines.append("translit(`%s', `%s')" % (s, g.c(["aeiou", " ", "a-m", "xyz"])))
        elif k == 6:
            v = g.c(["5", "-3", "0", "2147483647", "-2147483648", "+4", "007"])
            lines.append("incr(%s) decr(%s)" % (v, v))
        elif k == 7:
            lines.append("define(`s', `%s')len(s) substr(s, 1, 3) index(s, `o')" % s)
        elif k == 8:
            lines.append("translit(`%s', `a-zA-Z', `n-za-mN-ZA-M')" % s)
        else:
            lines.append("substr(`%s', 2, 1)substr(`abc', 0, 0)substr(`abc', 3)substr(`abc', 1, 100)" % s)
    return single("\n".join(lines) + "\n")


def rand_regex(g, depth=0):
    atoms = ["a", "b", "o", "e", "l", "x", ".", "[a-c]", "[^ ]", "[aeiou]", "\\w", "\\W",
             "[0-9]", "[]x]", "[^a-z]", "\\s", "-", " ", "[[.a.]b]", "[a-]"]
    n = g.n(1, 4)
    parts = []
    for _ in range(n):
        k = g.n(0, 9)
        if k <= 4 or depth > 1:
            a = g.c(atoms)
        elif k <= 6:
            a = "\\(%s\\)" % rand_regex(g, depth + 1)
        elif k == 7:
            a = "\\(%s\\|%s\\)" % (rand_regex(g, depth + 1), rand_regex(g, depth + 1))
        elif k == 8:
            a = g.c(["\\b", "\\<", "\\>", "^", "$", "\\`", "\\'", "\\B"])
        else:
            a = g.c(["\\1", "\\{", "+", "*"])
        if g.p(0.35) and a not in ("^", "$", "\\b", "\\<", "\\>", "\\`", "\\'", "\\B"):
            a += g.c(["*", "+", "?", "*"])
        parts.append(a)
    return "".join(parts)


def cat_regex(g):
    lines = []
    for _ in range(g.n(4, 8)):
        s = g.c([g.words(1, 5), "hello world", "aaa bbb abab", "foo-bar_baz 123", "x\ny\nxy",
                 "one two  three", "a.b.c", "abcabc", "", "The Quick Brown Fox"])
        re_ = rand_regex(g)
        k = g.n(0, 7)
        repl = g.c(["<\\&>", "[\\1]", "\\2\\1", "X", "", "\\\\", "(\\&)", "\\n", "-\\0-"])
        if k <= 2:
            lines.append("patsubst(`%s', `%s', `%s')" % (s, re_, repl))
        elif k == 3:
            lines.append("patsubst(`%s', `%s')" % (s, re_))
        elif k == 4:
            lines.append("regexp(`%s', `%s')" % (s, re_))
        elif k == 5:
            lines.append("regexp(`%s', `%s', `%s')" % (s, re_, repl))
        elif k == 6:
            lines.append(g.c([
                "patsubst(`%s', `^', `> ')" % s,
                "patsubst(`%s', `$', `;')" % s,
                "patsubst(`%s', `\\w+', `\\&\\&')" % s,
                "patsubst(`%s', `\\<\\(\\w\\)\\(\\w*\\)', `\\1.\\2')" % s,
                "patsubst(`%s', ` +', ` ')" % s,
                "regexp(`%s', `\\(\\w+\\) \\(\\w+\\)', `\\2 \\1')" % s,
                "patsubst(`%s', `\\(.\\)\\1', `<\\1\\1>')" % s,
                "regexp(`%s', `o*')" % s,
                "patsubst(`%s', `[aeiou]', `')" % s,
                "patsubst(`%s', `x*', `-')" % s,
            ]))
        else:
            lines.append("define(`upcase', `translit(`$*', `a-z', `A-Z')')"
                         "define(`cap', `regexp(`$1', `^\\(\\w\\)\\(\\w*\\)', "
                         "`upcase(`\\1')`\\2'')')"
                         "define(`capall', `patsubst(`$1', `\\w+', `cap(`\\&')')')capall(`%s')" % s)
    return single("\n".join(lines) + "\n")


def rand_spec(g):
    conv = g.c("dddiuxXoscffeEgGc%")
    flags = ""
    for _ in range(g.n(0, 2)):
        flags += g.c(["-", "+", " ", "0", "#"])
    width = g.c(["", "", str(g.n(1, 12)), "*"])
    prec = g.c(["", "", "." + str(g.n(0, 8)), ".", ".*"])
    lmod = g.c(["", "", "", "", "l", "h", "hh"])
    return "%" + flags + width + prec + lmod + conv, conv, width, prec


def cat_format(g):
    lines = []
    for _ in range(g.n(4, 8)):
        specs = []
        args = []
        for _ in range(g.n(1, 3)):
            spec, conv, width, prec = rand_spec(g)
            specs.append(spec)
            if width == "*":
                args.append(str(g.n(-8, 10)))
            if prec == ".*":
                args.append(str(g.n(-2, 6)))
            if conv in "diuxXo":
                args.append(str(g.c([0, 1, -1, 42, -42, 255, 65535, 2147483647, -2147483648, 123456789])))
            elif conv == "c":
                args.append(str(g.c([65, 97, 48, 0, 321, 10])))
            elif conv == "s":
                args.append("`%s'" % g.words(1, 2))
            elif conv in "feEgG":
                args.append(g.c(["3.14159", "0", "-2.5", "1e10", "123456.789", "0.0001234", "1e-5",
                                 "2.5", "0.5", "1.5", "100", "-0", "inf", "nan", "6.02e23", "1e100",
                                 "999999.5", "0.00001", "12345678"]))
        fmt = "[" + "|".join(specs) + "]"
        lines.append("format(`%s', %s)" % (fmt, ", ".join(args)) if args else "format(`%s')" % fmt)
    if g.p(0.3):
        lines.append("format(`%s and %s', `x')format(`%d%%', 50)format(`%5.2s|', `abcdef')")
    if g.p(0.3):
        lines.append("define(`hex', `format(`0x%04x', $1)')hex(255) hex(4096)")
    return single("\n".join(lines) + "\n")


def cat_cli(g):
    k = g.n(0, 7)
    if k == 0:
        prog = "NAME VAL ifdef(`FLAG', `flag on', `flag off')\n"
        args = ["-DNAME=" + g.word(), "-D", "VAL", "-DFLAG", "input.m4"]
        if g.p(0.5):
            args.insert(-1, "-UFLAG")
        return single(prog, args)
    if k == 1:
        prog = "m4_define(`x', `prefixed')x define len(`a') m4_len(`abc') m4_ifdef(`m4_eval', `yes')\nm4_dnl comment\nm4___line__ m4_divnum\n"
        return single(prog, ["-P", "input.m4"])
    if k == 2:
        files = {"a.m4": "define(`shared', `from a')a: shared\n", "b.m4": "b: shared __file__\n"}
        return files, ["a.m4", "b.m4"], "", False
    if k == 3:
        files = {"a.m4": "a1 __file__ __line__\n"}
        return files, ["a.m4", "-", "a.m4"], "stdin: __file__ __line__\n", False
    if k == 4:
        files = {"a.m4": "A=X\n"}
        return files, ["-DX=1", "a.m4", "-DX=2", "a.m4", "-UX", "a.m4"], "", False
    if k == 5:
        return {}, [], "from stdin len(`abcd') __file__ __line__\n", False
    if k == 6:
        prog = "eval(1/0)len(`x', `y')after\n"
        return single(prog, ["-E", "input.m4"], "", True)
    files = {"-x.m4": "dash file\n", "input.m4": "normal\n"}
    return files, ["input.m4", "--", "-x.m4"], "", False


def cat_diagnostics(g):
    lines = []
    for _ in range(g.n(3, 7)):
        k = g.n(0, 21)
        lines.append([
            "len",
            "len(`a', `b')",
            "index(`abc')",
            "substr(`abc', `x')",
            "incr(`5x') decr(` 3')",
            "eval(`1 +')",
            "eval(`(1')",
            "eval(`1 / 0')",
            "eval(`2 ** -1')",
            "eval(`1 = 1')",
            "eval(`1 += 1')",
            "eval(`a')",
            "eval(`10', `40')",
            "eval(`10', `2', `-1')",
            "indir(`nosuch')",
            "builtin(`nosuch')",
            "dumpdef(`nosuch', `len')",
            "regexp(`abc', `\\(')",
            "patsubst(`abc', `[a')",
            "format(`%z %d')",
            "errprint(`message', `two')",
            "define(`d', `x')dumpdef(`d')define(`e', defn(`len'))dumpdef(`e')",
        ][k])
    tail = g.n(0, 8)
    if tail == 0:
        lines.append("`unterminated quote")
    elif tail == 1:
        lines.append("define(`x', `y'")
    elif tail == 2:
        lines.append("text m4exit(%d) not reached" % g.c([0, 1, 3, 255]))
    elif tail == 3:
        lines.append("include(`nonexistent.m4')after")
    elif tail == 4:
        lines.append("changecom(`/*', `*/')/* unterminated comment")
    elif tail == 5:
        lines.append("m4exit(`abc')")
    elif tail == 6:
        lines.append("divert(1)lost?m4exit(2)")
    elif tail == 7:
        lines.append("text dnl")
        return single("\n".join(lines), check=True)
    return single("\n".join(lines) + "\n", check=True)


def cat_lines(g):
    lines = []
    lines.append("define(`here', `__file__:__line__')dnl")
    lines.append("define(`multi', `first")
    lines.append("second __line__')dnl")
    for _ in range(g.n(3, 6)):
        k = g.n(0, 6)
        if k == 0:
            lines.append("here")
        elif k == 1:
            lines.append("multi")
        elif k == 2:
            lines.append("define(`foo', `called at __line__')foo(")
            lines.append("  arg)")
        elif k == 3:
            lines.append("__line__ `__line__' dnl ignored __line__")
        elif k == 4:
            lines.append("len(`a")
            lines.append("b')eval(1/0)")
        elif k == 5:
            lines.append("index(`x')substr(`")
            lines.append("',`q')")
        else:
            lines.append("m4wrap(`here')")
    return single("\n".join(lines) + "\n", check=True)


TEMPLATES = [
    # C enum + string table generator
    """divert(-1)
define(`ENUMS', `')
define(`enum', `define(`ENUMS', ENUMS`'ifelse(ENUMS, `', `', `, ')`$1')')
define(`_arg1', `$1')
define(`foreach', `pushdef(`$1')_foreach($@)popdef(`$1')')
define(`_foreach', `ifelse(`$2', `()', `',
  `define(`$1', _arg1$2)$3`'$0(`$1', (shift$2), `$3')')')
define(`upcase', `translit(`$1', `a-z', `A-Z')')
%(enums)s
divert(0)dnl
enum color { foreach(`c', (ENUMS), `COLOR_`'upcase(c), ')COLOR_COUNT };
static const char *color_names[] = {
foreach(`c', (ENUMS), `  "c",
')};
""",
    # HTML templating
    """define(`TAG', `<$1>$2</$1>')dnl
define(`LINK', `<a href="$1">ifelse(`$2', `', `$1', `$2')</a>')dnl
define(`ITEMS', `ifelse(`$#', `0', `', `$#', `1', `TAG(`li', `$1')', `TAG(`li', `$1')
ITEMS(shift($@))')')dnl
TAG(`h1', `%(title)s')
<ul>
ITEMS(%(items)s)
</ul>
LINK(`http://example.com/%(word)s') LINK(`/%(word)s', `%(title)s')
""",
    # config header
    """divert(-1)
define(`DEFINES', `')
define(`AC_DEFINE', `define(`DEFINES', defn(`DEFINES')`#define $1 'ifelse(`$2', `', `1', `$2')
)')
%(defs)s
divert(0)dnl
/* config.h generated */
DEFINES()dnl
#ifndef %(guard)s
#define %(guard)s eval(%(num)s)
#endif
""",
    # counter / numbered list
    """define(`N', 0)dnl
define(`item', `define(`N', incr(N))N. $1')dnl
define(`section', `
== format(`%%-10s', `$1') ==')dnl
section(`%(title)s')
%(items_lines)s
total: N
""",
    # struct printer
    """divert(-1)
define(`FIELDS', `(%(fields)s)')
define(`_arg1', `$1')
define(`foreach', `pushdef(`$1')_foreach($@)popdef(`$1')')
define(`_foreach', `ifelse(`$2', `()', `',
  `define(`$1', _arg1$2)$3`'$0(`$1', (shift$2), `$3')')')
define(`field_type', `ifelse(regexp(`$1', `^n'), `-1', `const char *', `int')')
define(`field_fmt', `ifelse(regexp(`$1', `^n'), `-1', `%%s', `%%d')')
divert(0)dnl
struct rec {
foreach(`f', FIELDS, `  field_type(f) f;
')};
void print_rec(const struct rec *r) {
foreach(`f', FIELDS, `  printf("f=field_fmt(f)\\n", r->f);
')}
""",
]


def cat_programs(g):
    k = g.n(0, len(TEMPLATES) - 1)
    words = [w for w in TEXT_WORDS if w.isalpha()]
    d = {
        "enums": "\n".join("enum(`%s')" % w for w in g.r.sample(words, g.n(1, 5))),
        "title": g.words(1, 3),
        "items": ", ".join("`%s'" % g.words(1, 2) for _ in range(g.n(1, 5))),
        "word": g.word(),
        "defs": "\n".join("AC_DEFINE(`HAVE_%s', %s)" % (w.upper(), g.c(["", "0", "`\"yes\"'"])) for w in g.r.sample(words, g.n(1, 4))),
        "guard": "CONFIG_" + g.c(words).upper(),
        "num": "%d * %d" % (g.n(1, 50), g.n(1, 50)),
        "items_lines": "\n".join("item(`%s')" % g.words(1, 3) for _ in range(g.n(1, 6))),
        "fields": ", ".join(g.r.sample(["name", "num", "id", "label", "nitems", "title", "nbytes"], g.n(1, 4))),
    }
    return single(TEMPLATES[k] % d)


# ---------------------------------------------------------------- deep cases

def rx_atom(g, depth, ngroups):
    k = g.n(0, 15)
    if k <= 3:
        return g.c(["a", "b", "ab", "ba", "c", "aa"]), ngroups
    if k == 4:
        return g.c([".", "\\.", "\\*", "\\[", "{", "|", "(", ")", "\\n", "\\+", "\\?"]), ngroups
    if k == 5:
        return g.c(["[ab]", "[^a]", "[a-c]", "[]a]", "[^]b]", "[a-]", "[-b]", "[c-a]", "[[.a.]]",
                    "[[=b=]c]", "[.]", "[*+]", "[^\n]", "[\\]", "[[:alpha:]]"]), ngroups
    if k == 6:
        return g.c(["\\w", "\\W", "\\s", "\\S"]), ngroups
    if k == 7:
        return g.c(["^", "$", "\\b", "\\B", "\\<", "\\>", "\\`", "\\'"]), ngroups
    if k == 8 and ngroups > 0:
        return "\\%d" % g.n(1, min(ngroups, 9)), ngroups
    if depth < 3 and k in (9, 10, 11):
        inner, ngroups2 = rx_seq(g, depth + 1, ngroups + 1)
        if g.p(0.4):
            alt, ngroups2 = rx_seq(g, depth + 1, ngroups2)
            inner = inner + "\\|" + alt
        return "\\(" + inner + "\\)", ngroups2
    if k == 12:
        return g.c(["*a", "\\(*a\\)", "a\\|*b", "^*", "a**", "a+?", "\\(\\)", "\\(^a\\)", "\\(a$\\)", "a\\{2\\}"]), ngroups
    return g.c(["a", "b"]), ngroups


def rx_seq(g, depth=0, ngroups=0):
    out = []
    for _ in range(g.n(1, 4)):
        a, ngroups = rx_atom(g, depth, ngroups)
        if g.p(0.35) and not a.startswith("^") and a not in ("$", "\\b", "\\B", "\\<", "\\>", "\\`", "\\'"):
            a += g.c(["*", "+", "?"])
        out.append(a)
    return "".join(out), ngroups


def rx_victim(g):
    k = g.n(0, 4)
    if k == 0:
        return "".join(g.c("ab") for _ in range(g.n(0, 12)))
    if k == 1:
        return "".join(g.c("abc .") for _ in range(g.n(0, 14)))
    if k == 2:
        return g.words(1, 4)
    if k == 3:
        return "".join(g.c(["ab", "ba", "a", "b", "\n", " ", "c", "aab"]) for _ in range(g.n(1, 6)))
    return g.c(["aaa", "abab", "abba", "a.b*c", "x{2}y", "(a|b)", "a\nb\nab", "", " a  b ", "ab_c d-e"])


def cat_regex_deep(g):
    lines = []
    for _ in range(g.n(5, 10)):
        r, ng = rx_seq(g)
        v = rx_victim(g)
        repl = g.c(["<\\&>", "[\\1]", "[\\1|\\2]", "\\2\\1", "-", "", "{\\&}\\\\", "\\3", "x\\", "\\\\&"])
        k = g.n(0, 4)
        if k <= 1:
            lines.append("patsubst(`%s', `%s', `%s')" % (v, r, repl))
        elif k == 2:
            lines.append("regexp(`%s', `%s')" % (v, r))
        elif k == 3:
            lines.append("regexp(`%s', `%s', `%s')" % (v, r, repl))
        else:
            lines.append("patsubst(`%s', `%s')" % (v, r))
    return single("\n".join(lines) + "\n", check=True)


def cat_format_deep(g):
    lines = []
    nums = ["0", "1", "-1", "0.5", "1.5", "2.5", "-2.5", "0.125", "1e-5", "1e-4", "123456", "1234567",
            "0.000123456", "99999.95", "9.9999995", "1e21", "-0", "3.0", "1.0e+2", "  7", "5x", "",
            "0x1p4", "0x10", "1e308", "2.2250738585072014e-308", "inf", "-inf", "nan", "1/3", ".5", "5."]
    ints = ["0", "1", "-1", "127", "128", "255", "256", "-129", "32767", "32768", "65535", "-32769",
            "2147483647", "-2147483648", "2147483648", "4294967295", "9223372036854775807",
            "-9223372036854775808", "99999999999999999999", "12abc", " 3", "+5", "", "0x1f", "010"]
    for _ in range(g.n(5, 9)):
        k = g.n(0, 5)
        flags = "".join(g.c(["-", "+", " ", "0", "#", "'"]) for _ in range(g.n(0, 2)))
        width = g.c(["", "", "8", "12", "1", "0", "20"])
        prec = g.c(["", "", ".0", ".1", ".3", ".10", ".", ".17"])
        if k <= 2:
            conv = g.c("feEgGaAF")
            lines.append("format(`[%%%s%s%s%s]', `%s')" % (flags, width, prec, conv, g.c(nums)))
        elif k == 3:
            conv = g.c("diouxX")
            lmod = g.c(["", "", "l", "h", "hh"])
            lines.append("format(`[%%%s%s%s%s%s]', `%s')" % (flags, width, prec, lmod, conv, g.c(ints)))
        elif k == 4:
            lines.append("format(`[%%*.*s|%%-*c|%%.*d]', %d, %d, `%s', %d, %d, %d, %s)" % (
                g.n(-6, 9), g.n(-2, 5), g.words(1, 2), g.n(-5, 5), g.c([65, 120, 48]), g.n(-1, 6), g.c(ints)))
        else:
            lines.append("format(`%s', %s)" % (g.c(["%5%|%-5d|", "%s %s %s", "%c%c%c", "%", "abc%",
                                                     "%%%d%%", "%1$d", "%n", "%p", "%lc", "%ls",
                                                     "%hs", "%Lf", "%qd", "%#c", "%+s", "%.3c"]),
                                           ", ".join(g.c(ints[:8]) for _ in range(g.n(0, 3)))))
    return single("\n".join(lines) + "\n", check=True)


def cat_rescan_edge(g):
    lines = []
    for _ in range(g.n(4, 8)):
        k = g.n(0, 17)
        lines.append([
            "define(`open', `define(')open`x', `X')x",
            "define(`lp', `(')define(`f', `[$1]')f`'lp`a)' f lp`b)'",
            "define(`cm', `,')define(`two', `$#:$1:$2')two(a cm b) two(a`'cm`'b) two(`a,b')",
            "define(`rp', `)')define(`g', `<$*>')g(a rp b",
            "define(`dl', `$')define(`m', `dl`'1 $`'1 $$1 ${1} $')m(X)",
            "define(`m', `$10 $1`'0 $01 $9')m(a,b,c,d,e,f,g,h,i,j,k)",
            "define(`fn', defn(`len'))fn(`abc') define(`u', `[$1]')u(defn(`len')) u(defn(`len')`x')",
            "define(`x', `y')define(`y', `z')x `x' ``x'' changequote([,])x [x] [[x]]",
            "define(`d', `dnl')d this vanishes\nnext",
            "define(`e', `len(`abc'')e)",
            "define(`a', `b')define(`b', `c')define(`c', `end')a(1) b() `a'(x)",
            "define(`q', `changequote(`[', `]')')q[x] `y' changequote`'`z'",
            "define(`w', `m4wrap(`$1')')w(`wrapped')w(`second ')after",
            "define(`x', `define(`x', `second')first')x x x",
            "pushdef(`p', `1')pushdef(`p', defn(`p')`2')p popdef(`p')p",
            "define(`echo', `$@')echo(`a', `b c', (d,e))|echo()|echo(,)",
            "define(`nest', `$1(`$2')')nest(`len', `abcd') nest(`nest', `len')",
            "define(`hash', `#')hash not comment # comment `#' quoted",
        ][k])
    return single("\n".join(lines) + "\n")


def cat_multichar(g):
    pairs = [("<<", ">>"), ("<<<", ">"), ("[[", "]"), ("{", "}}"), ("ab", "ba"), ("Q<", ">Q"),
             ("<!--", "-->"), ("\"", "\""), ("((", "))"), ("#", "!"), (",", ";"), ("x", "y")]
    lines = ["define(`v', `V')define(`w', `v.v')dnl"]
    for _ in range(g.n(2, 4)):
        lq, rq = g.c(pairs)
        if g.p(0.5):
            lines.append("changequote(`%s', `%s')dnl" % (lq, rq))
            body = [lq + "v" + rq, lq + lq + "w" + rq + rq, lq[:1] + "v", lq + "a" + rq[:1] + "b" + rq,
                    "v" + lq + "," + rq + "w", "define(" + lq + "z" + rq + ", " + lq + "Z" + rq + ")z"]
            lines.append(" ".join(g.r.sample(body, g.n(2, len(body)))))
            lines.append("changequote" + g.c(["", "()", "(`)"]) + "dnl")
            lines.append("changequote`'dnl")
        else:
            lines.append("changecom(`%s', `%s')dnl" % (lq, rq))
            body = [lq + " v " + rq + " w", lq[:1] + "v w", "w" + lq + "w" + rq + "w", "`" + lq + "' v"]
            lines.append(" ".join(g.r.sample(body, g.n(2, len(body)))))
            lines.append("changecom(`#')dnl")
    lines.append("v w `v'")
    return single("\n".join(lines) + "\n")


LIBS = [
    # insertion sort of integers
    """divert(-1)
define(`insert', `ifelse(`$2', `', `$1',
  `ifelse(eval(`$1' <= _first($2)), `1', `$1,$2', `_first($2),insert(`$1', _rest($2))')')')
define(`_first', `$1')
define(`_rest', `ifelse(`$#', `1', `', `shift($@)')')
define(`sort', `ifelse(`$#', `0', `', `$1', `', `', `insert(`$1', sort(shift($@)))')')
divert`'dnl
sort(%(nums)s)
sort(%(nums2)s)
""",
    # string utilities from the manual
    """divert(-1)
define(`upcase', `translit(`$*', `a-z', `A-Z')')
define(`downcase', `translit(`$*', `A-Z', `a-z')')
define(`capitalize1',
     `regexp(`$1', `^\\(\\w\\)\\(\\w*\\)', `upcase(`\\1')`'downcase(`\\2')')')
define(`capitalize', `patsubst(`$1', `\\w+', `capitalize1(`\\&')')')
define(`reverse', `ifelse(len(`$1'), 0, , `reverse(substr(`$1', 1))`'substr(`$1', 0, 1)')')
define(`trim', `patsubst(patsubst(`$1', `^ +'), ` +$')')
divert`'dnl
capitalize(`%(phrase)s')
reverse(`%(word)s') len(`%(phrase)s')
[trim(`   %(word)s  ')]
upcase(`%(phrase)s') downcase(`ABC Def')
""",
    # counters, stacks and diversion-based sections
    """divert(-1)
define(`SEC', 0)
define(`section', `define(`SEC', incr(SEC))divert(SEC)`'
Section SEC: $1
divert(-1)')
define(`toc', `define(`TOC', defn(`TOC')`  'SEC. $1
)')
define(`TOC', `')
define(`chapter', `toc(`$1')section(`$1')')
%(chapters)s
divert(0)dnl
Contents:
TOC`'dnl
undivert
""",
    # base conversion table
    """define(`row', `eval($1, 10, 3) eval($1, 2, 8) eval($1, 8, 3) eval($1, 16, 2) eval($1, 36)')dnl
define(`table', `ifelse(eval($1 > $2), 1, , `row($1)
table(incr($1), $2)')')dnl
table(%(lo)d, %(hi)d)dnl
""",
    # macro-generated macros
    """define(`defvar', `define(`get_$1', `$2')define(`set_$1', `define(`get_$1', `$'`1')')')dnl
defvar(`%(w1)s', `%(v1)s')defvar(`%(w2)s', `%(v2)s')dnl
get_%(w1)s get_%(w2)s
set_%(w1)s(`changed')get_%(w1)s
define(`apply', `$1(shift($@))')apply(`format', `%%s-%%s', `a', `b')
indir(`get_%(w2)s') ifdef(`set_%(w2)s', `has setter')
""",
]


def cat_library(g):
    k = g.n(0, len(LIBS) - 1)
    words = [w for w in TEXT_WORDS if w.isalpha()]
    nums = [str(g.n(-20, 99)) for _ in range(g.n(1, 7))]
    lo = g.n(0, 40)
    d = {
        "nums": ",".join(nums),
        "nums2": ",".join(str(g.n(0, 9)) for _ in range(g.n(2, 8))),
        "phrase": g.words(2, 5),
        "word": g.c(words),
        "chapters": "\n".join("chapter(`%s')" % g.words(1, 3) for _ in range(g.n(1, 5))),
        "lo": lo, "hi": lo + g.n(0, 12),
        "w1": g.c(words), "w2": g.c(words) + "2", "v1": g.words(1, 2), "v2": g.words(1, 2),
    }
    return single(LIBS[k] % d)


CATEGORIES = [
    ("quote_rescan", cat_quote_rescan),
    ("args", cat_args),
    ("defs", cat_defs),
    ("control", cat_control),
    ("quotes_comments", cat_quotes_comments),
    ("diversions", cat_diversions),
    ("wrap", cat_wrap),
    ("include", cat_include),
    ("eval", cat_eval),
    ("strings", cat_strings),
    ("regex", cat_regex),
    ("format", cat_format),
    ("cli", cat_cli),
    ("diagnostics", cat_diagnostics),
    ("lines", cat_lines),
    ("programs", cat_programs),
    ("regex_deep", cat_regex_deep),
    ("format_deep", cat_format_deep),
    ("rescan_edge", cat_rescan_edge),
    ("multichar", cat_multichar),
    ("library", cat_library),
]


def main():
    seed = int(sys.argv[1])
    per = int(sys.argv[2])
    out = sys.argv[3]
    prefix = sys.argv[5] if len(sys.argv) > 5 and sys.argv[4] == "--prefix" else "c"
    rnd = random.Random(seed)
    g = G(rnd)
    seen = set()
    n = 0
    with open(out, "w") as f:
        for cname, fn in CATEGORIES:
            made = 0
            tries = 0
            while made < per and tries < per * 20:
                tries += 1
                files, args, stdin, check = fn(g)
                key = json.dumps([files, args, stdin], sort_keys=True)
                if key in seen:
                    continue
                seen.add(key)
                n += 1
                made += 1
                f.write(json.dumps({"id": "%s%05d" % (prefix, n), "category": cname,
                                    "args": args, "files": files, "stdin": stdin,
                                    "check_stderr": bool(check)}) + "\n")


if __name__ == "__main__":
    main()
