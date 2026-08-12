"""Planted secrets, at every door that could leak one.

The rule the Rust suite pins, ported: values stay out of diagnostics.
`explain` is the documented exception, and it redacts the fields the model
itself typed as secret — nobody keeps a second list.
"""

from __future__ import annotations

from pathlib import Path
from typing import Optional

import pytest
from pydantic import BaseModel, SecretBytes, SecretStr

from dynamic_config import DynamicConfig, InvalidError, secret_paths

PLANTED = "hunter2-planted-secret"


class Credentials(BaseModel):
    user: str
    password: SecretStr


class Service(BaseModel):
    host: str
    token: SecretStr
    fallback: Optional[SecretStr] = None
    signature: SecretBytes = SecretBytes(b"")
    credentials: Credentials


def write_config(port_line: str = "") -> None:
    Path("config.toml").write_text(
        f"""
[svc]
host = "example.internal"
token = "{PLANTED}"
fallback = "{PLANTED}"
signature = "{PLANTED}"
{port_line}

[svc.credentials]
user = "app"
password = "{PLANTED}"
"""
    )


def test_the_secret_list_is_derived_from_the_model() -> None:
    paths = set(secret_paths(Service))

    assert paths == {
        "token",
        "fallback",
        "signature",
        "credentials.password",
    }, "SecretStr, SecretBytes, Optional and nested models all count"


def test_a_self_referencing_model_does_not_recurse_forever() -> None:
    class Node(BaseModel):
        secret: SecretStr
        child: Optional[Node] = None

    Node.model_rebuild()

    assert secret_paths(Node) == ["secret"]


def test_explain_redacts_every_secret_shape(workspace: Path) -> None:
    write_config()

    config = DynamicConfig(Service, key="svc").file("config.toml")
    config.init()

    for path in ("token", "fallback", "signature", "credentials.password"):
        rendered = str(config.explain(path))

        assert PLANTED not in rendered, f"{path} leaked through explain"
        assert "***" in rendered, f"{path} should render redacted"

    # The non-secret neighbour still shows its value: redaction is not a
    # blanket, it is the model's own statement about which fields matter.
    assert "example.internal" in str(config.explain("host"))


def test_no_diagnostic_surface_carries_the_value(workspace: Path) -> None:
    write_config()

    config = DynamicConfig(Service, key="svc").file("config.toml")
    config.init()

    surfaces = [
        repr(config),
        repr(config.snapshot()),
        repr(config.check()),
        repr(config.explain("token")),
        str(config.source_of("token")),
        repr(config.watch(debounce=0.05, poll_interval=0.05)),
    ]

    for rendered in surfaces:
        assert PLANTED not in rendered, f"a secret reached {rendered!r}"


def test_a_validation_failure_does_not_echo_the_secret(workspace: Path) -> None:
    class Strict(BaseModel):
        token: SecretStr
        port: int

    Path("config.toml").write_text(
        f'[svc]\ntoken = "{PLANTED}"\nport = "not-a-number"\n'
    )

    config = DynamicConfig(Strict, key="svc").file("config.toml")

    with pytest.raises(InvalidError) as failure:
        config.init()

    assert PLANTED not in str(failure.value)
    assert "not-a-number" not in str(failure.value), (
        "Pydantic echoes the offending input by default; this boundary does not"
    )

    for report in getattr(failure.value, "errors", []):
        assert PLANTED not in repr(report)
        assert "input" not in report


def test_the_redacted_cache_leaves_secrets_off_disk(workspace: Path) -> None:
    write_config()

    config = (
        DynamicConfig(Service, key="svc")
        .file("config.toml")
        .cache("last.json", "redacted")
    )
    config.init()

    written = Path("last.json").read_text()

    assert PLANTED not in written, "a redacted cache is the point"
    assert "example.internal" in written, "and it still holds what is not secret"


def test_the_snapshot_hands_over_values_because_that_is_its_job(
    workspace: Path,
) -> None:
    write_config()

    config = DynamicConfig(Service, key="svc").file("config.toml")
    config.init()

    # Handover, not a diagnostic: `to_dict` is how a program *gets* the
    # configuration, exactly like reading the model. The rule governs what
    # this library prints, not what it hands you.
    assert config.snapshot().to_dict()["token"] == PLANTED
    assert PLANTED not in repr(config.snapshot())


def test_changed_paths_sees_secrets_but_reports_only_paths() -> None:
    """A changed password has to be *noticed* without being *printed*.

    Comparing the mask Pydantic renders would make two different secrets
    look equal — an audit trail that quietly misses the one change most
    worth noticing — so the comparison sees the value and the output does
    not.
    """
    from dynamic_config import changed_paths

    before = Credentials(user="app", password=SecretStr(PLANTED))
    after = Credentials(user="app", password=SecretStr("a-different-secret"))

    changes = changed_paths(before, after)

    assert [change.path for change in changes] == ["password"]
    assert changes[0].kind == "changed"

    rendered = "".join(str(change) for change in changes)
    assert PLANTED not in rendered
    assert "a-different-secret" not in rendered

    # And an unchanged secret is not reported as a change.
    assert changed_paths(before, before) == []


def test_aliases_decide_the_name_a_secret_is_known_by(workspace: Path) -> None:
    """A config file carries the alias, so the secret list must too."""
    from pydantic import Field

    class Aliased(BaseModel):
        token: SecretStr = Field(alias="api_token")
        host: str = Field(validation_alias="hostname")

    # Both spellings, deliberately. A file carries the alias today and may
    # carry the field name tomorrow — `populate_by_name`, an edit, an
    # `AliasChoices`. Redacting a name nothing supplies costs nothing;
    # missing one costs a secret, in a diagnostic and in the cache on disk.
    assert set(secret_paths(Aliased)) == {"token", "api_token"}

    Path("config.toml").write_text(
        f'[svc]\napi_token = "{PLANTED}"\nhostname = "example.internal"\n'
    )

    config = (
        DynamicConfig(Aliased, key="svc")
        .file("config.toml")
        .cache("aliased-cache.json", "redacted")
    )
    config.init()

    assert config.current().token.get_secret_value() == PLANTED
    assert PLANTED not in str(config.explain("api_token"))
    assert PLANTED not in Path("aliased-cache.json").read_text()
