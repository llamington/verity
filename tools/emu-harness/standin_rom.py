#!/usr/bin/env python3
"""Writes a small stand-in for a Gen 4/5 ROM that keeps only what verity reads (header, ARM9,
file tables, the font, icon, species-data and move files), so the DS can build its font and icon caches
in the emulator without a 512 MB SD image. Output goes under build/ (gitignored), never the repo.

usage: standin_rom.py <rom.nds> <out.nds>
"""
import struct
import sys

KEEP = {b"CPU": ["graphic/pl_font.narc", "poketool/icongra/pl_poke_icon.narc", "poketool/waza/pl_waza_tbl.narc",
                 "itemtool/itemdata/item_icon.narc", "msgdata/pl_msg.narc",
                 "graphic/pl_pst_gra.narc"],
        b"ADA": ["graphic/font.narc", "poketool/icongra/poke_icon.narc", "poketool/waza/waza_tbl.narc",
                 "itemtool/itemdata/item_icon.narc", "msgdata/msg.narc"],
        b"APA": ["graphic/font.narc", "poketool/icongra/poke_icon.narc", "poketool/waza/waza_tbl.narc",
                 "itemtool/itemdata/item_icon.narc", "msgdata/msg.narc"],
        b"IPK": ["a/0/1/6", "a/0/2/0", "a/0/1/1", "a/0/1/8", "a/0/2/7", "a/0/3/9"], b"IPG": ["a/0/1/6", "a/0/2/0", "a/0/1/1", "a/0/1/8", "a/0/2/7", "a/0/3/9"]}
GEN5 = ["a/0/2/3", "a/0/0/7", "a/0/1/6", "a/0/2/1", "a/0/2/5", "a/0/0/2", "a/0/7/7", "a/0/7/8"]


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
    paths = KEEP.get(d[0x0C:0x0F], GEN5)
    arm9_off, _, _, arm9_len = struct.unpack_from("<4I", d, 0x20)
    fnt_off, fnt_len, fat_off, fat_len = struct.unpack_from("<4I", d, 0x40)
    h = bytearray(d[:0x200])
    body = bytearray()
    base = 0x200

    def put(blob):
        at = base + len(body)
        body.extend(blob)
        body.extend(b"\0" * (-len(body) % 4))
        return at

    struct.pack_into("<I", h, 0x20, put(d[arm9_off:arm9_off + arm9_len]))
    new_fnt = put(d[fnt_off:fnt_off + fnt_len])
    fat = bytearray(fat_len)
    new_fat = put(fat)
    for path in paths:
        fid = file_id(d, fnt_off, path)
        s, e = struct.unpack_from("<II", d, fat_off + 8 * fid)
        at = put(d[s:e])
        struct.pack_into("<II", fat, 8 * fid, at, at + e - s)
    # ARM9 overlays (Gen 5 keeps the ribbon table in one): the table and every overlay file.
    y9_off, y9_len = struct.unpack_from("<2I", d, 0x50)
    if y9_len:
        struct.pack_into("<I", h, 0x50, put(d[y9_off:y9_off + y9_len]))
        for k in range(y9_len // 32):
            fid = struct.unpack_from("<I", d, y9_off + 32 * k + 0x18)[0]
            s, e = struct.unpack_from("<II", d, fat_off + 8 * fid)
            at = put(d[s:e])
            struct.pack_into("<II", fat, 8 * fid, at, at + e - s)
    body[new_fat - base:new_fat - base + fat_len] = fat
    struct.pack_into("<4I", h, 0x40, new_fnt, fnt_len, new_fat, fat_len)
    with open(out, "wb") as f:
        f.write(bytes(h) + bytes(body))


main()
