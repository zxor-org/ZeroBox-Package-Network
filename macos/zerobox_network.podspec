Pod::Spec.new do |s|
  s.name             = 'zerobox_network'
  s.version          = '0.1.0'
  s.summary          = 'Userspace IP gateway for Xiaomi wearable devices'
  s.description      = <<-DESC
Native network package used by ZeroBox for Xiaomi Vela OS and MiWear devices.
                       DESC
  s.homepage         = 'https://github.com/zxor-org/ZeroBox-Package-Network'
  s.license          = { :type => 'AGPL-3.0', :file => '../LICENSE' }
  s.author           = { 'ZeroBox contributors' => 'https://github.com/zxor-org' }
  s.source           = { :path => '.' }
  s.platform         = :osx, '10.15'
  s.swift_version    = '5.0'
  s.source_files     = 'Classes/**/*'
  s.vendored_libraries = 'Frameworks/libzerobox_network.dylib'
  s.prepare_command = <<-CMD
    set -eu
    VERSION=0.1.0
    BASE="https://github.com/zxor-org/ZeroBox-Package-Network/releases/download/v${VERSION}"
    ASSET="zerobox-network-macos-universal.tar.gz"
    mkdir -p Frameworks
    curl -fL --retry 3 "${BASE}/${ASSET}" -o "/tmp/${ASSET}"
    curl -fL --retry 3 "${BASE}/${ASSET}.sha256" -o "/tmp/${ASSET}.sha256"
    cd /tmp
    shasum -a 256 -c "${ASSET}.sha256"
    cd - >/dev/null
    tar -xzf "/tmp/${ASSET}" -C Frameworks --strip-components=1 lib/libzerobox_network.dylib
  CMD
end
