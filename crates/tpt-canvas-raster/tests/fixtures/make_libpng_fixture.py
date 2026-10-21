"""Writes libpng_rgba.png with PyMuPDF, which uses libpng, and libpng_rgba.expected with the
straight RGBA bytes. The expected bytes come from Python's zlib and an independent unfilter,
not from the Rust decoder. Note that PyMuPDF's pixel samples are premultiplied, so they are
not used as the reference.
"""
import random
import struct
import zlib

import pymupdf

W, H = 16, 12
random.seed(7)
data = bytes(random.randrange(256) for _ in range(W * H * 4))
pm = pymupdf.Pixmap(pymupdf.csRGB, W, H, data, True)
with open("libpng_rgba.png", "wb") as f:
    f.write(pm.tobytes("png"))

# Read the PNG back in Python: concatenate IDAT, inflate, undo the filters.
png = open("libpng_rgba.png", "rb").read()
pos, idat = 8, b""
while pos < len(png):
    n = struct.unpack(">I", png[pos:pos + 4])[0]
    kind = png[pos + 4:pos + 8]
    if kind == b"IDAT":
        idat += png[pos + 8:pos + 8 + n]
    pos += 12 + n
raw = zlib.decompress(idat)
bpp, stride = 4, W * 4
out, prev = bytearray(), bytearray(stride)
for y in range(H):
    f = raw[y * (stride + 1)]
    src = raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)]
    cur = bytearray(stride)
    for i in range(stride):
        a = cur[i - bpp] if i >= bpp else 0
        b = prev[i]
        c = prev[i - bpp] if i >= bpp else 0
        if f == 0:
            p = 0
        elif f == 1:
            p = a
        elif f == 2:
            p = b
        elif f == 3:
            p = (a + b) // 2
        else:
            pp = a + b - c
            pa, pb, pc = abs(pp - a), abs(pp - b), abs(pp - c)
            p = a if pa <= pb and pa <= pc else (b if pb <= pc else c)
        cur[i] = (src[i] + p) & 0xFF
    out += cur
    prev = cur
with open("libpng_rgba.expected", "wb") as f:
    f.write(bytes(out))
print("wrote libpng_rgba.png and libpng_rgba.expected, filters used:",
      sorted({raw[y * (stride + 1)] for y in range(H)}))
