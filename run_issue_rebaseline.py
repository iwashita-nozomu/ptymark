#!/usr/bin/env python3
from __future__ import annotations

import base64
import zlib
from pathlib import Path

payload = Path("issue_rebaseline_payload.b85").read_text(encoding="ascii").strip()
source = zlib.decompress(base64.b85decode(payload)).decode("utf-8")
exec(compile(source, "rebaseline_issues_alpha1.py", "exec"))
