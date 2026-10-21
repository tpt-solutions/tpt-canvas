"""Writes the PNG fixtures used by tests/png_decode.rs.

Run from this directory: python make_png_fixtures.py
Each image has a formula for its pixels, which the Rust tests repeat.
The compressed data comes from Python's zlib, a real encoder, so the fixtures exercise
dynamic Huffman blocks (level 9) and stored blocks (level 0).
"""
import struct
import zlib


def chunk(kind, data):
    body = kind + data
    return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)


def paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    return b if pb <= pc else c


def filter_row(kind, row, prev, bpp):
    out = bytearray(len(row))
    for i, x in enumerate(row):
        a = row[i - bpp] if i >= bpp else 0
        b = prev[i]
        c = prev[i - bpp] if i >= bpp else 0
        if kind == 0:
            p = 0
        elif kind == 1:
            p = a
        elif kind == 2:
            p = b
        elif kind == 3:
            p = (a + b) // 2
        else:
            p = paeth(a, b, c)
        out[i] = (x - p) & 0xFF
    return bytes(out)


def png(width, height, color_type, depth, rows, bpp, level=9, filters=None, extra=b""):
    """rows: one bytes object per row, already packed at the given bit depth."""
    raw = bytearray()
    prev = bytes(len(rows[0]))
    for y, row in enumerate(rows):
        kind = 0 if filters is None else filters[y % len(filters)]
        raw.append(kind)
        raw += filter_row(kind, row, prev, bpp)
        prev = row
    ihdr = struct.pack(">IIBBBBB", width, height, depth, color_type, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + extra
        + chunk(b"IDAT", zlib.compress(bytes(raw), level))
        + chunk(b"IEND", b"")
    )


def write(name, data):
    with open(name, "wb") as f:
        f.write(data)
    print("wrote", name, len(data), "bytes")


W, H = 16, 16

# RGBA 8-bit, all five filters, dynamic Huffman. r = 17x, g = 17y, b = x*y mod 256, a = 255 - 8x.
rows = []
for y in range(H):
    row = bytearray()
    for x in range(W):
        row += bytes([(17 * x) & 0xFF, (17 * y) & 0xFF, (x * y) & 0xFF, (255 - 8 * x) & 0xFF])
    rows.append(bytes(row))
write("rgba8_filters.png", png(W, H, 6, 8, rows, bpp=4, level=9, filters=[0, 1, 2, 3, 4]))

# RGB 8-bit stored (level 0). r = x*10, g = y*10, b = 200.
rows = []
for y in range(8):
    row = bytearray()
    for x in range(8):
        row += bytes([x * 10, y * 10, 200])
    rows.append(bytes(row))
write("rgb8_stored.png", png(8, 8, 2, 8, rows, bpp=3, level=0))

# Gray 1-bit. Value is 255 when (x // 4 + y) is odd, else 0. Width 16 packs into 2 bytes per row.
rows = []
for y in range(4):
    row = bytearray(2)
    for x in range(W):
        bit = (x // 4 + y) % 2
        row[x // 8] |= bit << (7 - x % 8)
    rows.append(bytes(row))
write("gray1.png", png(W, 4, 0, 1, rows, bpp=1, level=9))

# Gray 2-bit. Level is (x + y) mod 4, scaled to 0, 85, 170, 255.
rows = []
for y in range(4):
    row = bytearray(4)
    for x in range(W):
        v = (x + y) % 4
        row[x // 4] |= v << (6 - 2 * (x % 4))
    rows.append(bytes(row))
write("gray2.png", png(W, 4, 0, 2, rows, bpp=1, level=9))

# Gray 16-bit. Sample is (x * 4096 + y * 256) as 16-bit, so its high byte is (16x + y) mod 256.
rows = []
for y in range(4):
    row = bytearray()
    for x in range(W):
        v = (x * 4096 + y * 256) & 0xFFFF
        row += struct.pack(">H", v)
    rows.append(bytes(row))
write("gray16.png", png(W, 4, 0, 16, rows, bpp=2, level=9))

# Palette 4-bit with tRNS. Five colours; index = (x + y) mod 5. Entries 0 and 1 have alpha 0 and 128.
palette = [(255, 0, 0), (0, 255, 0), (0, 0, 255), (255, 255, 0), (0, 255, 255)]
plte = chunk(b"PLTE", bytes(c for rgb in palette for c in rgb))
trns = chunk(b"tRNS", bytes([0, 128]))
rows = []
for y in range(H):
    row = bytearray(8)
    for x in range(W):
        v = (x + y) % 5
        row[x // 2] |= v << (4 - 4 * (x % 2))
    rows.append(bytes(row))
write("palette4.png", png(W, H, 3, 4, rows, bpp=1, level=9, extra=plte + trns))

# Gray + alpha 8-bit. gray = 20x + 5y, alpha = 16y + 15.
rows = []
for y in range(4):
    row = bytearray()
    for x in range(8):
        row += bytes([(20 * x + 5 * y) & 0xFF, (16 * y + 15) & 0xFF])
    rows.append(bytes(row))
write("graya8.png", png(8, 4, 4, 8, rows, bpp=2, level=9, filters=[4, 3, 2, 1]))

# RGB 16-bit. High bytes: r = 16x, g = 16y, b = 128. Low bytes are 0x34 so the high byte is what counts.
rows = []
for y in range(4):
    row = bytearray()
    for x in range(8):
        row += struct.pack(">HHH", (16 * x) << 8 | 0x34, (16 * y) << 8 | 0x34, 128 << 8 | 0x34)
    rows.append(bytes(row))
write("rgb16.png", png(8, 4, 2, 16, rows, bpp=6, level=9))

# Tiny RGB image. Small inputs usually compress with fixed Huffman codes, which the other
# fixtures may not use. The block type is printed so the coverage is known.
rows = [bytes([255, 0, 0, 0, 255, 0]), bytes([0, 0, 255, 255, 255, 0])]
tiny = png(2, 2, 2, 8, rows, bpp=3, level=9)
write("tiny_rgb.png", tiny)
idat_start = tiny.index(b"IDAT") + 4
block_type = (tiny[idat_start + 2] >> 1) & 3
print("tiny_rgb.png first deflate block type:", block_type, "(1 = fixed Huffman, 2 = dynamic, 0 = stored)")
