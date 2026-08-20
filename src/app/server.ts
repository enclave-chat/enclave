import { base58 } from "@scure/base";
import * as ed from "@noble/ed25519";
import EnclaveWebSocket from "./ws";
import { sha512 } from "@noble/hashes/sha2.js";
import { getWSUrl } from "@/lib/serverList";
import { ServerMeta, StoredMessage } from "@/lib/types";

ed.hashes.sha512 = sha512;

/**
 * Represents a known Enclave server, whether connected or not.
 *
 * Most `EnclaveServer` instances exist purely as metadata — name,
 * description, icon, and the server's public key — fetched from the
 * server's HTTP endpoints, with no live connection. A server only
 * connects when the user opens it (e.g. clicking its icon in the UI).
 *
 * On connect, `EnclaveServer` performs the key exchange and identity
 * handshake itself, storing the resulting `serverPublicKey`, and creates
 * the underlying `EnclaveWebSocket` for the live protocol connection.
 * Once connected, it also manages channels and messages for that server.
 *
 * `Enclave` owns and manages multiple `EnclaveServer` instances — one
 * per known server, connected or not.
 */
export default class EnclaveServer {
  public serverPublicKey?: Uint8Array;
  public hostname: string;
  public isSecure: boolean;
  public websocket?: EnclaveWebSocket;
  public meta?: ServerMeta;
  public messages: Record<string, Set<StoredMessage>>;

  public constructor(hostname: string, isSecure: boolean) {
    this.hostname = hostname;
    this.isSecure = isSecure;
    this.messages = {};
  }

  public disconnect() {
    this.websocket?.websocket.close();
    this.websocket = undefined;
  }

  public async connect(
    clientPublicKey: Uint8Array,
    clientSecretKey: Uint8Array,
  ) {
    this.websocket = new EnclaveWebSocket(
      getWSUrl(this.hostname, this.isSecure),
    );

    const publicKeyString = base58.encode(clientPublicKey);

    const timestamp = Date.now();

    const msg = new TextEncoder().encode(`${timestamp}@${this.hostname}`);

    this.websocket.send({
      method: "Initialize",
      public_key: publicKeyString,
      signature: base58.encode(ed.sign(msg, clientSecretKey)),

      timestamp,
      hostname: this.hostname,
    });

    const initialized = await this.websocket.read();

    if (initialized.method !== "Initialized") {
      this.disconnect();
      throw Error("Invalid method from server, closing. ");
    }

    if (initialized.hostname !== this.hostname) {
      this.disconnect();
      throw Error("Server is trying to be a middle man");
    }

    if (Math.abs(initialized.timestamp - timestamp) > 2000) {
      this.disconnect();
      throw Error("Server timestamp is desynced");
    }

    const sigMsg = new TextEncoder().encode(
      `${initialized.timestamp}@${this.hostname}@${publicKeyString}`,
    );

    this.serverPublicKey = base58.decode(initialized.public_key);
    const signature = base58.decode(initialized.signature);

    if (!ed.verify(signature, sigMsg, this.serverPublicKey)) {
      this.websocket.websocket.close();
      throw Error("Invalid signature");
    }
  }
}
