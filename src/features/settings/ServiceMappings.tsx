import { Button, Select } from '../../shared/ui/Controls';
import { useEffect, useState } from 'react';
import type { AppStatus, ServiceMapping } from '../../shared/contracts/generated';
import { native } from '../../shared/client/native';
import './tools.css';

export function ServiceMappings({ status }: { status?: AppStatus }) {
  const [saved, setSaved] = useState<ServiceMapping[]>([]);
  const [editing, setEditing] = useState(false);
  const [source, setSource] = useState('');
  const [service, setService] = useState('');
  const [project, setProject] = useState('');
  const [aliases, setAliases] = useState('');
  const [label, setLabel] = useState('');
  const [root, setRoot] = useState('');
  const [revision, setRevision] = useState('');
  const [error, setError] = useState('');
  const [message, setMessage] = useState('');
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  useEffect(() => {
    let alive = true;
    setLoading(true);
    native.serviceMappings().then(result => { if (alive) setSaved(result); }).catch(e => { if (alive) setError(String(e)); }).finally(() => { if (alive) setLoading(false); });
    return () => { alive = false; };
  }, [status?.health.policy_revision]);
  function edit(mapping?: ServiceMapping) {
    setSource(mapping?.source ?? ''); setService(mapping?.service ?? ''); setProject(mapping?.project ?? '');
    setAliases(mapping?.aliases.join(', ') ?? ''); setLabel(mapping?.repo_label ?? '');
    setRoot(mapping?.repo_root ?? ''); setRevision(mapping?.revision ?? ''); setError(''); setMessage(''); setEditing(true);
  }
  async function save(e: React.FormEvent) {
    e.preventDefault(); setError(''); setMessage(''); setBusy(true);
    try {
      await native.saveServiceMapping({ source, service, project, aliases: aliases.split(',').map(s => s.trim()).filter(Boolean), repo_label: label, repo_root: root, revision: revision || null });
      setSaved(await native.serviceMappings()); setEditing(false); setMessage('Mapping saved. Existing related grants and contexts have been revoked.');
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }
  async function remove(id: string) {
    setBusy(true); setError(''); setMessage('');
    try { await native.deleteServiceMapping(id); setSaved(await native.serviceMappings()); setMessage('Mapping removed. Existing related grants and contexts have been revoked.'); }
    catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }
  const sourceLabel = (id: string) => {
    const item = status?.sources.find(s => s.id === id);
    if (!item) return id;
    const host = status?.connections.find(c => c.host.id === item.host_id);
    return `${host?.host.target ?? item.host_id} · ${item.spec.kind === 'file' ? item.spec.path : item.spec.kind === 'journal' ? item.spec.unit : item.spec.container_id}`;
  };
  return <section className="settings-section" aria-label="Service and local code mappings">
    <div className="tool-heading"><div><h2>Service mappings</h2><p className="subtle">Give log sources a service name and a declared local checkout.</p></div>{!editing && <Button variant="primary" disabled={busy} onClick={() => edit()}>Add mapping</Button>}</div>
    {error && <p role="alert" className="notice">{error}</p>}{message && <p role="status" className="notice">{message}</p>}
    {editing ? <section className="tool-card mapping-form"><h3>{saved.some(m => m.source === source) ? 'Edit mapping' : 'New mapping'}</h3><p className="hint">Saving changes revokes existing related agent grants and contexts. These are declarations; TailPro does not open or index your repository.</p>
      <form onSubmit={save}><fieldset disabled={busy} className="tool-fieldset">
        <label>Source<Select value={source} onChange={e => setSource(e.target.value)} required><option value="">Choose source</option>{source && !status?.sources.some(s => s.id === source) && <option value={source}>{source} · unavailable</option>}{status?.sources.map(s => <option key={s.id} value={s.id}>{sourceLabel(s.id)}</option>)}</Select></label>
        <div className="fields"><label>Service<input value={service} onChange={e => setService(e.target.value)} maxLength={128} placeholder="checkout-api" required/></label><label>Project<input value={project} onChange={e => setProject(e.target.value)} maxLength={128} placeholder="shop"/></label></div>
        <label>Aliases, comma separated<input value={aliases} onChange={e => setAliases(e.target.value)} maxLength={2048} placeholder="api, checkout"/></label>
        <label>Repository label<input value={label} onChange={e => setLabel(e.target.value)} maxLength={128} placeholder="shop-api" required/></label>
        <label>Declared local absolute root<input value={root} onChange={e => setRoot(e.target.value)} maxLength={4096} pattern="/.*" placeholder="/home/user/projects/shop-api" required/></label>
        <label>Declared revision · optional, unverified<input value={revision} onChange={e => setRevision(e.target.value)} maxLength={128}/></label>
        <div className="mapping-actions"><Button type="submit" variant="primary" disabled={!source || busy}>{busy ? 'Saving…' : 'Save mapping'}</Button><Button type="button" disabled={busy} onClick={() => setEditing(false)}>Cancel</Button></div>
      </fieldset></form>
    </section> : <div className="mapping-list">{loading ? <p className="hint" role="status">Loading mappings…</p> : !saved.length ? <div className="mapping-empty"><h3>No service mappings yet</h3><p>Add a mapping to relate a source to a service and local code hints.</p><Button disabled={busy} onClick={() => edit()}>Add your first mapping</Button></div> : saved.map(m => <article className="mapping-item" key={m.source}><div><strong>{m.service}</strong><p>{sourceLabel(m.source)}</p><p>{m.project || 'No project'} · {m.repo_label} · revision {m.revision ?? 'unknown'}</p><small>{m.repo_root}</small>{m.aliases.length > 0 && <p>Aliases: {m.aliases.join(', ')}</p>}</div><div className="actions"><Button disabled={busy} onClick={() => edit(m)}>Edit</Button><Button disabled={busy} onClick={() => void remove(m.source)}>Remove</Button></div></article>)}</div>}
    <p className="hint">Stack hints are relative paths. Your coding client must verify the checkout and deployed revision; a mapping does not prove what is running.</p>
  </section>;
}
