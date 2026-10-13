"""Restate OpenRazer's device facts as Secretarybird's devices.json.

OpenRazer (GPL-2.0) is the public reference for Razer hardware. This script reads
facts from a local checkout -- product ids, names, device type, DPI maximum, key
grid size, which lighting zones exist, which command set a device speaks -- and
writes them in our own format. No OpenRazer code is copied or executed: the Python
files are parsed with `ast`, the C files are scanned as text.

usage: python -I tools/restate_openrazer.py <openrazer checkout> <devices.json>
"""

import ast
import json
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(sys.argv[1])
OUT = Path(sys.argv[2])
HW = ROOT / "daemon" / "openrazer_daemon" / "hardware"
DRV = ROOT / "driver"

KIND_BY_FILE = {
    "mouse.py": "mouse",
    "keyboards.py": "keyboard",
    "headsets.py": "headset",
    "mouse_mat.py": "mousepad",
    "accessory.py": "accessory",
    "core.py": "accessory",
    "monitor.py": "accessory",
}
DRIVER_BY_KIND = {
    "mouse": "razermouse_driver.c",
    "keyboard": "razerkbd_driver.c",
    "keypad": "razerkbd_driver.c",
    "laptop": "razerkbd_driver.c",
    "mousepad": "razeraccessory_driver.c",
    "accessory": "razeraccessory_driver.c",
    "headset": "razeraccessory_driver.c",
}

# LED ids (Razer's own numbering, as documented by OpenRazer).
SCROLL, LOGO, BACKLIGHT, LEFT, RIGHT, ALL = 0x01, 0x04, 0x05, 0x11, 0x10, 0x00
CHARGING, FAST_CHARGING, FULLY_CHARGED = 0x20, 0x21, 0x22


# ---------------------------------------------------------------- C driver scan

def usb_ids():
    ids = {}
    for h in DRV.glob("*.h"):
        for name, val in re.findall(r"#define\s+(USB_DEVICE_ID_RAZER_\w+)\s+(0x[0-9A-Fa-f]+)", h.read_text(errors="replace")):
            ids[name] = int(val, 16)
    return ids


TOKEN = re.compile(
    r"case\s+(USB_DEVICE_ID_RAZER_\w+)\s*:"
    r"|(razer_chroma_(?:mouse_extended|extended|standard)_\w+)\s*\("
    r"|transaction_id\.id\s*=\s*(0x[0-9A-Fa-f]+)"
    r"|\b(break|return|switch)\b"
)


def family_of(builder):
    if builder.startswith("razer_chroma_mouse_extended_"):
        return "mouse-extended"
    if builder.startswith("razer_chroma_extended_"):
        return "extended"
    return "standard"


def scan_driver(path, ids):
    """Votes per product id: which lighting command set and txid its switch cases use."""
    families, txids = defaultdict(Counter), defaultdict(Counter)
    group, fam_done, tx_done = [], False, False
    for m in TOKEN.finditer(path.read_text(errors="replace")):
        label, builder, txid, flow = m.groups()
        if label:
            if fam_done or tx_done:  # a new case group after the previous one's body
                group, fam_done, tx_done = [], False, False
            if label in ids:
                group.append(ids[label])
        elif builder and group and not fam_done:
            # Only lighting builders say which command set a device speaks.
            if re.search(r"(matrix|led|effect|frame|brightness)", builder):
                for pid in group:
                    families[pid][family_of(builder)] += 1
                fam_done = True
        elif txid and group and not tx_done:
            for pid in group:
                txids[pid][int(txid, 16)] += 1
            tx_done = True
        elif flow:
            group, fam_done, tx_done = [], False, False
    return families, txids


# ------------------------------------------------------------- daemon classes

