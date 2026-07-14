#!/usr/bin/env python3
from __future__ import annotations

import base64
import hashlib
import zlib
from pathlib import Path

parts = sorted(Path("wiki_payload").glob("part*.b85"))
if len(parts) != 5:
    raise SystemExit(f"expected 5 Wiki payload parts, found {len(parts)}")
payload = "".join(path.read_text(encoding="ascii").strip() for path in parts)
source = zlib.decompress(base64.b85decode(payload)).decode("utf-8")
digest = hashlib.sha256(source.encode("utf-8")).hexdigest()
expected = "9da2be39733a7874f49511a13888c10876a70c87572e4892dcd1b431ad8c2770"
if digest != expected:
    raise SystemExit(f"Wiki publisher payload digest mismatch: {digest}")
exec(compile(source, "wiki_publish_bilingual_payload.py", "exec"))
