# LAN Chat wire protocol — version 1

LAN Chat is serverless. Every device runs a WebSocket server on a random TCP
port and also dials every peer it discovers. There is no relay and no central
registry: the protocol only describes the frames two peers exchange after they
find each other on the LAN.

This document is normative for `core/src/protocol`. The Rust types are the
source of truth; unknown JSON fields are ignored on decode, and new optional
fields default when absent. Within major version `1`, changes must stay
additive.

- Transport: WebSocket text frames (JSON, UTF-8), URL path `/lanchat`
- Protocol version: `1` (`"v": 1` in every envelope and every discovery beacon)
- Maximum frame: 8 MiB of JSON text (`MAX_FRAME_BYTES`)
- Text limit: 16 000 characters per message
- Display/device name limit: 64 characters
- File chunk payload: 64 KiB before base64 (`FILE_CHUNK_BYTES`)

---

## 1. Envelope

Every frame — including the handshake — is a JSON object with envelope fields
at the top level and the body flattened into the same object. The body is
discriminated by `type`.

```json
{
  "v": 1,
  "id": "6f1c5b8e-6a37-4f8a-9f7d-1fbfa1e6c3a2",
  "from": "4b8e6a2c-2f0e-4d6a-9e30-7c9a2f3b5d10",
  "to": "0d9e8f13-5b25-4d77-a4e3-2c7a6b9d1f44",
  "ts": 1731427200123,
  "type": "message",
  "conv": "0d9e8f13-5b25-4d77-a4e3-2c7a6b9d1f44",
  "conv_kind": "direct",
  "seq": 42,
  "kind": "text",
  "text": "Hi from the kitchen",
  "mentions": []
}
```

| Field  | Type            | Required | Meaning |
| ------ | --------------- | -------- | ------- |
| `v`    | number          | yes      | Wire version. Must be `1`. Frames with any other value are rejected with `UnsupportedVersion`. |
| `id`   | UUID            | yes      | Frame id. For `message` frames this **is** the message id used by `ack`. |
| `from` | UUID            | yes      | Sender peer id. Must never be nil. |
| `to`   | UUID \| absent  | no       | Intended recipient. Direct frames set it; broadcast-ish handshake rejections may omit it. A peer drops frames whose `to` is not its own id. |
| `ts`   | number          | yes      | Sender wall clock, milliseconds since the Unix epoch. Ordering hint only; receivers store local insertion time. |
| `type` | string          | yes      | Body discriminator: `hello`, `hello_ack`, `presence`, `message`, `ack`, `typing`, `group_create`, `group_update`, `file_offer`, `file_chunk`, `file_ack`, `sealed`, `error`. |

Body field names below are flattened into the envelope. Enum values are
`snake_case` strings.

Shared enums:

| Enum | Values |
| ---- | ------ |
| `conv_kind` | `direct`, `group` |
| `message.kind` | `text`, `image`, `video`, `audio`, `file`, `system` |
| `ack.state` | `queued`, `sending`, `sent`, `delivered`, `read`, `failed` |
| `group_update.op` | `add_member`, `remove_member`, `rename`, `leave`, `sync` |
| `file_ack.status` | `accept`, `reject`, `resume`, `progress`, `done`, `cancel` |

### Address rules

- A direct conversation id **is** the remote peer's UUID. `message.conv` must
  equal `envelope.from` for `direct` frames; receivers drop mismatches.
- A group conversation id is the group UUID. Group messages are not broadcast:
  the sender emits one unicast envelope per member. Each member receives the
  same message id.
- The automatic "Everyone" group has the fixed id
  `00000000-0000-0000-0000-000000000001` (`EVERYONE_GROUP_ID`). It is implicit
  and is never sent with `group_create`.

### Frame processing rules

1. Decode JSON (reject > 8 MiB before parsing).
2. Require `v == 1`.
3. On an established connection, require `from ==` the authenticated peer id
   and, when present, `to ==` this device's id.
4. Validate length limits for the body.
5. Unknown fields are ignored; unknown `type` values are a decode error.

Malformed or invalid frames are dropped without closing the connection.
Transport-level and handshake violations may be answered with a fatal `error`
frame (see §3).

---

## 2. Handshake

The dialing peer (`initiator`) always speaks first. The acceptor never dials
back on the same connection.

```
initiator                         acceptor
    | ---- hello (plaintext) -------> |
    | <--- hello_ack (plaintext) ---- |
    | ===== sealed frames both ways ===
```

