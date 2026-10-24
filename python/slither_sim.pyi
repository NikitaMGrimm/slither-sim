"""In-process slither-sim sessions: slither-sim-json-v2 without JSON on the hot path."""

from array import array
from collections.abc import Iterable
import sys
from typing import Any, Final, final

if sys.version_info >= (3, 12):
    from collections.abc import Buffer
else:
    from typing_extensions import Buffer

__all__ = ["__version__", "PROTOCOL", "NONE", "NO_ORIGIN", "ORIGINS", "REASONS", "Session", "Frame"]

__version__: Final[str]
PROTOCOL: Final[str]
"""The JSON protocol of `Session.request`: "slither-sim-json-v2"."""
NONE: Final[int]
"""Row or slot index meaning none (2**32 - 1): `view_self` of dead slots, `death_killer` without a killer."""
NO_ORIGIN: Final[int]
"""`food_origin` of food without a recorded origin (255)."""
ORIGINS: Final[tuple[str, ...]]
"""Names of the `food_origin` codes."""
REASONS: Final[tuple[str, ...]]
"""Names of the `death_reason` codes."""

@final
class Session:
    """A session of independent worlds, as `slither-sim serve` keeps one per connection.

    Use one session from one thread at a time; overlapping calls raise RuntimeError.
    Separate sessions step concurrently in separate threads.
    """

    def __init__(self) -> None: ...
    def reset(
        self,
        seed: int = 1,
        worlds: int = 1,
        agents: int = 4,
        decision_ticks: int = 6,
        config: dict[str, Any] | None = None,
    ) -> None:
        """Replace the worlds like a JSON reset; raise ValueError on rejection, keeping the old ones."""
    def step(
        self,
        headings: Buffer | Iterable[float | None],
        boosts: Buffer | Iterable[object],
    ) -> None:
        """Step every world with `worlds * agents` world-major float64 headings and bool/uint8 boosts.

        Buffers may use any byte order. A NaN (or, in sequences, None) heading means no
        action, which only dead slots may give. Raise ValueError on rejection, changing
        no world.
        """
    def frame(self, world: int) -> Frame:
        """A copy of world `world`'s current frame; IndexError for any other index, negative included."""
    def request(self, message: dict[str, Any] | str | bytes) -> dict[str, Any]:
        """Answer one JSON request; return the parsed reply with `protocol` and `result` or `error`."""
    @property
    def worlds(self) -> int:
        """Worlds of the current session; 0 before the first reset."""
    @property
    def agents(self) -> int:
        """Agent slots per world; 0 before the first reset."""

@final
class Frame:
    """One world's frame. Columns are `array.array` copies: 'd' float64, 'I' uint32, 'Q' uint64, 'B' uint8."""

    @property
    def time(self) -> float: ...
    @property
    def ticks(self) -> int: ...
    @property
    def living(self) -> int: ...
    @property
    def radius(self) -> float: ...
    @property
    def vision(self) -> float: ...
    @property
    def snake_slot(self) -> array[int]: ...
    @property
    def snake_x(self) -> array[float]: ...
    @property
    def snake_y(self) -> array[float]: ...
    @property
    def snake_heading(self) -> array[float]: ...
    @property
    def snake_speed(self) -> array[float]: ...
    @property
    def snake_radius(self) -> array[float]: ...
    @property
    def snake_length(self) -> array[float]: ...
    @property
    def snake_segments(self) -> array[int]: ...
    @property
    def snake_fam(self) -> array[float]: ...
    @property
    def snake_score(self) -> array[float]: ...
    @property
    def snake_kills(self) -> array[int]: ...
    @property
    def snake_kill_score(self) -> array[int]: ...
    @property
    def snake_body_start(self) -> array[int]: ...
    @property
    def snake_body_x(self) -> array[float]: ...
    @property
    def snake_body_y(self) -> array[float]: ...
    @property
    def food_x(self) -> array[float]: ...
    @property
    def food_y(self) -> array[float]: ...
    @property
    def food_value(self) -> array[float]: ...
    @property
    def food_size(self) -> array[float]: ...
    @property
    def food_origin(self) -> array[int]: ...
    @property
    def prey_id(self) -> array[int]: ...
    @property
    def prey_x(self) -> array[float]: ...
    @property
    def prey_y(self) -> array[float]: ...
    @property
    def prey_heading(self) -> array[float]: ...
    @property
    def prey_target_heading(self) -> array[float]: ...
    @property
    def prey_speed(self) -> array[float]: ...
    @property
    def prey_size(self) -> array[float]: ...
    @property
    def view_self(self) -> array[int]: ...
    @property
    def view_snakes_start(self) -> array[int]: ...
    @property
    def view_snakes(self) -> array[int]: ...
    @property
    def view_food_start(self) -> array[int]: ...
    @property
    def view_food(self) -> array[int]: ...
    @property
    def view_prey_start(self) -> array[int]: ...
    @property
    def view_prey(self) -> array[int]: ...
    @property
    def death_time(self) -> array[float]: ...
    @property
    def death_victim(self) -> array[int]: ...
    @property
    def death_killer(self) -> array[int]: ...
    @property
    def death_reason(self) -> array[int]: ...
    @property
    def death_food(self) -> array[int]: ...
