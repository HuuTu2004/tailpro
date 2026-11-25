import { Button, Select } from '../../shared/ui/Controls';
import { useEffect, useRef, useState } from 'react';
import { native } from '../../shared/client/native';
import type { AppStatus, Enrollment, GrantInfo, ExplainProvider } from '../../shared/contracts/generated';
import './tools.css';

type Section = 'provider' | 'access';
type Props = { status?: AppStatus; section?: Section };

export function AiSettings({ status, section = 'access' }: Props) {
  const [grants, setGrants] = useState<GrantInfo[]>([]);
  const [client, setClient] = useState('');
  const [scope, setScope] = useState<string[]>([]);
  const [consent, setConsent] = useState(false);
  const [enrollment, setEnrollment] = useState<Enrollment>();
  const [message, setMessage] = useState('');
  const [kind, setKind] = useState<'disabled' | 'ollama' | 'byok'>('disabled');
  const [endpoint, setEndpoint] = useState('http://127.0.0.1:11434/api/chat');
  const [model, setModel] = useState('');
  const [key, setKey] = useState('');
  const [applied, setApplied] = useState<ExplainProvider | null>(null);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const alive = useRef(false);
  const sourceRevision = status?.sources.map(s => `${s.id}:${s.generation}`).join('|');
  const policyRevision = status?.health.policy_revision;

  useEffect(() => {
    alive.current = true;
    setLoading(true);
    const refresh = async () => {
      try {
        const result = await native.grants();
        if (!alive.current) return;
        setGrants(result);
        setEnrollment(current => current && !result.some(g => g.id === current.grant.id) ? undefined : current);
      } catch (e) { if (alive.current) setMessage(String(e)); }
    };
    void (async () => {
      try {
        if (section === 'access') await refresh();
        else {
          const provider = await native.explainProvider();
          if (!alive.current) return;
          setApplied(provider ?? null);
          setKind(provider?.kind ?? 'disabled');
          if (provider) { setEndpoint(provider.endpoint); setModel(provider.model); }
        }
      } catch (e) { if (alive.current) setMessage(String(e)); }
      finally { if (alive.current) setLoading(false); }
    })();
    const timer = section === 'access' ? setInterval(() => void refresh(), 2000) : undefined;
    return () => { alive.current = false; clearInterval(timer); };
  }, [section]);
  useEffect(() => {
    setConsent(false); setEnrollment(undefined);
    setScope(current => current.filter(id => status?.sources.some(source => source.id === id)));
  }, [sourceRevision, policyRevision]);

  async function run(action: () => Promise<unknown>) {
    setBusy(true); setMessage('');
    try { await action(); }
    catch (e) { if (alive.current) setMessage(String(e)); }
    finally { if (alive.current) setBusy(false); }
  }
  const config = enrollment ? JSON.stringify({ mcpServers: { tailpro: { command: '/absolute/path/to/tailpro-mcp', env: { TAILPRO_RUNTIME_DIR: enrollment.runtime_dir, TAILPRO_SESSION: enrollment.session, TAILPRO_GRANT: enrollment.grant.id, TAILPRO_TOKEN: enrollment.token } } } }, null, 2) : '';
  const providerApplied = applied?.kind === kind && applied.endpoint === endpoint && applied.model === model;

  if (section === 'access') return <section className="settings-section" aria-label="Coding client access">
    <div className="tool-heading"><div><h2>Coding client access</h2><p className="subtle">Connect a coding client to only the sources you choose.</p></div><span className="badge">{grants.length} active grants</span></div>
    {message && <p className="notice" role="status">{message}</p>}
    <section className="tool-card">
      <h3>Grant source access</h3>
      <p className="hint">Your coding client and its model provider may receive these redacted logs. Grants expire after one hour, app restart or source configuration changes.</p>
      <label>Client name<input value={client} maxLength={80} placeholder="e.g. My coding client" disabled={busy || loading} onChange={e => { setClient(e.target.value); setConsent(false); }}/></label>
      <fieldset className="settings-source-list" disabled={busy || loading}><legend>Sources to share</legend>
        {!status?.sources.length && <p className="hint">Start a log source before granting access.</p>}
        {status?.sources.map(s => {
          const host = status.connections.find(h => h.host.id === s.host_id);
          return <label className="settings-check" key={s.id}><input type="checkbox" checked={scope.includes(s.id)} onChange={e => { setConsent(false); setEnrollment(undefined); setScope(e.target.checked ? [...scope, s.id] : scope.filter(id => id !== s.id)); }}/><span><strong>{s.spec.kind === 'file' ? s.spec.path : s.spec.kind === 'journal' ? s.spec.unit : s.spec.container_id}</strong><small>{host?.host.target} · {host?.host.environment}</small></span></label>;
        })}
      </fieldset>
      <label className="settings-check consent-row"><input type="checkbox" checked={consent} disabled={busy || loading} onChange={e => setConsent(e.target.checked)}/><span>I allow this coding client and its model provider to receive redacted logs from these sources.</span></label>
      <Button variant="primary" disabled={busy || loading || !consent || !scope.length || !client.trim() || scope.some(id => !status?.sources.some(s => s.id === id))} onClick={() => void run(async () => {
        const result = await native.enroll(client.trim(), scope, consent);
        if (!alive.current) return;
        setEnrollment(result); setConsent(false);
        const updated = await native.grants();
        if (alive.current) setGrants(updated);
      })}>{busy ? 'Working…' : 'Grant selected sources'}</Button>
    </section>
    {enrollment && <section className="tool-card enrollment-card"><div className="section-heading"><h3>One-time client configuration</h3><Button onClick={() => setEnrollment(undefined)}>Hide secret</Button></div><p className="hint">Keep this secret in your client's private config and replace the executable path. It is hidden when you leave this page. Re-enroll after restart.</p><pre className="output-preview">{config}</pre><Button onClick={() => void navigator.clipboard.writeText(config).then(() => { if (alive.current) setMessage('Client configuration copied.'); }).catch(() => { if (alive.current) setMessage('Clipboard unavailable. Select and copy the configuration.'); })}>Copy client configuration</Button></section>}
    <section className="tool-card"><h3>Active grants</h3>{loading ? <p className="hint" role="status">Loading grants…</p> : !grants.length ? <p className="hint">No active grants. Your sources have not been shared with a coding client.</p> : grants.map(g => <article className="grant-row" key={g.id}><div><strong>{g.client}</strong><p className="hint">{g.sources.length} sources · expires {new Date(Number(g.expires_at_ms)).toLocaleTimeString()}</p></div><Button disabled={busy} onClick={() => void run(async () => { await native.revoke(g.id); if (!alive.current) return; if (enrollment?.grant.id === g.id) setEnrollment(undefined); const updated = await native.grants(); if (alive.current) { setGrants(updated); setMessage('Client access revoked.'); } })}>Revoke access</Button></article>)}</section>
    <details className="tool-help"><summary>How to verify a change with MCP</summary><p className="hint">Edit/build in your coding client → get_recent_logs(limit=0, query, sources) → reproduce → search_logs with that cursor and unchanged query/sources → inspect evidence. Gaps or no expected request mean insufficient evidence; logs do not verify deployment or a fix.</p></details>
  </section>;

  return <section className="settings-section" aria-label="Explain Error provider">
    <div className="tool-heading"><div><h2>Explain Error</h2><p className="subtle">Optional explanations using your own local model or provider.</p></div><span className="badge">{applied ? applied.kind === 'ollama' ? 'Local Ollama' : 'BYOK cloud' : 'Disabled'}</span></div>
    {message && <p className="notice" role="status">{message}</p>}
    <section className="tool-card provider-card"><h3>Model provider</h3><p className="hint">Each explanation requires an exact redacted preview and separate consent. Requests go directly to the chosen endpoint, without a TailPro server or automatic cloud fallback.</p>
      <label>Provider<Select value={kind} disabled={busy || loading} onChange={e => { const next = e.target.value as typeof kind; setKind(next); setEndpoint(next === 'ollama' ? 'http://127.0.0.1:11434/api/chat' : ''); setModel(''); setKey(''); }}><option value="disabled">Disabled</option><option value="ollama">Local Ollama</option><option value="byok">BYOK cloud · OpenAI-compatible API</option></Select></label>
      {kind !== 'disabled' && <><label>Exact endpoint<input value={endpoint} disabled={busy || loading} placeholder={kind === 'ollama' ? 'http://127.0.0.1:11434/api/chat' : 'https://provider.example/v1/chat/completions'} onChange={e => { setEndpoint(e.target.value); setKey(''); }}/></label><label>{kind === 'ollama' ? 'Installed local model' : 'Provider model'}<input value={model} disabled={busy || loading} onChange={e => { setModel(e.target.value); setKey(''); }}/></label><p className="hint">{kind === 'ollama' ? 'Use an already installed model on a literal loopback address. TailPro does not install models.' : 'Use an HTTPS endpoint. The redacted preview leaves your machine when you approve an explanation.'}</p></>}
      <Button variant="primary" disabled={busy || loading || (kind !== 'disabled' && (!endpoint.trim() || !model.trim()))} onClick={() => void run(async () => {
        const provider: ExplainProvider | null = kind === 'disabled' ? null : { kind, endpoint, model };
        setKey(''); await native.configureExplainer(provider);
        if (alive.current) { setApplied(provider); setMessage('Provider applied for this app session.'); }
      })}>{busy ? 'Working…' : 'Apply provider'}</Button>
    </section>
    {kind === 'byok' && <section className="tool-card provider-card"><h3>Provider credential</h3><p className="hint">Apply the provider above before saving its API key. Credentials use OS Secret Service; BYOK is unavailable if the secret store cannot be used.</p><label>API key<input type="password" autoComplete="off" value={key} disabled={busy || loading || !providerApplied} onChange={e => setKey(e.target.value)}/></label><Button disabled={busy || loading || !key || !providerApplied} onClick={() => void run(async () => { const value = key; setKey(''); const current = await native.explainProvider(); if (!current || current.kind !== 'byok' || current.endpoint !== endpoint || current.model !== model) throw new Error('The applied provider changed. Apply it again before storing a credential.'); await native.storeProviderKey(value); if (alive.current) setMessage('Credential saved in OS Secret Service.'); })}>Save in OS Secret Service</Button></section>}
  </section>;
}
