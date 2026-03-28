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


# ---------------------------------------------------------------------------
# Loop-aware helpers
# ---------------------------------------------------------------------------


def _exec(loop: asyncio.AbstractEventLoop, coro):
    """Run a coroutine or awaitable on the given loop."""
    if asyncio.iscoroutine(coro):
        return loop.run_until_complete(coro)
    # QuerySet / other awaitable: wrap in a coroutine
    async def _wrap():
        return await coro
    return loop.run_until_complete(_wrap())


@pytest.fixture
def loop():
    """Provide a dedicated event loop for synchronous benchmark functions."""
    lp = asyncio.new_event_loop()
    yield lp
    lp.close()


# ---------------------------------------------------------------------------
# INSERT benchmarks
# ---------------------------------------------------------------------------


@pytest.mark.benchmark(group="insert")
def test_single_insert(benchmark, loop):
    """Single-row INSERT via ORM create()."""
    _exec(loop, ormpi.configure({"default": {"ENGINE": "sqlite", "NAME": ":memory:"}}))
    _exec(loop, create_table("bench.BenchAuthor"))

    counter = [0]

    def insert_one():
        counter[0] += 1
        _exec(loop, BenchAuthor.objects.create(
            name=f"T{counter[0]}", email=f"t{counter[0]}@bench.com"
        ))

    benchmark(insert_one)
    _exec(loop, ormpi.close())


@pytest.mark.benchmark(group="bulk-insert")
def test_bulk_insert_1000(benchmark, loop):
    """bulk_create() inserting 1000 rows (full round-trip per round)."""

    def do_bulk():
        async def _inner():
            await ormpi.configure({"default": {"ENGINE": "sqlite", "NAME": ":memory:"}})
            await create_table("bench.BenchAuthor")
            rows = [BenchAuthor(name=f"B{i}", email=f"b{i}@bench.com") for i in range(1000)]
            count = await BenchAuthor.objects.bulk_create(rows)
            assert count == 1000
            await ormpi.close()
        loop.run_until_complete(_inner())

    benchmark(do_bulk)


# ---------------------------------------------------------------------------
# SELECT benchmarks — populated table (1000 rows)
# ---------------------------------------------------------------------------


@pytest.fixture
def populated_loop():
    """Event loop with 1000 rows pre-inserted."""
    lp = asyncio.new_event_loop()

    async def setup():
        await ormpi.configure({"default": {"ENGINE": "sqlite", "NAME": ":memory:"}})
        await create_table("bench.BenchAuthor")
        rows = [BenchAuthor(name=f"A{i}", email=f"a{i}@bench.com", age=i % 80)
                for i in range(1000)]
        await BenchAuthor.objects.bulk_create(rows)

    lp.run_until_complete(setup())
    yield lp
    lp.run_until_complete(ormpi.close())
    lp.close()


@pytest.mark.benchmark(group="select")
def test_full_table_scan(benchmark, populated_loop):
    """Full table scan — await all() over 1000 rows."""

    def scan():
        result = _exec(populated_loop, BenchAuthor.objects.all())
        assert len(result) == 1000

    benchmark(scan)


@pytest.mark.benchmark(group="filter")
def test_filtered_query(benchmark, populated_loop):
    """Filtered SELECT (age__gte=50) over 1000 rows."""

    def query():
        _exec(populated_loop, BenchAuthor.objects.filter(age__gte=50))

    benchmark(query)


@pytest.mark.benchmark(group="count")
def test_count_query(benchmark, populated_loop):
    """COUNT(*) over 1000 rows."""

    def count():
        n = _exec(populated_loop, BenchAuthor.objects.count())
        assert n == 1000

    benchmark(count)


@pytest.mark.benchmark(group="get")
def test_get_by_pk(benchmark, populated_loop):
    """Fetch a single row by PK."""
    # Grab a known PK from the table
    rows = _exec(populated_loop, BenchAuthor.objects.filter(name="A42"))
    pk = rows[0].pk

    def get_one():
        _exec(populated_loop, BenchAuthor.objects.get(pk=pk))

    benchmark(get_one)
