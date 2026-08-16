import { ClientMethod, ServerMethod } from "./protocol";

export class EnclaveWebSocket {
  public websocket: WebSocket;
  onOpenQueue: Array<() => void>;

  public clientPublicKey: Uint8Array;
  public clientSecretKey: Uint8Array;

  public serverPublicKey?: Uint8Array;

  public constructor(url: string | URL) {
    this.clientPublicKey = new Uint8Array();
    this.clientSecretKey = new Uint8Array();
    this.onOpenQueue = new Array();

    this.websocket = new WebSocket(url);
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
