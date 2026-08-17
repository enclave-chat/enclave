import EnclaveServer from "./server";
import * as ed from "@noble/ed25519";

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
  public server: EnclaveServer;

  public constructor() {
    this.clientSecretKey = ed.utils.randomSecretKey();
    this.clientPublicKey = ed.getPublicKey(this.clientSecretKey);

    this.server = new EnclaveServer("localhost:3415");

    this.server.connect(this.clientPublicKey, this.clientSecretKey);
  }
}
