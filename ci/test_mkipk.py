#!/usr/bin/env python3
import hashlib
import io
import json
import shutil
import tarfile
import tempfile
import unittest
from pathlib import Path

import mkipk

APPINFO = {
    "id": "com.harbor.player",
    "version": "0.1.0",
    "vendor": "Harbor",
    "type": "native",
    "main": "harbor",
    "title": "Harbor",
    "icon": "icon.png",
    "largeIcon": "largeIcon.png",
    "splashBackground": "splash.png",
    "resolution": "1920x1080",
    "appDescription": "Native media player for LG webOS",
    "disableBackHistoryAPI": True,
}


def fake_arm_elf() -> bytes:
    header = bytearray(52)
    header[0:4] = b"\x7fELF"
    header[4] = 1
    header[5] = 1
    header[6] = 1
    header[16:18] = (2).to_bytes(2, "little")
    header[18:20] = (40).to_bytes(2, "little")
    return bytes(header) + b"\x00" * 64


def fake_x86_elf() -> bytes:
    header = bytearray(fake_arm_elf())
    header[18:20] = (3).to_bytes(2, "little")
    return bytes(header)


def stage_repo(root: Path, binary: bytes) -> Path:
    app_dir = root / "ipkroot" / "data" / "usr" / "palm" / "applications" / APPINFO["id"]
    app_dir.mkdir(parents=True)
    (app_dir / "harbor").write_bytes(binary)
    (app_dir / "harbor").chmod(0o755)
    (app_dir / "appinfo.json").write_text(json.dumps(APPINFO, indent=2) + "\n")
    (app_dir / "icon.png").write_bytes(b"png")
    pkg = root / "pkg"
    pkg.mkdir()
    (pkg / "appinfo.json").write_text(json.dumps(APPINFO, indent=2) + "\n")
    return root


def ar_members(ipk: Path) -> list[tuple[str, bytes]]:
    data = ipk.read_bytes()
    assert data.startswith(b"!<arch>\n")
    off = 8
    out = []
    while off + 60 <= len(data):
        header = data[off : off + 60]
        name = header[:16].decode().strip()
        size = int(header[48:58].decode().strip())
        off += 60
        blob = data[off : off + size]
        out.append((name, blob))
        off += size + (size % 2)
    return out


class MkipkTests(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(lambda: shutil.rmtree(self.tmp, ignore_errors=True))

    def test_packages_arm_elf(self):
        repo = stage_repo(self.tmp / "ok", fake_arm_elf())
        ipk = mkipk.package(repo)
        self.assertEqual(ipk.name, "com.harbor.player_0.1.0_arm.ipk")
        members = ar_members(ipk)
        self.assertEqual([n for n, _ in members], ["debian-binary", "control.tar.gz", "data.tar.gz"])
        self.assertFalse(any(n.endswith("/") for n, _ in members))
        self.assertEqual(members[0][1], b"2.0\n")
        with tarfile.open(fileobj=io.BytesIO(members[1][1]), mode="r:gz") as tf:
            control = tf.extractfile("control").read().decode()
        self.assertIn("Package: com.harbor.player", control)
        self.assertIn("Architecture: arm", control)
        self.assertIn("Installed-Size:", control)
        with tarfile.open(fileobj=io.BytesIO(members[2][1]), mode="r:gz") as tf:
            names = {m.name for m in tf.getmembers() if m.isfile()}
        self.assertIn("usr/palm/applications/com.harbor.player/harbor", names)
        self.assertIn("usr/palm/packages/com.harbor.player/packageinfo.json", names)
        info = json.loads(
            tarfile.open(fileobj=io.BytesIO(members[2][1]), mode="r:gz")
            .extractfile("usr/palm/packages/com.harbor.player/packageinfo.json")
            .read()
        )
        self.assertEqual(info["id"], "com.harbor.player")
        self.assertEqual(info["app"], "com.harbor.player")
        self.assertEqual(info["services"], [])

    def test_rejects_non_arm(self):
        repo = stage_repo(self.tmp / "x86", fake_x86_elf())
        with self.assertRaises(SystemExit) as ctx:
            mkipk.package(repo)
        self.assertIn("ELF32", str(ctx.exception))

    def test_rejects_missing_binary(self):
        repo = stage_repo(self.tmp / "missing", fake_arm_elf())
        (repo / "ipkroot" / "data" / "usr" / "palm" / "applications" / APPINFO["id"] / "harbor").unlink()
        with self.assertRaises(SystemExit) as ctx:
            mkipk.package(repo)
        self.assertIn("missing staged binary", str(ctx.exception))

    def test_reproducible(self):
        a = stage_repo(self.tmp / "a", fake_arm_elf())
        b = stage_repo(self.tmp / "b", fake_arm_elf())
        ipk_a = mkipk.package(a)
        ipk_b = mkipk.package(b)
        self.assertEqual(hashlib.sha256(ipk_a.read_bytes()).hexdigest(), hashlib.sha256(ipk_b.read_bytes()).hexdigest())

    def test_arm_elf_helper(self):
        path = self.tmp / "elf"
        path.write_bytes(fake_arm_elf())
        self.assertTrue(mkipk.is_arm_elf32(path))
        path.write_bytes(fake_x86_elf())
        self.assertFalse(mkipk.is_arm_elf32(path))


if __name__ == "__main__":
    unittest.main()
