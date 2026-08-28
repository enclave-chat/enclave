import { invoke } from "@tauri-apps/api/core";
import { ServerMeta } from "./types";

export interface KnownServer {
  hostname: string;
  publicKey: string;
  isSecure: boolean;
  meta: ServerMeta;
}

export type ServerList = KnownServer[];

export async function saveServerList(servers: ServerList): Promise<void> {
  await invoke("save_server_list", { servers });
}

export async function getServerList(): Promise<ServerList> {
  return await invoke("get_server_list");
}

export function getHTTPUrl(hostname: string, isSecure: boolean, path: string) {
  return (isSecure ? "https://" : "http://") + hostname + path;
}

export function getWSUrl(hostname: string, isSecure: boolean) {
  return (isSecure ? "wss://" : "ws://") + hostname;
}
