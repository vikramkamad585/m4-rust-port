#!/usr/bin/env python3
"""Generate recutils test invocations (inputs only).

Usage: gen_rec.py SEED PER_CATEGORY OUT.jsonl PREFIX

Each line: {"id", "category", "tool", "args", "files", "stdin", "check_stderr"}
"""
import json
import random
import sys

FIRST = ["Ada", "Alan", "Grace", "Linus", "Ken", "Dennis", "Barbara", "Edsger", "Donald", "Margaret",
         "Radia", "John", "Frances", "Niklaus", "Leslie", "Anita", "Bjarne", "Guido", "Yukihiro", "Rob"]
LAST = ["Lovelace", "Turing", "Hopper", "Torvalds", "Thompson", "Ritchie", "Liskov", "Dijkstra",
        "Knuth", "Hamilton", "Perlman", "McCarthy", "Allen", "Wirth", "Lamport", "Borg", "Stroustrup",
        "van Rossum", "Matsumoto", "Pike"]
WORDS = ["alpha", "beta", "gamma", "delta", "systems", "compilers", "networks", "databases", "theory",
         "graphics", "security", "kernel", "editor", "parser", "runtime", "cache", "queue", "stream"]
DEPTS = ["Eng", "Ops", "Sales", "Research", "Support", "Legal"]
CITIES = ["Paris", "Berlin", "Tokyo", "Lima", "Oslo", "Cairo", "Austin", "Pune"]


class G:
    def __init__(self, r):
        self.r = r

    def c(self, s):
        return self.r.choice(s)

    def n(self, a, b):
        return self.r.randint(a, b)

    def p(self, x):
        return self.r.random() < x

    def words(self, a=1, b=3):
        return " ".join(self.c(WORDS) for _ in range(self.n(a, b)))

    def person(self):
        return "%s %s" % (self.c(FIRST), self.c(LAST))

    def num(self):
        k = self.n(0, 12)
        if k == 0:
            return "0%d" % self.n(1, 7)        # octal-looking
        if k == 1:
            return "0x%X" % self.n(1, 255)
        if k == 2:
            return str(-self.n(1, 50))
        if k == 3:
            return " %d" % self.n(0, 9)
        if k == 4:
            return "%d.%d" % (self.n(0, 99), self.n(0, 99))
        if k == 5:
            return ""
        if k == 6:
            return "%dabc" % self.n(1, 9)
        return str(self.n(0, 120))

    def real(self):
        return self.c(["%d.%02d" % (self.n(0, 999), self.n(0, 99)), str(self.n(0, 500)), "1e3", "-2.5", "0.5", ".75"])

    def text(self):
        k = self.n(0, 9)
        if k == 0:
            return self.words(1, 2) + "\n" + self.words(1, 2)          # multi-line
        if k == 1:
            return 'say "%s"' % self.c(WORDS)
        if k == 2:
            return self.c(WORDS).upper()
        if k == 3:
            return "  " + self.c(WORDS) + "  "
        if k == 4:
            return self.c(WORDS) + "\\" + self.c(WORDS)
        return self.words(1, 3)


def encode_value(g, v):
    """Render a value in rec syntax, choosing among equivalent encodings."""
    if "\n" in v:
        lines = v.split("\n")
        return lines[0] + "".join("\n+ " + l if (l and g.p(0.8)) else "\n+" + l for l in lines[1:])
    if g.p(0.05) and len(v) > 4:
        cut = len(v) // 2
        return v[:cut] + "\\\n" + v[cut:]
    return v


def render_record(g, fields, comments=True):
    out = []
    for name, val in fields:
        if comments and g.p(0.04):
            out.append("# note about %s" % name)
        sep = g.c([": ", ": ", ": ", ":", ":\t"]) if val else g.c([":", ": "])
        if val.startswith(" ") or val.startswith("\t"):
            sep = ": "
        out.append(name + sep + encode_value(g, val))
    return "\n".join(out)


