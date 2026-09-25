import { useState } from "preact/hooks";
import { AuthField } from "../ui/fields";
import { Icon } from "../ui/Icon";
import type { ApiClient } from "../api/client";

export function AuthScreen({ client, onSignedIn }: { client:ApiClient; onSignedIn:()=>void }) {
  const [pending, setPending] = useState(false);
  const [username, setUsername] = useState(""); const [password, setPassword] = useState(""); const [confirmation,setConfirmation]=useState("");const [currentPassword,setCurrentPassword]=useState("");const [error, setError] = useState("");
  const path=window.location.pathname;const params=new URLSearchParams(window.location.search);const mode=path.includes("invite")?"invite":path.includes("reset")?"reset":path.includes("change-password")?"change":"signin";const token=params.get("token")??"";
  const submit = (event: Event) => { event.preventDefault();if(pending)return;if(mode!=="signin"&&password!==confirmation){setError("New password and confirmation must match.");return}setPending(true); setError("");const request=mode==="invite"?client.acceptInvite(token,username,password):mode==="reset"?client.resetPassword(token,password):mode==="change"?client.changePassword(currentPassword,password):client.signIn(username,password);request.then(()=>{if(mode!=="signin")window.history.replaceState({},"","/");onSignedIn();}).catch((failure: Error) => setError(failure.message)).finally(() => setPending(false)); };
  return <main class="auth-shell">
    <section class="auth-story" aria-label="Product introduction">
      <div class="brand brand--large"><span class="brand__mark"><Icon name="feed" size={23}/></span><span>Reader</span></div>
      <div class="auth-story__copy"><p class="eyebrow">Your signal, uninterrupted</p><h1>A quieter place<br/>for the open web.</h1><p>Follow every source. Keep every useful word. Read at your own pace.</p></div>
      <div class="auth-story__status"><span class="status-dot"/> Private, invitation-only access</div>
    </section>
    <section class="auth-panel">
      <form class="auth-card" onSubmit={submit} aria-busy={pending}>
        <p class="eyebrow">Private reader</p><h2>{mode==="invite"?"Accept invitation":mode==="reset"?"Choose a new password":mode==="change"?"Change password":"Welcome back"}</h2><p>{mode==="signin"?"Sign in to continue to your workspaces.":"Complete this secure account action."}</p>
        {(mode==="signin"||mode==="invite")&&<label>Username<AuthField authRole="username" required autoFocus value={username} onInput={(e) => setUsername(e.currentTarget.value)} placeholder="you@example.com" /></label>}
        {mode==="change"&&<label>Current password<AuthField authRole="current-password" required value={currentPassword} onInput={e=>setCurrentPassword(e.currentTarget.value)}/></label>}
        <label>{mode==="signin"?"Password":"New password"}<AuthField authRole={mode==="signin"?"current-password":"new-password"} required value={password} onInput={(e) => setPassword(e.currentTarget.value)} placeholder="Your password" /></label>
        {mode!=="signin"&&<label>Confirm new password<AuthField authRole="new-password" purpose="confirmation" required value={confirmation} onInput={(e)=>setConfirmation(e.currentTarget.value)} placeholder="Repeat your new password"/></label>}
        <p class="field-help field-help--error" role="alert">{error}</p>
        <button class="primary-button primary-button--wide" disabled={pending||((mode==="invite"||mode==="reset")&&!token)}>{pending ? <><span class="spinner"/> Saving…</> : mode==="signin"?"Sign in":"Continue"}</button>
        <p class="auth-help">Access is invitation only. Password resets are handled by an administrator.</p>
      </form>
    </section>
  </main>;
}
