#!/usr/bin/env python3
"""Subset Noto Sans SC to the characters Harbor actually draws.

Emits:
  crates/harbor-gfx/assets/harbor-font.otf   embedded by harbor-gfx (include_bytes!)
  crates/harbor-gfx/assets/OFL.txt           the font's redistribution license

Re-run whenever the i18n dictionary or any string literal the UI draws gains
characters (the harbor-gfx dictionary-coverage test fails first if you forget):

  python3 ci/subset_font.py

Requires fonttools:  python3 -m pip install --user fonttools
The ~10 MB upstream font is downloaded once into target/font-source/ (gitignored).
"""

from __future__ import annotations

import sys
import urllib.error
import urllib.request
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
CACHE = REPO / "target" / "font-source"
OUT_DIR = REPO / "crates" / "harbor-gfx" / "assets"
OUT = OUT_DIR / "harbor-font.otf"

SOURCE_URLS = [
    # The raw GitHub host resets long transfers from here; the CDN resumes.
    "https://cdn.jsdelivr.net/gh/notofonts/noto-cjk@main/"
    "Sans/SubsetOTF/SC/NotoSansSC-Regular.otf",
    "https://raw.githubusercontent.com/notofonts/noto-cjk/main/"
    "Sans/SubsetOTF/SC/NotoSansSC-Regular.otf",
]
OFL_URL = "https://raw.githubusercontent.com/google/fonts/main/ofl/notosanssc/OFL.txt"
SOURCE = CACHE / "NotoSansSC-Regular.otf"
OFL = CACHE / "OFL.txt"

MAX_BYTES = 400 * 1024
# Typography the UI may draw that is not necessarily present in a literal yet.
# U+25A1 WHITE SQUARE is the replacement glyph for uncovered characters
# (Noto Sans SC has no U+FFFD, verified against the source cmap).
EXTRA = "\u25a1…–—‘’“”•·×→←↑↓≥≤±°§"
# Characters every build must be able to draw: a Latin letter, a dictionary
# hanzi, an ellipsis, and the replacement glyph.
REQUIRED = "A\u4e2d\u2026\u25a1"


def fail(msg: str) -> None:
    sys.exit(f"subset_font: error: {msg}")


def fetch(urls: list[str], dest: Path, min_bytes: int) -> None:
    """Download `dest` from the first working candidate, resuming a partial
    file with HTTP Range requests (the ~10 MB font and the flaky uplink need
    both)."""
    if dest.exists() and dest.stat().st_size >= min_bytes:
        return
    dest.parent.mkdir(parents=True, exist_ok=True)
    part = dest.with_suffix(dest.suffix + ".part")
    for url in urls:
        for attempt in range(1, 7):
            have = part.stat().st_size if part.exists() else 0
            req = urllib.request.Request(url)
            if have:
                req.add_header("Range", f"bytes={have}-")
            try:
                with urllib.request.urlopen(req, timeout=120) as resp:
                    if have and resp.status != 206:
                        have = 0  # server ignored the Range: start over
                    cl = resp.headers.get("Content-Length")
                    expected = None
                    if cl is not None:
                        expected = (have if resp.status == 206 else 0) + int(cl)
                    label = "resuming at" if have else "downloading"
                    print(
                        f"subset_font: {label} {have} B "
                        f"(attempt {attempt}): {url.split('/')[2]}"
                    )
                    mode = "ab" if have else "wb"
                    with open(part, mode) as fh:
                        done = have
                        while chunk := resp.read(1 << 16):
                            fh.write(chunk)
                            done += len(chunk)
                            if done // (1 << 20) != (done - len(chunk)) // (1 << 20):
                                print(f"subset_font:   {done >> 20} MiB")
            except urllib.error.HTTPError as exc:
                if exc.code == 416 and part.stat().st_size > 0:
                    part.replace(dest)
                    print(f"subset_font: already complete: {dest.name}")
                    return
                print(f"subset_font: retry after HTTP {exc.code}")
                continue
            except OSError as exc:
                print(f"subset_font: retry after {exc}")
                continue
            done = part.stat().st_size
            if expected is None:
                expected = done  # no length advertised; the parse will judge
            if done >= expected and done >= min_bytes:
                part.replace(dest)
                return
            print(f"subset_font: got {done} of {expected} B; retrying")
    fail(f"could not download {dest.name} from any candidate URL")


