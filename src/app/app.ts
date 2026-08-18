import {
  getServerList,
  getWSUrl,
  KnownServer,
  ServerList,
} from "@/lib/serverList";
import EnclaveServer from "./server";
import * as ed from "@noble/ed25519";
import { sha512 } from "@noble/hashes/sha2.js";
import { base58 } from "@scure/base";

ed.hashes.sha512 = sha512;

/**
 * Top-level application state and client-side logic.
 *
 * Owns all active connections — currently `EnclaveServer` instances, one
 * per connected server, keyed by server id — and will later also own
 * `DMServer` connections once direct messages are implemented.
 *
 * This is where UI-facing logic lives: handling user actions (button
 * clicks, switching the active server, sending a message from an input
 * box), and exposing state for the UI layer to render. The UI itself
 * should stay a pure display of what `Enclave` exposes — it should not
 * reach into `EnclaveServer` or `EnclaveWebSocket` directly.
 */
export default class Enclave {
  private clientSecretKey: Uint8Array;
  private clientPublicKey: Uint8Array;
  public server?: EnclaveServer;
  public serverList: ServerList;

  public constructor() {
    this.clientSecretKey = ed.utils.randomSecretKey();
    this.clientPublicKey = ed.getPublicKey(this.clientSecretKey);
    this.serverList = {};
  }

  public async init() {
    this.serverList = await getServerList();
  }

  public async connectToServer(hostname: string, isSecure: boolean) {
    if (this.server) {
      this.server.disconnect();
    }

    this.server = new EnclaveServer(getWSUrl(hostname, isSecure));

    if (!this.server.serverPublicKey) {
      console.error("Failed to get public key");
      return;
    }

    this.serverList[hostname] = {
      name: "",
      description: "",
      isSecure,
      publicKey: base58.encode(this.server.serverPublicKey),
    };
  }
}
