#!/usr/bin/env python3
import gzip
import io
import json
import tarfile
from pathlib import Path

EPOCH = 1262304000
BIN_NAME = "harbor"


def load_appinfo(repo: Path) -> dict:
    return json.loads((repo / "pkg" / "appinfo.json").read_text())


def is_arm_elf32(path: Path) -> bool:
    header = path.read_bytes()[:20]
    return (
        len(header) >= 20
        and header[:4] == b"\x7fELF"
        and header[4] == 1
        and header[5] == 1
        and header[18:20] == b"\x28\x00"
    )


def app_stage(repo: Path, app_id: str) -> Path:
    return repo / "ipkroot" / "data" / "usr" / "palm" / "applications" / app_id


def write_packageinfo(data: Path, app: dict) -> None:
    pkg_dir = data / "usr" / "palm" / "packages" / app["id"]
    pkg_dir.mkdir(parents=True, exist_ok=True)
    body = {
        "app": app["id"],
        "services": [],
        "id": app["id"],
        "loc_name": app["title"],
        "package_format_version": 2,
        "vendor": app["vendor"],
        "version": app["version"],
    }
    (pkg_dir / "packageinfo.json").write_text(json.dumps(body, indent=4) + "\n")


def add_tree(tf: tarfile.TarFile, src: Path, arc_root: str) -> None:
    entries = sorted(src.rglob("*"), key=lambda p: p.relative_to(src).as_posix())
    for path in entries:
        rel = path.relative_to(src).as_posix()
        ti = tf.gettarinfo(str(path), arcname=f"{arc_root}/{rel}" if arc_root else rel)
        ti.uid = ti.gid = 0
        ti.uname = ti.gname = "root"
        ti.mtime = EPOCH
        executable = path.is_dir() or path.name == BIN_NAME
        ti.mode = 0o755 if executable else 0o644
        if ti.isfile():
            with open(path, "rb") as fh:
                tf.addfile(ti, fh)
        else:
            tf.addfile(ti)


def write_targz(out: Path, src: Path, arc_root: str, extra: dict | None = None) -> None:
    raw = io.BytesIO()
    with tarfile.open(fileobj=raw, mode="w", format=tarfile.GNU_FORMAT) as tf:
        add_tree(tf, src, arc_root)
        for name, blob in sorted((extra or {}).items()):
            ti = tarfile.TarInfo(f"{arc_root}/{name}" if arc_root else name)
            ti.size = len(blob)
            ti.mode = 0o644
            ti.mtime = EPOCH
            ti.uid = ti.gid = 0
            ti.uname = ti.gname = "root"
            tf.addfile(ti, io.BytesIO(blob))
    with open(out, "wb") as fh:
        with gzip.GzipFile(filename="", mode="wb", fileobj=fh, mtime=0, compresslevel=9) as gz:
            gz.write(raw.getvalue())


def installed_kib(data: Path) -> int:
    total = sum(p.stat().st_size for p in data.rglob("*") if p.is_file())
    return (total + 1023) // 1024


def control_text(app: dict, kib: int) -> str:
    return (
        f"Package: {app['id']}\n"
        f"Version: {app['version']}\n"
        "Architecture: arm\n"
        f"Installed-Size: {kib}\n"
        "Maintainer: Harbor contributors\n"
        f"Description: {app.get('appDescription', app['title'])}\n"
        "webOS-Package-Format-Version: 2\n"
        "webOS-Packager-Version: harbor-mkipk\n"
    )


def write_ar(out: Path, members: list) -> None:
    with open(out, "wb") as fh:
        fh.write(b"!<arch>\n")
        for name, blob in members:
            if len(name) > 16:
                raise SystemExit(f"{name} is longer than 16 bytes")
            fh.write(f"{name:<16}{EPOCH:<12}{0:<6}{0:<6}{'100644':<8}{len(blob):<10}".encode())
            fh.write(b"`\n")
            fh.write(blob)
            if len(blob) % 2:
                fh.write(b"\n")


def package(repo: Path) -> Path:
    app = load_appinfo(repo)
    if app.get("type") != "native" or app.get("main") != BIN_NAME:
        raise SystemExit("pkg/appinfo.json must be type=native main=harbor")
    staged = app_stage(repo, app["id"])
    binary = staged / BIN_NAME
    if not binary.is_file():
        raise SystemExit(f"missing staged binary {binary}")
    if not is_arm_elf32(binary):
        raise SystemExit(f"{binary} is not ELF32 little-endian ARM")
    data = repo / "ipkroot" / "data"
    write_packageinfo(data, app)
    kib = installed_kib(data)
    ctl_dir = repo / "ipkroot" / "ctl"
    ctl_dir.mkdir(parents=True, exist_ok=True)
    write_targz(
        repo / "ipkroot" / "control.tar.gz",
        ctl_dir,
        "",
        extra={"control": control_text(app, kib).encode()},
    )
    write_targz(repo / "ipkroot" / "data.tar.gz", data, "")
    debian = repo / "ipkroot" / "debian-binary"
    debian.write_bytes(b"2.0\n")
    ipk = repo / "pkg" / f"{app['id']}_{app['version']}_arm.ipk"
    write_ar(
        ipk,
        [
            (name, (repo / "ipkroot" / name).read_bytes())
            for name in ("debian-binary", "control.tar.gz", "data.tar.gz")
        ],
    )
    return ipk


def main() -> int:
    ipk = package(Path(__file__).resolve().parent.parent)
    print(f"wrote {ipk.name} ({ipk.stat().st_size} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