def book_set(g, nrec, typed=True, name="Book"):
    desc = ["%rec: " + name]
    if typed:
        desc.append("%type: Year int")
        if g.p(0.6):
            desc.append("%type: Price real")
        if g.p(0.3):
            desc.append("%type: Available bool")
        if g.p(0.3):
            desc.append("%sort: " + g.c(["Year", "Title", "Price", "Year Title"]))
        if g.p(0.2):
            desc.append("%key: Title")
    recs = []
    for _ in range(nrec):
        f = [("Title", g.words(1, 3).title()), ("Author", g.person())]
        if g.p(0.3):
            f.append(("Author", g.person()))
        if g.p(0.9):
            f.append(("Year", g.c([str(g.n(1950, 2024)), str(g.n(1950, 2024)), g.num()])))
        if g.p(0.7):
            f.append(("Price", g.real()))
        for _ in range(g.n(0, 3)):
            f.append(("Tag", g.c(WORDS)))
        if g.p(0.4):
            f.append(("Available", g.c(["yes", "no", "true", "false", "1", "0", "maybe"])))
        if g.p(0.3):
            f.append(("Note", g.text()))
        if g.p(0.3):
            g.r.shuffle(f)
        recs.append(f)
    return desc, recs


def staff_set(g, nrec, name="Staff"):
    desc = ["%rec: " + name, "%type: Age int", "%type: Salary real"]
    if g.p(0.4):
        desc.append("%type: Level range 1 9")
    if g.p(0.3):
        desc.append("%typedef: Dept_t enum " + " ".join(DEPTS))
        desc.append("%type: Dept Dept_t")
    if g.p(0.3):
        desc.append("%sort: " + g.c(["Age", "Name", "Salary", "Dept Age", "Level"]))
    recs = []
    for i in range(nrec):
        f = [("Id", str(i + 1)), ("Name", g.person())]
        if g.p(0.9):
            f.append(("Age", g.c([str(g.n(18, 70)), g.num()])))
        if g.p(0.8):
            f.append(("Salary", g.c(["%d" % (g.n(20, 200) * 1000), "%d.50" % (g.n(20, 200) * 1000), g.real()])))
        f.append(("Dept", g.c(DEPTS)))
        if g.p(0.5):
            f.append(("Level", str(g.n(1, 9))))
        for _ in range(g.n(0, 2)):
            f.append(("Email", "%s@example.org" % g.c(FIRST).lower()))
        if g.p(0.3):
            f.append(("City", g.c(CITIES)))
        recs.append(f)
    return desc, recs


def render_set(g, desc, recs, with_desc=True):
    parts = []
    if g.p(0.2):
        parts.append("# " + g.words(2, 4))
    if with_desc and desc:
        parts.append("\n".join(desc))
    for r in recs:
        parts.append(render_record(g, r))
    sep = g.c(["\n\n", "\n\n", "\n\n\n", "\n\n# between\n\n"])
    return sep.join(parts) + "\n"


def dataset(g, kind=None, nrec=None):
    kind = kind or g.c(["book", "staff"])
    nrec = nrec if nrec is not None else g.n(2, 9)
    if kind == "book":
        d, r = book_set(g, nrec)
    else:
        d, r = staff_set(g, nrec)
    return kind, d, r


def field_names(kind):
    return {"book": ["Title", "Author", "Year", "Price", "Tag", "Available", "Note"],
            "staff": ["Id", "Name", "Age", "Salary", "Dept", "Level", "Email", "City"]}[kind]


def type_name(kind):
    return {"book": "Book", "staff": "Staff"}[kind]


def rand_cond(g, kind, depth=0):
    names = field_names(kind)
    numeric = {"book": ["Year", "Price"], "staff": ["Age", "Salary", "Level", "Id"]}[kind]
    textual = {"book": ["Title", "Author", "Tag", "Note", "Available"], "staff": ["Name", "Dept", "Email", "City"]}[kind]
    k = g.n(0, 15)
    if depth < 2 and k <= 2:
        op = g.c(["&&", "||", "=>"])
        return "(%s) %s (%s)" % (rand_cond(g, kind, depth + 1), op, rand_cond(g, kind, depth + 1))
    if depth < 2 and k == 3:
        return "!(%s)" % rand_cond(g, kind, depth + 1)
    if k <= 7:
        f = g.c(numeric)
        op = g.c(["<", ">", "<=", ">=", "=", "!="])
        rhs = g.c([str(g.n(0, 2010)), str(g.n(0, 100)), "%d.5" % g.n(0, 100), g.c(numeric)])
        lhs = f
        if g.p(0.2):
            lhs = "%s %s %d" % (f, g.c(["+", "*", "/", "%"]), g.n(1, 9))
        return "%s %s %s" % (lhs, op, rhs)
    if k <= 10:
        f = g.c(textual)
        op = g.c(["=", "!=", "~"])
        if op == "~":
            rhs = g.c(["'^%s'" % g.c(WORDS)[:2], "'%s$'" % g.c(WORDS)[-2:], "'[aeiou]{2}'", "'^[A-Z]'",
                       "'(ta|er)'", "'%s|%s'" % (g.c(WORDS)[:3], g.c(LAST)[:3]), "'e.*a'", "'^$'", "'[[:upper:]][[:lower:]]+ [A-Z]'"])
        else:
            rhs = g.c(['"%s"' % g.c(WORDS), "'%s'" % g.c(DEPTS + CITIES), '"%s"' % g.c(FIRST), '""'])
        return "%s %s %s" % (f, op, rhs)
    if k == 11:
        return "#%s %s %d" % (g.c(names), g.c(["=", ">", "<", ">="]), g.n(0, 3))
    if k == 12:
        return "%s[%d] %s %s" % (g.c(textual), g.n(0, 2), g.c(["=", "!="]), '"%s"' % g.c(WORDS + FIRST))
    if k == 13:
        return "(%s %s %d) ? %s : %s" % (g.c(numeric), g.c([">", "<"]), g.n(0, 100), g.c(["1", "0"]), g.c(["1", "0", "#%s" % g.c(names)]))
    if k == 14:
        return "%s & '%s' = '%s'" % (g.c(textual), g.c(["x", ""]), g.c(WORDS))
    return "%s" % g.c(numeric)


