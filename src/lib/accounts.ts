import { invoke } from "@tauri-apps/api/core";

export interface Account {
  displayName: string;
  privateKey: string;
}

export interface AccountsFile {
  activeAccount: number | null;
  accounts: Account[];
}

export async function getAccounts(): Promise<AccountsFile> {
  return await invoke("get_accounts");
}

export async function saveAccounts(data: AccountsFile): Promise<void> {
  await invoke("save_accounts", { data });
}