def rust_string_literals(src: str) -> list[str]:
    """Extract every string literal from Rust source, skipping comments.

    Handles normal strings with escapes, raw strings (r"..." and r#"..."#),
    and leaves char literals alone so `'"'` cannot derail the scan.
    """
    out: list[str] = []
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if c == "/" and src.startswith("//", i):
            j = src.find("\n", i)
            i = n if j < 0 else j + 1
        elif c == "/" and src.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if src.startswith("/*", j):
                    depth += 1
                    j += 2
                elif src.startswith("*/", j):
                    depth -= 1
                    j += 2
                else:
                    j += 1
            i = j
        elif c == "r" and i + 1 < n and src[i + 1] in '"#':
            j = i + 1
            hashes = 0
            while j < n and src[j] == "#":
                hashes += 1
                j += 1
            if j >= n or src[j] != '"':  # r#raw_ident, not a raw string
                i += 1
                continue
            j += 1
            end = '"' + "#" * hashes
            k = src.find(end, j)
            if k < 0:
                break
            out.append(src[j:k])
            i = k + len(end)
        elif c == "'":
            # Char literal ('a', '\n', '\''): skip it so a quote inside a char
            # cannot start a bogus string scan.
            j = i + 1
            if j < n and src[j] == "\\":
                j += 2
            elif j < n:
                j += 1
            if j < n and src[j] == "'":
                i = j + 1
                continue
            i += 1
        elif c == '"':
            buf: list[str] = []
            j = i + 1
            while j < n:
                if src[j] == "\\" and j + 1 < n:
                    esc = src[j + 1]
                    simple = {"n": "\n", "t": "\t", "r": "\r", "\\": "\\", '"': '"', "0": "\0"}
                    if esc in simple:
                        buf.append(simple[esc])
                        j += 2
                        continue
                    if esc == "u" and j + 2 < n and src[j + 2] == "{":
                        j += 3
                        hexs: list[str] = []
                        while j < n and src[j] != "}":
                            hexs.append(src[j])
                            j += 1
                        j += 1  # closing brace
                        buf.append(chr(int("".join(hexs), 16)))
                        continue
                    buf.append(esc)
                    j += 2
                    continue
                if src[j] == '"':
                    break
                buf.append(src[j])
                j += 1
            out.append("".join(buf))
            i = j + 1
        else:
            i += 1
    return out


def collect_chars() -> list[str]:
    chars: set[str] = set(chr(c) for c in range(0x20, 0x7F))  # printable ASCII
    chars.update(EXTRA)
    chars.add("\ufffd")
    for path in sorted((REPO / "crates").rglob("*.rs")):
        for literal in rust_string_literals(path.read_text(encoding="utf-8")):
            chars.update(literal)  # add the literal's characters, not itself
    # Drop control characters (newlines/tabs inside literals) - no glyph, no use.
    return sorted(c for c in chars if not (ord(c) < 0x20 or ord(c) == 0x7F))


def main() -> None:
    try:
        from fontTools import subset as ft_subset
        from fontTools.ttLib import TTFont
    except ImportError:
        fail(
            "fonttools is not installed.\n"
            "  install it with:  python3 -m pip install --user fonttools\n"
            "  (on a bare host:  curl -sS https://bootstrap.pypa.io/get-pip.py"
            " | python3 - --user --break-system-packages)"
        )

    fetch(SOURCE_URLS, SOURCE, 1 << 20)  # the real font is ~10 MB
    fetch([OFL_URL], OFL, 1024)

    drawable = collect_chars()
    chars_file = CACHE / "chars.txt"
    chars_file.write_text("".join(drawable), encoding="utf-8")

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    print(f"subset_font: subsetting {len(drawable)} characters")
    ft_subset.main(
        args=[
            str(SOURCE),
            f"--text-file={chars_file}",
            f"--output-file={OUT}",
            "--layout-features=",
        ]
    )

    size = OUT.stat().st_size
    if size > MAX_BYTES:
        fail(f"subset is {size} bytes, over the {MAX_BYTES} byte budget")
    cmap = TTFont(str(OUT)).getBestCmap() or {}
    missing = [c for c in REQUIRED if ord(c) not in cmap]
    if missing:
        fail(f"subset lacks required characters: {missing!r}")

    (OUT_DIR / "OFL.txt").write_bytes(OFL.read_bytes())
    rel = OUT.relative_to(REPO)
    print(
        f"subset_font: wrote {rel} - {size} bytes "
        f"(source {SOURCE.stat().st_size} bytes), plus OFL.txt"
    )


if __name__ == "__main__":
    main()
