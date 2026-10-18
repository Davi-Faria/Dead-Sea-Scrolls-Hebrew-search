#!/usr/bin/env python3
"""Serve this folder on your own computer and open the search page in your browser.

    python serve.py              # then use the address it prints (default http://localhost:8000)
    python serve.py --port 9000

Only Python's standard library is used. The page needs a web server (not a double-click)
because browsers do not let a page read local data files directly.
"""

import argparse
import functools
import http.server
import socketserver
import webbrowser
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--port", type=int, default=8000, help="port to listen on (default 8000)")
    parser.add_argument("--no-browser", action="store_true", help="do not open a browser window")
    args = parser.parse_args()

    root = Path(__file__).resolve().parent
    if not (root / "data" / "dss.json").exists():
        raise SystemExit("data/dss.json is missing. Build it first with: python build_data.py")

    handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=str(root))
    socketserver.TCPServer.allow_reuse_address = True
    # Listen on this computer only (127.0.0.1), so nobody else on your network can reach it.
    with socketserver.TCPServer(("127.0.0.1", args.port), handler) as server:
        url = f"http://localhost:{args.port}/"
        print(f"Serving at {url}  (press Ctrl+C to stop)")
        if not args.no_browser:
            webbrowser.open(url)
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            print("\nStopped.")


if __name__ == "__main__":
    main()
