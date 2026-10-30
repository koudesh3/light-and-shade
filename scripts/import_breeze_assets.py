#!/usr/bin/env python3
"""One-off: unpack the assets embedded in a '5_pothos_in_a_breeze.html' into a project folder.

usage: import_breeze_assets.py <breeze.html> <project_dir> <name>
"""
import base64, json, pathlib, re, sys

html, out, name = pathlib.Path(sys.argv[1]).read_text(), pathlib.Path(sys.argv[2]), sys.argv[3]
start = html.index("const A=") + len("const A=")
assets, _ = json.JSONDecoder().raw_decode(html[start:])


def write(uri, stem):
    m = re.match(r"data:image/(\w+);base64,(.*)", uri, re.S)
    rel = f"layers/{stem}.{m[1]}"
    (out / rel).write_bytes(base64.b64decode(m[2]))
    return rel


layers = []
for L in assets["layers"]:
    stem = f"z{L['z']:02d}" + (f"_leaf{L['leaf']:02d}" if L["leaf"] else "_untraced")
    layers.append({
        "name": f"Leaf {L['leaf']}" if L["leaf"] else "Untraced",
        "leaf": L["leaf"] or None, "z": L["z"],
        "x": L["x"], "y": L["y"], "w": L["w"], "h": L["h"],
        "pivot": [L["px"], L["py"]], "reach": L["len"],
        "color": write(L["color"], stem + "_color"),
        "normal": write(L["normal"], stem + "_normal"),
    })
(out / "project.json").write_text(json.dumps(
    {"name": name, "width": assets["w"], "height": assets["h"], "layers": layers}, indent=1) + "\n")
print(len(layers), "layers", assets["w"], assets["h"])
