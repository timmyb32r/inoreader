import { useEffect, useRef, useState } from "preact/hooks";

const isPopup = (path: string) => path.split("?")[0].startsWith("/subscriptions");
const currentUrl = () => location.pathname + location.search + location.hash;
const parents = (): string[] => history.state?.popupParents ?? [];

// Each history entry remembers only windows actually visited, including the
// reader's query/cursor. Direct links have no parent and close to the reader.
export function usePopupNavigation(confirmDiscard: () => boolean) {
  const [url, setUrl] = useState(currentUrl);
  const previous = useRef(url);
  const previousState = useRef(history.state);
  const closing = useRef(false);
  const confirm = useRef(confirmDiscard);
  confirm.current = confirmDiscard;
  useEffect(() => {
    const pop = () => {
      if (!closing.current && !confirm.current()) { history.pushState(previousState.current, "", previous.current); return; }
      closing.current = false;
      previous.current = currentUrl();
      previousState.current = history.state;
      setUrl(previous.current);
    };
    window.addEventListener("popstate", pop);
    return () => window.removeEventListener("popstate", pop);
  }, []);
  const navigate = (next: string) => {
    if (!confirm.current()) return false;
    const trail = isPopup(next) ? [...parents(), currentUrl()] : [];
    history.pushState({ popupParents: trail }, "", next);
    previousState.current = history.state;
    previous.current = currentUrl();
    setUrl(previous.current);
    return true;
  };
  const close = () => {
    if (closing.current || !confirm.current()) return;
    if (parents().length) {
      closing.current = true;
      history.back();
      return;
    }
    history.replaceState({}, "", "/reader");
    previousState.current = history.state;
    previous.current = currentUrl();
    setUrl(previous.current);
  };
  return { path: url.split("?")[0], backgroundPath: (parents()[0] ?? "/reader").split("?")[0], navigate, close };
}
