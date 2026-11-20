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
  const [saving, setSaving] = useState(false);
  const [consent, setConsent] = useState(false);
  const [sessionConsent, setSessionConsent] = useState(false);
  const [target, setTarget] = useState('');
  const [port, setPort] = useState('');
  const [user, setUser] = useState('');
  const [agent, setAgent] = useState('');
  const [environment, setEnvironment] = useState<HostProfile['environment']>('development');
  const [config, setConfig] = useState<ConfigImport>();
  const [saved, setSaved] = useState<HostProfile[]>([]);
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
  const busy = connecting || testing || saving;

  useEffect(() => {
    let alive = true;
    mounted.current = true;
    const loadProfiles = () => native.savedHosts().then(value => { if (alive) setSaved(value); }).catch(error => { if (alive) setPageMessage(String(error)); });
    let unlisten: (() => void) | undefined;
    if (!standaloneEditor) {
      void loadProfiles();
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
    setConsent(false); setTestResult(undefined); setDraftError('');
  }
  function openDraft() {
    setPageMessage('');
    void native.openConnectionEditor().catch(error => { if (mounted.current) setPageMessage(String(error)); });
  }
  function closeDraft() {
    revision.current++;
    if (testInFlight.current) {
      testInFlight.current = false;
      void native.cancelConnectionTest().catch((error: unknown) => { if (mounted.current) setPageMessage(`Could not cancel connection test: ${String(error)}`); });
    }
    setTesting(false); setSaving(false); clearDraft();
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
      if (mounted.current && revision.current === current) setTestResult({ key, success: true, message: 'Connection successful. Nothing has been saved yet.' });
    } catch (error) {
      if (mounted.current && revision.current === current) setTestResult({ key, success: false, message: String(error) });
    } finally {
      if (revision.current === current) {
        testInFlight.current = false;
        if (mounted.current) setTesting(false);
      }
    }
  }
  async function saveProfile() {
    if (busy || !form.current?.reportValidity()) return;
    const current = ++revision.current;
    setSaving(true); setDraftError('');
    try {
      await native.saveHostProfile(profile());
      const profiles = await native.savedHosts();
      if (!mounted.current) return;
      setSaved(profiles);
      if (revision.current === current) {
        if (standaloneEditor) await native.closeConnectionEditor();
        else { clearDraft(); setPageMessage('Connection profile saved. Use Connect when you want to connect to this host.'); }
      }
    } catch (error) {
      if (mounted.current && revision.current === current) setDraftError(String(error));
    } finally { if (mounted.current && revision.current === current) setSaving(false); }
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

  if (standaloneEditor) return <section className="native-connection-editor" aria-label="New connection" onKeyDown={event => {
    if (event.key === 'Escape') { event.preventDefault(); closeDraft(); }
  }}>
      <p className="hint connection-draft-intro">Enter your SSH details. You can test the connection before saving.</p>
      <form ref={form} onSubmit={event => { event.preventDefault(); void saveProfile(); }}>
        <label>SSH alias or hostname<input ref={targetInput} required disabled={testing || saving} value={target} onChange={event => { changed(); setTarget(event.target.value); }} placeholder="app-server" list="ssh-aliases" autoComplete="off"/></label>
        <datalist id="ssh-aliases">{config?.aliases.map(alias => <option key={alias} value={alias}/>)}</datalist>
        <div className="fields"><label>User<input disabled={testing || saving} value={user} onChange={event => { changed(); setUser(event.target.value); }} placeholder="SSH config"/></label><label>Port<input disabled={testing || saving} type="number" min="1" max="65535" value={port} onChange={event => { changed(); setPort(event.target.value); }} placeholder="SSH config"/></label></div>
        <label>Environment<select disabled={testing || saving} value={environment} onChange={event => { changed(); setEnvironment(event.target.value as HostProfile['environment']); }}><option value="development">Development</option><option value="staging">Staging</option><option value="production">Production</option></select></label>
        <details className="connection-advanced"><summary>Advanced SSH options</summary><label>IdentityAgent socket <small>optional</small><input disabled={testing || saving} value={agent} onChange={event => { changed(); setAgent(event.target.value); }} placeholder="Use configured SSH agent"/></label><label className="connection-consent"><input disabled={testing || saving} type="checkbox" checked={consent} onChange={event => { changed(); setConsent(event.target.checked); }}/><span>Allow native SSH dynamic config for this test<small>May run local commands from your SSH configuration. This permission is not saved with the profile.</small></span></label></details>
        <button className="text-button" type="button" disabled={testing || saving} onClick={() => {
          const current = revision.current;
          void native.importConfig().then(value => { if (mounted.current && revision.current === current) setConfig(value); }).catch(error => { if (mounted.current && revision.current === current) setDraftError(String(error)); });
        }}>Import static SSH aliases</button>
        {config?.disclosures.map(disclosure => <p className="hint" key={disclosure}>{disclosure}</p>)}
        {testing && <p role="status" className="connection-test-progress">Testing SSH connection… Cancel closes the temporary connection.</p>}
        {currentTest && <p role={currentTest.success ? 'status' : 'alert'} className={`connection-test-result ${currentTest.success ? 'success' : 'failure'}`}>{currentTest.success ? '✓ ' : ''}{currentTest.message}</p>}
        {draftError && <p role="alert" className="notice">{draftError}</p>}
        <div className="connection-dialog-actions"><button type="button" disabled={busy || !target.trim()} onClick={() => void testConnection()}>Test connection</button><span/><button type="button" onClick={closeDraft}>Cancel</button><button className="primary" type="submit" disabled={busy || !target.trim()}>{saving ? 'Saving…' : 'Save'}</button></div>
      </form>
  </section>;

  return <section className="connections-page" aria-label="SSH connections">
    <div className="section-heading connection-page-heading"><div><h2>SSH connections</h2><p className="subtle">Save a host profile, then connect when you need its logs.</p></div><button className="primary" disabled={busy} onClick={openDraft}>+ Add connection</button></div>
    {pageMessage && <p role="status" className="notice">{pageMessage}</p>}
    <section className="page-card connection-list"><div className="section-heading"><h3>Current hosts</h3><span>{connections.length} hosts</span></div>
      {!connections.length && <p className="hint connection-empty">No active connections. Add a connection or choose a saved one.</p>}
      <div className="host-list">{connections.map(connection => <article key={connection.host.id} className={`connection-row ${connection.host.environment} ${selected === connection.host.id ? 'is-selected' : ''}`}>
        <button className="connection-select" aria-pressed={selected === connection.host.id} aria-label={`${connection.host.target}, ${connection.host.environment}`} onClick={() => select(connection.host.id)}>
          <span className={`dot ${connection.state}`}/><span><strong>{connection.host.target}</strong><small>{connection.state.replaceAll('_', ' ')}{selected === connection.host.id ? ' · Selected' : ''}</small></span><span className={`badge ${connection.host.environment}`}>{connection.host.environment}</span>
        </button>
        <div className="connection-row-actions"><label>Environment<select disabled={busy} aria-label={`Environment for ${connection.host.target}`} value={connection.host.environment} onChange={event => run(() => native.setEnvironment(connection.host.id, event.target.value as HostProfile['environment']))}><option value="development">Development</option><option value="staging">Staging</option><option value="production">Production</option></select></label><button disabled={busy} onClick={() => run(() => native.disconnect(connection.host.id))}>Disconnect</button></div>
        {connection.detail && <p className="hint connection-detail">{connection.detail}</p>}
      </article>)}</div>
    </section>
    {saved.length > 0 && <section className="page-card saved-profile-section"><div className="section-heading"><h3>Saved profiles</h3><span>{saved.length} profiles</span></div><div className="saved-profile-grid">{saved.map(host => <button className="saved-profile" disabled={busy} key={host.id} onClick={() => connect(host)}><strong>{host.target}</strong><span className={`badge ${host.environment}`}>{host.environment}</span><small>{connecting ? 'Connecting…' : 'Connect →'}</small></button>)}</div>
      <details className="saved-ssh-options"><summary>SSH session options</summary><label className="connection-consent"><input disabled={busy} type="checkbox" checked={sessionConsent} onChange={event => setSessionConsent(event.target.checked)}/><span>Allow native SSH dynamic config for the next connection<small>May run local commands from your SSH configuration. This permission is not stored in the profile.</small></span></label></details>
    </section>}
    <details className="connection-safety"><summary>SSH security</summary><p className="hint">Host keys must already be trusted in OpenSSH. TailPro uses your existing agent and never accepts a new or changed host key automatically. Production is a label, not a permission.</p></details>

  </section>;
}