def cat_parse(g):
    kind, d, r = dataset(g)
    k = g.n(0, 4)
    text = render_set(g, d, r, with_desc=g.p(0.6))
    files = {"data.rec": text}
    if k == 0:
        return "recsel", ["data.rec"], files, ""
    if k == 1:
        return "recsel", [], {}, text
    if k == 2:
        return "recsel", ["-C", "data.rec"], files, ""
    if k == 3:
        return "recsel", ["-d", "data.rec"], files, ""
    return "recsel", ["-t", type_name(kind), "data.rec"] if "%rec" in text else ["data.rec"], files, ""


def cat_parse_edge(g):
    k = g.n(0, 11)
    v = g.words(1, 3)
    texts = [
        "a: 1\nb: 2",                                     # no trailing newline
        "a: x\\\ny\nb: line1\n+ line2\n+\n+  indented\n",
        "\n\n  \n\t\na: 1\n\n\n\nb: 2\n\n",
        "# c1\n# c2\na: 1\n# inner\nb: 2\n\n# trailing\n",
        "a:no space\nb:\t tab\nc:\n",
        "%rec: T\n\n%rec: U\n\nx: 1\n",                     # empty first set
        "x: 1\n\n%rec: T\n\ny: 2\n",                       # anonymous then typed
        "a: 1\nbad line\n",
        "a: 1\n b: 2\n",
        "1a: x\n",
        "a: 1\n\n%rec: T\n%rec: T2\n\nb: 2\n",
        "Name: %s\nName: %s\nValue: \\\\back\\slash\n" % (v, v),
    ]
    text = texts[k]
    args = g.c([[], ["-t", "T"], ["-C"], ["-c"], ["-d", "-t", "T"]])
    return "recsel", args + ["in.rec"], {"in.rec": text}, ""


def cat_select(g):
    kind, d, r = dataset(g, nrec=g.n(3, 10))
    files = {"data.rec": render_set(g, d, r)}
    cond = rand_cond(g, kind)
    args = ["-t", type_name(kind)]
    if g.p(0.15):
        args.append("-i")
    args += ["-e", cond]
    if g.p(0.3):
        args += ["-p", ",".join(g.r.sample(field_names(kind), g.n(1, 3)))]
    if g.p(0.15):
        args.append("-c")
        args = [a for a in args if a not in ("-p",)]
        if "-p" in args:
            pass
    args.append("data.rec")
    # -p and -c conflict is fine: it is part of the surface
    return "recsel", args, files, ""


def cat_print(g):
    kind, d, r = dataset(g)
    files = {"data.rec": render_set(g, d, r)}
    names = field_names(kind)
    elems = []
    for _ in range(g.n(1, 4)):
        nm = g.c(names)
        k = g.n(0, 5)
        if k == 0:
            elems.append("%s[%d]" % (nm, g.n(0, 2)))
        elif k == 1:
            elems.append("%s[%d-%d]" % (nm, g.n(0, 1), g.n(1, 3)))
        elif k == 2:
            elems.append("%s:%s" % (nm, g.c(["Alias", "X", "out_name"])))
        else:
            elems.append(nm)
    flag = g.c(["-p", "-p", "-P", "-R"])
    args = ["-t", type_name(kind), flag, ",".join(elems)]
    if g.p(0.3):
        args.insert(0, "-C")
    if g.p(0.25):
        args.insert(0, "-U")
    args.append("data.rec")
    return "recsel", args, files, ""


