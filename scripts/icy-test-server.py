#!/usr/bin/env python3
"""Tiny Shoutcast/Icecast-style test server.

Serves a looping MP3 (/mp3) and AAC (/aac) stream with ICY metadata whose StreamTitle
changes every --interval seconds ("Test Artist - Song 1", "... Song 2", ...).
Usage: icy-test-server.py --port 8765 --interval 20 --media DIR   (DIR holds tone.mp3 / tone.aac)
"""
import argparse, os, socketserver, threading, time
from http.server import BaseHTTPRequestHandler

ap = argparse.ArgumentParser()
ap.add_argument("--port", type=int, default=8765)
ap.add_argument("--interval", type=float, default=20)
ap.add_argument("--media", required=True)
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
                # 2 s burst on connect, then real time
                ahead = sent / BYTES_PER_SEC - (time.time() - t0) - 2
                if ahead > 0: time.sleep(ahead)
        except (BrokenPipeError, ConnectionResetError):
            pass

class Server(socketserver.ThreadingMixIn, socketserver.TCPServer):
    allow_reuse_address = True
    daemon_threads = True

Server(("127.0.0.1", args.port), Handler).serve_forever()