class Classes:
    def __init__(self):
        self.defs = {}  # name -> (file, ClassDef)
        self.consts = {}
        for py in HW.glob("*.py"):
            tree = ast.parse(py.read_text(encoding="utf-8"))
            for node in tree.body:
                if isinstance(node, ast.ClassDef):
                    self.defs[node.name] = (py.name, node)
                elif isinstance(node, ast.Assign) and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name):
                    self.consts[node.targets[0].id] = node.value

    def attr(self, cls, name, seen=None):
        """Class attribute, following inheritance. Returns a Python value or None."""
        seen = seen or set()
        if cls in seen or cls not in self.defs:
            return None
        seen.add(cls)
        _, node = self.defs[cls]
        for stmt in node.body:
            if isinstance(stmt, ast.Assign):
                for t in stmt.targets:
                    if isinstance(t, ast.Name) and t.id == name:
                        return self.value(stmt.value, cls)
        for base in node.bases:
            if isinstance(base, ast.Name):
                v = self.attr(base.id, name, seen)
                if v is not None:
                    return v
        return None

    def value(self, expr, cls):
        if isinstance(expr, ast.Constant):
            return expr.value
        if isinstance(expr, (ast.List, ast.Tuple)):
            items = [self.value(e, cls) for e in expr.elts]
            return None if any(i is None for i in items) else items
        if isinstance(expr, ast.BinOp) and isinstance(expr.op, ast.Add):
            a, b = self.value(expr.left, cls), self.value(expr.right, cls)
            return a + b if isinstance(a, list) and isinstance(b, list) else None
        if isinstance(expr, ast.Attribute) and isinstance(expr.value, ast.Name):
            return self.attr(expr.value.id, expr.attr)
        if isinstance(expr, ast.Name) and expr.id in self.consts:
            return self.value(self.consts[expr.id], cls)
        if isinstance(expr, ast.ListComp) and len(expr.generators) == 1:
            # [m for m in X.METHODS if m not in [...]] / if m != '...'
            gen = expr.generators[0]
            src = self.value(gen.iter, cls)
            if not isinstance(src, list):
                return None
            out = []
            for item in src:
                keep = True
                for cond in gen.ifs:
                    if isinstance(cond, ast.Compare) and len(cond.ops) == 1:
                        other = self.value(cond.comparators[0], cls)
                        op = cond.ops[0]
                        if isinstance(op, ast.NotIn) and isinstance(other, list):
                            keep &= item not in other
                        elif isinstance(op, ast.NotEq):
                            keep &= item != other
                        else:
                            return None
                    else:
                        return None
                if keep:
                    out.append(item)
            return out
        return None

    def display_name(self, cls):
        doc = ast.get_docstring(self.defs[cls][1]) or ""
        m = re.search(r"Class for the (?:Razer )?(.+?)\s*$", doc.strip().splitlines()[0] if doc.strip() else "")
        if m:
            return m.group(1).strip().rstrip(".")
        return re.sub(r"(?<!^)(?=[A-Z])", " ", cls.removeprefix("Razer"))


def kind_for(file, cls, name):
    kind = KIND_BY_FILE[file]
    if kind == "keyboard":
        if re.search(r"Blade|Book", cls):
            return "laptop"
        if re.search(r"Tartarus|Orbweaver|Nostromo|Orbweaver", cls):
            return "keypad"
    return kind


def zones(methods, kind, family, has_matrix):
    out = []

    def add(name, led):
        if all(z["id"] != led for z in out):
            out.append({"name": name, "id": led})

    if any(m.startswith("set_logo_static") for m in methods):
        add("logo", LOGO)
    if any(m.startswith("set_scroll_static") for m in methods):
        add("scroll", SCROLL)
    if "set_left_static" in methods:
        add("left", LEFT)
    if "set_right_static" in methods:
        add("right", RIGHT)
    if "set_backlight_static" in methods or "bw_set_static" in methods:
        add("backlight", BACKLIGHT)
    if "set_charging_static" in methods:
        add("charging", CHARGING)
    if "set_fast_charging_static" in methods:
        add("fast charging", FAST_CHARGING)
    if "set_fully_charged_static" in methods:
        add("fully charged", FULLY_CHARGED)
    if "set_static_effect" in methods:
        # Old-style grids take one whole-device effect; brightness goes to the backlight LED.
        if kind in ("keyboard", "keypad", "laptop") or (family == "standard" and has_matrix):
            add("backlight", BACKLIGHT)
        elif family == "extended" and not out:
            add("all", ALL)
    return out


