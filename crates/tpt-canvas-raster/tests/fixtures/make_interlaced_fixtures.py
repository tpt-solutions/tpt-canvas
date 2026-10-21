"""Writes interlaced (Adam7) PNG fixtures for tests/png_decode.rs.

Run from this directory: python make_interlaced_fixtures.py
Odd sizes make the passes end on partial rows and columns. Each image has a formula for its
pixels, which the Rust tests repeat. The formulas were also checked with PyMuPDF (libpng).
"""
import struct
import zlib

# Adam7 passes: (first column, first row, column step, row step).
ADAM7 = [(0, 0, 8, 8), (4, 0, 8, 8), (0, 4, 4, 8), (2, 0, 4, 4), (0, 2, 2, 4), (1, 0, 2, 2), (0, 1, 1, 2)]


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
        p = [0, a, b, (a + b) // 2, paeth(a, b, c)][kind]
        out[i] = (x - p) & 0xFF
    return bytes(out)


def pack_row(samples, depth):
    """samples: ints, one per channel per pixel, in pixel order."""
    if depth == 8:
        return bytes(samples)
    if depth == 16:
        return b"".join(struct.pack(">H", s) for s in samples)
    out = bytearray((len(samples) * depth + 7) // 8)
    for k, s in enumerate(samples):
        bit = k * depth
        out[bit // 8] |= s << (8 - depth - bit % 8)
    return bytes(out)


def interlaced_png(width, height, color_type, depth, channels, pixel, filters):
    """pixel(x, y) returns the list of channel values for that pixel."""
    bpp = max(1, channels * depth // 8)
    raw = bytearray()
    pass_index = 0
    for x0, y0, dx, dy in ADAM7:
        pw = 0 if width <= x0 else (width - x0 + dx - 1) // dx
        ph = 0 if height <= y0 else (height - y0 + dy - 1) // dy
        if pw == 0 or ph == 0:
            continue
        prev = bytes((pw * channels * depth + 7) // 8)
        for j in range(ph):
            samples = []
            for i in range(pw):
                samples.extend(pixel(x0 + i * dx, y0 + j * dy))
            row = pack_row(samples, depth)
            kind = filters[pass_index % len(filters)]
            pass_index += 1
            raw.append(kind)
            raw += filter_row(kind, row, prev, bpp)
            prev = row
    ihdr = struct.pack(">IIBBBBB", width, height, depth, color_type, 0, 0, 1)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


def write(name, data):
    with open(name, "wb") as f:
        f.write(data)
    print("wrote", name, len(data), "bytes")


# RGBA 8-bit, 13 by 11. r = 17x, g = 17y, b = x*y mod 256, a = 255 - 8x.
write(
    "rgba8_interlaced.png",
    interlaced_png(
        13, 11, 6, 8, 4,
        lambda x, y: [(17 * x) & 0xFF, (17 * y) & 0xFF, (x * y) & 0xFF, (255 - 8 * x) & 0xFF],
        filters=[0, 1, 2, 3, 4],
    ),
)

# Gray 2-bit, 10 by 9. Level = (x + y) mod 4, which the decoder scales to 0, 85, 170, 255.
write(
    "gray2_interlaced.png",
    interlaced_png(10, 9, 0, 2, 1, lambda x, y: [(x + y) % 4], filters=[4, 2, 0]),
)
