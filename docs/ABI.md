# ZeroBox Network ABI

The public native boundary is the versioned C ABI declared in [`include/zerobox_network.h`](../include/zerobox_network.h)

## Ownership

- `zb_network_open` creates a session and returns an opaque non-zero handle
- The caller owns the handle and must call `zb_network_close` exactly once
- Buffers passed to `zb_network_push` are copied before the function returns
- Event payloads remain owned by the native session until `zb_network_event_read` copies and removes them
- The pointer returned by `zb_network_last_error` is thread-local and remains valid until the next ABI call on that thread

## Event delivery

The wake callback is a notification only. It may run on a native runtime thread and must not perform blocking work or call back into the session directly

After a wake, the host drains the queue with `zb_network_event_peek` and `zb_network_event_read` until `ZB_NETWORK_NO_EVENT` is returned. Dart uses `NativeCallable.listener`, so native runtime threads only enqueue isolate messages while all FFI reads stay on the Dart isolate

Packet events contain raw IPv4 packets that the host sends back to the Xiaomi protocol network channel. Statistics events contain UTF-8 JSON. State and warning events contain UTF-8 text

## Compatibility

Both `zb_network_abi_version()` and the `abi_version` fields currently return `1`. A caller must reject a library with a different major ABI version

Struct fields may only be appended in a future compatible ABI. Existing field order, width, signedness, and function signatures are stable for ABI version 1
