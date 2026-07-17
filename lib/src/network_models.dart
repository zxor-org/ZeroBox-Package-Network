import 'dart:typed_data';

const zeroboxNetworkAbiVersion = 1;

class ZeroboxNetworkConfig {
  const ZeroboxNetworkConfig({
    this.mtu = 800,
    this.ingressCapacity = 256,
    this.stackCapacity = 256,
    this.outboundCapacity = 128,
    this.meterWindowSeconds = 5,
    this.statsIntervalMilliseconds = 1000,
    this.capturePath,
  });

  final int mtu;
  final int ingressCapacity;
  final int stackCapacity;
  final int outboundCapacity;
  final int meterWindowSeconds;
  final int statsIntervalMilliseconds;
  final String? capturePath;
}

enum ZeroboxNetworkEventType { packet, state, statistics, warning, closed }

sealed class ZeroboxNetworkEvent {
  const ZeroboxNetworkEvent(this.type);
  final ZeroboxNetworkEventType type;
}

class ZeroboxNetworkPacket extends ZeroboxNetworkEvent {
  const ZeroboxNetworkPacket(this.bytes)
    : super(ZeroboxNetworkEventType.packet);
  final Uint8List bytes;
}

class ZeroboxNetworkStatus extends ZeroboxNetworkEvent {
  const ZeroboxNetworkStatus(this.message)
    : super(ZeroboxNetworkEventType.state);
  final String message;
}

class ZeroboxNetworkStatistics extends ZeroboxNetworkEvent {
  const ZeroboxNetworkStatistics({
    required this.bytesFromDevice,
    required this.bytesToDevice,
    required this.readBytesPerSecond,
    required this.writeBytesPerSecond,
    required this.activeSessions,
    required this.droppedPackets,
  }) : super(ZeroboxNetworkEventType.statistics);

  final int bytesFromDevice;
  final int bytesToDevice;
  final double readBytesPerSecond;
  final double writeBytesPerSecond;
  final int activeSessions;
  final int droppedPackets;
}

class ZeroboxNetworkWarning extends ZeroboxNetworkEvent {
  const ZeroboxNetworkWarning(this.message)
    : super(ZeroboxNetworkEventType.warning);
  final String message;
}

class ZeroboxNetworkClosed extends ZeroboxNetworkEvent {
  const ZeroboxNetworkClosed() : super(ZeroboxNetworkEventType.closed);
}

class ZeroboxNetworkSnapshot {
  const ZeroboxNetworkSnapshot({
    required this.bytesFromDevice,
    required this.bytesToDevice,
    required this.activeSessions,
    required this.droppedPackets,
  });

  final int bytesFromDevice;
  final int bytesToDevice;
  final int activeSessions;
  final int droppedPackets;
}
