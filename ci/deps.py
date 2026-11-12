#!/usr/bin/env python3
import hashlib
import json
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEPS = json.loads((ROOT / "ci" / "dependencies.json").read_text())


def entry(dep_id: str) -> dict:
    for item in DEPS:
        if item["id"] == dep_id:
            return item
    sys.exit(f"unknown dependency {dep_id}")


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def main() -> int:
    if len(sys.argv) < 3:
        sys.exit("usage: deps.py get <id> <field> | deps.py fetch <id> --asset <name> --into <dir>")
    cmd = sys.argv[1]
    dep_id = sys.argv[2]
    dep = entry(dep_id)
    if cmd == "get":
        field = sys.argv[3]
        print(dep[field])
        return 0
    if cmd != "fetch":
        sys.exit(f"unknown command {cmd}")
    args = sys.argv[3:]
    asset = None
    into = None
    while args:
        if args[0] == "--asset":
            asset = args[1]
            args = args[2:]
        elif args[0] == "--into":
            into = args[1]
            args = args[2:]
        else:
            sys.exit(f"unknown flag {args[0]}")
    if not asset or not into:
        sys.exit("fetch needs --asset and --into")
    info = dep["assets"][asset]
    dest_dir = Path(into)
    dest_dir.mkdir(parents=True, exist_ok=True)
    dest = dest_dir / info["name"]
    if dest.is_file() and sha256_file(dest) == info["sha256"]:
        print(dest)
        return 0
    urllib.request.urlretrieve(info["url"], dest)
    got = sha256_file(dest)
    if got != info["sha256"]:
        dest.unlink(missing_ok=True)
        sys.exit(f"sha256 mismatch for {info['name']}: {got}")
    print(dest)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
