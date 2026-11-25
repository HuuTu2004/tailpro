import { useEffect, useRef, useState } from 'react';
import type { Candidate, ConnectionInfo, DiscoveryResult, SourceInfo, SourceSpec } from '../../shared/contracts/generated';
import { native } from '../../shared/client/native';
import './sources.css';
import { ParserSettings } from './ParserSettings';
import { KubernetesSource } from './KubernetesSource';
type Props = { host?: ConnectionInfo; sources: SourceInfo[]; requestedPath?: { value: string; revision: number; mode?: 'file' | 'discover' }; run: (action: () => Promise<unknown>) => void };
export function Sources({ host, sources, requestedPath, run }: Props) {
  const [diagnostics, setDiagnostics] = useState('');
  const [composerOpen, setComposerOpen] = useState(sources.length === 0);
  const [mode, setMode] = useState<'file' | 'discover'>('file');
  const [pending, setPending] = useState('');
  const [path, setPath] = useState('');
  const pathInput = useRef<HTMLInputElement>(null);
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
  const hostRevision = useRef(0);
  const [pins, setPins] = useState<string[]>([]);
  useEffect(() => {
    let alive = true;
    hostRevision.current++;
    setResult(undefined); setSuggestions([]); setPath(''); setPins([]); setBusy(false); setPending(''); setConsent(false); setConfigs(''); setConfigRoot(''); setRoot(''); setPm2(''); setPids(''); setUnits(''); version.current++; discoveryVersion.current++;
    if (host) native.pins(host.host.id).then(value => { if (alive) setPins(value); }).catch(() => { if (alive) setPins([]); });
    return () => { alive = false; hostRevision.current++; version.current++; discoveryVersion.current++; if (host) void native.cancelDiscovery(host.host.id).catch(() => {}); };
  }, [host?.host.id]);
  useEffect(() => { if (requestedPath) { setComposerOpen(true); setMode(requestedPath.mode ?? 'file'); setPath(requestedPath.value); if (requestedPath.mode !== 'discover') requestAnimationFrame(() => pathInput.current?.focus()); } }, [requestedPath]);
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
    } finally { if (revision === discoveryVersion.current) setBusy(false); }
  }
  function cancelDiscovery() {
    if (!host) return;
    const hostId = host.host.id;
    const revision = ++discoveryVersion.current;
    run(async () => {
      try { await native.cancelDiscovery(hostId); }
      finally { if (revision === discoveryVersion.current) setBusy(false); }
    });
  }
  function startSource(spec: SourceSpec, label: string) {
    if (!host || host.state !== 'connected' || pending) return;
    const revision = hostRevision.current;
    setPending(label);
    run(async () => { try { return await native.start(host.host.id, spec); } finally { if (revision === hostRevision.current) setPending(''); } });
  }
  function pinSource(source: SourceInfo) {
    if (source.spec.kind !== 'file') return;
    const revision = hostRevision.current;
    const sourcePath = source.spec.path;
    run(async () => {
      await native.pin(source.host_id, sourcePath);
      const paths = await native.pins(source.host_id);
      if (revision === hostRevision.current) setPins(paths);
    });
  }
  function start(candidate: Candidate) {
    if (!host) return;
    startSource(candidate.provider === 'systemd_unit' ? { kind: 'journal', unit: candidate.path } : candidate.container_id ? { kind: 'docker', container_id: candidate.container_id, aliases: [candidate.path, ...candidate.tags].slice(0, 16).map(s => s.slice(0,128)) } : { kind: 'file', path: candidate.path }, candidate.id);
  }
  return <section className={`sources-page ${host?.host.environment ?? ''}`} aria-label={`Sources ${host?.host.environment ?? ''}`}>
    <div className="section-heading sources-page-heading"><div><h2>{host?.host.target ?? 'Choose a connection'}</h2><p className="subtle">Manage reading sources, or add a file, Docker container or journal.</p></div><div className="actions">{host?.host.environment === 'production' && <span className="badge production">Production</span>}<button className="primary" disabled={!host} aria-expanded={composerOpen} aria-controls="source-composer" onClick={() => setComposerOpen(open => !open)}>{composerOpen ? 'Hide add source' : '+ Add source'}</button></div></div>
    {!host && <p className="notice">Choose a host from Connections before adding sources.</p>}
    {host && host.state !== 'connected' && <p className="notice">{host.host.target} is {host.state.replaceAll('_', ' ')}. Connect this host before starting or discovering sources.</p>}
    <button onClick={() => run(async () => setDiagnostics(JSON.stringify(JSON.parse(await native.ingestionDiagnostics()), null, 2)))}>Refresh ingestion diagnostics</button>{diagnostics && <details><summary>Local counters & coverage</summary><pre className="raw-text">{diagnostics}</pre></details>}
    <section className="page-card reading-source-section"><div className="section-heading"><h3>Reading sources</h3><span>{sources.length} / 8</span></div>
    {!sources.length && <p className="subtle">No sources are reading on this host. Add a source to get started.</p>}
    {sources.map(s => <article className="candidate" key={s.id}><div><strong>{s.spec.kind === 'file' ? s.spec.path : s.spec.kind === 'journal' ? s.spec.unit : s.spec.kind === 'kubernetes' ? `${s.spec.namespace}/${s.spec.pod}/${s.spec.container}` : s.spec.container_id}</strong><p><span className={`source-kind source-kind-${s.spec.kind}`}>{s.spec.kind === 'journal' ? 'Journal' : s.spec.kind === 'docker' ? 'Docker' : s.spec.kind === 'kubernetes' ? 'Kubernetes' : 'File'}</span> · {s.state.replaceAll('_',' ')}</p>{s.detail && <small>{s.detail}</small>}<ParserSettings source={s.id}/></div><div className="actions">{s.spec.kind === 'file' && <button onClick={() => pinSource(s)}>Pin path</button>}<button onClick={() => run(() => native.stop(s.id))}>Stop</button></div></article>)}
    </section>
    {host && <KubernetesSource key={host.host.id} hostId={host.host.id} disabled={host.state !== 'connected' || Boolean(pending) || sources.length >= 8} start={startSource}/>}
    {composerOpen && <section className="page-card source-composer" id="source-composer"><div className="section-heading"><h3>Add a source</h3><div className="source-mode" role="tablist" aria-label="Add source method" onKeyDown={event => { if (event.key === 'ArrowLeft' || event.key === 'ArrowRight' || event.key === 'Home' || event.key === 'End') { event.preventDefault(); const next = event.key === 'Home' ? 'file' : event.key === 'End' ? 'discover' : mode === 'file' ? 'discover' : 'file'; setMode(next); document.getElementById(`source-${next}-tab`)?.focus(); } }}><button id="source-file-tab" role="tab" tabIndex={mode === 'file' ? 0 : -1} aria-selected={mode === 'file'} aria-controls="source-file-panel" onClick={() => setMode('file')}>File path</button><button id="source-discover-tab" role="tab" tabIndex={mode === 'discover' ? 0 : -1} aria-selected={mode === 'discover'} aria-controls="source-discover-panel" onClick={() => setMode('discover')}>Discover sources</button></div></div>
    <div id="source-file-panel" role="tabpanel" aria-labelledby="source-file-tab" hidden={mode !== 'file'}>
    <p className="subtle">Enter an absolute path. Reading starts only when you press Start file.</p>
    <form className="path-form" onSubmit={e => { e.preventDefault(); startSource({ kind: 'file', path }, 'file'); }}>
      <label className="grow">Absolute log path<input ref={pathInput} value={path} onChange={e => setPath(e.target.value)} placeholder="/var/log/nginx/error.log" list="paths" required /></label>
      <datalist id="paths">{suggestions.map(c => <option key={c.id} value={c.path}/>)}</datalist>
      <button className="primary" disabled={!host || host.state !== 'connected' || Boolean(pending) || !path.startsWith('/')}>{pending === 'file' ? 'Starting…' : 'Start file'}</button>
    </form>
    {pins.length > 0 && <div className="pins"><span className="hint">Pinned paths · select to recheck on Start</span>{pins.map(p => <button key={p} onClick={() => setPath(p)}>{p}</button>)}</div>}
    </div>
    <div id="source-discover-panel" role="tabpanel" aria-labelledby="source-discover-tab" hidden={mode !== 'discover'}><p className="subtle">Find files, Docker containers and systemd units using bounded metadata discovery. Choose a result, then Start.</p>
    <details className="discovery-options"><summary>Discovery scope & permissions</summary>
      <div className="check-row"><label><input type="checkbox" checked={docker} onChange={e => setDocker(e.target.checked)}/> Docker / Compose metadata</label><label><input type="checkbox" checked={varLog} onChange={e => setVarLog(e.target.checked)}/> /var/log, depth 2</label></div>
      <div className="fields"><label>Project root<input value={root} onChange={e => setRoot(e.target.value)} placeholder="/srv/my-app"/></label><label>PM2 logs directory<input value={pm2} onChange={e => setPm2(e.target.value)} placeholder="/home/app/.pm2/logs"/></label></div>
      <div className="fields"><label>Selected PIDs (comma separated)<input value={pids} onChange={e => setPids(e.target.value)}/></label><label>Selected systemd units<input value={units} onChange={e => setUnits(e.target.value)}/></label></div>
      <label className="check-row"><input type="checkbox" checked={consent} onChange={e => setConsent(e.target.checked)}/> Allow reading these config files to extract path hints</label>
      {consent && <><label>Approved config root<input value={configRoot} onChange={e => setConfigRoot(e.target.value)}/></label><label>Exact Nginx / Caddy / Supervisor config files, one per line<textarea value={configs} onChange={e => setConfigs(e.target.value)}/></label></>}
      <p className="hint">Discovery reads metadata. Start grants log reading for that source. Config reading is separate. External agent access requires a separate source grant. Production is a label; it grants no permission.</p>
    </details>
    <div className="section-heading"><h2>Available sources</h2>{busy && host && <button onClick={cancelDiscovery}>Cancel</button>}<button disabled={!host || host.state !== 'connected' || busy} onClick={() => run(discover)}>{busy ? 'Discovering…' : 'Discover sources'}</button></div>
    {result && !result.complete && <p className="notice">Partial metadata: {result.reasons.join(' · ') || 'Discovery did not finish'}</p>}
    <div className="candidate-list">{result?.candidates.map(c => <article className="candidate" key={c.id}><div><strong>{c.path}</strong><p>{c.provider} · {c.access} · {c.activity}</p><small>{c.provenance}</small><input className="tag-input" aria-label={`Tags for ${c.path}`} placeholder="Tags (hints only)" value={c.tags.join(', ')} maxLength={255} onChange={e => setResult(previous => previous ? {...previous, candidates: previous.candidates.map(item => item.id === c.id ? {...item, tags: e.target.value.split(',').slice(0, 8).map(tag => tag.trim().slice(0, 32))} : item)} : previous)}/></div><button disabled={c.access !== 'ready' || !host || host.state !== 'connected' || Boolean(pending)} onClick={() => start(c)}>{pending === c.id ? 'Starting…' : 'Start'}</button></article>)}
      {!result && <div className="empty"><span>⌁</span><h3>Your sources, in one place</h3><p>Connect a host to discover containers and log files.<br/>You can also enter a file path directly.</p></div>}
      {result?.candidates.length === 0 && <p className="subtle">No matching sources in this scope.</p>}
    </div>
    </div></section>}
  </section>;
}
