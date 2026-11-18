#!/usr/bin/env python3
"""Tiny Shoutcast/Icecast-style test server.

Serves a looping MP3 (/mp3) and AAC (/aac) stream with ICY metadata whose StreamTitle
changes every --interval seconds ("Test Artist - Song 1", "... Song 2", ...).
Usage: icy-test-server.py --port 8765 --interval 20 --media DIR   (DIR holds tone.mp3 / tone.aac)

Test options:
  --drop-icy-after N   close the app's title connection (ICY metadata, User-Agent "Shortwave…")
                       after N seconds; the player's audio connection keeps going. Tests the reconnect.
  /playlist            a .pls pointing at /mp3, served as audio/x-scpls at a URL with no
                       extension (the local-station content-type probe).
"""
import argparse, os, socketserver, threading, time
from http.server import BaseHTTPRequestHandler

ap = argparse.ArgumentParser()
ap.add_argument("--port", type=int, default=8765)
ap.add_argument("--interval", type=float, default=20)
ap.add_argument("--media", required=True)
ap.add_argument("--drop-icy-after", type=float, default=0)
args = ap.parse_args()

START = time.time()
BYTES_PER_SEC = 16000          # 128 kbps
METAINT = 8000
FILES = {"/mp3": ("tone.mp3", "audio/mpeg"), "/aac": ("tone.aac", "audio/aac")}

def title_now():
    n = int((time.time() - START) // args.interval) + 1
    return f"Test Artist - Song {n}"

def meta_block(title):
    text = f"StreamTitle='{title}';StreamUrl='';".encode()
    pad = (-len(text)) % 16
    text += b"\0" * pad
    return bytes([len(text) // 16]) + text

class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.0"
    def log_message(self, *a): pass

    def do_GET(self):
        if self.path.split("?")[0] == "/playlist":
            host = self.headers.get("Host", f"127.0.0.1:{args.port}")
            body = f"[playlist]\nNumberOfEntries=1\nFile1=http://{host}/mp3\nTitle1=Test\nLength1=-1\nVersion=2\n".encode()
            self.send_response(200)
            self.send_header("Content-Type", "audio/x-scpls")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        if self.path not in FILES:
            self.send_error(404); return
        name, ctype = FILES[self.path]
        data = open(os.path.join(args.media, name), "rb").read()
        icy = self.headers.get("Icy-MetaData") == "1"
        self.send_response(200)
        self.send_header("Content-Type", ctype)
        self.send_header("icy-name", "Shortwave Test")
        self.send_header("icy-br", "128")
        if icy: self.send_header("icy-metaint", str(METAINT))
        self.end_headers()
        pos, sent, t0 = 0, 0, time.time()
        # Only the app's own title connection (User-Agent "Shortwave…"): players such as AVPlayer
        # also ask for ICY metadata on the audio connection, which must keep going.
        title_conn = icy and self.headers.get("User-Agent", "").startswith("Shortwave")
        drop_at = t0 + args.drop_icy_after if title_conn and args.drop_icy_after > 0 else None
        last_title = None
        try:
            while True:
                chunk = data[pos:pos + METAINT]
                if len(chunk) < METAINT:
                    chunk += data[:METAINT - len(chunk)]
                pos = (pos + METAINT) % len(data)
                self.wfile.write(chunk)
                if icy:
                    t = title_now()
                    self.wfile.write(meta_block(t) if t != last_title else b"\0")
                    last_title = t
                sent += len(chunk)
                if drop_at and time.time() >= drop_at:
                    return   # closes the title connection mid-stream
                # 2 s burst on connect, then real time
                ahead = sent / BYTES_PER_SEC - (time.time() - t0) - 2
                if ahead > 0: time.sleep(ahead)
        except (BrokenPipeError, ConnectionResetError, ConnectionAbortedError):  # the last is Windows
            pass

class Server(socketserver.ThreadingMixIn, socketserver.TCPServer):
    allow_reuse_address = True
    daemon_threads = True

Server(("127.0.0.1", args.port), Handler).serve_forever()
