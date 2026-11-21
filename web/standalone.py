#!/usr/bin/env python3
"""Package a Trunk build as a local HTML file that needs no HTTP server."""

from base64 import b64encode
from pathlib import Path

dist = Path(__file__).resolve().parent / "dist"
assets = {}
for pattern in ("ga144-web-*.js", "ga144-web-*_bg.wasm", "studio-*.css"):
    matches = list(dist.glob(pattern))
    if len(matches) != 1:
        raise SystemExit(f"Build the web app with Trunk before packaging ({pattern}).")
    assets[pattern] = matches[0]

script = assets["ga144-web-*.js"].read_text(encoding="utf-8")
styles = assets["studio-*.css"].read_text(encoding="utf-8")
wasm = b64encode(assets["ga144-web-*_bg.wasm"].read_bytes()).decode("ascii")
page = dist / "ga144-local.html"
page.write_text(
    f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>GA144 Studio</title>
<style>{styles}</style>
</head>
<body>
<script type="module">
{script}
const binary = Uint8Array.from(atob("{wasm}"), char => char.charCodeAt(0));
await __wbg_init({{ module_or_path: binary }});
</script>
</body>
</html>
""",
    encoding="utf-8",
)
print(page)
