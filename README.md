# ormpi

A Django-style **async ORM for Python**, with the query engine written in **Rust**.

You define models and write queries the Django way (`Model.objects.filter(...)`, `Q`, `F`, `select_related`, migrations). Underneath, Rust builds the SQL with [sea-query](https://github.com/SeaQL/sea-query) and runs it with [sqlx](https://github.com/launchbadge/sqlx), so queries never block the event loop and row decoding happens outside Python.

Supports **SQLite**, **PostgreSQL** and **MySQL**.

## Example

```python
import asyncio
import ormpi
from ormpi.fields import CharField, IntegerField, BooleanField, ForeignKey, CASCADE
from ormpi.expr import Q, F, Count


class Author(ormpi.Model):
    name = CharField(max_length=100)
    email = CharField(max_length=200, unique=True)
    active = BooleanField(default=True)


class Article(ormpi.Model):
    title = CharField(max_length=200)
    views = IntegerField(default=0)
    author = ForeignKey("Author", on_delete=CASCADE)


async def main():
    await ormpi.configure(
        {"default": {"ENGINE": "sqlite", "NAME": ":memory:"}},
        auto_sync=True,
    )

    alice = await Author.objects.create(name="Alice", email="alice@example.com")
    await Article.objects.create(title="Hello", views=10, author=alice)
    await Article.objects.create(title="Rust + Python", views=250, author=alice)

    popular = await Article.objects.filter(
        Q(views__gte=100) | Q(title__icontains="hello")
    ).exclude(author__active=False).order_by("-views")

    for article in popular:
        print(article.title, article.views)

    await Article.objects.filter(pk=popular[0].pk).update(views=F("views") + 1)

    stats = await Article.objects.aggregate(total=Count("id"))
    print(stats)

    await ormpi.close()


asyncio.run(main())
```

## Features

**Models and fields**
- 26 field types: integers, floats, decimals, text, slug, email, URL, IP, boolean, date and time, duration, UUID, JSON and binary
- Relations: `ForeignKey`, `OneToOneField`, `ManyToManyField` with `on_delete` behaviour
- Type stubs (`.pyi`) for the whole public API, checked with mypy and pyright

**Queries**
- `get`, `filter`, `exclude`, `all`, `first`, `last`, `earliest`, `latest`, `count`, `exists`
- Field lookups: `exact`, `gt`, `gte`, `lt`, `lte`, `in`, `isnull`, `contains`, `icontains`, `startswith`, `endswith`, `range` and more
- `Q` objects (AND, OR, NOT) and `F` expressions
- Aggregates and annotations: `Count`, `Sum`, `Avg`, `Max`, `Min`
- `values`, `values_list`, `only`, `defer`, `distinct`, ordering and slicing
- Eager loading with `select_related` (JOIN) and `prefetch_related` (separate query)
- Async `iterator()` for chunked streaming, plus raw SQL

**Databases**
- Several databases at once, with `using(alias)` and Django-style database routers
- Connection pooling through sqlx

**Migrations CLI** (written in Rust)

```bash
export DATABASE_URL=sqlite://./app.sqlite3

ormpi makemigrations    # detect model changes and write migration files
ormpi migrate           # apply or reverse migrations
ormpi showmigrations    # list migrations and whether they are applied
ormpi sqlmigrate <name> # print the SQL for a migration
ormpi inspectdb         # generate models from an existing database
```

## Architecture

```
python/ormpi/     Python API: Model, fields, Q/F expressions, routers, type stubs
      │  PyO3
ormpi-py/         Rust extension module: models, QuerySet, value conversion, async runtime
      │
ormpi-core/       Rust core: query state, filters, joins, aggregates, migrations,
      │           field types, connection pools
      ├── sea-query   SQL building for SQLite, PostgreSQL and MySQL
      └── sqlx        async drivers
ormpi-cli/        Rust CLI for migrations and introspection
```

The Python layer stays thin. A `QuerySet` collects its state in Rust, and Rust turns it into one SQL statement only when it is awaited. Python and Rust are built together into one wheel with [maturin](https://www.maturin.rs).

## Benchmarks

`benchmark/` compares CRUD throughput for ormpi, Django ORM and SQLAlchemy on the same schema:

```bash
cd benchmark
uv run python run.py
```

## Development

Requirements: Rust, Python 3.9+ and [uv](https://docs.astral.sh/uv/).

```bash
make install      # sync dependencies and build the Rust extension
make test         # pytest suite: 258 tests covering create, read, update, delete
make cargo-test   # Rust unit tests
make check        # ruff, mypy, rustfmt and clippy
make build        # build a release wheel into target/wheels/
```

Design notes are in [`docs/`](docs/), for example [async transaction patterns](docs/async-orm-transaction-patterns.md) and the [low-overhead instance layout](docs/proposals/low-overhead-instance-layout.md) proposal.

## License

MIT or Apache-2.0, at your option.
