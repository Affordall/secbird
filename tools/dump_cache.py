"""Dump Chromium simple-cache entries (CacheStorage) to files named by URL.

usage: python -I dump_cache.py <CacheStorage dir> <out dir>
"""
import gzip, os, re, struct, sys, zlib
from urllib.parse import urlsplit

INITIAL_MAGIC = 0xFCFB6D1BA7725C30
EOF_MAGIC = struct.pack("<Q", 0xF4FA6F45970D41D8)

src, out = sys.argv[1], sys.argv[2]
index = []
for root, _, files in os.walk(src):
    for f in files:
        if not f.endswith("_0"):
            continue
        data = open(os.path.join(root, f), "rb").read()
        if len(data) < 24:
            continue
        magic, ver, klen, khash = struct.unpack_from("<QIII", data, 0)
        if magic != INITIAL_MAGIC:
            continue
        key = data[24:24 + klen].decode("utf-8", "replace")
        end = data.find(EOF_MAGIC, 24 + klen)
        body = data[24 + klen:end] if end > 0 else b""
        # Key may carry a prefix like "1/0/_dk_..." -> take the last URL in it.
        urls = re.findall(r"https?://\S+", key)
        url = urls[-1] if urls else key
        if body[:2] == b"\x1f\x8b":
            try:
                body = gzip.decompress(body)
            except Exception:
                pass
        parts = urlsplit(url)
        path = parts.path.lstrip("/") or "index"
        if path.endswith("/"):
            path += "index"
        if parts.query:
            path += "_" + re.sub(r"[^A-Za-z0-9._-]", "_", parts.query)[:80]
        rel = os.path.join(parts.netloc or "nohost", *path.split("/"))
        rel = re.sub(r'[<>:"|?*]', "_", rel)
        dst = os.path.join(out, rel)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        if os.path.exists(dst) and os.path.getsize(dst) >= len(body):
            continue
        with open(dst, "wb") as fh:
            fh.write(body)
        index.append((len(body), url))

with open(os.path.join(out, "_index.tsv"), "w", encoding="utf-8") as fh:
    for size, url in sorted(index, key=lambda x: x[1]):
        fh.write(f"{size}\t{url}\n")
print(len(index), "entries")