def cat_sort(g):
    kind, d, r = dataset(g, nrec=g.n(3, 10))
    files = {"data.rec": render_set(g, d, r)}
    names = field_names(kind)
    args = ["-t", type_name(kind)]
    if g.p(0.85):
        args += ["-S", ",".join(g.r.sample(names, g.n(1, 2)))]
    args += [g.c(["-p", "-R", "-P"]), ",".join(g.r.sample(names, g.n(1, 3)))]
    if g.p(0.3):
        args.append("-C")
    args.append("data.rec")
    return "recsel", args, files, ""


def cat_group(g):
    kind, d, r = dataset(g, nrec=g.n(3, 12))
    files = {"data.rec": render_set(g, d, r)}
    names = field_names(kind)
    numeric = {"book": ["Year", "Price"], "staff": ["Age", "Salary", "Level"]}[kind]
    grp = {"book": ["Author", "Year", "Available", "Tag"], "staff": ["Dept", "City", "Level", "Age"]}[kind]
    funcs = []
    for _ in range(g.n(1, 3)):
        fn = g.c(["Count", "Sum", "Avg", "Min", "Max", "count", "AVG"])
        fld = g.c(numeric + names)
        alias = (":" + g.c(["Total", "N", "Result"])) if g.p(0.25) else ""
        funcs.append("%s(%s)%s" % (fn, fld, alias))
    args = ["-t", type_name(kind)]
    if g.p(0.7):
        gb = g.c(grp)
        args += ["-G", gb, "-p", ",".join([gb] + funcs)]
    else:
        args += ["-p", ",".join(funcs)]
        if g.p(0.3):
            args += ["-e", rand_cond(g, kind)]
    if g.p(0.2):
        args += ["-S", g.c(names)]
    args.append("data.rec")
    return "recsel", args, files, ""


def cat_quick_index(g):
    kind, d, r = dataset(g, nrec=g.n(3, 10))
    files = {"data.rec": render_set(g, d, r)}
    args = ["-t", type_name(kind)]
    k = g.n(0, 2)
    if k == 0:
        args += ["-q", g.c(WORDS + [x[:3] for x in FIRST] + ["e", "ALPHA", "Eng"])]
        if g.p(0.4):
            args.append("-i")
    elif k == 1:
        parts = []
        for _ in range(g.n(1, 3)):
            a = g.n(0, 8)
            parts.append(str(a) if g.p(0.6) else "%d-%d" % (a, a + g.n(0, 4)))
        args += ["-n", ",".join(parts)]
    else:
        args += ["-n", str(g.n(0, 5)), "-p", g.c(field_names(kind))]
    if g.p(0.3):
        args += ["-S", g.c(field_names(kind))]
    args.append("data.rec")
    return "recsel", args, files, ""


def cat_join(g):
    nauth = g.n(2, 5)
    authors = g.r.sample(FIRST, nauth)
    adesc = ["%rec: Author", "%key: Handle"]
    arecs = [[("Handle", a.lower()), ("Name", "%s %s" % (a, g.c(LAST))), ("Born", str(g.n(1900, 1990)))] for a in authors]
    bdesc = ["%rec: Book", "%type: Writer rec Author" if g.p(0.85) else "%type: Writer line", "%type: Year int"]
    brecs = []
    for _ in range(g.n(2, 7)):
        f = [("Title", g.words(1, 3).title())]
        for _ in range(g.n(0, 2)):
            f.append(("Writer", g.c(authors).lower() if g.p(0.85) else "nobody"))
        f.append(("Year", str(g.n(1950, 2020))))
        brecs.append(f)
    text = render_set(g, bdesc, brecs) + "\n" + render_set(g, adesc, arecs)
    args = ["-t", "Book", "-j", "Writer"]
    if g.p(0.4):
        args += ["-p", g.c(["Title,Writer_Name", "Writer_Handle,Year", "Title,Writer_Born,Writer_Name"])]
    if g.p(0.3):
        args += ["-e", g.c(["Writer_Born > 1950", "Year < 2000", "#Writer_Name > 0"])]
    if g.p(0.2):
        args = ["-t", "Book", "-j", g.c(["Title", "Writer"]), "-d"]
    args.append("lib.rec")
    return "recsel", args, {"lib.rec": text}, ""


