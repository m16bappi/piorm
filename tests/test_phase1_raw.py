import pytest

import ormpi


@pytest.fixture(autouse=True)
async def setup():
    await ormpi.configure({"default": {"ENGINE": "sqlite", "NAME": ":memory:"}})


async def test_select_one():
    result = await ormpi.execute_raw("SELECT 1 as n")
    # result is list of (col_name, value) tuples
    assert len(result) == 1
    col, val = result[0]
    assert col == "n"
    assert val == 1


async def test_unknown_backend():
    with pytest.raises(Exception, match="Unknown database backend"):
        await ormpi.configure({"default": {"ENGINE": "oracle", "NAME": "test"}})
