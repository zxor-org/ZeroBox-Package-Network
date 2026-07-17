import 'dart:ffi';
import 'dart:io';

import 'package:ffi/ffi.dart';

final class ZbNetworkConfigNative extends Struct {
  @Uint32()
  external int abiVersion;

  @Uint16()
  external int mtu;

  @Uint16()
  external int reserved;

  @Uint32()
  external int ingressCapacity;

  @Uint32()
  external int stackCapacity;

  @Uint32()
  external int outboundCapacity;

  @Uint32()
  external int meterWindowMilliseconds;

  @Uint32()
  external int statisticsIntervalMilliseconds;

  external Pointer<Utf8> capturePath;
}

final class ZbNetworkSnapshotNative extends Struct {
  @Uint32()
  external int abiVersion;

  @Uint8()
  external int active;

  @Array<Uint8>(3)
  external Array<Uint8> reserved;

  @Uint32()
  external int activeSessions;

  @Uint64()
  external int bytesFromDevice;

  @Uint64()
  external int bytesToDevice;

  @Uint64()
  external int droppedPackets;
}

typedef ZbWakeNative = Void Function(Uint64 handle);

typedef AbiVersionNative = Uint32 Function();
typedef AbiVersionDart = int Function();
typedef OpenNative =
    Int32 Function(
      Pointer<ZbNetworkConfigNative>,
      Pointer<NativeFunction<ZbWakeNative>>,
      Pointer<Uint64>,
    );
typedef OpenDart =
    int Function(
      Pointer<ZbNetworkConfigNative>,
      Pointer<NativeFunction<ZbWakeNative>>,
      Pointer<Uint64>,
    );
typedef PushNative = Int32 Function(Uint64, Pointer<Uint8>, Size);
typedef PushDart = int Function(int, Pointer<Uint8>, int);
typedef EventPeekNative =
    Int32 Function(Uint64, Pointer<Uint32>, Pointer<Size>);
typedef EventPeekDart = int Function(int, Pointer<Uint32>, Pointer<Size>);
typedef EventReadNative =
    Int32 Function(
      Uint64,
      Pointer<Uint8>,
      Size,
      Pointer<Uint32>,
      Pointer<Size>,
    );
typedef EventReadDart =
    int Function(int, Pointer<Uint8>, int, Pointer<Uint32>, Pointer<Size>);
typedef SnapshotNative =
    Int32 Function(Uint64, Pointer<ZbNetworkSnapshotNative>);
typedef SnapshotDart = int Function(int, Pointer<ZbNetworkSnapshotNative>);
typedef CloseNative = Int32 Function(Uint64);
typedef CloseDart = int Function(int);
typedef LastErrorNative = Pointer<Utf8> Function();
typedef LastErrorDart = Pointer<Utf8> Function();

class ZeroboxNetworkBindings {
  ZeroboxNetworkBindings._(DynamicLibrary library)
    : abiVersion = library.lookupFunction<AbiVersionNative, AbiVersionDart>(
        'zb_network_abi_version',
      ),
      open = library.lookupFunction<OpenNative, OpenDart>('zb_network_open'),
      push = library.lookupFunction<PushNative, PushDart>('zb_network_push'),
      eventPeek = library.lookupFunction<EventPeekNative, EventPeekDart>(
        'zb_network_event_peek',
      ),
      eventRead = library.lookupFunction<EventReadNative, EventReadDart>(
        'zb_network_event_read',
      ),
      snapshot = library.lookupFunction<SnapshotNative, SnapshotDart>(
        'zb_network_get_snapshot',
      ),
      close = library.lookupFunction<CloseNative, CloseDart>(
        'zb_network_close',
      ),
      lastError = library.lookupFunction<LastErrorNative, LastErrorDart>(
        'zb_network_last_error',
      );

  factory ZeroboxNetworkBindings.load() =>
      ZeroboxNetworkBindings._(_openLibrary());

  final AbiVersionDart abiVersion;
  final OpenDart open;
  final PushDart push;
  final EventPeekDart eventPeek;
  final EventReadDart eventRead;
  final SnapshotDart snapshot;
  final CloseDart close;
  final LastErrorDart lastError;

  String errorMessage() {
    final pointer = lastError();
    return pointer == nullptr ? 'unknown native error' : pointer.toDartString();
  }
}

DynamicLibrary _openLibrary() {
  if (Platform.isWindows) {
    return DynamicLibrary.open('zerobox_network.dll');
  }
  if (Platform.isMacOS) {
    return DynamicLibrary.open('libzerobox_network.dylib');
  }
  if (Platform.isLinux || Platform.isAndroid) {
    return DynamicLibrary.open('libzerobox_network.so');
  }
  throw UnsupportedError(
    'ZeroBox Network is unsupported on ${Platform.operatingSystem}',
  );
}
