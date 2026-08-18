import { invoke } from "@tauri-apps/api/core";

type KnownServer = {
  name: string;
  description: string;
  publicKey: string;
  hostname: string;
  isWss: boolean;
};

export async function saveServerList(servers: KnownServer[]): Promise<void> {
  await invoke("save_server_list", { servers });
}

export async function getServerList(): Promise<KnownServer[]> {
  return await invoke("get_server_list");
}
