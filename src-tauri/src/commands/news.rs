use quick_xml::events::Event;
use quick_xml::Reader;
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NewsItem {
    pub title: String,
    pub link: String,
    pub source: String,
    pub published_at: i64,
}

struct Feed {
    url: &'static str,
    source: &'static str,
}

const FEEDS: &[Feed] = &[
    Feed {
        url: "https://feeds.feedburner.com/TheHackersNews",
        source: "The Hacker News",
    },
    Feed {
        url: "https://www.cisa.gov/cybersecurity-advisories/all.xml",
        source: "CISA",
    },
    Feed {
        url: "https://krebsonsecurity.com/feed/",
        source: "Krebs on Security",
    },
    Feed {
        url: "https://www.bleepingcomputer.com/feed/",
        source: "BleepingComputer",
    },
];

const MAX_ITEMS: usize = 15;

#[derive(Default, Clone)]
struct RssItem {
    title: String,
    link: String,
    pub_date: String,
}

fn field_map() -> &'static [(&'static [u8], &'static str)] {
    &[
        (b"title", "title"),
        (b"link", "link"),
        (b"pubDate", "pubDate"),
    ]
}

fn parse_feed(xml: &str) -> Vec<RssItem> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut items = Vec::new();
    let mut current: Option<RssItem> = None;
    // Stack of open element names (lowercased) while inside an <item>.
    let mut stack: Vec<Vec<u8>> = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = e.local_name().as_ref().to_ascii_lowercase();
                if name == b"item" {
                    current = Some(RssItem::default());
                }
                stack.push(name);
            }
            Ok(Event::End(e)) => {
                let name = e.local_name().as_ref().to_ascii_lowercase();
                if name == b"item" {
                    if let Some(item) = current.take() {
                        if !item.title.is_empty() && !item.link.is_empty() {
                            items.push(item);
                        }
                    }
                }
                stack.pop();
            }
            Ok(Event::Text(e)) => {
                if let Some(item) = current.as_mut() {
                    if let Ok(text) = std::str::from_utf8(e.as_ref()) {
                        for (tag, field) in field_map() {
                            if stack.last().map(|s| s.as_slice()) == Some(*tag) {
                                match *field {
                                    "title" => item.title.push_str(text),
                                    "link" => item.link.push_str(text),
                                    "pubDate" => item.pub_date.push_str(text),
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    for item in items.iter_mut() {
        item.title = item.title.trim().to_string();
        item.link = item.link.trim().to_string();
        item.pub_date = item.pub_date.trim().to_string();
    }

    items
}

/// Best-effort RFC 822 RSS date parsing; returns 0 on any failure.
fn parse_date(value: &str) -> i64 {
    if value.is_empty() {
        return 0;
    }
    let value = value.split_once(',').map(|(_, rest)| rest).unwrap_or(value);
    let parts: Vec<&str> = value.split_whitespace().collect();
    if parts.len() < 5 {
        return 0;
    }

    let day: i64 = parts[0].parse().unwrap_or(0);
    let month = match parts[1] {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => 0,
    };
    let year: i64 = parts[2].parse().unwrap_or(0);
    let hm: Vec<&str> = parts[3].split(':').collect();
    if hm.len() < 3 || year == 0 || month == 0 || day == 0 {
        return 0;
    }
    let hour: i64 = hm[0].parse().unwrap_or(0);
    let minute: i64 = hm[1].parse().unwrap_or(0);
    let second: i64 = hm[2].parse().unwrap_or(0);

    let days_before_month = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let yday = days_before_month[(month - 1) as usize] + day + if leap && month > 2 { 1 } else { 0 };

    let days = (year - 1970) * 365 + (year - 1969) / 4 + yday - 1;
    days * 86_400 + hour * 3_600 + minute * 60 + second
}

#[tauri::command]
pub async fn get_cyber_news() -> Result<Vec<NewsItem>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("failed to build http client: {e}"))?;

    let mut all_items: Vec<NewsItem> = Vec::new();

    for feed in FEEDS {
        let resp = client
            .get(feed.url)
            .header("User-Agent", "Mozilla/5.0")
            .header(
                "Accept",
                "application/rss+xml, application/xml;q=0.9, text/xml;q=0.8, */*;q=0.7",
            )
            .send()
            .await;

        let Ok(resp) = resp else { continue };
        if !resp.status().is_success() {
            continue;
        }
        let Ok(body) = resp.text().await else {
            continue;
        };

        all_items.extend(
            parse_feed(&body)
                .into_iter()
                .map(|item| NewsItem {
                    title: item.title,
                    link: item.link,
                    source: feed.source.to_string(),
                    published_at: parse_date(&item.pub_date),
                }),
        );
    }

    if all_items.is_empty() {
        return Err("No cyber news available right now.".to_string());
    }

    all_items.sort_by(|a, b| b.published_at.cmp(&a.published_at));

    let mut seen = std::collections::HashSet::new();
    let mut deduped = Vec::new();
    for item in all_items {
        if seen.insert(item.link.clone()) {
            deduped.push(item);
            if deduped.len() >= MAX_ITEMS {
                break;
            }
        }
    }

    Ok(deduped)
}
