import { Button, Select, TextField } from '../../shared/ui/Controls';
import { useEffect, useRef, useState } from 'react';
import type { ConfigImport, ConnectionInfo, HostProfile } from '../../shared/contracts/generated';
import { native } from '../../shared/client/native';
import { listen } from '@tauri-apps/api/event';
import './connections.css';

type Props = { standaloneEditor?: boolean; connections: ConnectionInfo[]; selected: string; select: (id: string) => void; run: (action: () => Promise<unknown>) => void };
type TestResult = { key: string; success: boolean; message: string };

export function Connections({ standaloneEditor = false, connections, selected, select, run }: Props) {
  const [connecting, setConnecting] = useState(false);
  const [testing, setTesting] = useState(false);
  const [consent, setConsent] = useState(false);
  const [sessionConsent, setSessionConsent] = useState(false);
  const [target, setTarget] = useState('');
  const [port, setPort] = useState('');
  const [user, setUser] = useState('');
  const [agent, setAgent] = useState('');
  const [environment, setEnvironment] = useState<HostProfile['environment']>('development');
  const [config, setConfig] = useState<ConfigImport>();
  const [saved, setSaved] = useState<HostProfile[]>([]);
  const [savedProfile, setSavedProfile] = useState('');
  const [testResult, setTestResult] = useState<TestResult>();
  const [draftError, setDraftError] = useState('');
  const [pageMessage, setPageMessage] = useState('');
  const form = useRef<HTMLFormElement>(null);
  const targetInput = useRef<HTMLInputElement>(null);
  const mounted = useRef(false);
  const testInFlight = useRef(false);
  const revision = useRef(0);
  const draftId = useRef('');
  const currentSelection = useRef(selected);
  currentSelection.current = selected;
  const draftKey = JSON.stringify({ target, port, user, agent, environment, consent });
  const busy = connecting || testing;

  useEffect(() => {
    let alive = true;
    mounted.current = true;
    const loadProfiles = () => native.savedHosts().then(value => { if (alive) setSaved(value); }).catch(error => { if (alive) setPageMessage(String(error)); });
    let unlisten: (() => void) | undefined;
    void loadProfiles();
    if (!standaloneEditor) {
      void listen('connection-profiles-changed', () => void loadProfiles()).then(stop => { if (alive) unlisten = stop; else stop(); }).catch(() => {});
      window.addEventListener('focus', loadProfiles);
    }
    return () => {
      alive = false; mounted.current = false; revision.current++;
      unlisten?.(); window.removeEventListener('focus', loadProfiles);
      if (testInFlight.current) { testInFlight.current = false; void native.cancelConnectionTest().catch(() => {}); }
    };
  }, [standaloneEditor]);
  useEffect(() => {
    if (standaloneEditor) { if (!draftId.current) draftId.current = crypto.randomUUID(); targetInput.current?.focus(); }
  }, [standaloneEditor]);

  function clearDraft() {
    setTarget(''); setUser(''); setPort(''); setAgent(''); setEnvironment('development');
    setConsent(false); setTestResult(undefined); setDraftError(''); setSavedProfile('');
    draftId.current = crypto.randomUUID();
  }
  function openDraft() {
    setPageMessage('');
    void native.openConnectionEditor().catch(error => { if (mounted.current) setPageMessage(String(error)); });
  }
  function closeDraft() {
    if (connecting) return;
    revision.current++;
    if (testInFlight.current) {
      testInFlight.current = false;
      void native.cancelConnectionTest().catch((error: unknown) => { if (mounted.current) setPageMessage(`Could not cancel connection test: ${String(error)}`); });
    }
    setTesting(false); clearDraft();
    void native.closeConnectionEditor().catch(error => { if (mounted.current) setDraftError(String(error)); });
  }
  function changed() {
    revision.current++; setTestResult(undefined); setDraftError('');
  }
  function profile(): HostProfile {
    return { id: draftId.current, target: target.trim(), port: port ? Number(port) : null, user: user.trim() || null, identity_agent: agent.trim() || null, environment };
  }
  async function testConnection() {
    if (busy || !form.current?.reportValidity()) return;
    const current = ++revision.current;
    const key = draftKey;
    setTesting(true); setDraftError(''); setTestResult(undefined); testInFlight.current = true;
    try {
      await native.testConnection(profile(), consent);
      if (mounted.current && revision.current === current) setTestResult({ key, success: true, message: 'Connection successful. No profile or active session was changed.' });
    } catch (error) {
      if (mounted.current && revision.current === current) setTestResult({ key, success: false, message: String(error) });
    } finally {
      if (revision.current === current) {
        testInFlight.current = false;
        if (mounted.current) setTesting(false);
      }
    }
  }
  async function connectDraft() {
    if (busy || !form.current?.reportValidity()) return;
    const current = ++revision.current;
    setConnecting(true); setDraftError('');
    try {
      await native.connect(profile(), consent);
      if (!mounted.current || revision.current !== current) return;
      await native.closeConnectionEditor();
    } catch (error) {
      if (mounted.current && revision.current === current) setDraftError(String(error));
    } finally { if (mounted.current && revision.current === current) setConnecting(false); }
  }
  function chooseSavedProfile(id: string) {
    changed();
    const host = saved.find(item => item.id === id);
    if (!host) { clearDraft(); targetInput.current?.focus(); return; }
    draftId.current = host.id; setSavedProfile(host.id);
    setTarget(host.target); setUser(host.user ?? ''); setPort(host.port?.toString() ?? '');
    setAgent(host.identity_agent ?? ''); setEnvironment(host.environment); setConsent(false);
  }
  function connect(host: HostProfile) {
    if (busy) return;
    const previousSelection = currentSelection.current;
    setConnecting(true); setPageMessage('');
    run(async () => {
      try {
        await native.connect(host, sessionConsent);
        if (!mounted.current) return;
        if (currentSelection.current === previousSelection) select(host.id);
        const profiles = await native.savedHosts();
        if (mounted.current) setSaved(profiles);
      } finally { if (mounted.current) { setConnecting(false); setSessionConsent(false); } }
    });
  }
  const currentTest = testResult?.key === draftKey ? testResult : undefined;

  if (standaloneEditor) return <section className="native-connection-editor" aria-label="Connect to host" onKeyDown={event => {
    if (event.key === 'Escape') { event.preventDefault(); closeDraft(); }
  }}>
      <p className="hint connection-draft-intro">Enter your SSH details and connect. You can test the connection first.</p>
      <form ref={form} onSubmit={event => { event.preventDefault(); void connectDraft(); }}>
        {saved.length > 0 && <label>Saved connection<Select disabled={busy} value={savedProfile} onChange={event => chooseSavedProfile(event.target.value)}><option value="">New connection</option>{saved.map(host => <option key={host.id} value={host.id}>{host.target} · {host.environment}</option>)}</Select></label>}
        <TextField label="SSH alias or hostname" ref={targetInput} required disabled={busy} value={target} onChange={event => { changed(); setTarget(event.target.value); }} placeholder="app-server" list="ssh-aliases" autoComplete="off"/>
        <datalist id="ssh-aliases">{config?.aliases.map(alias => <option key={alias} value={alias}/>)}</datalist>
        <div className="fields"><TextField label="User" disabled={busy} value={user} onChange={event => { changed(); setUser(event.target.value); }} placeholder="SSH config"/><TextField label="Port" disabled={busy} type="number" min="1" max="65535" value={port} onChange={event => { changed(); setPort(event.target.value); }} placeholder="SSH config"/></div>
        <label>Environment<Select disabled={busy} value={environment} onChange={event => { changed(); setEnvironment(event.target.value as HostProfile['environment']); }}><option value="development">Development</option><option value="staging">Staging</option><option value="production">Production</option></Select></label>
        <details className="connection-advanced"><summary>Advanced SSH options</summary><label>IdentityAgent socket <small>optional</small><input disabled={busy} value={agent} onChange={event => { changed(); setAgent(event.target.value); }} placeholder="Use configured SSH agent"/></label><label className="connection-consent"><input disabled={busy} type="checkbox" checked={consent} onChange={event => { changed(); setConsent(event.target.checked); }}/><span>Allow native SSH dynamic config for this connection or test<small>May run local commands from your SSH configuration. This permission is not saved with the profile.</small></span></label></details>
        <Button className="text-button" type="button" disabled={busy} onClick={() => {
          const current = revision.current;
          void native.importConfig().then(value => { if (mounted.current && revision.current === current) setConfig(value); }).catch(error => { if (mounted.current && revision.current === current) setDraftError(String(error)); });
        }}>Import static SSH aliases</Button>
        {config?.disclosures.map(disclosure => <p className="hint" key={disclosure}>{disclosure}</p>)}
        {testing && <p role="status" className="connection-test-progress">Testing SSH connection… Cancel closes the temporary connection.</p>}
        {currentTest && <p role={currentTest.success ? 'status' : 'alert'} className={`connection-test-result ${currentTest.success ? 'success' : 'failure'}`}>{currentTest.message}</p>}
        {pageMessage && <p role="status" className="notice">{pageMessage}</p>}
        {connecting && <p role="status" className="connection-test-progress">Starting SSH connection…</p>}
        {draftError && <p role="alert" className="notice">{draftError}</p>}
        <div className="connection-dialog-actions"><Button type="button" busy={testing} disabled={busy || !target.trim()} onClick={() => void testConnection()}>Test connection</Button><span/><Button type="button" disabled={connecting} onClick={closeDraft}>Cancel</Button><Button variant="primary" type="submit" busy={connecting} disabled={busy || !target.trim()}>{connecting ? 'Connecting…' : 'Connect'}</Button></div>
      </form>
  </section>;

  return <section className="connections-page" aria-label="SSH connections">
    <div className="section-heading connection-page-heading"><div><h2>SSH connections</h2><p className="subtle">Save a host profile, then connect when you need its logs.</p></div><Button variant="primary" disabled={busy} onClick={openDraft}>Add connection</Button></div>
    {pageMessage && <p role="status" className="notice">{pageMessage}</p>}
    <section className="page-card connection-list"><div className="section-heading"><h3>Current hosts</h3><span>{connections.length} hosts</span></div>
      {!connections.length && <p className="hint connection-empty">No active connections. Add a connection or choose a saved one.</p>}
      <div className="host-list">{connections.map(connection => <article key={connection.host.id} className={`connection-row ${connection.host.environment} ${selected === connection.host.id ? 'is-selected' : ''}`}>
        <Button className="connection-select" aria-pressed={selected === connection.host.id} aria-label={`${connection.host.target}, ${connection.host.environment}`} onClick={() => select(connection.host.id)}>
          <span className={`dot ${connection.state}`}/><span><strong>{connection.host.target}</strong><small>{connection.state.replaceAll('_', ' ')}{selected === connection.host.id ? ' · Selected' : ''}</small></span><span className={`badge ${connection.host.environment}`}>{connection.host.environment}</span>
        </Button>
        <div className="connection-row-actions"><label>Environment<Select disabled={busy} aria-label={`Environment for ${connection.host.target}`} value={connection.host.environment} onChange={event => run(() => native.setEnvironment(connection.host.id, event.target.value as HostProfile['environment']))}><option value="development">Development</option><option value="staging">Staging</option><option value="production">Production</option></Select></label><Button disabled={busy} onClick={() => run(() => native.disconnect(connection.host.id))}>Disconnect</Button></div>
        {connection.detail && <p className="hint connection-detail">{connection.detail}</p>}
      </article>)}</div>
    </section>
    {saved.length > 0 && <section className="page-card saved-profile-section"><div className="section-heading"><h3>Saved profiles</h3><span>{saved.length} profiles</span></div><div className="saved-profile-grid">{saved.map(host => <Button className="saved-profile" disabled={busy} key={host.id} onClick={() => connect(host)}><strong>{host.target}</strong><span className={`badge ${host.environment}`}>{host.environment}</span><small>{connecting ? 'Connecting…' : 'Connect →'}</small></Button>)}</div>
      <details className="saved-ssh-options"><summary>SSH session options</summary><label className="connection-consent"><input disabled={busy} type="checkbox" checked={sessionConsent} onChange={event => setSessionConsent(event.target.checked)}/><span>Allow native SSH dynamic config for the next connection<small>May run local commands from your SSH configuration. This permission is not stored in the profile.</small></span></label></details>
    </section>}
    <details className="connection-safety"><summary>SSH security</summary><p className="hint">Host keys must already be trusted in OpenSSH. TailPro uses your existing agent and never accepts a new or changed host key automatically. Production is a label, not a permission.</p></details>

  </section>;
}
