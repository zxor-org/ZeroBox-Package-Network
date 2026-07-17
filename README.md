# ZeroBox Package Network

[English](#english)

ZeroBox 的 Xiaomi 设备网络能力 Package，为 Xiaomi Vela OS / MiWear 设备提供网络数据包处理与宿主网络代理能力

本项目接收穿戴设备通过协议通道发送的原始 IPv4 数据包，在用户态完成 DHCP、TCP、UDP 与 ICMP 处理，并将设备流量转发至宿主网络

项目同时提供稳定的 C ABI、Dart FFI 封装、运行状态统计与可选 PCAP 捕获，供 ZeroBox 的 GUI、CLI 与 daemon 复用

仓库独立维护网络实现、跨平台构建流程、预构建产物与发布 CI

ZeroBox 的普通构建者无需安装 Rust 或 Cargo，只需使用 CI 发布的对应平台预构建包

支持平台：

- Android
- Linux
- macOS
- Windows

iOS 暂不支持，Web 仅提供明确的 unsupported 接口

## 鸣谢

本项目的 Xiaomi 网络实现从 [AstralSightStudios/AstroBox-NG-Module-Core](https://github.com/AstralSightStudios/AstroBox-NG-Module-Core) 拆分并参考其实现，由 AstroBox-NG 项目及其贡献者完成的原始工作为本项目提供了重要基础

AstroBox-NG-Module-Core 使用 GNU Affero General Public License v3.0，并附加署名要求

## 许可证

本项目使用 [GNU Affero General Public License v3.0](LICENSE) 授权

---

## English

ZeroBox Package Network provides Xiaomi device networking for Xiaomi Vela OS / MiWear wearable devices

It receives raw IPv4 packets transported through the wearable protocol channel, handles DHCP, TCP, UDP, and ICMP in userspace, and forwards device traffic through the host network.

The package also exposes a stable C ABI, a Dart FFI interface, runtime statistics, and optional PCAP capture for reuse by the ZeroBox GUI, CLI, and daemon

This repository independently maintains the networking implementation, cross-platform build pipeline, prebuilt artifacts, and release CI.

Regular ZeroBox builders do not need Rust or Cargo and can consume the prebuilt package for their target platform

Supported platforms:

- Android
- Linux
- macOS
- Windows

iOS is currently unsupported. Web exposes an explicit unsupported implementation only

## Acknowledgements

The Xiaomi networking implementation in this project is split from and based on [AstralSightStudios/AstroBox-NG-Module-Core](https://github.com/AstralSightStudios/AstroBox-NG-Module-Core). The original work by the AstroBox-NG project and its contributors provides an important foundation for this package

AstroBox-NG-Module-Core is licensed under the GNU Affero General Public License v3.0 with an additional attribution requirement

## License

This project is licensed under the [GNU Affero General Public License v3.0](LICENSE)
