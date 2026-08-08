#!/usr/bin/env python3
import http.server
import os
import sys


class Handler(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        if self.path.partition("?")[0].endswith(".surf.gz"):
            self.send_header("Content-Encoding", "gzip")
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        self.send_header("Cache-Control", "no-store")
        super().end_headers()


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8934
    os.chdir(os.path.dirname(os.path.abspath(__file__)))
    print(f"http://localhost:{port}/ (cross-origin isolated)")
    http.server.ThreadingHTTPServer(("", port), Handler).serve_forever()
