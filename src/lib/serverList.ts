import { ServerMeta } from "@/app/protocol";
import { invoke } from "@tauri-apps/api/core";

export interface KnownServer {
  meta: ServerMeta;
  publicKey: string;
  isSecure: boolean;
}

export type ServerList = Record<string, KnownServer>;

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