def main():
    ids = usb_ids()
    kraken_pids = {
        int(v, 16)
        for v in re.findall(r"#define\s+USB_DEVICE_ID_RAZER_\w+\s+(0x[0-9A-Fa-f]+)", (DRV / "razerkraken_driver.h").read_text(errors="replace"))
    }
    scans = {d: scan_driver(DRV / d, ids) for d in set(DRIVER_BY_KIND.values())}
    classes = Classes()

    devices, skipped = {}, []
    for cls, (file, _) in sorted(classes.defs.items()):
        if file not in KIND_BY_FILE:
            continue
        pid = classes.attr(cls, "USB_PID")
        vid = classes.attr(cls, "USB_VID")
        if not isinstance(pid, int) or vid not in (0x1532, None):
            continue
        methods = classes.attr(cls, "METHODS")
        if not isinstance(methods, list):
            skipped.append(f"{cls}: METHODS not resolvable")
            continue
        name = classes.display_name(cls)
        kind = kind_for(file, cls, name)

        if pid in kraken_pids:
            family = "kraken"
        else:
            fams, txs = scans[DRIVER_BY_KIND[kind]]
            family = fams[pid].most_common(1)[0][0] if fams[pid] else None
        entry = {"pid": f"{pid:04X}", "name": name, "kind": kind}
        if family != "kraken":
            fams, txs = scans[DRIVER_BY_KIND[kind]]
            if txs[pid]:
                entry["txid"] = f"{txs[pid].most_common(1)[0][0]:02X}"
        if "set_dpi_xy" in methods or "max_dpi" in methods:
            dpi_max = classes.attr(cls, "DPI_MAX")
            entry["dpi"] = {"min": 100, "max": dpi_max if isinstance(dpi_max, int) else 16000, "step": 50}
        if "set_poll_rate" in methods:
            rates = classes.attr(cls, "POLL_RATES") or [125, 500, 1000]
            entry["polling"] = "high" if max(rates) > 1000 else "standard"
        if family:
            entry["lighting"] = family
        has_matrix = bool(classes.attr(cls, "HAS_MATRIX"))
        leds = zones(methods, kind, family, has_matrix) if family in ("extended", "standard", "mouse-extended") else []
        if leds:
            entry["leds"] = leds
        dims = classes.attr(cls, "MATRIX_DIMS")
        if classes.attr(cls, "HAS_MATRIX") and isinstance(dims, list) and len(dims) == 2 and kind != "mouse" and family in ("extended", "standard"):
            entry["matrix"] = {"rows": dims[0], "cols": dims[1]}
        if "get_battery" in methods:
            entry["wireless"] = True
        entry["source"] = "openrazer"
        # Same product id under two classes (rare): keep the richer entry.
        if entry["pid"] in devices and len(json.dumps(devices[entry["pid"]])) >= len(json.dumps(entry)):
            continue
        devices[entry["pid"]] = entry

    # Hand-verified entries in the existing file win.
    existing = json.loads(OUT.read_text(encoding="utf-8-sig")) if OUT.exists() else []
    verified = {e["pid"].upper(): e for e in existing if "verified" in e.get("source", "")}
    for pid, e in verified.items():
        devices[pid] = e

    rows = [json.dumps(devices[p], separators=(",", ":")) for p in sorted(devices, key=lambda p: int(p, 16))]
    OUT.write_text("[\n" + ",\n".join(rows) + "\n]\n", encoding="utf-8")

    kinds = Counter(d["kind"] for d in devices.values())
    fams = Counter(d.get("lighting", "none") for d in devices.values())
    print(f"{len(devices)} devices -> {OUT}")
    print("kinds:", dict(kinds))
    print("lighting:", dict(fams))
    for s in skipped:
        print("skipped:", s)


main()
