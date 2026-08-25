import { invoke } from "@tauri-apps/api/core";

export type Config = {
  outputDeviceName: string | null;
  inputDeviceName: string | null;
  inputVolume: number;
  outputVolume: number;
};

export async function updateConfig(config: Config): Promise<void> {
  await invoke("update_config", { config });
}

export async function saveConfig(): Promise<void> {
  await invoke("save_config");
}

export async function getConfig(): Promise<Config> {
  return await invoke("get_config");
}
