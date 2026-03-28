import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "python"))

import ormpi
from ormpi import CharField, IntegerField

from bench_config import BenchResult, BULK_COUNT, timeit_async


class BenchAuthor(ormpi.Model):
    name = CharField(max_length=100)
    email = CharField(max_length=200)
    age = IntegerField(null=True)

    class Meta:
        db_table = "bench_ormpi_authors"


async def _setup():
    await ormpi.configure({"default": {"ENGINE": "sqlite", "NAME": ":memory:"}}, auto_sync=True)


def setup():
    loop = __import__("asyncio").new_event_loop()
    loop.run_until_complete(_setup())
    loop.close()


async def _teardown():
    await ormpi.close()


def teardown():
    loop = __import__("asyncio").new_event_loop()
    loop.run_until_complete(_teardown())
    loop.close()


def run() -> BenchResult:
    result = BenchResult(name="ormpi")

    async def create_single():
        await BenchAuthor.objects.create(name="OrmpiUser", email="ormpi@bench.com", age=30)

    result.create_ms = timeit_async(create_single)

    async def bulk_create():
        rows = [
            BenchAuthor(name=f"O{i}", email=f"o{i}@bench.com", age=i % 80)
            for i in range(BULK_COUNT)
        ]
        await BenchAuthor.objects.bulk_create(rows)

    result.bulk_create_ms = timeit_async(bulk_create)

    async def read_all():
        await BenchAuthor.objects.all()

    result.read_all_ms = timeit_async(read_all)

    async def read_filter():
        await BenchAuthor.objects.filter(age__gte=50)

    result.read_filter_ms = timeit_async(read_filter)

    async def _get_pk():
        rows = await BenchAuthor.objects.filter(name="O42")
        return rows[0].pk

    loop = __import__("asyncio").new_event_loop()
    pk = loop.run_until_complete(_get_pk())
    loop.run_until_complete(loop.shutdown_asyncgens())
    loop.close()

    async def read_get():
        await BenchAuthor.objects.get(pk=pk)

    result.read_get_ms = timeit_async(read_get)

    async def update():
        await BenchAuthor.objects.filter(age__gte=50).aupdate(email="updated@bench.com")

    result.update_ms = timeit_async(update)

    async def delete():
        await BenchAuthor.objects.filter(age__gte=70).adelete()

    result.delete_ms = timeit_async(delete)

    return result
