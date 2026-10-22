"""Generate original fpdf P assets using only Python's standard library."""
from pathlib import Path
import struct
import zlib

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "assets"
ASSETS.mkdir(exist_ok=True)


def png(size):
    rows = bytearray()
    for y in range(size):
        rows.append(0)
        for x in range(size):
            channels = [0, 0, 0, 0]
            for sy in range(4):
                for sx in range(4):
                    px = (x + (sx + 0.5) / 4) * 32 / size
                    py = (y + (sy + 0.5) / 4) * 32 / size
                    white = (8 <= px < 12 and 6 <= py < 27) or (11 <= px < 23 and (6 <= py < 10 or 15 <= py < 19)) or (21 <= px < 25 and 9 <= py < 16)
                    color = (255, 255, 255, 255) if white else (218, 51, 54, 255)
                    for index, value in enumerate(color):
                        channels[index] += value
            rows.extend(round(value / 16) for value in channels)
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(bytes(rows), 9)) + chunk(b"IEND", b"")


sizes = (16, 24, 32, 48, 64, 128, 256)
images = [png(size) for size in sizes]
entries = []
offset = 6 + 16 * len(images)
for size, data in zip(sizes, images):
    entries.append(struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(data), offset))
    offset += len(data)
(ASSETS / "fpdf.ico").write_bytes(struct.pack("<HHH", 0, 1, len(images)) + b"".join(entries) + b"".join(images))
(ASSETS / "fpdf.png").write_bytes(images[-1])
(ASSETS / "fpdf.svg").write_text('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><rect width="32" height="32" fill="#da3336"/><path fill="#fff" d="M8 6h4v21H8z M11 6h12v4H11z M11 15h12v4H11z M21 9h4v7h-4z"/></svg>\n', encoding="utf-8")
print("Generated original P icon in seven Windows sizes.")
