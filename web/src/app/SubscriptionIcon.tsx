import { useEffect, useState } from "preact/hooks";

export function SubscriptionIcon({ name, sourceUrl, size = 27 }: { name: string; sourceUrl?: string; size?: number }) {
  const iconUrl = faviconUrl(sourceUrl);
  const [loaded, setLoaded] = useState(false);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    setLoaded(false);
    setFailed(false);
  }, [iconUrl]);

  return <span class="subscription-icon" style={{ width: size, height: size }} aria-hidden="true">
    <span class="subscription-icon__fallback">{name.trim().slice(0, 1).toLocaleUpperCase() || "·"}</span>
    {iconUrl && !failed && <img
      class={loaded ? "subscription-icon__image loaded" : "subscription-icon__image"}
      src={iconUrl}
      alt=""
      width={size}
      height={size}
      loading="lazy"
      decoding="async"
      referrerPolicy="no-referrer"
      onLoad={() => setLoaded(true)}
      onError={() => setFailed(true)}
    />}
  </span>;
}

export function faviconUrl(sourceUrl?: string): string | undefined {
  if (!sourceUrl) return undefined;
  try {
    const url = new URL(sourceUrl);
    if (url.protocol !== "http:" && url.protocol !== "https:") return undefined;
    url.pathname = "/favicon.ico";
    url.search = "";
    url.hash = "";
    return url.toString();
  } catch {
    return undefined;
  }
}
