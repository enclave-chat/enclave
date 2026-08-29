import { getHTTPUrl, getServerList, ServerList } from "@/lib/serverList";
import EnclaveServer from "./server";
import * as ed from "@noble/ed25519";
import { sha512 } from "@noble/hashes/sha2.js";
import { base58 } from "@scure/base";

import axios from "axios";
import { Account, AccountsFile, getAccounts } from "@/lib/accounts";
import { Page } from "@/components/page/PageView";
import { Channel } from "@/lib/types";
import { invoke } from "@tauri-apps/api/core";
import { ClientMethod } from "./protocol";
import { BackendConfig, Config, getConfig } from "@/lib/config";
import * as sfx from "@/lib/soundEffects";

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
  public publicKey?: string;
  public server?: EnclaveServer;
  public serverList: ServerList;
  public isSettingsOpen: boolean;
  public forceRender: () => void;
  public page?: P;
  public sidebarPageKind: "server" | "main";
  public config?: Config;
  public backendConfig: BackendConfig;

  public constructor() {
    this.serverList = [];
    this.forceRender = () => {};
    this.isSettingsOpen = false;
    this.backendConfig = {
      isMuted: false,
      isDeaf: false,
    };
    this.sidebarPageKind = "main";
  }

  public async init() {
    this.serverList = await getServerList();
    this.accounts = await getAccounts();
    this.config = await getConfig();
  }

  public getAccount(): Account | null {
    const account = this.accounts?.accounts[this.accounts.activeAccount];

    if (account) {
      this.publicKey = base58.encode(
        ed.getPublicKey(base58.decode(account.privateKey)),
      );
    }

    return account || null;
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
    this.sidebarPageKind = "server";

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

    const serveridx = this.serverList.findIndex(
      (server) => server.hostname === hostname,
    );

    if (
      serveridx >= 0 &&
      this.serverList[serveridx].publicKey !==
        base58.encode(this.server.serverPublicKey)
    ) {
      console.error("Server's new public key doesn't match old one", {
        old: this.serverList[serveridx].publicKey,
        new: base58.encode(this.server.serverPublicKey),
      });
      return;
    }

    const account = this.getAccount();

    if (!account) {
      console.error("Failed to get account");
      return;
    }

    if (this.server.websocket) {
      this.server.websocket.send({ method: "Meta", ...account.meta });
      this.server.websocket.websocket.onmessage = async (msg) => {
        let buffer: ArrayBuffer;
        if (msg.data instanceof Blob) {
          buffer = await msg.data.arrayBuffer();
        } else {
          buffer = msg.data as ArrayBuffer;
        }

        const encrypted = new Uint8Array(buffer);
        const plaintext = this.server?.websocket?.decrypt(encrypted);
        const data = JSON.parse(new TextDecoder().decode(plaintext));

        this.onMessage(data);
      };
    }

    const metaResponse = await axios.get(
      getHTTPUrl(hostname, isSecure, "/meta"),
    );

    this.server.meta = metaResponse.data as any;

    const serverItem = {
      hostname,
      publicKey: base58.encode(this.server.serverPublicKey),
      isSecure,
      meta: metaResponse.data as any,
    };

    if (serveridx >= 0) {
      this.serverList[serveridx] = serverItem;
    } else {
      this.serverList.push(serverItem);
    }

    this.forceRender();
  }

  public joinVoice(channel: Channel) {
    const server = this.server;

    if (!server || channel.kind !== "voice") return;

    if (server.voiceChannelId) this.leaveVoice();

    server.voiceJoin = (pin, channelId) => {
      invoke("connect_to_vc", {
        hostname: server.hostname,
        pin,
        sharedSecret: server.websocket?.sharedSecret,
      })
        .then(() => {
          server.voiceChannelId = channelId;
          this.forceRender();
        })
        .catch(console.error);
    };

    server.voiceChannelId = channel.id;

    server.websocket?.send({ method: "JoinVoice", channel_id: channel.id });

    this.forceRender();
  }

  public leaveVoice() {
    const server = this.server;

    if (!server || !server.voiceChannelId) return;

    server.websocket?.send({ method: "LeaveVoice" });
    invoke("disconnect_from_vc").catch(console.error);

    server.voiceChannelId = undefined;
    server.voiceChatSpeakers = {};

    this.forceRender();

    sfx.playLeave();
  }

  public async onMessage(msg: ClientMethod) {
    const server = this.server;

    if (!server) {
      console.error("Unable to get server for message, MSG:", msg);
      return;
    }

    switch (msg.method) {
      case "Initialized":
        console.error("Already initialized");
        return;

      case "Error":
        console.error("Server error:", msg.error);
        return;

      case "Messages":
        const currentTimestamp = Date.now();
        let shouldPlaySfx = false;

        const authors = Object.entries(msg.messages).flatMap(
          ([channelId, messages]) => {
            if (!server.messages[channelId]) {
              server.messages[channelId] = {};
            }

            const page = this.page as Page | undefined;

            const isInvisibleChannel =
              page?.kind === "channel" && page?.channel.id !== channelId;

            return messages.map((message) => {
              if (
                Math.abs(currentTimestamp - message.timestamp) < 1000 &&
                (isInvisibleChannel || document.visibilityState === "hidden") &&
                message.author !== this.publicKey
              ) {
                shouldPlaySfx = true;
              }

              server.messages[channelId][message.id] = message;
              return message.author;
            });
          },
        );

        const dedupedAuthors = Array.from(new Set(authors));

        this.server?.getUsers(dedupedAuthors);

        this.forceRender();

        if (shouldPlaySfx) sfx.playMessage();

        return;

      case "Users":
        Object.entries(msg.users).forEach(([pubKey, user]) => {
          server.users[pubKey] = user;
        });

        this.forceRender();

        return;

      case "MessageDeleted":
        delete server.messages[msg.channel_id][msg.message_id];
        this.forceRender();
        return;

      case "MessageEdited":
        server.messages[msg.channel_id][msg.message.id] = msg.message;
        this.forceRender();
        return;

      case "JoinVoice":
        server.voiceJoin && server.voiceJoin(msg.pin, msg.channel_id);
        return;

      case "UserJoinedVoice":
        if (!server.voiceChatUsers[msg.channel_id])
          server.voiceChatUsers[msg.channel_id] = [];

        server.voiceChatUsers[msg.channel_id].push(msg.pubkey);
        this.server?.getUsers([msg.pubkey]);

        if (msg.channel_id === this.server?.voiceChannelId) {
          sfx.playJoin();
        }

        return;

      case "UserLeftVoice":
        if (!server.voiceChatUsers[msg.channel_id]) return;

        server.voiceChatUsers[msg.channel_id] = server.voiceChatUsers[
          msg.channel_id
        ].filter((v) => v !== msg.pubkey);

        this.forceRender();

        if (msg.channel_id === this.server?.voiceChannelId) {
          sfx.playLeave();
        }

        return;

      case "Speaking":
        clearTimeout(server.voiceChatSpeakers[msg.pubkey]);

        server.voiceChatSpeakers[msg.pubkey] = setTimeout(() => {
          delete server.voiceChatSpeakers[msg.pubkey];
          this.forceRender();
        }, 800);
        this.forceRender();
        return;
    }
  }
}
