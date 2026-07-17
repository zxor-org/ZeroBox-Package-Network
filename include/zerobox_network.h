#ifndef ZEROBOX_NETWORK_H
#define ZEROBOX_NETWORK_H

#include <stddef.h>
#include <stdint.h>

#ifdef _WIN32
#define ZB_NETWORK_API __declspec(dllimport)
#else
#define ZB_NETWORK_API
#endif

#ifdef __cplusplus
extern "C" {
#endif

#define ZB_NETWORK_ABI_VERSION 1u

enum zb_network_status {
  ZB_NETWORK_OK = 0,
  ZB_NETWORK_NO_EVENT = 1,
  ZB_NETWORK_INVALID_ARGUMENT = -1,
  ZB_NETWORK_NOT_FOUND = -2,
  ZB_NETWORK_BUFFER_TOO_SMALL = -3,
  ZB_NETWORK_INTERNAL = -4,
};

enum zb_network_event_kind {
  ZB_NETWORK_EVENT_PACKET = 1,
  ZB_NETWORK_EVENT_STATE = 2,
  ZB_NETWORK_EVENT_STATISTICS = 3,
  ZB_NETWORK_EVENT_WARNING = 4,
};

typedef void (*zb_network_wake_callback)(uint64_t handle);

typedef struct zb_network_config {
  uint32_t abi_version;
  uint16_t mtu;
  uint16_t reserved;
  uint32_t ingress_capacity;
  uint32_t stack_capacity;
  uint32_t outbound_capacity;
  uint32_t meter_window_ms;
  uint32_t statistics_interval_ms;
  const char *capture_path;
} zb_network_config;

typedef struct zb_network_snapshot {
  uint32_t abi_version;
  uint8_t active;
  uint8_t reserved[3];
  uint32_t active_sessions;
  uint64_t bytes_from_device;
  uint64_t bytes_to_device;
  uint64_t dropped_packets;
} zb_network_snapshot;

ZB_NETWORK_API uint32_t zb_network_abi_version(void);
ZB_NETWORK_API int32_t zb_network_open(const zb_network_config *config,
                                       zb_network_wake_callback callback,
                                       uint64_t *out_handle);
ZB_NETWORK_API int32_t zb_network_push(uint64_t handle, const uint8_t *data,
                                       size_t length);
ZB_NETWORK_API int32_t zb_network_event_peek(uint64_t handle,
                                             uint32_t *out_kind,
                                             size_t *out_length);
ZB_NETWORK_API int32_t zb_network_event_read(uint64_t handle, uint8_t *buffer,
                                             size_t capacity,
                                             uint32_t *out_kind,
                                             size_t *out_length);
ZB_NETWORK_API int32_t
zb_network_get_snapshot(uint64_t handle, zb_network_snapshot *out_snapshot);
ZB_NETWORK_API int32_t zb_network_close(uint64_t handle);
ZB_NETWORK_API const char *zb_network_last_error(void);

#ifdef __cplusplus
}
#endif

#endif
