import { invoke } from "@tauri-apps/api/core";

export interface KnownServer {
  name: string;
  description: string;
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
  return (
    (isSecure ? "https://" : "http://") +
    (hostname.includes(":") ? hostname : hostname + ":3415") +
    path
  );
}

export function getWSUrl(hostname: string, isSecure: boolean) {
  return (
    (isSecure ? "wss://" : "ws://") +
    (hostname.includes(":") ? hostname : hostname + ":3415")
  );
}
