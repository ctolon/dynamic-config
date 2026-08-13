"""`@dynamic_config`: the configuration attached to the model class.

For code that would rather import a class than be handed an object. It
attaches a `DynamicConfig` and the handful of methods that read through
it, and refuses a model that declares a field by one of those names.

**Inherit `Configured` if you type-check.** Attributes attached at
runtime are invisible to a type checker — Python has no way to say "this
class, plus these six members" — so `Database.current()` on a class that
only carries the decorator is an `attr-defined` error under mypy and
nothing at all to an editor's completion. `Configured` declares the six
where a checker can see them, and the decorator fills them in:

    @dynamic_config(key="db", files=["config.toml"])
    class Database(Configured, BaseModel):
        host: str = "localhost"

    Database.current().host        # typed as `str`, completes in an editor

The decorator alone still works, unchanged, for code that does not
type-check — but it cannot be made visible to one, and pretending
otherwise would be worse than saying it.
"""

from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING, Any, Callable, ClassVar, Optional, TypeVar, cast

from ._config import DynamicConfig
from ._core import DynamicConfigError
from ._diagnostics import Explanation, Origin
from ._schema import field_names

if TYPE_CHECKING:
    # 3.11 has it in `typing`; every type checker resolves it from
    # `typing_extensions` on older ones, and this import never runs.
    from typing_extensions import Self

M = TypeVar("M")


class Configured:
    """What `@dynamic_config` attaches, declared so a checker can see it.

    A mixin with no runtime behaviour of its own beyond delegating to
    `config`, which the decorator sets. Inheriting it is what makes
    `Database.current()` type-check and complete in an editor; the
    methods below are the same ones the decorator would otherwise attach
    at runtime.

    It declares no fields, so a Pydantic model gains nothing but the
    methods — `model_fields` is untouched.
    """

    #: The configuration the decorator built. Set at decoration; reading
    #: it before then is an `AttributeError` that names this class.
    config: ClassVar[DynamicConfig[Any]]

    @classmethod
    def current(cls) -> Self:
        """The installed model. Raises before the first successful load."""
        return cast("Self", cls.config.current())

    @classmethod
    def try_current(cls) -> Optional[Self]:
        """The installed model, or ``None`` before the first load."""
        return cast("Optional[Self]", cls.config.try_current())

    @classmethod
    def reload(cls) -> None:
        """One reload: load, validate, install, rewrite the cache."""
        cls.config.reload()

    @classmethod
    def source_of(cls, path: str) -> Optional[Origin]:
        """Which layer would supply ``path``, or ``None``."""
        return cls.config.source_of(path)

    @classmethod
    def explain(cls, path: str) -> Explanation:
        """Every layer's answer for ``path``, secrets redacted."""
        return cls.config.explain(path)


#: What the decorator puts on the class. A model declaring a field by one
#: of these names would have it shadowed, so that is refused rather than
#: silently overwritten.
ATTACHED = ("config", "current", "try_current", "reload", "source_of", "explain")


# ── The decorator ──────────────────────────────────────────────────────


def dynamic_config(
    *,
    key: str,
    files: Sequence[str] = (),
    discover: tuple[str, Sequence[str]] | None = None,
    env: str | None = None,
    nest: str | None = None,
    allow_empty_env: bool = False,
    strict_env: bool = False,
    env_files: Sequence[str] = (),
    profile_env: str | None = None,
    cache: str | None = None,
    cache_mode: str = "redacted",
    init: bool = False,
    watch: float | None = None,
) -> Callable[[type[M]], type[M]]:
    """Attaches a configuration to a Pydantic model class.

        @dynamic_config(key="db", files=["config.toml"], env="APP_")
        class Database(BaseModel):
            host: str
            port: int = 5432

        Database.config.init()
        Database.current()

    The decorator builds a :class:`DynamicConfig`, stores it as
    ``Model.config`` and attaches ``current``/``try_current``/``reload``/
    ``source_of``/``explain`` classmethods over it.

    It does **not** load by default: import time is the wrong time to read
    files, and a script that disagrees passes ``init=True``. Decorating one
    class twice is an error, mirroring the crate's one-configuration-per-type
    rule.
    """

    def decorate(model: type[M]) -> type[M]:
        """Attaches the configuration and the classmethods over it."""
        if "config" in vars(model):
            raise DynamicConfigError(
                f"{model.__name__} already has a configuration attached; "
                "one declaration per class"
            )

        # The decorator hangs six names on the class. A model that
        # declares a field called `config` or `reload` would have them
        # shadowed at class level and nowhere else — the kind of collision
        # that reads as a Pydantic bug three files away. Refuse it here,
        # where the cause is on screen.
        collisions = [name for name in ATTACHED if name in set(field_names(model))]

        if collisions:
            raise DynamicConfigError(
                f"{model.__name__} declares {', '.join(collisions)}, which the "
                "decorator would shadow; use DynamicConfig(...) directly for "
                "this model"
            )

        config: DynamicConfig[M] = DynamicConfig(model, key)

        for path in files:
            config.file(path)
        if discover is not None:
            config.discover(discover[0], discover[1])
        if env is not None:
            config.env(env)
        if nest is not None:
            config.nest(nest)
        if allow_empty_env:
            config.allow_empty_env()
        if strict_env:
            config.strict_env()
        for path in env_files:
            config.env_file(path)
        if profile_env is not None:
            config.profile_env(profile_env)
        if cache is not None:
            config.cache(cache, cache_mode)

        model.config = config  # type: ignore[attr-defined]

        # A class that inherits `Configured` already has the five readers,
        # typed, and they delegate through `config` — which was just set.
        # Attaching them again would shadow the typed ones with untyped
        # lambdas, which is the opposite of the point.
        if not issubclass(model, Configured):
            model.current = classmethod(lambda _cls: config.current())  # type: ignore[attr-defined]
            model.try_current = classmethod(lambda _cls: config.try_current())  # type: ignore[attr-defined]
            model.reload = classmethod(lambda _cls: config.reload())  # type: ignore[attr-defined]
            model.source_of = classmethod(  # type: ignore[attr-defined]
                lambda _cls, path: config.source_of(path)
            )
            model.explain = classmethod(  # type: ignore[attr-defined]
                lambda _cls, path: config.explain(path)
            )

        if init:
            config.init()
        if watch is not None:
            config.watch(watch).detach()

        return model

    return decorate
