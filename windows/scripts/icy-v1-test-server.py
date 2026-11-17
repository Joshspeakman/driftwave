#!/usr/bin/env python3
"""scripts/icy-test-server.py, answering like an old Shoutcast v1 server.

Shoutcast v1 starts its reply with "ICY 200 OK" instead of "HTTP/1.0 200 OK", which strict
HTTP clients reject. The Windows app falls back to a raw socket for these; this server lets the
recording test check that. Same options as icy-test-server.py.
"""
import os, runpy, sys
from http.server import BaseHTTPRequestHandler

_send_response_only = BaseHTTPRequestHandler.send_response_only

def send_response_only(self, code, message=None):
    if code == 200:
        self.wfile.write(b"ICY 200 OK\r\n")
        return
    _send_response_only(self, code, message)

BaseHTTPRequestHandler.send_response_only = send_response_only
here = os.path.dirname(os.path.abspath(__file__))
runpy.run_path(os.path.join(here, "..", "..", "scripts", "icy-test-server.py"), run_name="__main__")
