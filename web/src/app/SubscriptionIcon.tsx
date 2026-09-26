import { useEffect, useState } from "preact/hooks";

export function SubscriptionIcon({ name, iconDataUrl, size = 27 }: { name: string; iconDataUrl?: string; size?: number }) {
  const [loaded, setLoaded] = useState(false);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    setLoaded(false);
    setFailed(false);
  }, [iconDataUrl]);

  return <span class="subscription-icon" style={{ width: size, height: size }} aria-hidden="true">
    <span class="subscription-icon__fallback">{name.trim().slice(0, 1).toLocaleUpperCase() || "·"}</span>
    {iconDataUrl && !failed && <img
      class={loaded ? "subscription-icon__image loaded" : "subscription-icon__image"}
      src={iconDataUrl}
      alt=""
      width={size}
      height={size}
      loading="lazy"
      decoding="async"
      onLoad={() => setLoaded(true)}
      onError={() => setFailed(true)}
    />}
  </span>;
}