Both handshake frames travel in the clear so that they can carry the salt and
the passphrase proof. They contain display metadata (name, avatar colour,
device name, app version, capabilities). Once the proofs verify, every
subsequent frame is sealed (§4).

### 2.1 `hello`

First frame on every connection, sent by the dialing peer.

```json
{
  "v": 1,
  "id": "aa80f0b6-8a4d-4c1b-9c6e-3f8f9f94e0b1",
  "from": "4b8e6a2c-2f0e-4d6a-9e30-7c9a2f3b5d10",
  "to": "0d9e8f13-5b25-4d77-a4e3-2c7a6b9d1f44",
  "ts": 1731427200000,
  "type": "hello",
  "peer_id": "4b8e6a2c-2f0e-4d6a-9e30-7c9a2f3b5d10",
  "display_name": "Alice",
  "avatar_color": 14786674,
  "device_name": "macbook-air",
  "app_version": "0.1.0",
  "capabilities": ["files", "groups", "typing", "read_receipts", "encryption"],
  "secure": true,
  "salt_b64": "q83vJ0m1n2p4r6t8u0w2yA==",
  "proof": {
    "nonce_b64": "3J7m0pQ2sV4xZ6b8c0d2f4h6j8l0n2p4",
    "data_b64": "9r2m0kP4sT6vX8zA..."
  },
  "conn_id": "19b3b7c8-4d1e-4a62-9b8f-8c2f0d7a6e55"
}
```

| Field | Type | Required | Meaning |
| ----- | ---- | -------- | ------- |
| `peer_id` | UUID | yes | Must equal `from`. |
| `display_name` | string | yes | 64 characters max. |
| `avatar_color` | number | yes | Packed `0xRRGGBB`. |
| `device_name` | string | no | 64 characters max, default `""`. |
| `app_version` | string | no | Default `""`. |
| `capabilities` | string[] | no | Advertised features: `files`, `groups`, `typing`, `read_receipts`, `encryption`. |
| `secure` | boolean | no | `true` when this device has a LAN passphrase configured. |
| `salt_b64` | string | when secure | Base64 of the 16-byte session salt chosen by the initiator. |
| `proof` | `Sealed` | when secure | AEAD proof of passphrase knowledge bound to both peer ids (§4.1). |
| `conn_id` | UUID | no | Random per-connection id used for duplicate-connection tie-breaks. |

Validation: `peer_id == from`; names within limits; if `secure` is true then
`salt_b64` and `proof` are required.

### 2.2 `hello_ack`

Sent by the acceptor as the second frame.

```json
{
  "v": 1,
  "id": "f2c1a9d4-5e6b-4a70-8c2d-0f1e2d3c4b5a",
  "from": "0d9e8f13-5b25-4d77-a4e3-2c7a6b9d1f44",
  "to": "4b8e6a2c-2f0e-4d6a-9e30-7c9a2f3b5d10",
  "ts": 1731427200035,
  "type": "hello_ack",
  "peer_id": "0d9e8f13-5b25-4d77-a4e3-2c7a6b9d1f44",
  "display_name": "Bob",
  "avatar_color": 7122176,
  "device_name": "windows-desktop",
  "app_version": "0.1.0",
  "capabilities": ["files", "groups", "typing", "read_receipts", "encryption"],
  "secure": true,
  "proof": {
    "nonce_b64": "8a0b2c4d6e8f0a2b4c6d8e0f2a4b6c8d",
    "data_b64": "Q1w2E3r4T5y6U7i8..."
  },
  "group_version": 0,
  "conn_id": "19b3b7c8-4d1e-4a62-9b8f-8c2f0d7a6e55"
}
```

| Field | Type | Required | Meaning |
| ----- | ---- | -------- | ------- |
| `peer_id` | UUID | yes | Acceptor id; must match the id learned from discovery. |
| `display_name` | string | yes | 64 characters max. |
| `avatar_color` | number | yes | Packed `0xRRGGBB`. |
| `device_name` | string | no | Default `""`. |
| `app_version` | string | no | Default `""`. |
| `capabilities` | string[] | no | Feature list. |
| `secure` | boolean | no | Must equal the initiator's `secure` flag. |
| `proof` | `Sealed` | when secure | Acceptor's proof bound to the same ids (§4.1). |
| `group_version` | number | no | Reserved for group sync; currently `0`. |
| `conn_id` | UUID | no | Echoes the initiator's `conn_id`. |

### 2.3 Handshake failures

The receiver rejects the connection and sends a fatal `error` frame before
closing:

