#!/usr/bin/python3
"""Converte o snapshot JSON da TUI em HTML (para inspeção visual).

Uso: scripts/snapshot_to_html.py /tmp/dnfm-ui.json /tmp/dnfm-ui.html
"""
import html
import json
import sys

src = sys.argv[1] if len(sys.argv) > 1 else "/tmp/dnfm-ui.json"
dst = sys.argv[2] if len(sys.argv) > 2 else "/tmp/dnfm-ui.html"

data = json.load(open(src))
rows = data["rows"]

linhas = []
for row in rows:
    partes = []
    # run-length: agrupa células com o mesmo estilo
    i = 0
    while i < len(row):
        sym, fg, bg, bold = row[i]
        j = i + 1
        buf = [sym]
        while j < len(row) and row[j][1:] == [fg, bg, bold]:
            buf.append(row[j][0])
            j += 1
        texto = html.escape("".join(buf))
        style = []
        if fg:
            style.append(f"color:{fg}")
        if bg:
            style.append(f"background:{bg}")
        if bold:
            style.append("font-weight:700")
        css = ";".join(style)
        partes.append(f'<span style="{css}">{texto}</span>' if css else texto)
        i = j
    linhas.append("".join(partes))

corpo = "\n".join(linhas)
page = f"""<!doctype html>
<html><head><meta charset="utf-8">
<style>
  html, body {{ margin:0; padding:0; background:#14161c; }}
  .frame {{
    display:inline-block; padding:14px 16px; background:#14161c;
    border-radius:10px;
  }}
  pre {{
    margin:0; color:#dcdfe4; font-size:12.5px; line-height:1.32;
    font-family: "JetBrains Mono", "DejaVu Sans Mono", "Cascadia Mono", monospace;
    white-space:pre;
  }}
</style></head>
<body><div class="frame"><pre>{corpo}</pre></div></body></html>
"""
open(dst, "w").write(page)
print(f"{dst}: {len(rows)} linhas, {data['width']}x{data['height']}, chips={data.get('chips')}, grupos={data.get('groups')}")
