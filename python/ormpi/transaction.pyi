from __future__ import annotations

from typing import Any
from ormpi._core import Transaction

class atomic:
    def __init__(self, using: str = "default") -> None: ...
    async def __aenter__(self) -> Transaction: ...
    async def __aexit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: Any,
    ) -> bool: ...