| `code` | Cause |
| ------ | ----- |
| `expected_hello` | First frame on an accepted socket was not `hello`. |
| `expected_hello_ack` | Dialer received anything other than `hello_ack`. |
| `identity_mismatch` | `hello.peer_id != hello.from`, or `hello_ack.peer_id` differs from the discovered peer. |
| `self_connection` | A device dialed itself. |
| `passphrase_mismatch` | One side is secure and the other is not. |
| `passphrase_required` | Acceptor has a passphrase, initiator sent `secure: false`. |
| `passphrase_not_configured` | Acceptor has no passphrase, initiator sent `secure: true`. |
| `missing_proof` | Secure handshake frame arrived without `proof`. |
| `bad_salt` | `salt_b64` is not base64 or not 16 bytes. |

An invalid proof fails locally with a crypto error; the connection is closed.

---

## 3. Common frames

### 3.1 `presence`

Sent once right after the handshake and then every 5 seconds (heartbeat) on
each established connection. Receivers also treat any inbound frame as
liveness. A connection with no traffic for 20 seconds is dropped as timed out.

```json
{
  "v": 1,
  "id": "8b5c3a1d-2e4f-4a6b-8c0d-1e2f3a4b5c6d",
  "from": "4b8e6a2c-2f0e-4d6a-9e30-7c9a2f3b5d10",
  "to": "0d9e8f13-5b25-4d77-a4e3-2c7a6b9d1f44",
  "ts": 1731427205000,
  "type": "presence",
  "online": true,
  "display_name": "Alice",
  "avatar_color": 14786674,
  "group_version": 0
}
```

| Field | Type | Required | Meaning |
| ----- | ---- | -------- | ------- |
| `online` | boolean | yes | Always `true` while connected. Offline is inferred from connection loss. |
| `display_name` | string | no | Current name, used for live rename. |
| `avatar_color` | number | no | Current colour. |
| `group_version` | number | no | Reserved, currently `0`. |

Presence is never queued for offline peers.

### 3.2 `ack`

Delivery and read receipts. Sent by the receiver of a `message`.

```json
{
  "v": 1,
  "id": "c1d2e3f4-a5b6-4c7d-8e9f-0a1b2c3d4e5f",
  "from": "0d9e8f13-5b25-4d77-a4e3-2c7a6b9d1f44",
  "to": "4b8e6a2c-2f0e-4d6a-9e30-7c9a2f3b5d10",
  "ts": 1731427200300,
  "type": "ack",
  "conv": "0d9e8f13-5b25-4d77-a4e3-2c7a6b9d1f44",
  "message_ids": ["6f1c5b8e-6a37-4f8a-9f7d-1fbfa1e6c3a2"],
  "state": "delivered"
}
```

| Field | Type | Required | Meaning |
| ----- | ---- | -------- | ------- |
| `conv` | UUID | yes | Conversation the ids belong to. |
| `message_ids` | UUID[] | yes | Message/envelope ids being acknowledged. |
| `state` | string | yes | `delivered` after local persistence; `read` when the conversation was opened and read receipts are enabled. |

Receivers may batch multiple ids in one ack. Acks are best-effort and not
queued offline.

### 3.3 `typing`

```json
{
  "v": 1,
  "id": "d4e5f6a7-b8c9-4d0e-8f1a-2b3c4d5e6f70",
  "from": "4b8e6a2c-2f0e-4d6a-9e30-7c9a2f3b5d10",
  "to": "0d9e8f13-5b25-4d77-a4e3-2c7a6b9d1f44",
  "ts": 1731427201000,
  "type": "typing",
  "conv": "0d9e8f13-5b25-4d77-a4e3-2c7a6b9d1f44",
  "active": true
}
```

| Field | Type | Required | Meaning |
| ----- | ---- | -------- | ------- |
| `conv` | UUID | yes | Direct peer id or group id. |
| `active` | boolean | yes | `true` while composing, `false` when input is cleared or sent. |

Senders throttle `active: true` to at most once per 2 seconds. Receivers
auto-clear the indicator after 5 seconds. Typing is never queued offline.

### 3.4 `error`

Reported by a peer for fatal protocol violations; the connection is closed
when `fatal` is true.

```json
{
  "v": 1,
  "id": "e5f6a7b8-c9d0-4e1f-8a2b-3c4d5e6f7081",
  "from": "0d9e8f13-5b25-4d77-a4e3-2c7a6b9d1f44",
  "to": "4b8e6a2c-2f0e-4d6a-9e30-7c9a2f3b5d10",
  "ts": 1731427200050,
  "type": "error",
  "code": "passphrase_mismatch",
  "message": "one side uses a LAN passphrase and the other does not",
  "fatal": true
}
```

