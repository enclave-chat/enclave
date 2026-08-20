import { getHTTPUrl, getServerList, ServerList } from "@/lib/serverList";
import EnclaveServer from "./server";
import * as ed from "@noble/ed25519";
import { sha512 } from "@noble/hashes/sha2.js";
import { base58 } from "@scure/base";

import axios from "axios";
import { Account, AccountsFile, getAccounts } from "@/lib/accounts";
import { Page } from "@/components/page/PageView";

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
export default class Enclave<P = Page> {
  public accounts?: AccountsFile;
  public server?: EnclaveServer;
  public serverList: ServerList;
  public forceRender: () => void;
  public page?: P;

  public constructor() {
    this.serverList = {};
    this.forceRender = () => {};
  }

  public async init() {
    this.serverList = await getServerList();
    this.accounts = await getAccounts();
  }

  public getAccount(): Account | null {
    return this.accounts?.accounts[this.accounts.activeAccount] || null;
  }

  public getClientSecretKey() {
    if (!this.accounts) return null;

    const account = this.getAccount();

    return account ? base58.decode(account.privateKey) : null;
  }

  public sendMessage(content: string, channelId: string) {
    const clientSecretKey = this.getClientSecretKey();

    if (!clientSecretKey) {
      console.error("Acccount not found");
      return;
    }

    const timestamp = Date.now();

    if (!this.server?.serverPublicKey) {
      console.error("Server pubkey not initialized");
      return;
    }

    const signature = new TextEncoder().encode(
      `${timestamp}@${base58.encode(this.server?.serverPublicKey)}@${content}`,
    );

    this.server?.websocket?.send({
      method: "SendMessage",
      channel_id: channelId,
      data: {
        content,
        timestamp,
        signature: base58.encode(ed.sign(signature, clientSecretKey)),
      },
    });
  }

  public async connectToServer(hostname: string, isSecure: boolean) {
    if (this.server) {
      this.server.disconnect();
    }

    this.server = new EnclaveServer(hostname, isSecure);

    const clientSecretKey = this.getClientSecretKey();

    if (!clientSecretKey) {
      console.error("Failed to get client key");
      return;
    }

    const clientPublicKey = ed.getPublicKey(clientSecretKey);

    await this.server.connect(clientPublicKey, clientSecretKey);

    if (!this.server.serverPublicKey) {
      console.error("Failed to get public key");
      return;
    }

    if (
      this.serverList[hostname] &&
      this.serverList[hostname].publicKey !==
        base58.encode(this.server.serverPublicKey)
    ) {
      console.error("Server's new public key doesn't match old one", {
        old: this.serverList[hostname].publicKey,
        new: base58.encode(this.server.serverPublicKey),
      });
      return;
    }

    const account = this.getAccount();

    if (!account) {
      console.error("Failed to get account");
      return;
    }

    this.server.websocket?.send({ method: "Meta", ...account.meta });

    const metaResponse = await axios.get(
      getHTTPUrl(hostname, isSecure, "/meta"),
    );

    this.server.meta = metaResponse.data as any;

    this.serverList[hostname] = {
      meta: metaResponse.data as any,
      isSecure,
      publicKey: base58.encode(this.server.serverPublicKey),
    };

    this.forceRender();
  }
}
