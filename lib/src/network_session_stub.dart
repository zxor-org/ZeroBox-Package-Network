import 'dart:typed_data';

import 'network_models.dart';

class ZeroboxNetworkSession {
  ZeroboxNetworkSession._();

  static Future<ZeroboxNetworkSession> open({
    ZeroboxNetworkConfig config = const ZeroboxNetworkConfig(),
  }) =>
      throw UnsupportedError('ZeroBox Network is unavailable on this platform');

  Stream<ZeroboxNetworkEvent> get events => const Stream.empty();
  Stream<Uint8List> get outboundPackets => const Stream.empty();
  void pushInbound(Uint8List packet) => throw StateError('Session is closed');
  ZeroboxNetworkSnapshot get snapshot => throw StateError('Session is closed');
  Future<void> close() async {}
}
