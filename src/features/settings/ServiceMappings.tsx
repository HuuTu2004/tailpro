import { useEffect, useState } from 'react';
import type { AppStatus, ServiceMapping } from '../../shared/contracts/generated';
import { native } from '../../shared/client/native';
export function ServiceMappings({ status }: { status?: AppStatus }) {
  const [saved, setSaved] = useState<ServiceMapping[]>([]);
  const [source, setSource] = useState('');
  const [service, setService] = useState('');
  const [project, setProject] = useState('');
  const [aliases, setAliases] = useState('');
  const [label, setLabel] = useState('');
  const [root, setRoot] = useState('');
  const [revision, setRevision] = useState('');
  const [error, setError] = useState('');
  useEffect(() => { native.serviceMappings().then(setSaved).catch(e => setError(String(e))); }, []);
  async function save(e: React.FormEvent) {
    e.preventDefault(); setError('');
    try {
      await native.saveServiceMapping({ source, service, project, aliases: aliases.split(',').map(s => s.trim()).filter(Boolean), repo_label: label, repo_root: root, revision: revision || null });
      setSaved(await native.serviceMappings());
    } catch (e) { setError(String(e)); }
  }
  return <details className="output-panel"><summary>Service → local code mappings</summary>
    <p className="hint">Declare aliases and a checkout label/root. TailPro stores this declaration without opening or indexing the repository. Hints are relative stack paths; the coding client must verify checkout and deployed revision. Saving a mapping revokes existing agent grants and contexts.</p>
    {error && <p role="alert">{error}</p>}
    <form onSubmit={save}><label>Selected source<select value={source} onChange={e => setSource(e.target.value)} required><option value="">Choose source</option>{status?.sources.map(s => <option key={s.id} value={s.id}>{s.id} · {s.host_id}</option>)}</select></label>
      <div className="fields"><label>Service<input value={service} onChange={e => setService(e.target.value)} maxLength={128} required/></label><label>Project<input value={project} onChange={e => setProject(e.target.value)} maxLength={128}/></label></div>
      <label>Aliases, comma separated<input value={aliases} onChange={e => setAliases(e.target.value)} maxLength={2048}/></label>
      <div className="fields"><label>Repository label<input value={label} onChange={e => setLabel(e.target.value)} maxLength={128} required/></label><label>Declared local absolute root<input value={root} onChange={e => setRoot(e.target.value)} maxLength={4096} required/></label></div>
      <label>Declared revision (optional, unverified)<input value={revision} onChange={e => setRevision(e.target.value)} maxLength={128}/></label><button disabled={!source}>Save mapping</button>
    </form>
    {saved.map(m => <article className="candidate" key={m.source}><span>{m.source} → {m.service} · {m.project} · {m.repo_label} · revision {m.revision ?? 'unknown'}</span><button onClick={() => { void native.deleteServiceMapping(m.source).then(() => native.serviceMappings()).then(setSaved).catch(e => setError(String(e))); }}>Remove</button></article>)}
  </details>;
}
