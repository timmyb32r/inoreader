import { useEffect, useRef, useState } from "preact/hooks";
/** Navigation is read-only. Lock synchronously until the selected article loads
 * (or fails), preserving the return button's footprint throughout. */
export function useReturnToNews(onReturn?: () => string | null) {
  const target = useRef<string | null>(null),
    lock = useRef(false);
  const [pending, setPending] = useState(false);
  const finish = () => {
    target.current = null;
    lock.current = false;
    setPending(false);
  };
  useEffect(() => {
    const ready = (event: Event) => {
      if ((event as CustomEvent<string>).detail === target.current) finish();
    };
    window.addEventListener("reader-reading-ready", ready);
    return () => window.removeEventListener("reader-reading-ready", ready);
  }, []);
  useEffect(() => {
    if (
      target.current &&
      location.pathname + location.search !== target.current
    )
      finish();
  }, [location.pathname, location.search]);
  return {
    pending,
    open: () => {
      if (lock.current || !onReturn) return;
      lock.current = true;
      setPending(true);
      target.current = onReturn();
      if (!target.current) finish();
    },
  };
}
