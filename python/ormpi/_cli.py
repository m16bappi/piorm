"""Console script entry point for the ormpi CLI."""

import sys


def main() -> int:
    try:
        from ormpi._core import run_cli

        return run_cli(sys.argv[1:])
    except ImportError:
        print("ormpi CLI not available in this build.", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
