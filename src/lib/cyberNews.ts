import { invoke } from "@tauri-apps/api/core";

export type NewsItem = {
  title: string;
  link: string;
  source: string;
  publishedAt: number;
};

export async function getCyberNews(): Promise<NewsItem[]> {
  return await invoke<NewsItem[]>("get_cyber_news");
}
