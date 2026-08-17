import EnclaveWebSocket from "./ws";
/**
 * Manages a single connection to an Enclave server: channels, server
 * metadata, and messages for that server.
 *
 * Wraps one `EnclaveWebSocket` instance and layers server-level concepts
 * on top of the raw protocol (channels, messages, server meta) — it does
 * not know about the handshake/key exchange itself, only the connection
 * it's given.
 *
 * One `EnclaveServer` instance exists per connected server. It has no
 * knowledge of other servers, DMs, or app-level UI state — that lives in
 * `Enclave`, which owns and manages multiple `EnclaveServer` instances.
 */
export default class EnclaveServer {
  public websocket: EnclaveWebSocket;

  public constructor(
    hostname: string,
    clientSecretKey: Uint8Array,
    clientPublicKey: Uint8Array,
  ) {
    this.websocket = new EnclaveWebSocket(hostname);
    this.websocket.clientSecretKey = clientSecretKey;
    this.websocket.clientPublicKey = clientPublicKey;
    this.websocket.init();
  }
}
