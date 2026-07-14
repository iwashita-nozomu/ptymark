#!/usr/bin/env python3
from __future__ import annotations

import base64
import zlib
from pathlib import Path

payload = "".join(
    path.read_text(encoding="ascii").strip()
    for path in sorted(Path("wiki_payload").glob("part*.b85"))
)
source = zlib.decompress(base64.b85decode(payload)).decode("utf-8")
exec(compile(source, "wiki_publish_bilingual_payload.py", "exec"))
