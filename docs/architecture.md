# Architecture

## Layering

```text
borderless-cli ─┐
                ├── borderless-runtime ─── borderless-core traits ─── borderless-native
borderless-gui ─┘

borderless-core has no Win32 dependency and is testable as pure state transitions.
borderless-native owns every Win32 handle, unsafe call, and OS mutation.
```

## Sans I/O boundary

`borderless-core::reducer` turns UI/user intents into `Effect` values without touching the OS.
The CLI and GUI submit intents to the runtime controller. The controller queries the backend,
uses the pure core to create `BorderlessPlan`, then executes the plan through `WindowManipulator`.

## TypeState flow

```rust
BorderlessSession<Observed>
    .prepare(policy, monitors)?
    -> BorderlessSession<Prepared>
    .mark_applied()
    -> BorderlessSession<Applied>
    .mark_restored()
    -> BorderlessSession<Restored>
```

Only the prepared state exposes a concrete `BorderlessPlan`. Only the applied state carries the
`OriginalWindowState` needed for restore. This prevents accidental restore/apply order mistakes.

## Runtime / actor topology

```text
Supervisor
  ├─ ControllerActor: synchronous command/RPC surface for CLI and GUI
  ├─ WatcherActor: interval poller and favorite matcher
  └─ GuiBridgeActor: optional UI event fan-out and model snapshots
```

The actor messages are ADTs: commands, events, errors, and UI snapshots. This makes every cross-thread
boundary explicit and serializable later if we add IPC.

## Trait boundaries

- `WindowCatalog`: enumerate/query windows and monitors.
- `WindowManipulator`: apply plan, restore original state, taskbar/cursor/audio toggles.
- `SettingsStore`: load/save config.
- `EventSink`: push events to GUI/log/telemetry.

The CLI and GUI depend on those traits indirectly through actors, not on Win32 APIs.

## Windows-only policy

The application target is `x86_64-pc-windows-msvc`. The project does not maintain cross-platform
application shims; the core crate stays free of Win32 calls only so domain behavior remains easy to
test.