def cat_sexp_desc(g):
    kind, d, r = dataset(g, nrec=g.n(1, 5))
    text = render_set(g, d, r)
    k = g.n(0, 3)
    if k == 0:
        return "recsel", ["--print-sexps", "-t", type_name(kind), "data.rec"], {"data.rec": text}, ""
    if k == 1:
        return "recsel", ["-d", "-t", type_name(kind), "-e", rand_cond(g, kind), "data.rec"], {"data.rec": text}, ""
    if k == 2:
        return "recinf", ["-S", "-d", "data.rec"], {"data.rec": text}, ""
    return "recsel", ["--print-sexps", "-t", type_name(kind), "-p", ",".join(g.r.sample(field_names(kind), 2)), "data.rec"], {"data.rec": text}, ""


def cat_recinf(g):
    sets = []
    for _ in range(g.n(1, 3)):
        kind, d, r = dataset(g, nrec=g.n(0, 4))
        sets.append((kind, d, r))
    seen = set()
    parts = []
    if g.p(0.3):
        parts.append(render_record(g, [("Loose", "record")]) + "\n")
    for kind, d, r in sets:
        name = type_name(kind)
        if name in seen:
            name = name + "2"
            d = ["%rec: " + name] + d[1:]
        seen.add(name)
        parts.append(render_set(g, d, r))
    text = "\n".join(parts)
    args = g.c([[], ["-n"], ["-d"], ["-t", "Book"], ["-t", "Staff", "-d"], ["--names-only"], ["--type=Book"]])
    return "recinf", args + ["db.rec"], {"db.rec": text}, ""


def cat_csv(g):
    kind, d, r = dataset(g, nrec=g.n(1, 7))
    text = render_set(g, d, r)
    args = []
    if g.p(0.6):
        args += ["-t", type_name(kind)]
    if g.p(0.4):
        args += ["-S", g.c(field_names(kind))]
    if g.p(0.3):
        args += ["-d", g.c([";", "\t", "|"])]
    return "rec2csv", args + ["data.rec"], {"data.rec": text}, ""


def cat_recfmt(g):
    kind, d, r = dataset(g, nrec=g.n(1, 5))
    names = field_names(kind)
    numeric = {"book": ["Year", "Price"], "staff": ["Age", "Salary", "Level"]}[kind]
    slots = []
    for _ in range(g.n(1, 4)):
        k = g.n(0, 6)
        if k <= 2:
            slots.append("{{%s}}" % g.c(names))
        elif k == 3:
            slots.append("{{%s %s %d}}" % (g.c(numeric), g.c(["+", "*", "/"]), g.n(1, 9)))
        elif k == 4:
            slots.append("{{%s * 1.5}}" % g.c(numeric))
        elif k == 5:
            slots.append("{{#%s}}" % g.c(names))
        else:
            slots.append("{{%s[1]}}" % g.c(names))
    tmpl = g.c(["", "- ", "* "]) + g.c([" | ", ", ", " "]).join(slots) + g.c(["\n", "\n", ";", "\n\n"])
    data = render_set(g, d, r, with_desc=False)
    if g.p(0.4):
        return "recfmt", ["-f", "tmpl.txt"], {"tmpl.txt": tmpl}, data
    return "recfmt", [tmpl], {}, data


def cat_cli(g):
    kind, d, r = dataset(g, nrec=g.n(1, 4))
    files = {"data.rec": render_set(g, d, r)}
    T = type_name(kind)
    variants = [
        ["-e", "Age > 1", "-n", "1", "data.rec"],
        ["-n", "1", "-e", "Age > 1", "data.rec"],
        ["-q", "x", "-e", "1", "data.rec"],
        ["-p", "Name", "-c", "data.rec"],
        ["-c", "-p", "Name", "-t", T, "data.rec"],
        ["-t", "9bad", "data.rec"],
        ["-t", T, "-p", "Name[", "data.rec"],
        ["-t", T, "-p", "Foo(Name)", "data.rec"],
        ["-t", T, "-S", "a,,b", "data.rec"],
        ["-t", T, "-G", "x y", "data.rec"],
        ["-j", "Name", "data.rec"],
        ["-t", T, "-e", "Name = ", "data.rec"],
        ["-t", T, "-e", "Age ~ 3", "data.rec"],
        ["-t", T, "missing.rec"],
        ["-t", T, "-z", "data.rec"],
        ["--type", T, "--count", "data.rec"],
        ["--ty=" + T, "--print=Name", "data.rec"],
        ["--print", "data.rec"],
        ["-t", T, "--uniq", "--collapse", "data.rec", "data.rec"],
        ["-t", "Nothing", "data.rec"],
        ["-t", T, "-n", "1-a", "data.rec"],
        ["-t", T, "-S", "Name", "-S", "Age", "data.rec"],
        ["data.rec", "-t", T, "-p", "Name"],
        ["-t", T, "-e", "#Name.First = 0", "-p", "Name", "data.rec"],
    ]
    return "recsel", g.c(variants), files, ""


