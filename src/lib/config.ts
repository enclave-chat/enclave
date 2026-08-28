import { invoke } from "@tauri-apps/api/core";

export type Config = {
  outputDeviceName: string | null;
  inputDeviceName: string | null;
  inputVolume: number;
  outputVolume: number;
};

export type BackendConfig = {
  isMuted: boolean;
  isDeaf: boolean;
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

export async function updateBackendConfig(
  config: BackendConfig,
): Promise<void> {
  await invoke("update_backend_config", { config });
}
