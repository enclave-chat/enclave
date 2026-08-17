import { ClientMethod, ServerMethod } from "./protocol";
import { base58 } from "@scure/base";
import * as ed from "@noble/ed25519";
import { sha512 } from "@noble/hashes/sha2.js";

ed.hashes.sha512 = sha512;

export class EnclaveWebSocket {
  public websocket: WebSocket;
  public hostname: string;
  onOpenQueue: Array<() => void>;

  public clientPublicKey: Uint8Array;
  public clientSecretKey: Uint8Array;

  public serverPublicKey?: Uint8Array;

  public constructor(hostname: string) {
    this.clientPublicKey = new Uint8Array();
    this.clientSecretKey = new Uint8Array();
    this.onOpenQueue = new Array();

    this.hostname = hostname;
    this.websocket = new WebSocket("ws://" + hostname);
    this.websocket.onopen = () => {
      this.onOpenQueue.forEach((fun) => fun());
    };
  }

  public async init() {
    const publicKeyString = base58.encode(this.clientPublicKey);

    const timestamp = Date.now();

    const msg = new TextEncoder().encode(`${timestamp}@${this.hostname}`);

    this.send({
      method: "Initialize",
      public_key: publicKeyString,
      signature: base58.encode(ed.sign(msg, this.clientSecretKey)),

      timestamp,
      hostname: this.hostname,
    });

    const initialized = await this.read();

    if (initialized.method !== "Initialized") {
      this.websocket.close();
      throw Error("Invalid method from server, closing. ");
    }

    if (initialized.hostname !== this.hostname) {
      this.websocket.close();
      throw Error("Server is trying to be a middle man");
    }

    const sigMsg = new TextEncoder().encode(
      `${initialized.timestamp}@${this.hostname}@${publicKeyString}`,
    );

    this.serverPublicKey = base58.decode(initialized.public_key);
    const signature = base58.decode(initialized.signature);

    if (!ed.verify(signature, sigMsg, this.serverPublicKey)) {
      this.websocket.close();
      throw Error("Invalid signature");
    }
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
