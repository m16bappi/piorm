# Root package stubs. Only core setup symbols are exported here.
# Import everything else from the relevant sub-module:
#
#   from ormpi.fields import CharField, ForeignKey, CASCADE
#   from ormpi.expr import Q, F, Count, Sum
#   from ormpi.db import connections, configure, close
#   from ormpi.exceptions import DoesNotExist, ValidationError

from ormpi._core import execute_raw as execute_raw
from ormpi.db import configure as configure, close as close
from ormpi.models import Model as Model
from ormpi._core import (
    QuerySet as QuerySet,
    EvaluatedQuerySet as EvaluatedQuerySet,
    Manager as Manager,
)
from ormpi.fields import Field as Field

# Sub-module references
import ormpi.fields as fields
import ormpi.expr as expr
import ormpi.db as db
import ormpi.exceptions as exceptions

__version__: str
