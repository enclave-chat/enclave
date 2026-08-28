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
  private sharedSecret: Uint8Array | null = null;

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

    this.websocket.onmessage = (msg) => this.handleHandshakeMessage(msg);
  }

  private handleHandshakeMessage(msg: MessageEvent) {
    const serverPubkey = new Uint8Array(msg.data as ArrayBuffer);

    this.sharedSecret = x25519.getSharedSecret(
      this.myX25519PrivateKey,
      serverPubkey,
    );

    const myPublicKey = x25519.getPublicKey(this.myX25519PrivateKey);
    this.websocket.send(myPublicKey);

    this.websocket.onmessage = null;
    this.resolveHandshake();
  }

  private nextSendNonce(): Uint8Array {
    const nonce = new Uint8Array(12);
    const view = new DataView(nonce.buffer);
    view.setBigUint64(0, this.sendCounter, false); // Big-endian 64-bit counter
    nonce[11] |= 0b1000_0000; // Client-to-server direction flag bit
    this.sendCounter += 1n;
    return nonce;
  }

  public encrypt(plaintext: Uint8Array): Uint8Array {
    if (!this.sharedSecret) throw new Error("Handshake not complete");

    const nonce = this.nextSendNonce();
    const cipher = chacha20poly1305(this.sharedSecret, nonce);
    const ciphertext = cipher.encrypt(plaintext);

    // Prepend nonce (12 bytes) + ciphertext
    const out = new Uint8Array(nonce.length + ciphertext.length);
    out.set(nonce, 0);
    out.set(ciphertext, nonce.length);
    return out;
  }

  public decrypt(data: Uint8Array): Uint8Array {
    if (!this.sharedSecret) throw new Error("Handshake not complete");
    if (data.length < 12) {
      throw new Error("Message too short to contain a nonce");
    }

    const nonce = new Uint8Array(data.subarray(0, 12));
    const ciphertext = new Uint8Array(data.subarray(12));

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
      this.websocket.onmessage = async (msg) => {
        let buffer: ArrayBuffer;
        if (msg.data instanceof Blob) {
          buffer = await msg.data.arrayBuffer();
        } else {
          buffer = msg.data as ArrayBuffer;
        }

        const encrypted = new Uint8Array(buffer);
        const plaintext = this.decrypt(encrypted);
        const data = JSON.parse(new TextDecoder().decode(plaintext));
        ok(data);
        this.websocket.onmessage = null;
      };
    });
  }
}
