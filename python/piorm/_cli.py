"""Console script entry point for the piorm CLI."""

import sys


def main() -> int:
    try:
        from piorm._core import run_cli

        return run_cli(sys.argv[1:])
    except ImportError:
        print("piorm CLI not available in this build.", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
