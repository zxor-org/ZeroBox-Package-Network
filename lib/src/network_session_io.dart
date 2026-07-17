import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

import 'network_bindings.dart';
import 'network_models.dart';

const _noEvent = 1;

class ZeroboxNetworkSession {
  ZeroboxNetworkSession._(this._bindings, this._handle, this._wakeCallback);

  static Future<ZeroboxNetworkSession> open({
    ZeroboxNetworkConfig config = const ZeroboxNetworkConfig(),
  }) async {
    final bindings = ZeroboxNetworkBindings.load();
    final nativeAbi = bindings.abiVersion();
    if (nativeAbi != zeroboxNetworkAbiVersion) {
      throw StateError(
        'ZeroBox Network ABI mismatch: Dart $zeroboxNetworkAbiVersion, native $nativeAbi',
      );
    }

    late ZeroboxNetworkSession session;
    final callback = NativeCallable<ZbWakeNative>.listener((int _) {
      session._scheduleDrain();
    });
    final nativeConfig = calloc<ZbNetworkConfigNative>();
    final outHandle = calloc<Uint64>();
    final capturePath = config.capturePath?.toNativeUtf8();
    try {
      nativeConfig.ref
        ..abiVersion = zeroboxNetworkAbiVersion
        ..mtu = config.mtu
        ..reserved = 0
        ..ingressCapacity = config.ingressCapacity
        ..stackCapacity = config.stackCapacity
        ..outboundCapacity = config.outboundCapacity
        ..meterWindowMilliseconds = config.meterWindowSeconds * 1000
        ..statisticsIntervalMilliseconds = config.statsIntervalMilliseconds
        ..capturePath = capturePath ?? nullptr;
      final status = bindings.open(
        nativeConfig,
        callback.nativeFunction,
        outHandle,
      );
      if (status != 0) {
        throw StateError(bindings.errorMessage());
      }
      session = ZeroboxNetworkSession._(bindings, outHandle.value, callback);
      session._scheduleDrain();
      return session;
    } catch (_) {
      callback.close();
      rethrow;
    } finally {
      if (capturePath != null) calloc.free(capturePath);
      calloc.free(outHandle);
      calloc.free(nativeConfig);
    }
  }

  final ZeroboxNetworkBindings _bindings;
  final int _handle;
  final NativeCallable<ZbWakeNative> _wakeCallback;
  final _events = StreamController<ZeroboxNetworkEvent>.broadcast(sync: true);
  final _outboundPackets = StreamController<Uint8List>.broadcast(sync: true);

  bool _closed = false;
  bool _drainScheduled = false;

  Stream<ZeroboxNetworkEvent> get events => _events.stream;
  Stream<Uint8List> get outboundPackets => _outboundPackets.stream;

  void pushInbound(Uint8List packet) {
    _ensureOpen();
    final pointer = calloc<Uint8>(packet.length);
    try {
      pointer.asTypedList(packet.length).setAll(0, packet);
      _check(_bindings.push(_handle, pointer, packet.length));
    } finally {
      calloc.free(pointer);
    }
  }

  ZeroboxNetworkSnapshot get snapshot {
    _ensureOpen();
    final native = calloc<ZbNetworkSnapshotNative>();
    try {
      _check(_bindings.snapshot(_handle, native));
      return ZeroboxNetworkSnapshot(
        bytesFromDevice: native.ref.bytesFromDevice,
        bytesToDevice: native.ref.bytesToDevice,
        activeSessions: native.ref.activeSessions,
        droppedPackets: native.ref.droppedPackets,
      );
    } finally {
      calloc.free(native);
    }
  }

  Future<void> close() async {
    if (_closed) return;
    _closed = true;
    final status = _bindings.close(_handle);
    _wakeCallback.close();
    if (!_events.isClosed) {
      _events.add(const ZeroboxNetworkClosed());
    }
    await Future.wait([_events.close(), _outboundPackets.close()]);
    if (status != 0) {
      throw StateError(_bindings.errorMessage());
    }
  }

  void _scheduleDrain() {
    if (_closed || _drainScheduled) return;
    _drainScheduled = true;
    scheduleMicrotask(() {
      _drainScheduled = false;
      if (_closed) return;
      try {
        _drain();
      } catch (error, stackTrace) {
        if (!_events.isClosed) _events.addError(error, stackTrace);
      }
    });
  }

  void _drain() {
    final kind = calloc<Uint32>();
    final length = calloc<Size>();
    try {
      while (!_closed) {
        final peekStatus = _bindings.eventPeek(_handle, kind, length);
        if (peekStatus == _noEvent) return;
        _check(peekStatus);
        final buffer = length.value == 0
            ? nullptr.cast<Uint8>()
            : calloc<Uint8>(length.value);
        try {
          _check(
            _bindings.eventRead(_handle, buffer, length.value, kind, length),
          );
          final payload = length.value == 0
              ? Uint8List(0)
              : Uint8List.fromList(buffer.asTypedList(length.value));
          _emit(_decodeEvent(kind.value, payload));
        } finally {
          if (buffer != nullptr) calloc.free(buffer);
        }
      }
    } finally {
      calloc.free(length);
      calloc.free(kind);
    }
  }

  ZeroboxNetworkEvent _decodeEvent(int kind, Uint8List payload) {
    switch (kind) {
      case 1:
        return ZeroboxNetworkPacket(payload);
      case 2:
        return ZeroboxNetworkStatus(utf8.decode(payload));
      case 3:
        final value = jsonDecode(utf8.decode(payload)) as Map<String, dynamic>;
        return ZeroboxNetworkStatistics(
          bytesFromDevice: value['bytes_from_device'] as int,
          bytesToDevice: value['bytes_to_device'] as int,
          readBytesPerSecond: (value['read_bytes_per_second'] as num)
              .toDouble(),
          writeBytesPerSecond: (value['write_bytes_per_second'] as num)
              .toDouble(),
          activeSessions: value['active_sessions'] as int,
          droppedPackets: value['dropped_packets'] as int,
        );
      case 4:
        return ZeroboxNetworkWarning(utf8.decode(payload));
      default:
        return ZeroboxNetworkWarning('Unknown native event kind $kind');
    }
  }

  void _emit(ZeroboxNetworkEvent event) {
    if (_events.isClosed) return;
    _events.add(event);
    if (event case ZeroboxNetworkPacket(:final bytes)) {
      _outboundPackets.add(bytes);
    }
  }

  void _check(int status) {
    if (status != 0) throw StateError(_bindings.errorMessage());
  }

  void _ensureOpen() {
    if (_closed) throw StateError('ZeroBox Network session is closed');
  }
}
