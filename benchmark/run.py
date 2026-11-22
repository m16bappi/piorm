"""
CRUD Benchmark Runner: piorm vs Django ORM vs SQLAlchemy

Usage:
    cd benchmark/
    uv run python run.py
"""

import os
import sys

sys.path.insert(0, os.path.dirname(__file__))

from rich.console import Console
from bench_config import BenchResult, print_results

console = Console()


def run_piorm() -> BenchResult:
    from bench_piorm import setup, teardown, run as bench_run

    console.print("  [bold blue]Setting up piorm...[/bold blue]")
    setup()
    console.print("  [bold blue]Benchmarking piorm...[/bold blue]")
    result = bench_run()
    teardown()
    return result


def run_django() -> BenchResult:
    from bench_django import setup, teardown, run as bench_run

    console.print("  [bold blue]Setting up Django ORM...[/bold blue]")
    setup()
    console.print("  [bold blue]Benchmarking Django ORM...[/bold blue]")
    result = bench_run()
    teardown()
    return result


def run_sqlalchemy() -> BenchResult:
    from bench_sqlalchemy import setup, teardown, run as bench_run

    console.print("  [bold blue]Setting up SQLAlchemy...[/bold blue]")
    setup()
    console.print("  [bold blue]Benchmarking SQLAlchemy...[/bold blue]")
    result = bench_run()
    teardown()
    return result


def main():
    console.print("\n[bold]  CRUD Benchmark: piorm vs Django ORM vs SQLAlchemy[/bold]")
    console.print("  [dim]Each operation is timed over 10 rounds (median reported)[/dim]\n")

    results = []

    results.append(run_piorm())
    results.append(run_django())
    results.append(run_sqlalchemy())

    console.print()
    print_results(results)


if __name__ == "__main__":
    main()
