import { ClientMethod, ServerMethod } from "./protocol";

/**
 * A protocol-aware wrapper around the browser `WebSocket`.
 *
 * Exposes typed protocol methods (`ServerMethod` / `ClientMethod`) rather
 * than raw WebSocket messages. Has no concept of channels, messages, or
 * app state — `EnclaveServer` builds on top of this to add those.
 */
export default class EnclaveWebSocket {
  public websocket: WebSocket;
  onOpenQueue: Array<() => void>;

  public constructor(hostname: string) {
    this.onOpenQueue = new Array();

    this.websocket = new WebSocket(hostname);
    this.websocket.onopen = () => {
      this.onOpenQueue.forEach((fun) => fun());
    };
  }

  public async send(method: ServerMethod) {
    if (this.websocket.readyState !== WebSocket.OPEN) {
      return new Promise<void>((ok) => {
        this.onOpenQueue.push(() => {
          this.websocket.send(JSON.stringify(method));
          ok();
        });
      });
    }

    this.websocket.send(JSON.stringify(method));
  }

  public async read(): Promise<ClientMethod> {
    return new Promise((ok) => {
      this.websocket.onmessage = (msg) => {
        const data = JSON.parse(msg.data);
        ok(data);
        this.websocket.onmessage = null;
      };
    });
  }
}
