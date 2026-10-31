#!/usr/bin/env python3
"""Turns an unpacked LibreWolf install into Lunar Browser.

usage: brand.py <resources dir>

<resources dir> is the folder holding librewolf.cfg and omni.ja: the install
folder on Windows and Linux, LibreWolf.app/Contents/Resources on macOS.
Works the same on every OS; the per-OS scripts only rename and repackage.
"""

import base64
import io
import json
import re
import shutil
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LUNAR = ROOT / "lunar"
NAME = "Lunar Browser"
# LibreWolf's built-in engines without DuckDuckGo; Mojeek becomes the default.
REMOVED_ENGINES = {"ddg"}
DEFAULT_ENGINE = "mojeek"


def append_settings(res):
    cfg = res / "librewolf.cfg"
    text = cfg.read_text(encoding="utf-8")
    if "[CATEGORY] LUNAR BROWSER" not in text:
        cfg.write_text(text.rstrip("\n") + "\n" + (LUNAR / "lunar.cfg").read_text(encoding="utf-8"), encoding="utf-8")


def merge_policies(res):
    path = res / "distribution" / "policies.json"
    base = json.loads(path.read_text(encoding="utf-8")) if path.exists() else {"policies": {}}
    ours = json.loads((LUNAR / "policies.json").read_text(encoding="utf-8"))["policies"]
    base["policies"].update(ours)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(base, indent=4), encoding="utf-8")


def css_string(text):
    """Plain text -> the inside of a CSS "..." string, newlines kept."""
    text = text.rstrip("\n")
    return text.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\A ")


def install_theme(res):
    font = base64.b64encode((LUNAR / "branding" / "vt323.woff2").read_bytes()).decode()
    art = css_string((LUNAR / "theme" / "moon.txt").read_text(encoding="utf-8"))
    shields = css_string((LUNAR / "theme" / "shields.txt").read_text(encoding="utf-8"))
    # Loaded from here by the code at the end of lunar.cfg.
    folder = res / "lunar"
    folder.mkdir(exist_ok=True)
    for source, target in (("userChrome.css", "lunar-chrome.css"), ("userContent.css", "lunar-content.css")):
        css = (LUNAR / "theme" / source).read_text(encoding="utf-8")
        css = css.replace("{{FONT}}", font).replace("{{ART}}", art).replace("{{SHIELDS}}", shields)
        (folder / target).write_text(css, encoding="utf-8")


def rebrand_text(name, data):
    if name.endswith("branding/brand.ftl"):
        text = data.decode("utf-8")
        text = re.sub(r"^(-brand-(?:shorter|short|full|shortcut|product)-name|-vendor-short-name) = .*$",
                      lambda m: f"{m.group(1)} = {NAME}", text, flags=re.M)
        return text.encode("utf-8")
    if name.endswith("branding/brand.properties"):
        text = data.decode("utf-8")
        text = re.sub(r"^(brand(?:Shorter|Short|Full)Name|vendorShortName)=.*$",
                      lambda m: f"{m.group(1)}={NAME}", text, flags=re.M)
        return text.encode("utf-8")
    if name == "defaults/pref/local-settings.js":
        # Lets lunar.cfg (inside the install folder) load the theme.
        text = data.decode("utf-8").replace('pref("general.config.sandbox_enabled", true);',
                                            'pref("general.config.sandbox_enabled", false);')
        return text.encode("utf-8")
    if name == "defaults/settings/main/search-config-v2.json":
        config = json.loads(data)
        records = []
        for record in config["data"]:
            if record.get("recordType") == "engine" and record.get("identifier") in REMOVED_ENGINES:
                continue
            if record.get("recordType") == "defaultEngines":
                record["globalDefault"] = DEFAULT_ENGINE
                record["globalDefaultPrivate"] = DEFAULT_ENGINE
                record.pop("specificDefaults", None)
            records.append(record)
        config["data"] = records
        return json.dumps(config).encode("utf-8")
    return None


def patch_omni(path):
    """Rewrites branding, search engines and the config sandbox inside an omni.ja."""
    source = zipfile.ZipFile(path)
    out = io.BytesIO()
    changed = 0
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as target:
        for info in source.infolist():
            data = source.read(info)
            new = rebrand_text(info.filename, data)
            if new is not None and new != data:
                data = new
                changed += 1
            target.writestr(info, data, compress_type=info.compress_type)
    source.close()
    path.write_bytes(out.getvalue())
    return changed


def main():
    res = Path(sys.argv[1]).resolve()
    if not (res / "librewolf.cfg").exists():
        sys.exit(f"{res} has no librewolf.cfg: not a LibreWolf install folder")
    append_settings(res)
    merge_policies(res)
    install_theme(res)
    changed = patch_omni(res / "omni.ja") + patch_omni(res / "browser" / "omni.ja")
    print(f"Lunar: settings, policies and theme added; {changed} files changed inside omni.ja")
    shutil.copy(LUNAR / "branding" / "VT323-LICENSE.txt", res / "VT323-LICENSE.txt")


if __name__ == "__main__":
    main()
