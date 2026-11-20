import { useEffect, useRef, useState } from 'react';
import type { Candidate, ConnectionInfo, DiscoveryResult, SourceInfo } from '../../shared/contracts/generated';
import { native } from '../../shared/client/native';
type Props = { host?: ConnectionInfo; sources: SourceInfo[]; requestedPath?: { value: string; revision: number }; run: (action: () => Promise<unknown>) => void };
export function Sources({ host, sources, requestedPath, run }: Props) {
  const [path, setPath] = useState('');
  const pathInput = useRef<HTMLInputElement>(null);
  useEffect(() => { if (requestedPath) { setPath(requestedPath.value); requestAnimationFrame(() => pathInput.current?.focus()); } }, [requestedPath]);
  const [root, setRoot] = useState('');
  const [pm2, setPm2] = useState('');
  const [docker, setDocker] = useState(true);
  const [varLog, setVarLog] = useState(true);
  const [consent, setConsent] = useState(false);
  const [configs, setConfigs] = useState('');
  const [configRoot, setConfigRoot] = useState('');
  const [pids, setPids] = useState('');
  const [units, setUnits] = useState('');
  const [result, setResult] = useState<DiscoveryResult>();
  const [suggestions, setSuggestions] = useState<Candidate[]>([]);
  const [busy, setBusy] = useState(false);
  const version = useRef(0);
  const discoveryVersion = useRef(0);
  const [pins, setPins] = useState<string[]>([]);
  useEffect(() => { setResult(undefined); setSuggestions([]); setPath(''); version.current++; discoveryVersion.current++;
    if (host) native.pins(host.host.id).then(setPins).catch(() => setPins([]));
    return () => { if (host) void native.cancelDiscovery(host.host.id).catch(() => {}); }; }, [host?.host.id]);
  useEffect(() => {
    const current = ++version.current;
    const index = path.lastIndexOf('/');
    if (!host || index < 0) { setSuggestions([]); return; }
    const hostId = host.host.id;
    const timer = setTimeout(() => {
      native.complete(hostId, path.slice(0, index) || '/', path.slice(index + 1)).then(r => { if (current === version.current) setSuggestions(r.candidates); }).catch(() => { if (current === version.current) setSuggestions([]); });
    }, 200);
    return () => clearTimeout(timer);
  }, [path, host?.host.id]);
  async function discover() {
    if (!host) return;
    setBusy(true);
    const revision = ++discoveryVersion.current;
    try {
      const found = await native.discover({ host_id: host.host.id, project_roots: root ? [root] : [], docker, var_log: varLog, pm2_root: pm2 || null, selected_pids: pids.split(',').filter(Boolean).map(Number), selected_units: units.split(',').map(x => x.trim()).filter(Boolean), config_consent: consent, config_files: consent ? configs.split('\n').filter(Boolean) : [], config_roots: consent && configRoot ? [configRoot] : [] });
      if (revision === discoveryVersion.current) setResult(found);
    } finally { setBusy(false); }
  }
  function start(candidate: Candidate) {
    if (!host) return;
    run(() => native.start(host.host.id, candidate.provider === 'systemd_unit' ? { kind: 'journal', unit: candidate.path } : candidate.container_id ? { kind: 'docker', container_id: candidate.container_id, aliases: [candidate.path, ...candidate.tags].slice(0, 16).map(s => s.slice(0,128)) } : { kind: 'file', path: candidate.path }));
  }
  return <section className={`sources ${host?.host.environment ?? ''}`} aria-label={`Sources ${host?.host.environment ?? ''}`}>
    <div className="section-heading"><div><span className="eyebrow">SOURCE WORKSPACE</span><h1>{host?.host.target ?? 'Choose a connection'}</h1></div>{host?.host.environment === 'production' && <span className="badge production">Production</span>}</div>
    <p className="subtle">Discover metadata, choose a source, then start reading logs.</p>
    <form className="path-form" onSubmit={e => { e.preventDefault(); if (host) run(() => native.start(host.host.id, { kind: 'file', path })); }}>
      <label className="grow">Absolute log path<input ref={pathInput} value={path} onChange={e => setPath(e.target.value)} placeholder="/var/log/nginx/error.log" list="paths" required /></label>
      <datalist id="paths">{suggestions.map(c => <option key={c.id} value={c.path}/>)}</datalist>
      <button className="primary" disabled={!host || host.state !== 'connected'}>Start file</button>
    </form>
    <details className="discovery-options"><summary>Discovery scope & permissions</summary>
      <div className="check-row"><label><input type="checkbox" checked={docker} onChange={e => setDocker(e.target.checked)}/> Docker / Compose metadata</label><label><input type="checkbox" checked={varLog} onChange={e => setVarLog(e.target.checked)}/> /var/log, depth 2</label></div>
      <div className="fields"><label>Project root<input value={root} onChange={e => setRoot(e.target.value)} placeholder="/srv/my-app"/></label><label>PM2 logs directory<input value={pm2} onChange={e => setPm2(e.target.value)} placeholder="/home/app/.pm2/logs"/></label></div>
      <div className="fields"><label>Selected PIDs (comma separated)<input value={pids} onChange={e => setPids(e.target.value)}/></label><label>Selected systemd units<input value={units} onChange={e => setUnits(e.target.value)}/></label></div>
      <label className="check-row"><input type="checkbox" checked={consent} onChange={e => setConsent(e.target.checked)}/> Allow reading these config files to extract path hints</label>
      {consent && <><label>Approved config root<input value={configRoot} onChange={e => setConfigRoot(e.target.value)}/></label><label>Exact Nginx / Caddy / Supervisor config files, one per line<textarea value={configs} onChange={e => setConfigs(e.target.value)}/></label></>}
      <p className="hint">Discovery reads metadata. Start grants log reading for that source. Config reading is separate. External agent access requires a separate source grant. Production is a label; it grants no permission.</p>
    </details>
    {pins.length > 0 && <div className="pins"><span className="hint">Pinned paths · select to recheck on Start</span>{pins.map(p => <button key={p} onClick={() => setPath(p)}>{p}</button>)}</div>}
    <div className="section-heading"><h2>Available sources</h2>{busy && host && <button onClick={() => run(async () => { discoveryVersion.current++; await native.cancelDiscovery(host.host.id); setBusy(false); })}>Cancel</button>}<button disabled={!host || host.state !== 'connected' || busy} onClick={() => run(discover)}>{busy ? 'Discovering…' : 'Discover sources'}</button></div>
    {result && !result.complete && <p className="notice">Partial metadata: {result.reasons.join(' · ') || 'Discovery did not finish'}</p>}
    <div className="candidate-list">{result?.candidates.map(c => <article className="candidate" key={c.id}><div><strong>{c.path}</strong><p>{c.provider} · {c.access} · {c.activity}</p><small>{c.provenance}</small><input className="tag-input" aria-label={`Tags for ${c.path}`} placeholder="Tags (hints only)" value={c.tags.join(', ')} maxLength={255} onChange={e => setResult(previous => previous ? {...previous, candidates: previous.candidates.map(item => item.id === c.id ? {...item, tags: e.target.value.split(',').slice(0, 8).map(tag => tag.trim().slice(0, 32))} : item)} : previous)}/></div><button disabled={c.access !== 'ready'} onClick={() => start(c)}>Start</button></article>)}
      {!result && <div className="empty"><span>⌁</span><h3>Your sources, in one place</h3><p>Connect a host to discover containers and log files.<br/>You can also enter a file path directly.</p></div>}
      {result?.candidates.length === 0 && <p className="subtle">No matching sources in this scope.</p>}
    </div>
    <h2>Reading sources <small>{sources.length} / 8</small></h2>
    {sources.map(s => <article className="candidate" key={s.id}><div><strong>{s.spec.kind === 'file' ? s.spec.path : s.spec.kind === 'journal' ? s.spec.unit : s.spec.container_id}</strong><p>{s.state.replaceAll('_',' ')}</p>{s.detail && <small>{s.detail}</small>}</div><div className="actions">{s.spec.kind === 'file' && <button onClick={() => run(async () => { await native.pin(s.host_id, s.spec.kind === 'file' ? s.spec.path : ''); setPins(await native.pins(s.host_id)); })}>Pin</button>}<button onClick={() => run(() => native.stop(s.id))}>Stop</button></div></article>)}
  </section>;
}
