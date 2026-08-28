import { invoke } from "@tauri-apps/api/core";
import { x25519 } from "@noble/curves/ed25519.js";
import { chacha20poly1305 } from "@noble/ciphers/chacha.js";
import { ClientMethod, ServerMethod } from "./protocol";

/**
 * A protocol-aware wrapper around the browser `WebSocket`.
 *
 * Handles the x25519 key exchange handshake and ChaCha20-Poly1305
 * encryption/decryption of every message after it. Exposes typed protocol
 * methods (`ServerMethod` / `ClientMethod`) rather than raw WebSocket
 * messages. Has no concept of channels, messages, or app state —
 * `EnclaveServer` builds on top of this to add those.
 */
export default class EnclaveWebSocket {
  public websocket: WebSocket;
  onOpenQueue: Array<() => void>;

  private sendCounter = 0n;
  private recvCounter = 0n;
  public sharedSecret: Uint8Array | null = null;

  private readonly handshakeReady: Promise<void>;
  private resolveHandshake!: () => void;

  public constructor(
    hostname: string,
    private readonly myX25519PrivateKey: Uint8Array,
  ) {
    this.onOpenQueue = new Array();

    this.handshakeReady = new Promise((resolve) => {
      this.resolveHandshake = resolve;
    });

    this.websocket = new WebSocket(hostname);
    this.websocket.binaryType = "arraybuffer";

    this.websocket.onclose = () => {
      invoke("disconnect_from_vc");
    };

    this.websocket.onopen = () => {
      this.onOpenQueue.forEach((fun) => fun());
    };

    // First binary message is the server's raw x25519 pubkey — this
    // one-time listener handles the handshake, then hands off to
    // the normal encrypted read loop.
    this.websocket.onmessage = (msg) => this.handleHandshakeMessage(msg);
  }

  private handleHandshakeMessage(msg: MessageEvent) {
    const serverPubkey = new Uint8Array(msg.data as ArrayBuffer);

    this.sharedSecret = x25519.getSharedSecret(
      this.myX25519PrivateKey,
      serverPubkey,
    );

    // Respond with our own x25519 pubkey, in plaintext — this is the
    // one message on either side that can't be encrypted yet, since
    // the shared secret doesn't exist until both pubkeys are known.
    const myPublicKey = x25519.getPublicKey(this.myX25519PrivateKey);
    this.websocket.send(myPublicKey);

    // From here on, every message is encrypted.
    this.websocket.onmessage = null;
    this.resolveHandshake();
  }

  private nextSendNonce(): Uint8Array {
    const nonce = new Uint8Array(12);
    const view = new DataView(nonce.buffer);
    view.setBigUint64(0, this.sendCounter, false); // big-endian, first 8 bytes
    nonce[11] |= 0b1000_0000; // distinguishes client-send from server-send direction
    this.sendCounter += 1n;
    return nonce;
  }

  private nextRecvNonce(): Uint8Array {
    const nonce = new Uint8Array(12);
    const view = new DataView(nonce.buffer);
    view.setBigUint64(0, this.recvCounter, false);
    this.recvCounter += 1n;
    return nonce;
  }

  private encrypt(plaintext: Uint8Array): Uint8Array {
    if (!this.sharedSecret) throw new Error("Handshake not complete");

    const nonce = this.nextSendNonce();
    const cipher = chacha20poly1305(this.sharedSecret, nonce);
    const ciphertext = cipher.encrypt(plaintext);

    const out = new Uint8Array(nonce.length + ciphertext.length);
    out.set(nonce, 0);
    out.set(ciphertext, nonce.length);
    return out;
  }

  public decrypt(data: Uint8Array): Uint8Array {
    if (!this.sharedSecret) throw new Error("Handshake not complete");
    if (data.length < 12)
      throw new Error("Message too short to contain a nonce");

    const nonce = data.slice(0, 12);
    const ciphertext = data.slice(12);

    const cipher = chacha20poly1305(this.sharedSecret, nonce);
    return cipher.decrypt(ciphertext);
  }

  public async send(method: ServerMethod) {
    await this.handshakeReady;

    const send = () => {
      const plaintext = new TextEncoder().encode(JSON.stringify(method));
      const encrypted = this.encrypt(plaintext);
      this.websocket.send(encrypted);
    };

    if (this.websocket.readyState !== WebSocket.OPEN) {
      return new Promise<void>((ok) => {
        this.onOpenQueue.push(() => {
          send();
          ok();
        });
      });
    }

    send();
  }

  public async read(): Promise<ClientMethod> {
    await this.handshakeReady;

    return new Promise((ok) => {
      this.websocket.onmessage = (msg) => {
        const encrypted = new Uint8Array(msg.data as ArrayBuffer);
        const plaintext = this.decrypt(encrypted);
        const data = JSON.parse(new TextDecoder().decode(plaintext));
        ok(data);
        this.websocket.onmessage = null;
      };
    });
  }
}