| Field | Type | Required | Meaning |
| ----- | ---- | -------- | ------- |
| `code` | string | yes | Machine-readable error code (see §2.3 and §5.4). |
| `message` | string | yes | Human-readable detail. |
| `fatal` | boolean | no | Default `false`; when true the receiver closes the connection. |

---

## 4. Optional encryption

A shared LAN passphrase is configured per device. If either side has one, both
must have the same one; otherwise the handshake fails (see §2.3). Encryption is
transport-level: frame contents are protected, but the handshake metadata
(name, avatar colour, device name, app version) is visible to the LAN before
the tunnel is established.

### 4.1 Key derivation and proofs

- KDF: Argon2id, version `0x13`, memory 19 456 KiB, iterations 2, parallelism 1
- Salt: 16 random bytes generated by the **initiator** for each connection,
  sent base64 in `hello.salt_b64`
- Output: 32-byte AEAD key

Proofs use XChaCha20-Poly1305 with the derived key:

| Direction | Plaintext | AAD |
| --------- | --------- | --- |
| `hello.proof` | `lanchat-v1:hello:{initiator_id}:{responder_id}` | `lanchat/hello-v1` |
| `hello_ack.proof` | `lanchat-v1:hello_ack:{responder_id}:{initiator_id}` | `lanchat/hello-v1` |

Each proof is a `Sealed` object:

```json
{
  "nonce_b64": "base64 of 24 random bytes",
  "data_b64": "base64 of ciphertext+tag"
}
```

The proof doubles as a challenge: only a peer that can derive the key can
produce a proof that decrypts to the exact expected string, which also binds
the proof to both UUIDs (preventing replay across pairs).

### 4.2 `sealed` frames

After a successful handshake, **both directions** wrap every frame. The outer
frame is:

```json
{
  "v": 1,
  "id": "9a8b7c6d-5e4f-4a3b-9c2d-1e0f3a4b5c6d",
  "from": "4b8e6a2c-2f0e-4d6a-9e30-7c9a2f3b5d10",
  "to": "0d9e8f13-5b25-4d77-a4e3-2c7a6b9d1f44",
  "ts": 1731427201100,
  "type": "sealed",
  "nonce_b64": "base64 of 24 random bytes",
  "data_b64": "base64 of the encrypted inner envelope"
}
```

- The plaintext is the UTF-8 JSON of a **complete inner `Envelope`**, including
  its own `"v": 1` and a fresh `id`.
- Cipher: XChaCha20-Poly1305, 24-byte random nonce per frame, AAD
  `lanchat/frame-v1`.
- The outer `id`, `from`, `to`, and `ts` are not protected. After decryption
  the inner envelope's `from` must still match the authenticated connection
  peer, and `to`, when present, must be this device.
- A plaintext frame received on a secure session is dropped. A `sealed` frame
  received without a session key is dropped.

### 4.3 Passphrase changes

Changing the passphrase disconnects all peers and restarts discovery. Peers
with a different passphrase simply fail to re-handshake and remain offline.

---

## 5. Content frames

### 5.1 `message`

```json
{
  "v": 1,
  "id": "6f1c5b8e-6a37-4f8a-9f7d-1fbfa1e6c3a2",
  "from": "4b8e6a2c-2f0e-4d6a-9e30-7c9a2f3b5d10",
  "to": "0d9e8f13-5b25-4d77-a4e3-2c7a6b9d1f44",
  "ts": 1731427200123,
  "type": "message",
  "conv": "0d9e8f13-5b25-4d77-a4e3-2c7a6b9d1f44",
  "conv_kind": "direct",
  "seq": 42,
  "kind": "text",
  "text": "Hi from the kitchen",
  "file_id": null,
  "reply_to": null,
  "mentions": []
}
```

| Field | Type | Required | Meaning |
| ----- | ---- | -------- | ------- |
| `conv` | UUID | yes | Direct: the peer id. Group: the group id. |
| `conv_kind` | string | no | Default `direct`. |
| `seq` | number | no | Sender's best-effort per-conversation sequence hint. Default `0`. |
| `kind` | string | no | Default `text`. Attachments set `image`, `video`, `audio`, or `file`. `system` is local-only (never sent). |
| `text` | string \| null | conditional | Required for `kind: "text"`; optional caption for attachments; 16 000 characters max. |
| `file_id` | UUID \| null | conditional | Transfer id of the attachment (`file_offer.transfer_id`). |
| `reply_to` | UUID \| null | no | Quoted message id. |
| `mentions` | UUID[] | no | Peer ids mentioned with `@name` (group messages). |

