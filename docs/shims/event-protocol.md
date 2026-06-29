# Shim event protocol

This document defines the protocol used by protocol-compliant shims to report
invocation metadata to reportage.

For the conceptual shim model, see [../shims.md](../shims.md).

## Purpose

reportage does not parse action command text to infer shim usage. A shim knows
most reliably whether it was invoked. Therefore, protocol-compliant shims emit
invocation events.

The runner consumes these events and attaches them to action results,
diagnostics, and artifacts.

## Protocol status

This protocol is experimental during early v0. The schema may evolve before
stabilization. `schema_version: 1` is used for the initial shape.

## Runner-provided event directory

Before executing each action, the runner creates a fresh action-scoped event
directory. The runner exposes it via environment variable, preferably:

```txt
REPORTAGE_SHIM_EVENT_DIR=<action-scoped event directory>
```

Shims invoked during that action write event files into that directory. The
runner reads only that directory after action completion. Stale events from
previous actions must not be attached to later actions.

## Event file placement

Each shim invocation writes one event file. Event files are JSON. Event file
names must avoid collision within the action directory.

Exact file naming may be implementation-defined in v0. A single fixed filename
is not allowed because one action may invoke multiple shims.

Example:

```txt
$REPORTAGE_SHIM_EVENT_DIR/<unique-event-id>.json
```

## Event timing

POSIX wrappers may use `exec`. Therefore, shims must write event data before
delegating to the target invocation.

Event data records the shim invocation and target invocation, not post-execution
status. Exit code, stdout, and stderr remain action result data.

## Minimum event schema

```json
{
  "schema_version": 1,
  "event": "shim_invoked",
  "command_name": "reportage",
  "shim_path": "/tmp/reportage-selftest-xxxx/bin/reportage",
  "target": {
    "program": "/absolute/path/to/reportage",
    "args": []
  },
  "forwards_caller_args": true
}
```

Fields:

- `schema_version`: protocol schema version.
- `event`: event type. Initial value is `shim_invoked`.
- `command_name`: command name represented by the shim.
- `shim_path`: path to the shim file that was invoked.
- `target.program`: absolute program path used by the shim.
- `target.args`: fixed invocation arguments embedded before caller-provided arguments.
- `forwards_caller_args`: whether the shim forwards caller-provided args.

The target is an executable invocation, not merely a binary path.
Caller-provided args are not required to be recorded in v0.

## Runner collection semantics

The runner reads event files after action completion. Valid events are attached
to the corresponding `ActionResult`.

An action result may contain zero, one, or multiple shim invocation records.
Absence of events does not fail the action by itself.

## Malformed event files

Malformed event files must not silently corrupt action results. The runner
should surface malformed event files as diagnostics.

The implementation may decide exact diagnostic placement, but it must be visible
in artifacts or failure diagnostics.

## Event write failure

Runner-generated shims must not fail silently if they cannot write an event. The
initial behavior is:

- emit a prefixed stderr diagnostic, for example `reportage shim warning: failed to write shim invocation event: ...`;
- continue delegating to the target invocation.

These diagnostics are observable stderr. They are not automatically filtered out
from stdout/stderr assertions. They may affect `stderr empty`.

## Non-compliant shims

A shim that does not write compliant events may be indistinguishable from direct
target or ambient command invocation.

Metadata absence means no protocol-compliant shim invocation was observed.
Metadata absence does not prove no shim was used. Third-party shim validation is
deferred.

## Deferred topics

- stable third-party shim protocol;
- `reportage shim test <shim-file>` or equivalent validation interface;
- dedicated diagnostic side channel;
- run-level warning file;
- recording caller-provided args;
- schema stabilization.
