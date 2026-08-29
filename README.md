# Enclave

Privacy-focused messenger and voice client built by [ORUS](https://github.com/orus-dev).

Enclave is a desktop app for private, self-hosted community chat and voice. It
connects to servers that you or your community control — there is no central
company operating the network, and no third party that can reset your identity
or read your messages. Your identity is a cryptographic keypair generated
locally, and everything you send is signed so it can be verified to genuinely
come from you.

Select a server from the left rail and join a channel to start chatting.

## About this project

Enclave is one of several privacy-focused, self-hosted applications built by
ORUS using encrypted, decentralized architecture.

- **Self-hosted servers** — Enclave connects to servers you or your community
  control. There is no central company operating the network; your messages
  live where you choose. Servers are [trust-on-first-use pinned](#keypins) so
  their identity can't quietly change.
- **Keypair identity** — Your identity is an [Ed25519](https://github.com/paulmillr/noble-ed25519)
  keypair generated locally on first launch. There is no email or password, so
  no third party can reset your account or impersonate you.
- **Signed, verifiable messages** — Every message carries an Ed25519 signature
  over its content and timestamp. Anything you receive can be independently
  verified to come from its author.
- **Encrypted realtime transport** — Client and server exchange a shared
  secret over an encrypted WebSocket, and voice traffic is encrypted in transit.

## How Enclave works

Enclave is a [Tauri 2](https://v2.tauri.app/) desktop application with a React
front end and a Rust backend.

| Layer         | Tech                                                                                                                                   |
| ------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| UI            | React 19, TypeScript, Tailwind CSS 4, shadcn/ui, Base UI                                                                               |
| App shell     | Tauri 2                                                                                                                                |
| Cryptography  | [noble](https://github.com/paulmillr/noble-ed25519) (Ed25519, hashes, ciphers), [@scure/base](https://github.com/paulmillr/scure-base) |
| Realtime      | WebSocket via `EnclaveServer` / `EnclaveWebSocket`                                                                                     |
| Audio & voice | Rust with `cpal`, `ringbuf`, `rubato`, `chacha20poly1305`                                                                              |

### Architecture

The front end is a thin display of state owned by the `Enclave` class
(`src/app/app.ts`). It owns every active connection — one `EnclaveServer` per
connected server, keyed by server id — and is where all UI-facing logic lives
(connecting to a server, sending a message, joining voice). The UI never talks
directly to a server's WebSocket; it only reads and mutates what `Enclave`
exposes.

- `src/app/` — top-level application state, server connection logic, and the
  protocol between client and server.
- `src/lib/` — persistence (`accounts`, `serverList`, `config`) and shared
  types.
- `src/components/` — UI: server list, sidebar, channels, settings, and dialogs.
- `src-tauri/` — the Rust backend: window setup, storage, and audio/voice
  handling.

### A privacy-focused protocol

Enclave's protocol (`src/app/protocol.ts`) is built around public-key
cryptography rather than accounts:

- **Local keypairs** — On first launch you generate an Ed25519 keypair. Your
  public key is your handle; nothing is stored with a company, and there is no
  password to leak or reset.
- **Trust on first use** — When you connect to a server, its public key is
  pinned. If that server later presents a different key, Enclave refuses to
  connect, so you always know you're talking to the server you chose.
- **Signed messages** — Message contents are signed with your key before being
  sent (`src/app/app.ts`), and the server stores and relays the signatures so
  recipients can verify origin and integrity.
- **End-to-end confidentiality** — Traffic flows over encrypted channels and,
  when TLS is enabled on the server, an `https://` transport, so data in
  transit stays protected.

## Privacy & security

- **Own your data** — your identity is a keypair you hold. No central service
  stores your messages or your credentials.
- **Verify before you trust** — signatures and keypins mean you can confirm
  every message is authentic and every server is the one you picked.
- **Minimal attack surface** — built on Tauri's small native shell and
  dependency-light, audited noble cryptography primitives.
- **Self-hosting** — running your own server means you decide where data is
  stored and who can access it.

Your private key never leaves your device. Guard it like you would any secret —
back it up and keep it safe, and it is the only thing that proves who you are.

## Contributing

### Prerequisites

- [Node.js](https://nodejs.org/) + npm
- [Rust](https://www.rust-lang.org/) toolchain
- Tauri's platform prerequisites (see the
  [Tauri docs](https://v2.tauri.app/start/prerequisites/))

### Development

```bash
npm install
npm run tauri dev
```

### Build

```bash
npm run tauri build
```

### Scripts

| Command               | Description                        |
| --------------------- | ---------------------------------- |
| `npm run dev`         | Run the Vite dev server            |
| `npm run typecheck`   | Type-check the TypeScript codebase |
| `npm run build`       | Type-check and build the front end |
| `npm run tauri dev`   | Run the desktop app in development |
| `npm run tauri build` | Build a release bundle             |

## Repo layout

```
src/
  app/            # Enclave state, server logic, client/server protocol
  components/     # UI: server list, sidebar, channels, settings
  lib/            # persistence helpers and shared types
src-tauri/        # Rust backend (window, storage, audio/voice)
```