Behaviour:

- Direct messages must have `conv == from`; otherwise they are dropped.
- The receiver persists the message before sending a `delivered` ack.
- Group messages are fanned out as separate unicast envelopes; each receiver
  acks individually.
- `kind: "text"` without `text` is invalid. Attachment messages carry
  `file_id` and may carry a caption in `text`.
- A message is considered a duplicate when its id already exists locally; the
  receiver acks again but does not insert it twice.

### 5.2 `group_create`

Creates a group on every member's device. Sent unicast to each member after a
group is created, and again to a member whenever it reconnects (so a peer that
missed the original event catches up).

```json
{
  "v": 1,
  "id": "1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d",
  "from": "4b8e6a2c-2f0e-4d6a-9e30-7c9a2f3b5d10",
  "to": "0d9e8f13-5b25-4d77-a4e3-2c7a6b9d1f44",
  "ts": 1731427210000,
  "type": "group_create",
  "group": {
    "id": "b7e6d5c4-3a2f-4e1d-9c8b-7a6f5e4d3c2b",
    "name": "Design team",
    "avatar_color": 9089254,
    "members": [
      "0d9e8f13-5b25-4d77-a4e3-2c7a6b9d1f44",
      "4b8e6a2c-2f0e-4d6a-9e30-7c9a2f3b5d10"
    ],
    "created_by": "4b8e6a2c-2f0e-4d6a-9e30-7c9a2f3b5d10",
    "created_at": 1731427210000,
    "version": 1
  }
}
```

| Field | Type | Required | Meaning |
| ----- | ---- | -------- | ------- |
| `group.id` | UUID | yes | Group/conversation id. |
| `group.name` | string | yes | 64 characters max after sanitising. |
| `group.avatar_color` | number | no | Packed `0xRRGGBB`. |
| `group.members` | UUID[] | yes | Full membership including the creator. Bounded by `MAX_GROUP_MEMBERS` (256). |
| `group.created_by` | UUID | no | Creator id. |
| `group.created_at` | number | no | Milliseconds since epoch. |
| `group.version` | number | no | Monotonic group version; minimum 1. |

`EVERYONE_GROUP_ID` is ignored if received; it is always local and implicit.

### 5.3 `group_update`

Adds/removes members, renames, or leaves. `sync` is a full-state update. The
sender includes the complete `GroupDesc` when possible so receivers can
converge without replaying history.

```json
{
  "v": 1,
  "id": "2b3c4d5e-6f70-4a8b-9c0d-1e2f3a4b5c6d",
  "from": "4b8e6a2c-2f0e-4d6a-9e30-7c9a2f3b5d10",
  "to": "0d9e8f13-5b25-4d77-a4e3-2c7a6b9d1f44",
  "ts": 1731427220000,
  "type": "group_update",
  "group_id": "b7e6d5c4-3a2f-4e1d-9c8b-7a6f5e4d3c2b",
  "op": "add_member",
  "name": "Design team",
  "avatar_color": 9089254,
  "member": "5c4b3a2f-1e0d-4c2b-8a9f-6e5d4c3b2a10",
  "group_version": 2,
  "group": {
    "id": "b7e6d5c4-3a2f-4e1d-9c8b-7a6f5e4d3c2b",
    "name": "Design team",
    "avatar_color": 9089254,
    "members": [
      "0d9e8f13-5b25-4d77-a4e3-2c7a6b9d1f44",
      "4b8e6a2c-2f0e-4d6a-9e30-7c9a2f3b5d10",
      "5c4b3a2f-1e0d-4c2b-8a9f-6e5d4c3b2a10"
    ],
    "created_by": "4b8e6a2c-2f0e-4d6a-9e30-7c9a2f3b5d10",
    "created_at": 1731427210000,
    "version": 2
  }
}
```

| Field | Type | Required | Meaning |
| ----- | ---- | -------- | ------- |
| `group_id` | UUID | yes | Group being changed. |
| `op` | string | yes | `add_member`, `remove_member`, `rename`, `leave`, `sync`. |
| `name` | string \| null | no | New name (rename, or convenience copy). |
| `avatar_color` | number \| null | no | New colour. |
| `member` | UUID \| null | conditional | Subject of add/remove/leave. |
| `group_version` | number | no | New version; receivers ignore updates older than their copy unless `op == sync`. |
| `group` | `GroupDesc` \| null | no | Full state. When present, receivers replace their copy and stop. |

