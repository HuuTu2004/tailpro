import { Button, Select, TextField } from '../../shared/ui/Controls';
import { useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import type { AuthenticationChallenge, ConfigImport, ConnectionAttempt, ConnectionInfo, HostProfile } from '../../shared/contracts/generated';
import { native } from '../../shared/client/native';
import './connections.css';

type Props = { standaloneEditor?: boolean; connections: ConnectionInfo[]; selected: string; select: (id: string) => void; run: (action: () => Promise<unknown>) => void };
const emptyProfile = (): HostProfile => ({ id: crypto.randomUUID(), target: '', user: null, port: null, environment: 'development', identity_agent: null, authentication: 'config_agent', identity_file: null, proxy_jump: null });
const isPending = (attempt?: ConnectionAttempt) => attempt?.state === 'authenticating' || attempt?.state === 'checking_readers';

function AuthChallenge({ attempt, challenge }: { attempt: string; challenge: AuthenticationChallenge }) {
  const [value, setValue] = useState('');
  const [verified, setVerified] = useState(false);
  const [sending, setSending] = useState(false);
  const [error, setError] = useState('');
  const hostKey = challenge.kind === 'host_key';
  const confirmation = challenge.kind === 'confirmation';
  async function answer() {
    if (sending) return;
    setSending(true); setError('');
    const response = value; setValue('');
    try { await native.answerAuthentication(attempt, challenge.id, response, verified); }
    catch (reason) { setError(String(reason)); setSending(false); }
  }
  return <section className="authentication-challenge" aria-label={hostKey ? 'Verify host fingerprint' : 'Authentication request'} onKeyDown={event => {
    if (event.key === 'Enter') { event.preventDefault(); event.stopPropagation(); void answer(); }
  }}>
    <h3>{hostKey ? 'Verify this host before continuing' : confirmation ? 'Confirm authentication' : 'Authentication required'}</h3>
    <pre className="authentication-prompt">{challenge.prompt}</pre>
    {hostKey && <><p className="hint">Compare this fingerprint with a trusted source. It will be saved only for this connection profile.</p><code className="host-fingerprint">{challenge.fingerprint}</code></>}
    {!confirmation && <TextField autoFocus label={hostKey ? 'Verified fingerprint' : 'Password, passphrase or verification code'} type={hostKey ? 'text' : 'password'} value={value} disabled={sending} onChange={event => setValue(event.target.value)} autoComplete="off" spellCheck={false} maxLength={hostKey ? 128 : 8192}/>}
    {(hostKey || confirmation) && <label className="connection-consent"><input type="checkbox" checked={verified} disabled={sending} onChange={event => setVerified(event.target.checked)}/><span>{hostKey ? 'I verified the fingerprint using a trusted source.' : 'I confirm this authentication request.'}</span></label>}
    {error && <p role="alert" className="notice">{error}</p>}
    <Button variant="primary" disabled={sending || ((hostKey || confirmation) && !verified) || (hostKey && value !== challenge.fingerprint)} busy={sending} onClick={() => void answer()}>{hostKey ? 'Trust this fingerprint' : 'Continue'}</Button>
  </section>;
}

export function Connections({ standaloneEditor = false, connections, selected, select, run }: Props) {
  const [draft, setDraft] = useState<HostProfile>(emptyProfile);
  const [saved, setSaved] = useState<HostProfile[]>([]);
  const [savedId, setSavedId] = useState('');
  const [config, setConfig] = useState<ConfigImport>();
  const [consent, setConsent] = useState(false);
  const [beginning, setBeginning] = useState(false);
  const [attemptId, setAttemptId] = useState('');
  const [attempt, setAttempt] = useState<ConnectionAttempt>();
  const [error, setError] = useState('');
  const [pickingKey, setPickingKey] = useState(false);
  const form = useRef<HTMLFormElement>(null);
  const target = useRef<HTMLInputElement>(null);
  const mounted = useRef(false);
  const pendingRef = useRef(false);
  const pending = beginning || isPending(attempt);
  pendingRef.current = pending;
  const autoStarted = useRef(false);

  function changed(patch: Partial<HostProfile>) {
    setDraft(previous => ({ ...previous, ...patch })); setAttempt(undefined); setAttemptId(''); setError('');
  }
  function choose(profile?: HostProfile) {
    setSavedId(profile?.id ?? ''); setDraft(profile ? { ...profile } : emptyProfile());
    setConsent(false); setAttempt(undefined); setAttemptId(''); setError('');
  }
  async function begin(profile: HostProfile, testOnly: boolean) {
    if (pendingRef.current) return;
    pendingRef.current = true; setBeginning(true); setError(''); setAttempt(undefined);
    try {
      const id = await native.beginConnection({ ...profile, identity_file: profile.authentication === 'private_key' ? profile.identity_file : null }, consent, testOnly);
      if (mounted.current) { setAttemptId(id); setAttempt({ id, profile_id: profile.id, state: 'authenticating', challenge: null, error: null, test_only: testOnly, capabilities: null }); }
      else await native.cancelConnectionAttempt();
    } catch (reason) { if (mounted.current) setError(String(reason)); }
    finally { if (mounted.current) setBeginning(false); }
  }
  useEffect(() => {
    mounted.current = true;
    let alive = true;
    let unlisten: (() => void) | undefined;
    const load = async () => {
      try {
        const profiles = await native.savedHosts();
        if (!alive) return;
        setSaved(profiles);
        if (standaloneEditor && !autoStarted.current) {
          autoStarted.current = true;
          const route = new URLSearchParams(location.search);
          const profile = profiles.find(p => p.id === route.get('profile'));
          if (profile) { choose(profile); if (route.get('connect') === '1') void begin(profile, false); }
          else if (route.has('profile')) setError('This saved connection no longer exists.');
        }
      } catch (reason) { if (alive) setError(String(reason)); }
    };
    void load();
    if (!standaloneEditor) void listen('connection-profiles-changed', () => void load()).then(stop => { if (alive) unlisten = stop; else stop(); }).catch(reason => { if (alive) setError(String(reason)); });
    target.current?.focus();
    return () => { alive = false; mounted.current = false; unlisten?.(); if (standaloneEditor && pendingRef.current) void native.cancelConnectionAttempt().catch(() => {}); };
  }, [standaloneEditor]);
  useEffect(() => {
    if (!attemptId) return;
    let alive = true;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      try {
        const next = await native.connectionAttempt(attemptId);
        if (!alive) return;
        setAttempt(next);
        if (next.state === 'connected') { await native.closeConnectionEditor(next.profile_id); return; }
        if (isPending(next)) timer = setTimeout(poll, 200);
      } catch (reason) { if (alive) { setError(String(reason)); setAttemptId(''); setAttempt(undefined); } }
    }
    void poll();
    return () => { alive = false; clearTimeout(timer); };
  }, [attemptId]);

  async function close() {
    if (pendingRef.current) await native.cancelConnectionAttempt();
    await native.closeConnectionEditor();
  }
  async function pickKey() {
    setPickingKey(true); setError('');
    try { const path = await native.pickIdentityFile(); if (mounted.current && path) changed({ identity_file: path }); }
    catch (reason) { if (mounted.current) setError(String(reason)); }
    finally { if (mounted.current) setPickingKey(false); }
  }
  if (!standaloneEditor) return <section className="connections-page" aria-label="SSH connections">
    <div className="section-heading connection-page-heading"><h2>Connections</h2><Button variant="primary" onClick={() => run(() => native.openConnectionEditor())}>Add connection</Button></div>
    {error && <p role="alert" className="notice">{error}</p>}
    {saved.map(host => <article className="connection-row" key={host.id}><Button className="connection-select" aria-pressed={selected === host.id} onClick={() => select(host.id)}><strong>{host.target}</strong><small>{connections.find(c => c.host.id === host.id)?.state ?? 'Saved · Disconnected'}</small></Button><Button onClick={() => run(() => native.openConnectionEditor(host.id, true))}>Connect</Button><Button onClick={() => run(() => native.openConnectionEditor(host.id))}>Edit</Button><Button onClick={() => run(() => native.forgetHost(host.id))}>Forget</Button></article>)}
    {!saved.length && <p className="hint">No saved connections yet.</p>}
  </section>;

  return <section className="native-connection-editor" aria-label="Connect to server" onKeyDown={event => { if (event.key === 'Escape') { event.preventDefault(); void close().catch(reason => setError(String(reason))); } }}>
    <p className="connection-draft-intro hint">Connect, check available readers, then choose the logs to open.</p>
    <form ref={form} onSubmit={event => { event.preventDefault(); if (form.current?.reportValidity()) void begin(draft, false); }}>
      {saved.length > 0 && <label>Saved connection<Select disabled={pending} value={savedId} onChange={event => choose(saved.find(p => p.id === event.target.value))}><option value="">New connection</option>{saved.map(host => <option key={host.id} value={host.id}>{host.target} · {host.environment}</option>)}</Select></label>}
      <TextField label="Host / SSH alias" ref={target} required disabled={pending} value={draft.target} onChange={event => changed({ target: event.target.value })} list="ssh-aliases" autoComplete="off"/>
      <datalist id="ssh-aliases">{config?.aliases.map(alias => <option key={alias} value={alias}/>)}</datalist>
      <TextField label="User" disabled={pending} value={draft.user ?? ''} onChange={event => changed({ user: event.target.value || null })} placeholder="From SSH config"/>
      <label>Authentication<Select disabled={pending} value={draft.authentication} onChange={event => changed({ authentication: event.target.value as HostProfile['authentication'] })}><option value="config_agent">SSH config / agent</option><option value="private_key">Private key file</option><option value="interactive">Password / interactive</option></Select></label>
      {draft.authentication === 'private_key' && <div className="identity-file-field"><TextField label="Private key on this device" disabled={pending || pickingKey} required value={draft.identity_file ?? ''} onChange={event => changed({ identity_file: event.target.value || null })} autoComplete="off"/><Button disabled={pending || pickingKey} busy={pickingKey} onClick={() => void pickKey()}>Choose file…</Button></div>}
      {draft.authentication === 'interactive' && <p className="hint">Passwords and verification codes are requested only by the SSH attempt. They are not saved in this profile.</p>}
      <details className="connection-advanced"><summary>Advanced SSH options</summary>
        <TextField label="Port" disabled={pending} type="number" min="1" max="65535" value={draft.port ?? ''} onChange={event => changed({ port: event.target.value ? Number(event.target.value) : null })} placeholder="From SSH config"/>
        <TextField label="Jump host / SSH alias" disabled={pending} value={draft.proxy_jump ?? ''} onChange={event => changed({ proxy_jump: event.target.value || null })} placeholder="From SSH config"/>
        <TextField label="IdentityAgent socket" disabled={pending} value={draft.identity_agent ?? ''} onChange={event => changed({ identity_agent: event.target.value || null })} placeholder="Use configured SSH agent"/>
        <label>Environment<Select disabled={pending} value={draft.environment} onChange={event => changed({ environment: event.target.value as HostProfile['environment'] })}><option value="development">Development</option><option value="staging">Staging</option><option value="production">Production</option></Select></label>
        <label className="connection-consent"><input type="checkbox" checked={consent} disabled={pending} onChange={event => { setConsent(event.target.checked); setError(''); }}/><span>Allow dynamic SSH configuration for this session<small>May run local commands from your SSH configuration. Permission expires on disconnect.</small></span></label>
        <Button className="text-button" disabled={pending} onClick={() => void native.importConfig().then(setConfig).catch(reason => setError(String(reason)))}>Import SSH aliases</Button>
        {config?.disclosures.map(disclosure => <p className="hint" key={disclosure}>{disclosure}</p>)}
      </details>
      {pending && <p role="status" className="connection-test-progress">{attempt?.state === 'checking_readers' ? 'SSH connected · Checking available readers…' : 'Authenticating…'}{attempt?.test_only ? ' · Temporary test session' : ''}</p>}
      {attempt?.challenge && <AuthChallenge key={attempt.challenge.id} attempt={attempt.id} challenge={attempt.challenge}/>}
      {attempt?.state === 'tested' && <p role="status" className="connection-test-result success">{attempt.capabilities?.helper_ready ? 'Connection and reader check complete. No profile or active connection was changed.' : 'SSH connection succeeded. Reader prerequisites are unavailable; check the remote Python 3 environment.'}</p>}
      {(error || attempt?.error) && <p role="alert" className="notice">{error || attempt?.error}</p>}
      <div className="connection-dialog-actions"><Button disabled={pending || pickingKey || !draft.target.trim()} onClick={() => { if (form.current?.reportValidity()) void begin(draft, true); }}>Test connection</Button><span/><Button onClick={() => void close().catch(reason => setError(String(reason)))}>{pending ? 'Cancel attempt' : 'Cancel'}</Button><Button variant="primary" type="submit" disabled={pending || pickingKey || !draft.target.trim()} busy={pending}>{pending ? 'Connecting…' : attempt?.state === 'failed' ? 'Retry' : 'Connect'}</Button></div>
    </form>
  </section>;
}
