import { useEffect, useRef, useState } from "preact/hooks";
import { Icon } from "../../ui/Icon";
import { ModalDialog } from "../../ui/ModalDialog";
import { AutofillResistantField } from "../../ui/fields";
import {
  formatDuration,
  initialTimer,
  parseDuration,
  readTimer,
  remaining,
  type TimerState,
} from "./state";
import "./timer.css";

export function CountdownTimer({ accountId }: { accountId: string }) {
  const key = `reader:countdown:${accountId}`;
  const [error, setError] = useState("");
  const [draftError, setDraftError] = useState("");
  const [state, setState] = useState<TimerState>(initialTimer);
  const current = useRef(state);
  const [now, setNow] = useState(Date.now());
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("01:00:00");
  const [flash, setFlash] = useState(false);
  const audio = useRef<AudioContext>();
  const restore = () => {
    try {
      const next = readTimer(localStorage.getItem(key));
      current.current = next;
      setState(next);
      setError("");
    } catch {
      setError("Cannot restore timer. Check browser storage.");
    }
  };
  const save = (next: TimerState) => {
    try {
      localStorage.setItem(key, JSON.stringify(next));
      current.current = next;
      setState(next);
      setError("");
      return true;
    } catch {
      setError("Cannot save timer. Check browser storage.");
      return false;
    }
  };
  const unlockAudio = () => {
    try {
      audio.current ??= new AudioContext();
      void audio.current.resume().catch(() => {});
    } catch {
      /* Visual completion remains available. */
    }
  };
  const signal = () => {
    setFlash(true);
    const context = audio.current;
    if (!context || context.state !== "running") return;
    const tone = context.createOscillator(),
      gain = context.createGain();
    tone.connect(gain);
    gain.connect(context.destination);
    tone.frequency.value = 660;
    gain.gain.setValueAtTime(0.12, context.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.001, context.currentTime + 0.8);
    tone.start();
    tone.stop(context.currentTime + 0.8);
  };
  useEffect(() => {
    restore();
    let alive = true,
      checking = false;
    const tick = () => {
      const time = Date.now();
      setNow(time);
      if (
        current.current.mode !== "running" ||
        remaining(current.current, time) > 0 ||
        checking
      )
        return;
      checking = true;
      const finish = () => {
        if (!alive) return;
        try {
          const latest = readTimer(localStorage.getItem(key));
          if (
            latest.mode === "running" &&
            remaining(latest, Date.now()) === 0
          ) {
            if (save({ mode: "finished", duration: latest.duration })) signal();
          } else {
            current.current = latest;
            setState(latest);
          }
        } catch {
          setError("Cannot restore timer. Check browser storage.");
        }
      };
      // One completion signal across tabs; the persisted deadline is authoritative.
      if (navigator.locks)
        void navigator.locks.request(key, finish).finally(() => {
          checking = false;
        });
      else {
        finish();
        checking = false;
      }
    };
    const storage = (event: StorageEvent) => {
      if (event.key === key || event.key === null) restore();
    };
    const id = window.setInterval(tick, 200);
    window.addEventListener("storage", storage);
    window.addEventListener("focus", tick);
    document.addEventListener("visibilitychange", tick);
    // A fresh gesture also enables sound after a restored session.
    document.addEventListener("pointerdown", unlockAudio);
    document.addEventListener("keydown", unlockAudio);
    tick();
    return () => {
      alive = false;
      clearInterval(id);
      window.removeEventListener("storage", storage);
      window.removeEventListener("focus", tick);
      document.removeEventListener("visibilitychange", tick);
      document.removeEventListener("pointerdown", unlockAudio);
      document.removeEventListener("keydown", unlockAudio);
      void audio.current?.close();
    };
  }, [key]);
  const toggle = () => {
    if (error) return;
    unlockAudio();
    setFlash(false);
    const time = Date.now();
    setNow(time);
    const v = current.current;
    if (v.mode === "running") {
      const left = remaining(v, time);
      if (left > 0)
        save({ mode: "paused", duration: v.duration, remaining: left });
      else if (save({ mode: "finished", duration: v.duration })) signal();
    } else
      save({
        mode: "running",
        duration: v.duration,
        deadline: time + (v.mode === "paused" ? v.remaining : v.duration),
      });
  };
  return (
    <>
      <div
        class={`countdown ${state.mode} ${flash ? "countdown--flash" : ""}`}
        aria-label="Session timer"
        onAnimationEnd={() => setFlash(false)}
      >
        <button
          class="countdown__digits"
          disabled={state.mode === "running" || state.mode === "paused"}
          title={error || "Set timer duration"}
          aria-label={`Timer ${formatDuration(remaining(state, now))}. ${state.mode}. Set duration`}
          onClick={() => {
            setDraft(formatDuration(state.duration));
            setDraftError("");
            setEditing(true);
          }}
        >
          {error ? "Storage error" : formatDuration(remaining(state, now))}
        </button>
        <button
          class="countdown__control"
          disabled={!!error}
          title={
            state.mode === "running"
              ? "Pause timer"
              : state.mode === "paused"
                ? "Resume timer"
                : "Start timer"
          }
          aria-label={
            state.mode === "running"
              ? "Pause timer"
              : state.mode === "paused"
                ? "Resume timer"
                : "Start timer"
          }
          onClick={toggle}
        >
          {state.mode === "running" ? (
            <Icon name="pause" />
          ) : (
            <span aria-hidden="true">▶</span>
          )}
        </button>
        <button
          class="countdown__control"
          title="Stop timer"
          aria-label="Stop timer"
          disabled={state.mode === "idle" || !!error}
          onClick={() => {
            if (save({ mode: "idle", duration: state.duration }))
              setFlash(false);
          }}
        >
          <Icon name="stop" />
        </button>
        <span class="sr-only" role="status">
          {error ||
            (state.mode === "finished"
              ? "Timer finished"
              : state.mode === "paused"
                ? "Timer paused"
                : "")}
        </span>
      </div>
      {editing && (
        <ModalDialog title="Session timer" onClose={() => setEditing(false)}>
          <form
            class="countdown-editor"
            onSubmit={(event) => {
              event.preventDefault();
              try {
                const duration = parseDuration(draft);
                if (save({ mode: "idle", duration })) setEditing(false);
              } catch (e) {
                setDraftError((e as Error).message);
              }
            }}
          >
            <label>
              Duration (HH:MM:SS)
              <AutofillResistantField
                aria-label="Timer duration"
                value={draft}
                onInput={(e) => {
                  setDraft(e.currentTarget.value);
                  setDraftError("");
                }}
              />
            </label>
            <p>00:00:01–99:59:59. Runs continuously until paused or stopped.</p>
            <p class="countdown-editor__status" role="status">
              {draftError || error}
            </p>
            <button type="submit">Set duration</button>
          </form>
        </ModalDialog>
      )}
    </>
  );
}
