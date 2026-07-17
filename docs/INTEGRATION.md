# Host integration

ZeroBox Package Network owns the userspace IP gateway only. The host remains responsible for the Xiaomi transport and device lifecycle

## Start

1. Authenticate the Xiaomi device and establish its normal protocol transport
2. Open one `ZeroboxNetworkSession` for the connected device
3. Send Xiaomi `SyncNetworkStatus` with network capability `2`
4. Route payloads received on Xiaomi `L2Channel.Network` into `pushInbound`
5. Send every packet emitted by `outboundPackets` back through `L2Channel.Network` with the write opcode

The native session answers DHCP locally and forwards non-DHCP IPv4 traffic through the host network. No TUN device, VPN permission, root access, or platform network service is required

## Stop

Close the session before disposing the Xiaomi transport. Closing cancels the IP stack and all active TCP and UDP forwarding tasks. Opening a later session starts from a clean state

## Backpressure

Ingress and event queues are bounded. `pushInbound` throws when the ingress queue is full, and the native layer increments `droppedPackets` whenever a queued event must be discarded. The host should log these conditions without retrying stale packets

## Example

```dart
final network = await ZeroboxNetworkSession.open(
  config: ZeroboxNetworkConfig(
    capturePath: diagnosticsEnabled ? pcapPath : null,
  ),
);

final outbound = network.outboundPackets.listen((packet) {
  xiaomi.sendNetworkPacket(packet);
});

xiaomi.networkPackets.listen(network.pushInbound);

// On disconnect:
await outbound.cancel();
await network.close();
```
