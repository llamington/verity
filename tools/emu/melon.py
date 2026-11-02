#!/usr/bin/env python3
"""Drive a running melonDS (flatpak, X11 mode) for visual checks: screenshots, buttons, touch.

Start melonDS on X11 so its window can be captured and receive XTest input:

    tools/emu/melon.py launch nds/verity.nds

Then, e.g.:

    tools/emu/melon.py shot out.png          # both screens, 1x, menu bar cropped
    tools/emu/melon.py key Down 5            # press Down 5 times
    tools/emu/melon.py key A                 # DS button (A B X Y L R Start Select Up Down Left Right)
    tools/emu/melon.py tap 40 100            # touch at bottom-screen pixel (40, 100)
    tools/emu/melon.py drag 128 150 128 40   # touch drag on the bottom screen

Keyboard mapping is melonDS's default (A=X, B=Z, X=S, Y=A, L=Q, R=W, Start=Return,
Select=BackSpace, arrows). The window must be at 1x scale with the default vertical layout.
"""
import ctypes
import ctypes.util
import os
import re
import subprocess
import sys
import time

KEYSYMS = {
    'A': 0x78, 'B': 0x7A, 'X': 0x73, 'Y': 0x61, 'L': 0x71, 'R': 0x77,
    'Start': 0xFF0D, 'Select': 0xFF08,
    'Up': 0xFF52, 'Down': 0xFF54, 'Left': 0xFF51, 'Right': 0xFF53,
}

x11 = ctypes.CDLL(ctypes.util.find_library('X11'))
xtst = ctypes.CDLL(ctypes.util.find_library('Xtst'))
x11.XOpenDisplay.restype = ctypes.c_void_p
x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
x11.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
x11.XFlush.argtypes = [ctypes.c_void_p]
x11.XSetInputFocus.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_ulong]
x11.XRaiseWindow.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
xtst.XTestFakeKeyEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
xtst.XTestFakeButtonEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
xtst.XTestFakeMotionEvent.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_int, ctypes.c_int,
                                      ctypes.c_ulong]


def window():
    """(id, abs_x, abs_y, w, h) of the melonDS main window."""
    tree = subprocess.run(['xwininfo', '-root', '-tree'], capture_output=True, text=True).stdout
    for line in tree.splitlines():
        m = re.search(r'(0x[0-9a-f]+) "\[[^\]]*\] melonDS[^"]*".*?(\d+)x(\d+)\+-?\d+\+-?\d+\s+\+(-?\d+)\+(-?\d+)',
                      line)
        if m:
            wid, w, h, ax, ay = m.groups()
            return int(wid, 16), int(ax), int(ay), int(w), int(h)
    sys.exit('melonDS window not found (is it running with X11? use: melon.py launch ROM)')


def screen_origin(win):
    """Absolute position of the top-left of the top DS screen (below the menu bar)."""
    _, ax, ay, _, h = win
    return ax, ay + h - 384


def focus(d, win):
    x11.XRaiseWindow(d, win[0])
    x11.XSetInputFocus(d, win[0], 1, 0)
    x11.XFlush(d)
    time.sleep(0.05)


def key(d, name, times=1, hold=0.08):
    code = x11.XKeysymToKeycode(d, KEYSYMS[name])
    for _ in range(times):
        xtst.XTestFakeKeyEvent(d, code, 1, 0)
        x11.XFlush(d)
        time.sleep(hold)
        xtst.XTestFakeKeyEvent(d, code, 0, 0)
        x11.XFlush(d)
        time.sleep(0.08)


def touch_path(d, win, points, step=0.016):
    ox, oy = screen_origin(win)
    oy += 192
    x, y = points[0]
    xtst.XTestFakeMotionEvent(d, -1, ox + x, oy + y, 0)
    x11.XFlush(d)
    time.sleep(0.05)
    xtst.XTestFakeButtonEvent(d, 1, 1, 0)
    x11.XFlush(d)
    for (x0, y0), (x1, y1) in zip(points, points[1:]):
        n = max(abs(x1 - x0), abs(y1 - y0)) // 4 + 1
        for i in range(1, n + 1):
            xtst.XTestFakeMotionEvent(d, -1, ox + x0 + (x1 - x0) * i // n,
                                      oy + y0 + (y1 - y0) * i // n, 0)
            x11.XFlush(d)
            time.sleep(step)
    time.sleep(0.08)
    xtst.XTestFakeButtonEvent(d, 1, 0, 0)
    x11.XFlush(d)
    time.sleep(0.08)


def shot(win, out):
    here = os.path.dirname(os.path.abspath(__file__))
    tmp = out + '.xwd'
    subprocess.run(['xwd', '-id', hex(win[0]), '-out', tmp], check=True)
    subprocess.run([sys.executable, os.path.join(here, 'xwd2png.py'), tmp, out + '.full.png'],
                   check=True, capture_output=True)
    os.remove(tmp)
    from PIL import Image
    im = Image.open(out + '.full.png')
    im.crop((0, im.height - 384, 256, im.height)).save(out)
    os.remove(out + '.full.png')


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    cmd, args = sys.argv[1], sys.argv[2:]
    if cmd == 'launch':
        rom = os.path.abspath(args[0])
        subprocess.Popen(['flatpak', 'run', '--nosocket=wayland', '--env=QT_QPA_PLATFORM=xcb',
                          'net.kuribo64.melonDS', rom],
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
        for _ in range(40):
            time.sleep(0.5)
            if subprocess.run(['xwininfo', '-root', '-tree'], capture_output=True,
                              text=True).stdout.find('] melonDS') >= 0:
                time.sleep(2)
                return
        sys.exit('melonDS did not start')
    if cmd == 'quit':
        subprocess.run(['flatpak', 'kill', 'net.kuribo64.melonDS'])
        return
    win = window()
    if cmd == 'shot':
        shot(win, args[0])
        return
    d = x11.XOpenDisplay(None)
    focus(d, win)
    if cmd == 'key':
        key(d, args[0], int(args[1]) if len(args) > 1 else 1)
    elif cmd == 'hold':
        key(d, args[0], 1, float(args[1]))
    elif cmd == 'tap':
        touch_path(d, win, [(int(args[0]), int(args[1]))])
    elif cmd == 'drag':
        a = list(map(int, args))
        touch_path(d, win, [(a[i], a[i + 1]) for i in range(0, len(a), 2)])
    else:
        sys.exit(f'unknown command {cmd}')


main()
