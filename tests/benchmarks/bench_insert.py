"""
Performance benchmarks for ormpi ORM operations.

Run with: pytest tests/benchmarks/bench_insert.py -v --benchmark-disable-gc
Requires: pytest-benchmark>=4.0

Benchmark targets (SQLite):
  - Single INSERT     < 1ms
  - Bulk INSERT 1000  < 100ms
  - Full table scan   < 50ms
  - Filtered SELECT   < 5ms
  - COUNT             < 2ms
"""
from __future__ import annotations

import asyncio

import pytest

import ormpi
from ormpi import CharField, IntegerField
from ormpi._core import create_table


class BenchAuthor(ormpi.Model):
    name = CharField(max_length=100)
    email = CharField(max_length=200)
    age = IntegerField(null=True)

    class Meta:
        app_label = "bench"
        db_table = "bench_authors"


def _run_async(loop: asyncio.AbstractEventLoop, fn, *args, **kwargs):
    """
    Run an async function on the given loop.
    The function must be defined as `async def fn(...)`.
    """
    return loop.run_until_complete(fn(*args, **kwargs))


@pytest.fixture(scope="module")
def bench_loop():
    """Persistent event loop for the benchmark module."""
    lp = asyncio.new_event_loop()
    asyncio.set_event_loop(lp)
    yield lp
    lp.run_until_complete(lp.shutdown_asyncgens())
    lp.close()
    asyncio.set_event_loop(None)


@pytest.fixture(scope="module")
def populated_db(bench_loop):
    """Module-scoped: configure DB and insert 1000 rows once."""

    async def setup():
        await ormpi.configure({"default": {"ENGINE": "sqlite", "NAME": ":memory:"}})
        await create_table("bench.BenchAuthor")
        rows = [
            BenchAuthor(name=f"A{i}", email=f"a{i}@bench.com", age=i % 80)
            for i in range(1000)
        ]
        await BenchAuthor.objects.bulk_create(rows)

    bench_loop.run_until_complete(setup())
    yield bench_loop
    # In-memory SQLite: no explicit teardown needed; process cleanup handles it.


# ---------------------------------------------------------------------------
# INSERT benchmarks (each round is self-contained: setup+insert+teardown)
# ---------------------------------------------------------------------------


@pytest.mark.benchmark(group="bulk-insert")
def test_bulk_insert_1000(benchmark):
    """bulk_create() inserting 1000 rows (full round-trip per round)."""

    def do_bulk():
        async def _inner():
            await ormpi.configure({"default": {"ENGINE": "sqlite", "NAME": ":memory:"}})
            await create_table("bench.BenchAuthor")
            rows = [BenchAuthor(name=f"B{i}", email=f"b{i}@bench.com") for i in range(1000)]
            count = await BenchAuthor.objects.bulk_create(rows)
            assert count == 1000
            await ormpi.close()

        asyncio.run(_inner())

    benchmark(do_bulk)


@pytest.mark.benchmark(group="insert")
def test_single_insert(benchmark):
    """Single-row INSERT (full round-trip per 100 rounds)."""
    counter = [0]

    def do_insert():
        async def _inner():
            counter[0] += 1
            await BenchAuthor.objects.create(
                name=f"T{counter[0]}", email=f"t{counter[0]}@bench.com"
            )

        asyncio.run(_inner())

    # Setup: fresh DB. Teardown skipped — in-memory SQLite is ephemeral.
    asyncio.run(_setup_empty())
    benchmark(do_insert)


async def _setup_empty():
    await ormpi.configure({"default": {"ENGINE": "sqlite", "NAME": ":memory:"}})
    await create_table("bench.BenchAuthor")


# ---------------------------------------------------------------------------
# SELECT benchmarks — reuse populated_db
# ---------------------------------------------------------------------------


@pytest.mark.benchmark(group="select")
def test_full_table_scan(benchmark, populated_db):
    """Full table scan — await all() over 1000 rows."""
    lp = populated_db

    def scan():
        async def _inner():
            result = await BenchAuthor.objects.all()
            assert len(result) == 1000

        lp.run_until_complete(_inner())

    benchmark(scan)


@pytest.mark.benchmark(group="filter")
def test_filtered_query(benchmark, populated_db):
    """Filtered SELECT (age__gte=50) over 1000 rows."""
    lp = populated_db

    def query():
        async def _inner():
            await BenchAuthor.objects.filter(age__gte=50)

        lp.run_until_complete(_inner())

    benchmark(query)


@pytest.mark.benchmark(group="count")
def test_count_query(benchmark, populated_db):
    """COUNT(*) over 1000 rows."""
    lp = populated_db

    def count():
        async def _inner():
            n = await BenchAuthor.objects.count()
            assert n == 1000

        lp.run_until_complete(_inner())

    benchmark(count)


@pytest.mark.benchmark(group="get")
def test_get_by_pk(benchmark, populated_db):
    """Fetch a single row by PK."""
    lp = populated_db

    async def _find_pk():
        rows = await BenchAuthor.objects.filter(name="A42")
        return rows[0].pk

    pk = lp.run_until_complete(_find_pk())

    def get_one():
        async def _inner():
            await BenchAuthor.objects.get(pk=pk)

        lp.run_until_complete(_inner())

    benchmark(get_one)