Convergence rules:

- If the group is unknown locally, an update without a full `group` is ignored.
- The last writer with the highest `group_version` wins.
- A group whose membership becomes empty is deleted locally.
- The actor sends the update to every remaining member in one fan-out and then
  applies it locally. `leave` is sent by the leaving member before it drops the
  group locally.

### 5.4 `file_offer`

Announces a file transfer. The same offer is re-sent on reconnect to resume.

```json
{
  "v": 1,
  "id": "3c4d5e6f-7081-4a9b-8c1d-2e3f4a5b6c7d",
  "from": "4b8e6a2c-2f0e-4d6a-9e30-7c9a2f3b5d10",
  "to": "0d9e8f13-5b25-4d77-a4e3-2c7a6b9d1f44",
  "ts": 1731427230000,
  "type": "file_offer",
  "transfer_id": "71e6d5c4-3b2a-4190-8f7e-6d5c4b3a2f10",
  "message_id": "4d5e6f70-8192-4bac-9d2e-3f4a5b6c7d8e",
  "conv": "0d9e8f13-5b25-4d77-a4e3-2c7a6b9d1f44",
  "conv_kind": "direct",
  "name": "holiday photo.jpg",
  "mime": "image/jpeg",
  "size": 4829137,
  "chunk_size": 65536,
  "chunks": 74,
  "sha256": "9f2c1d...e4b7",
  "thumb": {
    "mime": "image/jpeg",
    "width": 480,
    "height": 320,
    "data_b64": "/9j/4AAQSkZJRg..."
  },
  "auto_accept": false,
  "caption": "From the coast"
}
```

| Field | Type | Required | Meaning |
| ----- | ---- | -------- | ------- |
| `transfer_id` | UUID | yes | Stable id for offer, chunks, and acks. |
| `message_id` | UUID | yes | Chat message that carries the attachment. |
| `conv` | UUID | yes | Direct peer id or group id. |
| `conv_kind` | string | no | Default `direct`. |
| `name` | string | yes | Sanitised file name, 255 characters max (locally sent names are capped at 180). |
| `mime` | string | no | Best-effort MIME type, default `""`. |
| `size` | number | yes | Exact file size in bytes. |
| `chunk_size` | number | yes | Payload bytes per chunk; ≤ 65 536. |
| `chunks` | number | yes | Must be positive; `ceil(size / chunk_size)` with a minimum of 1. |
| `sha256` | string | no | Lowercase hex digest of the whole file; verified by the receiver. |
| `thumb` | `Thumbnail` \| null | no | Locally generated JPEG preview for images/video. |
| `auto_accept` | boolean | no | Sender hint only; the receiver's own threshold decides. |
| `caption` | string \| null | no | Optional caption; also appears in the attachment message's `text`. |

`Thumbnail`:

| Field | Type | Meaning |
| ----- | ---- | ------- |
| `mime` | string | `image/jpeg` in this version. |
| `width` | number | Pixel width. |
| `height` | number | Pixel height. |
| `data_b64` | string | Base64 JPEG data. Senders skip thumbnails > 300 KB. |

Receiving rules:

- Offers larger than the receiver's `max_file_mb` are rejected immediately.
- Offers for a direct conversation with `conv != sender` are dropped.
- If the transfer already exists locally:
  - `done` → reply `file_ack` with `status: "done"`.
  - `downloading` / `paused` → resume from the local `next_index`.
  - `pending` → evaluate auto-accept again.
- New offers at or below the receiver's `auto_accept_files_mb` are accepted
  immediately; larger ones wait for the UI.
- Thumbnails are stored under the app data `thumbs/` directory and shown with
  the message so no download is needed to preview.

### 5.5 `file_chunk`

One sequential chunk. Chunks are sent only after the receiver accepts/resumes.

```json
{
  "v": 1,
  "id": "5e6f7081-92a3-4bcd-8e3f-4a5b6c7d8e9f",
  "from": "4b8e6a2c-2f0e-4d6a-9e30-7c9a2f3b5d10",
  "to": "0d9e8f13-5b25-4d77-a4e3-2c7a6b9d1f44",
  "ts": 1731427230150,
  "type": "file_chunk",
  "transfer_id": "71e6d5c4-3b2a-4190-8f7e-6d5c4b3a2f10",
  "index": 0,
  "data_b64": "/9j/4AAQSkZJRgABAQ..."
}
```

