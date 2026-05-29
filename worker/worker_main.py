#!/usr/bin/env python3
"""worker_main.py — entry-point stub for the AnvilML worker.

Future implementation:
    Parse CLI arguments, initialize hardware backends, start the
    IPC loop (reading frames from stdin and writing results to
    stdout), and handle graceful shutdown on signal.
"""

import sys


def main():
    print("worker stub — not implemented", file=sys.stderr)
    sys.exit(1)


if __name__ == "__main__":
    main()
