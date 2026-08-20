import { invoke } from "@tauri-apps/api/core";
import { ClientMeta } from "./types";

export interface Account {
  meta: ClientMeta;
  privateKey: string;
}

export interface AccountsFile {
  activeAccount: number;
  accounts: Account[];
}

export async function getAccounts(): Promise<AccountsFile> {
  return await invoke("get_accounts");
}

export async function saveAccounts(data: AccountsFile): Promise<void> {
  await invoke("save_accounts", { data });
}