def cat_typed(g):
    vals = [str(x) for x in (g.n(-5, 30) for _ in range(g.n(3, 7)))]
    t = g.c(["int", "real", "line", "range 0 100", "range MIN MAX", "bool", "integer", "size 10",
             "enum A B C", "regexp /^[0-9]+$/", "reals", "Num", "  int  "])
    desc = ["%rec: Item", "%type: V " + t]
    if t == "Num":
        desc.insert(1, "%typedef: Num " + g.c(["int", "real", "Other", "line"]))
        if g.p(0.3):
            desc.insert(1, "%typedef: Other int")
    recs = []
    for v in vals:
        if g.p(0.2):
            v = g.c(["0x10", "010", "1.5", "abc", "", "-0", "yes", "true", "0", "1e2"])
        f = [("Name", g.c(WORDS))]
        if g.p(0.9):
            f.append(("V", v))
        recs.append(f)
    text = render_set(g, desc, recs)
    args = ["-t", "Item", "-S", g.c(["V", "V,Name", "Name,V"]), "-p", "Name,V"]
    if g.p(0.3):
        args = ["-t", "Item", "-G", "V", "-p", "V,Count(Name)"]
    return "recsel", args + ["items.rec"], {"items.rec": text}, ""


def cat_diag(g):
    k = g.n(0, 9)
    if k == 0:
        return "recsel", ["bad.rec"], {"bad.rec": "a: 1\n%s\n" % g.c(["oops", "-x: 2", "b c: 3"])}, ""
    if k == 1:
        return "recsel", ["-t", "A", "a.rec", "b.rec"], {"a.rec": "%rec: A\n\nx: 1\n", "b.rec": "%rec: A\n\nx: 2\n"}, ""
    if k == 2:
        return "recinf", ["bad.rec"], {"bad.rec": "x: 1\n\n y: 2\n"}, ""
    if k == 3:
        return "recfmt", ["{{Name + }}"], {}, "Name: x\n"
    if k == 4:
        return "recfmt", ["{{Name ~ 'x'}}{{Age / 0}}"], {}, "Name: x\nAge: 3\n"
    if k == 5:
        return "rec2csv", ["-S", "1x", "d.rec"], {"d.rec": "a: 1\n"}, ""
    if k == 6:
        return "recsel", ["-t", "A", "-e", "a < 'x' && b", "d.rec"], {"d.rec": "%rec: A\n\na: 1\nb: 2\n"}, ""
    if k == 7:
        return "recinf", ["nofile.rec"], {}, ""
    if k == 8:
        return "recsel", ["x.rec", "y.rec"], {"x.rec": "a: 1\n", "y.rec": "b: 2\n\nc: 3\n"}, ""
    return "recfmt", ["-f", "missing.tmpl"], {}, "a: 1\n"


CATEGORIES = [
    ("parse", cat_parse), ("parse_edge", cat_parse_edge), ("select", cat_select),
    ("print", cat_print), ("sort", cat_sort), ("group", cat_group), ("quick_index", cat_quick_index),
    ("join", cat_join), ("sexp_desc", cat_sexp_desc), ("recinf", cat_recinf), ("csv", cat_csv),
    ("recfmt", cat_recfmt), ("cli", cat_cli), ("typed", cat_typed), ("diag", cat_diag),
]


def main():
    seed, per, out, prefix = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3], sys.argv[4]
    g = G(random.Random(seed))
    seen = set()
    n = 0
    with open(out, "w") as f:
        for cname, fn in CATEGORIES:
            made = tries = 0
            while made < per and tries < per * 30:
                tries += 1
                tool, args, files, stdin = fn(g)
                key = json.dumps([tool, args, files, stdin], sort_keys=True)
                if key in seen:
                    continue
                seen.add(key)
                made += 1
                n += 1
                f.write(json.dumps({"id": "%s%05d" % (prefix, n), "category": cname, "tool": tool,
                                    "args": args, "files": files, "stdin": stdin,
                                    "check_stderr": cname in ("diag", "cli", "parse_edge")}) + "\n")


if __name__ == "__main__":
    main()
