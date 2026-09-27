import { useEffect, useRef, useState } from "preact/hooks";
import { ApiError, type ApiClient, type Bootstrap } from "../api/client";
import { AuthScreen } from "./AuthScreen";
import { ReaderApplication } from "./ReaderApplication";
import { readArticlePagePosition } from "./readerLocation";

type Session =
  | { status: "loading" }
  | { status: "signedOut" }
  | { status: "failed"; error: string }
  | { status: "ready"; data: Bootstrap };
/** Authentication owns the lifetime of all account-local state and in-flight work. */
export function App({ client }: { client: ApiClient }) {
  const [session, setSession] = useState<Session>({ status: "loading" });
  const generation = useRef(0);
  const reload = () => {
    generation.current++;
    setSession({ status: "loading" });
  };
  const signOut = () => {
    generation.current++;
    setSession({ status: "signedOut" });
  };
  useEffect(() => {
    window.addEventListener("reader:unauthorized", signOut);
    return () => {
      generation.current++;
      window.removeEventListener("reader:unauthorized", signOut);
    };
  }, []);
  useEffect(() => {
    if (session.status !== "loading") return;
    const token = ++generation.current;
    client
      .bootstrap(readArticlePagePosition())
      .then((data) => {
        if (token === generation.current) setSession({ status: "ready", data });
      })
      .catch((error: Error) => {
        if (token === generation.current)
          setSession(
            error instanceof ApiError && error.status === 401
              ? { status: "signedOut" }
              : { status: "failed", error: error.message },
          );
      });
    return () => {
      generation.current++;
    };
  }, [client, session.status]);
  if (session.status === "failed")
    return (
      <main class="fatal-state">
        <h1>Reader is unavailable</h1>
        <p>{session.error}</p>
        <button onClick={reload}>Try again</button>
      </main>
    );
  if (session.status === "loading")
    return (
      <main
        class="bootstrap-loading"
        role="status"
        aria-label="Opening your library"
        aria-busy="true"
      >
        <span class="bootstrap-loading__spinner" aria-hidden="true" />
      </main>
    );
  if (
    session.status === "signedOut" ||
    /\/(invite|reset-password|change-password)/.test(location.pathname)
  )
    return <AuthScreen client={client} onSignedIn={reload} />;
  return (
    <ReaderApplication
      key={session.data.account.id}
      client={client}
      bootstrap={session.data}
      onSignOut={signOut}
    />
  );
}
