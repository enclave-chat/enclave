import axios from "axios";

export type NewsItem = {
  title: string;
  link: string;
  source: string;
  publishedAt: number;
};

const FEEDS: { href: string; source: string }[] = [
  {
    href: "/news/thn",
    source: "The Hacker News",
  },
  {
    href: "/news/cisa",
    source: "CISA",
  },
  {
    href: "/news/krebs",
    source: "Krebs on Security",
  },
  {
    href: "/news/bc",
    source: "BleepingComputer",
  },
];

const MAX_ITEMS = 15;
const REQUEST_TIMEOUT_MS = 10_000;

function parseDate(value: string | null | undefined): number {
  if (!value) return 0;

  const ms = Date.parse(value);
  return Number.isNaN(ms) ? 0 : ms;
}

function textOf(el: Element | undefined | null, tag: string): string {
  return el?.getElementsByTagName(tag)[0]?.textContent?.trim() ?? "";
}

function dateOf(el: Element): number {
  const pubDate = textOf(el, "pubDate");
  if (pubDate) return parseDate(pubDate);

  const dc = el.getElementsByTagName("dc:date")[0]?.textContent?.trim();
  return parseDate(dc);
}

function parseFeed(xml: string, source: string): NewsItem[] {
  const doc = new DOMParser().parseFromString(xml, "text/xml");

  if (doc.querySelector("parsererror")) return [];

  const items: NewsItem[] = [];

  doc.querySelectorAll("item").forEach((item) => {
    const title = textOf(item, "title");
    const link = textOf(item, "link");
    if (!title || !link) return;

    items.push({
      title,
      link,
      source,
      publishedAt: dateOf(item),
    });
  });

  return items;
}

export async function getCyberNews(): Promise<NewsItem[]> {
  const results = await Promise.allSettled(
    FEEDS.map(({ href, source }) =>
      axios
        .get<string>(href, { timeout: REQUEST_TIMEOUT_MS })
        .then((res) => parseFeed(res.data, source)),
    ),
  );

  const items: NewsItem[] = [];

  results.forEach((result) => {
    if (result.status === "fulfilled") {
      items.push(...result.value);
    }
  });

  if (items.length === 0) {
    throw new Error("No cyber news available right now.");
  }

  const seen = new Set<string>();
  const deduped = items
    .sort((a, b) => b.publishedAt - a.publishedAt)
    .filter((item) => {
      const key = item.link;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });

  return deduped.slice(0, MAX_ITEMS);
}
