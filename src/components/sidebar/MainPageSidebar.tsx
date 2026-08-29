import { useCallback, useEffect, useState } from "react";
import Enclave from "@/app/app";
import { getCyberNews, NewsItem } from "@/lib/cyberNews";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Button } from "@/components/ui/button";
import { RefreshCw, Rss } from "lucide-react";
import { cn } from "@/lib/utils";

function formatTime(publishedAt: number): string {
  if (!publishedAt) return "";

  const diff = Date.now() - publishedAt;
  const minutes = Math.floor(diff / 60_000);

  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes}m ago`;

  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;

  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

function StaticFallback() {
  const staticLinks = [
    {
      title: "Surveillance Self-Defense",
      source: "EFF",
      href: "https://ssd.eff.org",
    },
    {
      title: "Privacy Guides",
      source: "Community",
      href: "https://www.privacyguides.org",
    },
    {
      title: "The Hacker News",
      source: "News",
      href: "https://thehackernews.com",
    },
    {
      title: "Krebs on Security",
      source: "News",
      href: "https://krebsonsecurity.com",
    },
    {
      title: "CISA Alerts",
      source: "Gov",
      href: "https://www.cisa.gov/news-events/cybersecurity-advisories",
    },
  ];

  return (
    <div className="flex flex-col gap-2.5 px-3 py-4">
      <p className="text-xs text-muted-foreground">
        Live news couldn't be loaded right now. Here are security resources you
        can open manually:
      </p>
      {staticLinks.map((link) => (
        <button
          key={link.href}
          className="group w-full rounded-md px-2 py-1.5 text-left hover:bg-accent"
          onClick={() => openUrl(link.href)}
        >
          <span className="block truncate text-sm text-foreground whitespace-normal leading-tight group-hover:text-foreground">
            {link.title}
          </span>
          <span className="text-xs text-muted-foreground">{link.source}</span>
        </button>
      ))}
    </div>
  );
}

export default function MainPageSidebar(_props: {
  appRef: React.RefObject<Enclave | null>;
}) {
  const [items, setItems] = useState<NewsItem[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);

  const load = useCallback(() => {
    setLoading(true);
    setError(false);

    getCyberNews()
      .then((news) => setItems(news))
      .catch(() => {
        setItems(null);
        setError(true);
      })
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  return (
    <div className="flex h-screen w-full flex-col">
      <header className="flex items-center justify-between px-3 py-2.5 border-b border-b-border">
        <h1 className="text-lg font-semibold flex items-center gap-2">
          <Rss className="h-4 w-4 text-primary" />
          Security News
        </h1>
        <Button
          variant="ghost"
          size="icon-sm"
          disabled={loading}
          onClick={load}
        >
          <RefreshCw className={cn("h-4 w-4", loading && "animate-spin")} />
        </Button>
      </header>

      <section className="flex-1 overflow-y-scroll scrollbar-none [scrollbar-width:none] [&::-webkit-scrollbar]:hidden px-2 py-2">
        {loading && items === null && (
          <p className="px-2 py-4 text-sm text-muted-foreground">
            Loading news…
          </p>
        )}

        {error && !loading && <StaticFallback />}

        {items && (
          <div className="flex flex-col gap-0.5">
            {items.map((item) => (
              <button
                key={item.link}
                className="group flex w-full flex-col gap-1 rounded-md px-2 py-1.5 text-left hover:bg-accent"
                onClick={() => openUrl(item.link)}
              >
                <span className="text-sm text-foreground leading-tight">
                  {item.title}
                </span>
                <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
                  <span className="truncate">{item.source}</span>
                  {item.publishedAt > 0 && (
                    <>
                      <span aria-hidden>·</span>
                      <span className="shrink-0">
                        {formatTime(item.publishedAt)}
                      </span>
                    </>
                  )}
                </span>
              </button>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
