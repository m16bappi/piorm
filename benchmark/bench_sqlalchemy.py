import os
import sys

from sqlalchemy import create_engine, Column, Integer, String
from sqlalchemy.orm import sessionmaker, declarative_base

from bench_config import BenchResult, BULK_COUNT, timeit

Base = declarative_base()


class BenchAuthor(Base):
    __tablename__ = "bench_sqlalchemy_authors"

    id = Column(Integer, primary_key=True, autoincrement=True)
    name = Column(String(100), nullable=False)
    email = Column(String(200), nullable=False)
    age = Column(Integer, nullable=True)


engine = None
SessionLocal = None


def setup():
    global engine, SessionLocal
    engine = create_engine("sqlite:///:memory:", echo=False)
    SessionLocal = sessionmaker(bind=engine)
    Base.metadata.create_all(engine)


def teardown():
    global engine
    if engine:
        Base.metadata.drop_all(engine)
        engine.dispose()


def run() -> BenchResult:
    result = BenchResult(name="SQLAlchemy")

    def create_single():
        with SessionLocal() as session:
            author = BenchAuthor(name="SAUser", email="sa@bench.com", age=30)
            session.add(author)
            session.commit()

    result.create_ms = timeit(create_single)

    def bulk_create():
        with SessionLocal() as session:
            session.add_all(
                [
                    BenchAuthor(name=f"S{i}", email=f"s{i}@bench.com", age=i % 80)
                    for i in range(BULK_COUNT)
                ]
            )
            session.commit()

    result.bulk_create_ms = timeit(bulk_create)

    def read_all():
        with SessionLocal() as session:
            session.query(BenchAuthor).all()

    result.read_all_ms = timeit(read_all)

    def read_filter():
        with SessionLocal() as session:
            session.query(BenchAuthor).filter(BenchAuthor.age >= 50).all()

    result.read_filter_ms = timeit(read_filter)

    with SessionLocal() as session:
        first_pk = session.query(BenchAuthor).first().id

    def read_get():
        with SessionLocal() as session:
            session.query(BenchAuthor).get(first_pk)

    result.read_get_ms = timeit(read_get)

    def update():
        with SessionLocal() as session:
            session.query(BenchAuthor).filter(BenchAuthor.age >= 50).update(
                {BenchAuthor.email: "updated@bench.com"}, synchronize_session=False
            )
            session.commit()

    result.update_ms = timeit(update)

    def delete():
        with SessionLocal() as session:
            session.query(BenchAuthor).filter(BenchAuthor.age >= 70).delete(
                synchronize_session=False
            )
            session.commit()

    result.delete_ms = timeit(delete)

    return result
