#!/usr/bin/env python3
"""Writes a small stand-in for a Gen 4/5 ROM that keeps only the header, the file tables and the
font file, at its real layout, so the DS can build its font cache in the emulator without a
512 MB SD image. Output goes under build/ (gitignored) and never into the repo.

usage: font_rom.py <rom.nds> <out.nds>
"""
import struct
import sys

FONT = {b"CPU": "graphic/pl_font.narc", b"ADA": "graphic/font.narc", b"APA": "graphic/font.narc",
        b"IPK": "a/0/1/6", b"IPG": "a/0/1/6"}


def file_id(d, fnt, path):
    parts = path.split("/")
    dirid = 0
    for k, name in enumerate(parts):
        off, fid = struct.unpack_from("<IH", d, fnt + 8 * dirid)
        p = fnt + off
        while True:
            t = d[p]
            p += 1
            if t == 0:
                raise SystemExit(f"{path} not found")
            n = d[p:p + (t & 0x7F)].decode("latin1")
            p += t & 0x7F
            if t & 0x80:
                sub, = struct.unpack_from("<H", d, p)
                p += 2
                if name == n and k < len(parts) - 1:
                    dirid = sub & 0xFFF
                    break
            else:
                if name == n and k == len(parts) - 1:
                    return fid
                fid += 1


def main():
    src, out = sys.argv[1], sys.argv[2]
    d = open(src, "rb").read()
    path = FONT.get(d[0x0C:0x0F], "a/0/2/3")
    fnt_off, fnt_len, fat_off, fat_len = struct.unpack_from("<4I", d, 0x40)
    fid = file_id(d, fnt_off, path)
    s, e = struct.unpack_from("<II", d, fat_off + 8 * fid)
    h = bytearray(d[:0x200])
    new_fat = 0x200 + fnt_len
    data = new_fat + fat_len
    fat = bytearray(fat_len)
    struct.pack_into("<II", fat, 8 * fid, data, data + e - s)
    struct.pack_into("<4I", h, 0x40, 0x200, fnt_len, new_fat, fat_len)
    with open(out, "wb") as f:
        f.write(bytes(h) + d[fnt_off:fnt_off + fnt_len] + bytes(fat) + d[s:e])


main()
