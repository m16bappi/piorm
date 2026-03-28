import os
import sys
import django

os.environ.setdefault("DJANGO_SETTINGS_MODULE", "django_settings")

sys.path.insert(0, os.path.dirname(__file__))
django.setup()

from django.db import models, connection

from bench_config import BenchResult, BULK_COUNT, timeit


class BenchAuthor(models.Model):
    name = models.CharField(max_length=100)
    email = models.CharField(max_length=200)
    age = models.IntegerField(null=True, blank=True)

    class Meta:
        app_label = "benchmark"


def setup():
    with connection.schema_editor() as editor:
        editor.create_model(BenchAuthor)


def teardown():
    with connection.schema_editor() as editor:
        editor.delete_model(BenchAuthor)


def run() -> BenchResult:
    result = BenchResult(name="Django ORM")
    connection.ensure_connection()

    def create_single():
        BenchAuthor.objects.create(name="DjangoUser", email="django@bench.com", age=30)

    result.create_ms = timeit(create_single)

    def bulk_create():
        BenchAuthor.objects.bulk_create(
            [
                BenchAuthor(name=f"D{i}", email=f"d{i}@bench.com", age=i % 80)
                for i in range(BULK_COUNT)
            ]
        )

    result.bulk_create_ms = timeit(bulk_create)

    def read_all():
        list(BenchAuthor.objects.all())

    result.read_all_ms = timeit(read_all)

    def read_filter():
        list(BenchAuthor.objects.filter(age__gte=50))

    result.read_filter_ms = timeit(read_filter)

    first_pk = BenchAuthor.objects.first().pk

    def read_get():
        BenchAuthor.objects.get(pk=first_pk)

    result.read_get_ms = timeit(read_get)

    def update():
        BenchAuthor.objects.filter(age__gte=50).update(email="updated@bench.com")

    result.update_ms = timeit(update)

    def delete():
        BenchAuthor.objects.filter(age__gte=70).delete()

    result.delete_ms = timeit(delete)

    return result
