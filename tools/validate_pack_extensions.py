#!/usr/bin/env python3
"""
Validate that packed gtpack manifests contain the canonical provider extension key.
"""

from __future__ import annotations

import argparse
import json
import sys
import zipfile
from pathlib import Path
from typing import Any, Dict, Tuple

PROVIDER_EXTENSION_ID = "greentic.provider-extension.v1"


class CBORDecoder:
    """
    Minimal CBOR decoder for the pack manifest structure.
    Supports the types used by pack manifests (maps, arrays, text, ints, bools, floats).
    """

    def __init__(self, data: bytes):
        self.data = data
        self.pos = 0

    def read(self, n: int) -> bytes:
        if self.pos + n > len(self.data):
            raise ValueError("truncated CBOR input")
        chunk = self.data[self.pos : self.pos + n]
        self.pos += n
        return chunk

    def decode_uint(self, addl: int) -> int:
        if addl < 24:
            return addl
        if addl == 24:
            return self.read(1)[0]
        if addl == 25:
            return int.from_bytes(self.read(2), "big")
        if addl == 26:
            return int.from_bytes(self.read(4), "big")
        if addl == 27:
            return int.from_bytes(self.read(8), "big")
        raise ValueError(f"unsupported additional length: {addl}")

    def decode(self) -> Any:
        if self.pos >= len(self.data):
            raise EOFError("unexpected end of CBOR input")
        initial = self.read(1)[0]
        major = initial >> 5
        addl = initial & 0x1F

        if major == 0:  # unsigned int
            return self.decode_uint(addl)
        if major == 1:  # negative int
            return -1 - self.decode_uint(addl)
        if major == 2:  # bytes
            length = self.decode_uint(addl)
            return self.read(length)
        if major == 3:  # text
            length = self.decode_uint(addl)
            return self.read(length).decode("utf-8")
        if major == 4:  # array
            items = []
            if addl == 31:
                while True:
                    if self.data[self.pos] == 0xFF:
                        self.pos += 1
                        break
                    items.append(self.decode())
            else:
                length = self.decode_uint(addl)
                for _ in range(length):
                    items.append(self.decode())
            return items
        if major == 5:  # map
            obj: Dict[Any, Any] = {}
            if addl == 31:
                while True:
                    if self.data[self.pos] == 0xFF:
                        self.pos += 1
                        break
                    key = self.decode()
                    obj[key] = self.decode()
            else:
                length = self.decode_uint(addl)
                for _ in range(length):
                    key = self.decode()
                    obj[key] = self.decode()
            return obj
        if major == 6:  # tag (ignored)
            _ = self.decode_uint(addl)
            return self.decode()
        if major == 7:  # floats/simple
            if addl == 20:
                return False
            if addl == 21:
                return True
            if addl == 22 or addl == 23:
                return None
            if addl == 26:
                import struct

                return struct.unpack(">f", self.read(4))[0]
            if addl == 27:
                import struct

                return struct.unpack(">d", self.read(8))[0]
        raise ValueError(f"unsupported CBOR major/additional: {major}/{addl}")


def load_manifest_from_pack(path: Path) -> Dict[str, Any]:
    with zipfile.ZipFile(path, "r") as zf:
        try:
            data = zf.read("manifest.cbor")
        except KeyError as exc:
            raise ValueError(f"{path} missing manifest.cbor") from exc
    decoder = CBORDecoder(data)
    manifest = decoder.decode()
    if not isinstance(manifest, dict):
        raise ValueError(f"{path} manifest is not a CBOR map")
    return manifest


def validate_pack(path: Path) -> None:
    if not path.stem.startswith("messaging-"):
        return

    manifest = load_manifest_from_pack(path)
    extensions = manifest.get("extensions")
    if not isinstance(extensions, dict):
        raise ValueError(f"{path} manifest has no extensions map")

    ext = extensions.get(PROVIDER_EXTENSION_ID)
    if ext is None:
        keys = ", ".join(sorted(k for k in extensions.keys() if isinstance(k, str)))
        raise ValueError(
            f"{path} missing provider extension {PROVIDER_EXTENSION_ID} (keys: {keys})"
        )

    if isinstance(ext, dict):
        kind = ext.get("kind")
        if kind != PROVIDER_EXTENSION_ID:
            raise ValueError(
                f"{path} provider extension kind={kind!r}, expected {PROVIDER_EXTENSION_ID!r}"
            )


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Validate provider extension key in gtpack manifests."
    )
    parser.add_argument("packs", nargs="+", type=Path, help="Paths to .gtpack files")
    args = parser.parse_args()

    errors = []
    for pack_path in args.packs:
        try:
            validate_pack(pack_path)
        except Exception as exc:  # pylint: disable=broad-except
            errors.append(str(exc))

    if errors:
        for err in errors:
            sys.stderr.write(err + "\n")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
