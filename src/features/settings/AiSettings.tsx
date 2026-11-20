import { useEffect, useState } from 'react';
import { native } from '../../shared/client/native';
import type { AppStatus, Enrollment, GrantInfo, ExplainProvider } from '../../shared/contracts/generated';
export function AiSettings({ status }: { status?: AppStatus }) {
  const [open, setOpen] = useState(false); const [grants, setGrants] = useState<GrantInfo[]>([]);
  const [client, setClient] = useState(''); const [scope, setScope] = useState<string[]>([]); const [consent, setConsent] = useState(false);
  const [enrollment, setEnrollment] = useState<Enrollment>(); const [message, setMessage] = useState('');
  const [kind, setKind] = useState<'disabled' | 'ollama' | 'byok'>('disabled');
  const [endpoint, setEndpoint] = useState('http://127.0.0.1:11434/api/chat'); const [model, setModel] = useState(''); const [key, setKey] = useState('');
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (!open) return;
    let alive = true;
    const refresh = () => native.grants().then(g => { if (alive) setGrants(g); }).catch(e => { if (alive) setMessage(String(e)); });
    void refresh();
    void native.explainProvider().then(p => { if (!alive) return; setKind(p?.kind ?? 'disabled'); if (p) { setEndpoint(p.endpoint); setModel(p.model); } }).catch(e => { if (alive) setMessage(String(e)); });
    const timer = setInterval(refresh, 2000); return () => { alive = false; clearInterval(timer); };
  }, [open]);
  async function run(action: () => Promise<unknown>) { setBusy(true); setMessage(''); try { await action(); setGrants(await native.grants()); } catch (e) { setMessage(String(e)); } finally { setBusy(false); } }
  const config = enrollment ? JSON.stringify({ mcpServers: { tailpro: { command: '/absolute/path/to/tailpro-mcp', env: { TAILPRO_RUNTIME_DIR: enrollment.runtime_dir, TAILPRO_SESSION: enrollment.session, TAILPRO_GRANT: enrollment.grant.id, TAILPRO_TOKEN: enrollment.token } } } }, null, 2) : '';
  return <section className="output-panel"><button onClick={() => { setOpen(!open); setEnrollment(undefined); }}>AI & MCP settings</button>{open && <div>
    <h3>Coding client access</h3><p>Only grant sources you intend to share. The external coding client and its model provider may receive these redacted logs. Production sources retain their label. Grants expire after one hour and on app restart or source configuration changes.</p>
    <label>Client name <input value={client} maxLength={80} onChange={e => setClient(e.target.value)}/></label>
    {status?.sources.map(s => { const host = status.connections.find(h => h.host.id === s.host_id); return <label key={s.id}><input type="checkbox" checked={scope.includes(s.id)} onChange={e => { setConsent(false); setScope(e.target.checked ? [...scope, s.id] : scope.filter(id => id !== s.id)); }}/>{host?.host.target} · {s.spec.kind === 'file' ? s.spec.path : s.spec.kind === 'journal' ? s.spec.unit : s.spec.container_id} · {host?.host.environment}</label>; })}
    <label><input type="checkbox" checked={consent} onChange={e => setConsent(e.target.checked)}/> I allow this client and its model provider to receive logs from these sources.</label>
    <button disabled={busy || !consent || !scope.length || !client.trim()} onClick={() => void run(async () => { setEnrollment(await native.enroll(client, scope, consent)); setConsent(false); })}>Grant selected sources</button>
    {enrollment && <><p className="hint">Enrollment secret shown once. Keep it in your client's private config and replace the executable path. Re-enroll after restart.</p><pre className="output-preview">{config}</pre><button onClick={() => void navigator.clipboard.writeText(config).then(() => setMessage('Client configuration copied.')).catch(() => setMessage('Clipboard unavailable.'))}>Copy client configuration</button><button onClick={() => setEnrollment(undefined)}>Hide secret</button></>}
    {grants.map(g => <p key={g.id}>{g.client} · {g.sources.length} sources · expires {new Date(Number(g.expires_at_ms)).toLocaleTimeString()} <button disabled={busy} onClick={() => void run(async () => { await native.revoke(g.id); if (enrollment?.grant.id === g.id) setEnrollment(undefined); })}>Revoke now</button></p>)}
    <p className="hint">Workflow: edit/build in your coding client → get_recent_logs(limit=0, query, sources) → reproduce → search_logs with that cursor and unchanged query/sources → inspect evidence. Gaps or no expected request mean insufficient evidence; logs do not verify deployment or a fix.</p>
    <h3>Explain Error · opt-in</h3><p>Ollama uses a locally installed model on literal loopback. BYOK sends the preview to your HTTPS provider directly. Requests do not pass through a TailPro server. Each explanation requires a separate preview and consent.</p>
    <label>Provider <select value={kind} onChange={e => { const k = e.target.value as typeof kind; setKind(k); setEndpoint(k === 'ollama' ? 'http://127.0.0.1:11434/api/chat' : ''); setModel(''); }}><option value="disabled">Disabled</option><option value="ollama">Local Ollama</option><option value="byok">BYOK cloud · OpenAI-compatible API</option></select></label>
    {kind !== 'disabled' && <><label>Exact endpoint <input value={endpoint} onChange={e => setEndpoint(e.target.value)}/></label><label>Installed model / provider model <input value={model} onChange={e => setModel(e.target.value)}/></label></>}
    <button disabled={busy} onClick={() => void run(async () => { const provider: ExplainProvider | null = kind === 'disabled' ? null : { kind, endpoint, model }; await native.configureExplainer(provider); setMessage('Provider setting applied for this app session.'); })}>Apply provider</button>
    {kind === 'byok' && <><label>API key for the applied provider <input type="password" autoComplete="off" value={key} onChange={e => setKey(e.target.value)}/></label><button disabled={busy || !key} onClick={() => void run(async () => { const value = key; setKey(''); await native.storeProviderKey(value); setMessage('Credential saved in OS Secret Service.'); })}>Save in OS Secret Service</button><p className="hint">BYOK is unavailable when the OS secret store cannot be used.</p></>}
    {message && <p role="status">{message}</p>}
  </div>}</section>;
}