| Field | Type | Required | Meaning |
| ----- | ---- | -------- | ------- |
| `transfer_id` | UUID | yes | Transfer this chunk belongs to. |
| `index` | number | yes | Zero-based chunk index. |
| `data_b64` | string | yes | Base64 payload; decoded length ≤ 65 536. |

- The receiver writes a chunk only when `index == next_index`, at byte offset
  `index * chunk_size`, into `<final path>.part`. Chunks may arrive out of
  order only after a reconnect; otherwise a mismatch triggers a `resume` ack
  with the receiver's `next_index` and the sender restarts from there.
- After the last chunk, the receiver verifies `sha256` when present. On match
  it renames `.part` to the final path and replies `done`; on mismatch it
  deletes the partial file and replies `cancel`.
- A 0-byte file is sent as exactly one chunk with empty `data_b64`.
- Chunks are not queued offline. If the connection drops mid-stream, the
  sender pauses and re-offers after the peer reconnects.

### 5.6 `file_ack`

Receiver → sender status for a transfer.

```json
{
  "v": 1,
  "id": "6f708192-a3b4-4cde-9f40-5b6c7d8e9fa0",
  "from": "0d9e8f13-5b25-4d77-a4e3-2c7a6b9d1f44",
  "to": "4b8e6a2c-2f0e-4d6a-9e30-7c9a2f3b5d10",
  "ts": 1731427230200,
  "type": "file_ack",
  "transfer_id": "71e6d5c4-3b2a-4190-8f7e-6d5c4b3a2f10",
  "status": "progress",
  "next_index": 16,
  "received": 1048576
}
```

| Field | Type | Required | Meaning |
| ----- | ---- | -------- | ------- |
| `transfer_id` | UUID | yes | Transfer being acknowledged. |
| `status` | string | yes | `accept`, `reject`, `resume`, `progress`, `done`, `cancel`. |
| `next_index` | number | no | Next chunk the receiver expects (0 on initial accept). |
| `received` | number | no | Bytes persisted so far. |

Sender state machine:

| Received | Sender action |
| -------- | ------------- |
| `accept` | Start at `next_index` (normally 0). |
| `resume` | Seek to `next_index * chunk_size` and continue. |
| `progress` | Update UI only. |
| `done` | Mark transfer complete. |
| `reject` / `cancel` | Mark rejected and stop. |

The receiver emits `progress` every 16 chunks; the final chunk is followed by
`done` (or `cancel` on hash failure). The sender allows 45 seconds for an
accept/resume, retries the offer, and gives up after 10 minutes overall. After
the last byte it waits up to 20 seconds for `done`; if that ack is lost it
marks the transfer complete locally so the UI does not stall.

### 5.7 Transfer resume summary

```
offer ──> accept/resume(next_index) ──> chunk 0..n ──> done
  ^                                        |
  |           connection loss              |
  +──────── re-offer on reconnect <────────+
```

- The **receiver** is authoritative for `next_index`.
- The sender keeps the transfer row and source path; it re-offers instead of
  restarting from zero.
- If the receiver already completed the file, it answers `done` to the
  re-offer and the sender stops.
- `.part` files and transfer rows survive app restarts, so a transfer can
  resume across a restart of either side.

---

## 6. Delivery states and offline queue

| State | Set when |
| ----- | -------- |
| `queued` | Message was persisted locally but no peer connection was available. |
| `sending` | UI/internal handoff; transient. |
| `sent` | At least one recipient connection accepted the frame (or it was handed to the writer task). |
| `delivered` | A peer acked the message id. |
| `read` | A peer acked with `read` after opening the conversation. |
| `failed` | Encoding/sending failed or the transfer was rejected. Outgoing chat messages themselves are not retried beyond the outbox. |

Frames queued for offline peers:

- `message`, `group_create`, `group_update` (including direct attachments'
  `message` frames).
- The **Everyone** group is not queued per recipient; it reaches whoever is
  online at send time, and later joiners do not receive old messages.
- Presence, typing, ack, `file_offer`, `file_chunk`, and `file_ack` are never
  queued. Files resume through the transfer state machine instead.

On (re)connect the sender:

1. sends `presence`,
2. re-sends `group_create` for every non-default group shared with the peer,
3. flushes up to 200 queued frames for that peer in insertion order,
4. re-offers pending outgoing file transfers.

Queued frames are stored as encoded JSON. Messages in a group are queued per
member, so members who were offline receive them when they return.

---

## 7. Discovery

Two mechanisms run concurrently. Both are best-effort and carry the same
information.

### 7.1 mDNS / DNS-SD

