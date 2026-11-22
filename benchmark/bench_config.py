import os
import time
import statistics
import asyncio
from dataclasses import dataclass
from typing import Callable, List, Optional

from rich.console import Console
from rich.table import Table

console = Console()

NUM_WARMUP = 3
NUM_ROUNDS = 10
BULK_COUNT = 10000


@dataclass
class BenchResult:
    name: str
    create_ms: Optional[float] = None
    bulk_create_ms: Optional[float] = None
    read_all_ms: Optional[float] = None
    read_filter_ms: Optional[float] = None
    read_get_ms: Optional[float] = None
    update_ms: Optional[float] = None
    delete_ms: Optional[float] = None


def timeit(fn: Callable, rounds: int = NUM_ROUNDS, warmup: int = NUM_WARMUP) -> float:
    for _ in range(warmup):
        fn()
    times: List[float] = []
    for _ in range(rounds):
        start = time.perf_counter()
        fn()
        end = time.perf_counter()
        times.append((end - start) * 1000)
    return statistics.median(times)


def timeit_async(fn: Callable, rounds: int = NUM_ROUNDS, warmup: int = NUM_WARMUP) -> float:
    loop = asyncio.new_event_loop()

    async def _run():
        for _ in range(warmup):
            await fn()
        times: List[float] = []
        for _ in range(rounds):
            start = time.perf_counter()
            await fn()
            end = time.perf_counter()
            times.append((end - start) * 1000)
        return statistics.median(times)

    result = loop.run_until_complete(_run())
    loop.run_until_complete(loop.shutdown_asyncgens())
    loop.close()
    return result


def print_results(results: List[BenchResult]):
    table = Table(title="\n  CRUD Benchmark: piorm vs Django ORM vs SQLAlchemy  ", show_lines=True)
    table.add_column("Operation", style="bold cyan", min_width=20)
    for r in results:
        table.add_column(r.name, justify="right", min_width=14)

    operations = [
        ("Create (single)", "create_ms"),
        ("Bulk Create (10k)", "bulk_create_ms"),
        ("Read All", "read_all_ms"),
        ("Read Filtered", "read_filter_ms"),
        ("Read by PK (get)", "read_get_ms"),
        ("Update", "update_ms"),
        ("Delete", "delete_ms"),
    ]

    for label, attr in operations:
        row = [label]
        values = []
        for r in results:
            val = getattr(r, attr)
            if val is not None:
                values.append(val)
                row.append(f"{val:.3f} ms")
            else:
                row.append("-")

        if len(values) >= 2:
            winner_idx = values.index(min(values))
            row[winner_idx + 1] = f"[bold green]{row[winner_idx + 1]}[/bold green]"

        table.add_row(*row)

    console.print(table)
    console.print(
        f"\n  Rounds: {NUM_ROUNDS} (median) | Warmup: {NUM_WARMUP} | Bulk: {BULK_COUNT} rows"
    )
    console.print("  Database: SQLite (:memory:)\n")
