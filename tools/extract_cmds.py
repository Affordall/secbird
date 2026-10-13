"""Extract Razer protocol command tuples ([size, class, id]) and their names.

Within each webpack module, constants appear as runs of Uint8Array tuples
followed by a descriptive string, e.g.
    u = new Uint8Array([3, 12, 2]), l = "Set Analog Control Sensitivity",
A tuple with id >= 0x80 is the GET variant of the same command (id | 0x80).

usage: python -I extract_cmds.py <pretty dir> <out.json>
"""
import json, os, re, sys

src, out = sys.argv[1], sys.argv[2]
MOD = re.compile(r"^\s{8}(\d+): function\(", re.M)
TOK = re.compile(r'\b[\w$]+ = (?:new Uint8Array\(\[(\d+), (\d+), (\d+)\]\)|"((?:Get|Set|Reset|Clear|Enable|Disable|Read|Write|Start|Stop|Request)\b[^"]{2,80})")')
GAP = 200

cmds = {}
for fn in sorted(os.listdir(src)):
    if not fn.endswith(".js"):
        continue
    text = open(os.path.join(src, fn), encoding="utf-8", errors="replace").read()
    starts = [m.start() for m in MOD.finditer(text)] + [len(text)]
    for a, b in zip(starts, starts[1:]):
        pending = []
        last = a
        for m in TOK.finditer(text, a, b):
            if m.start() - last > GAP:
                pending = []
            last = m.end()
            if m.group(1):
                pending.append(tuple(int(x) for x in m.group(1, 2, 3)))
            else:
                name = m.group(4)
                for size, cls, cid in pending:
                    key = (cls, cid & 0x7F)
                    e = cmds.setdefault(key, {"class": cls, "id": cid & 0x7F, "names": {}, "sizes": {}, "files": set()})
                    e["names"][name] = e["names"].get(name, 0) + 1
                    e["sizes"]["get" if cid & 0x80 else "set"] = size
                    e["files"].add(fn)
                pending = []
        for size, cls, cid in pending:  # tuples with no name nearby
            key = (cls, cid & 0x7F)
            e = cmds.setdefault(key, {"class": cls, "id": cid & 0x7F, "names": {}, "sizes": {}, "files": set()})
            e["sizes"].setdefault("get" if cid & 0x80 else "set", size)
            e["files"].add(fn)

rows = []
for k in sorted(cmds):
    e = cmds[k]
    rows.append({"class": e["class"], "id": e["id"], "name": max(e["names"], key=e["names"].get) if e["names"] else None, "alt_names": sorted(n for n in e["names"] if n != (max(e["names"], key=e["names"].get))),
                 "sizes": e["sizes"], "files": sorted(e["files"])})
json.dump(rows, open(out, "w"), indent=1)
print(len(rows), "commands;", sum(1 for r in rows if r["name"]), "named")