- Service type: `_lanchat._tcp.local.`
- Instance name: the peer UUID
- Host name: `lanchat-<first 8 hex chars of UUID>.local.`
- SRV target port: the instance's WebSocket port
- TXT properties:

| Key | Value |
| --- | ----- |
| `peer_id` | UUID string |
| `name` | Display name (≤ 60 chars) |
| `avatar` | Packed `0xRRGGBB` as decimal |
| `device` | Device name (≤ 60 chars) |
| `proto` | `1` |
| `secure` | `1` or `0` |
| `disc_port` | UDP discovery port this instance listens on |

Service removal (`ServiceRemoved`) marks the peer offline unless a WebSocket
connection is still alive. If mDNS cannot start, discovery silently falls back
to UDP.

### 7.2 UDP broadcast beacon

- Default port: **47823** (`DEFAULT_DISCOVERY_PORT`); configurable in Settings
  (requires an app restart) or overridden at startup by
  `LANCHAT_DISCOVERY_PORT`.
- Sent every 5 seconds to `255.255.255.255:port` and to each non-loopback
  interface's broadcast address.
- If the port is already bound (second instance on the same machine), the
  socket falls back to an ephemeral port; the beacon's `disc_port` field tells
  other peers where to send unicast replies.
- Beacons with a different `disc_port` get a unicast reply so both instances
  can see each other.

Payload (JSON, UTF-8, one datagram):

```json
{
  "v": 1,
  "peer_id": "4b8e6a2c-2f0e-4d6a-9e30-7c9a2f3b5d10",
  "display_name": "Alice",
  "avatar_color": 14786674,
  "device_name": "macbook-air",
  "port": 53124,
  "disc_port": 47823,
  "secure": true,
  "sent_at": 1731427200123
}
```

| Field | Type | Required | Meaning |
| ----- | ---- | -------- | ------- |
| `v` | number | yes | Must be `1`. |
| `peer_id` | UUID | yes | Announcer id. |
| `display_name` | string | no | ≤ 64 chars. |
| `avatar_color` | number | no | Packed `0xRRGGBB`. |
| `device_name` | string | no | Device name. |
| `port` | number | yes | Must be non-zero; WebSocket port. |
| `disc_port` | number | no | UDP port the announcer listens on. |
| `secure` | boolean | no | Announcer has a passphrase. |
| `sent_at` | number | yes | Sender clock in milliseconds. |

Receivers ignore their own id, invalid versions, and beacons without a port.

### 7.3 Connection policy

- On discovery, if not already connected, dial
  `ws://<addr>:<port>/lanchat` (IPv6 in brackets).
- Dial attempts are rate-limited to one per peer per 4 seconds.
- After a connection drops, retries run at 2, 4, 8, 16 and 30 second intervals.
- Interface changes (Wi-Fi switch, cable) are detected every 8 seconds; the
  device re-announces and re-dials disconnected peers.
- **Duplicate connections**: both devices may dial each other. The connection
  where the peer with the **lower UUID** is the initiator wins. For duplicate
  connections in the same direction, the older one is kept. The loser is
  closed without affecting the winner.
- Self-connections are rejected during the handshake.

---

## 8. Limits and validation

| Limit | Value | Enforced by |
| ----- | ----- | ----------- |
| Frame size | 8 MiB | `Envelope::decode` / `encode` |
| Message text | 16 000 chars | `Envelope::validate` |
| Display/device name | 64 chars | `Envelope::validate` |
| Group name | 64 chars | sanitised before send |
| File name | 255 bytes | `Envelope::validate`; sanitised on receive |
| File chunk payload | 64 KiB decoded | `Envelope::validate`; offer `chunk_size` |
| File size | Receiver `max_file_mb` (default 2048 MB) | offer handling |
| Sealed nonce | 64 chars base64 | `Envelope::validate` |
| Sealed data | 8 MiB base64 | `Envelope::validate` |
| Group members | 256 | group sanitisation |
| Outbox flush per connect | 200 frames | reconnect logic |

Frames that violate these limits are dropped. Fatal handshake problems are
answered with `error` and a close.

---

## 9. Versioning

- `"v": 1` appears in every envelope and every discovery beacon.
- Receivers reject other versions instead of guessing.
- Backwards-compatible changes (new optional fields, new capability strings,
  new frame types that old peers can ignore) may ship within version 1.
- Breaking changes (removed fields, changed semantics, new required fields)
  require `v: 2` and a transition strategy.
- Peers advertise features in `hello.capabilities`; unknown capabilities must
  be ignored.
