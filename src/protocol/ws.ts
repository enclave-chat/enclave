import { ClientMethod, ServerMethod } from "./protocol";

export class EnclaveWebSocket {
  public websocket: WebSocket;
  onmessage: ((this: WebSocket, ev: MessageEvent) => any) | null;

  public clientPublicKey: Uint8Array;
  public clientSecretKey: Uint8Array;

  public serverPublicKey: Uint8Array;

  public constructor(url: string | URL) {
    this.websocket = new WebSocket(url);
    this.clientPublicKey = new Uint8Array();
    this.clientSecretKey = new Uint8Array();

    this.serverPublicKey = new Uint8Array();

    this.onmessage = null;
  }

  public send(method: ServerMethod) {
    this.websocket.send(JSON.stringify(method));
  }

  public async read(): Promise<ClientMethod> {
    return new Promise((ok) => {
      this.onmessage = (msg) => {
        const data = JSON.parse(msg.data);
        ok(data);
        this.onmessage = null;
      };
    });
  }
}
