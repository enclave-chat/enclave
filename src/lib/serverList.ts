import { invoke } from "@tauri-apps/api/core";

export interface KnownServer {
  name: string;
  description: string;
  publicKey: string;
  hostname: string;
  isSecure: boolean;
}

export async function saveServerList(servers: KnownServer[]): Promise<void> {
  await invoke("save_server_list", { servers });
}

export async function getServerList(): Promise<KnownServer[]> {
  return await invoke("get_server_list");
}

export function getHTTPUrl(
  server: { hostname: string; isSecure: boolean },
  path: string,
) {
  const hostname = server.hostname.includes(":")
    ? server.hostname
    : server.hostname + ":3415";

  return (server.isSecure ? "https://" : "http://") + hostname + path;
}

export function getWSUrl(server: { hostname: string; isSecure: boolean }) {
  const hostname = server.hostname.includes(":")
    ? server.hostname
    : server.hostname + ":3415";

  return (server.isSecure ? "wss://" : "ws://") + hostname;
}
