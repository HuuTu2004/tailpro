import { Button } from '../../shared/ui/Controls';
import { useEffect, useRef, useState } from 'react';
import type { AppStatus, TopologyGraph } from '../../shared/contracts/generated';
import { native } from '../../shared/client/native';
import '../settings/tools.css';
export function Topology({ status, hostId, selectSources }: { status?: AppStatus; hostId: string; selectSources: (sources: string[]) => void }) {
  const [view, setView] = useState('');
  const [files, setFiles] = useState('');
  const [roots, setRoots] = useState('');
  const [names, setNames] = useState('');
  const [consent, setConsent] = useState(false);
  const [graph, setGraph] = useState<TopologyGraph>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [configure, setConfigure] = useState(false);
  useEffect(() => { setConsent(false); }, [hostId]);
  const generation = useRef(0);
  const hostSources = status?.sources.filter(s => s.host_id === hostId).map(s => s.id) ?? [];
  const key = status?.sources.map(s => `${s.id}:${s.generation}`).join('|');
  useEffect(() => {
    let alive = true; let id = '';
    native.openView().then(v => { id = v; if (alive) setView(v); else void native.closeView(v); }).catch(e => setError(String(e)));
    return () => { alive = false; generation.current++; if (id) void native.closeView(id).catch(() => {}); };
  }, []);
  useEffect(() => {
    if (!view) return;
    let alive = true; let ack: string | null = null;
    const timer = setInterval(() => { void native.viewNotice(view, ack).then(n => { if (alive) ack = n.revision; }).catch(() => {}); }, 15000);
    return () => { alive = false; clearInterval(timer); };
  }, [view]);
  useEffect(() => { generation.current++; setGraph(undefined); setBusy(false); if (view) void native.cancelPage(view, generation.current).catch(() => {}); }, [hostId, key, files, roots, names, consent, view, status?.health.policy_revision]);
  useEffect(() => {
    if (!graph || !consent || busy) return;
    let alive = true; let checking = false;
    const timer = setInterval(() => {
      if (checking) return;
      checking = true; const current = ++generation.current;
      void native.topologyRevision({ view_id: view, generation: current, host_id: hostId, files: files.split('\n').filter(Boolean), roots: roots.split('\n').filter(Boolean), endpoint_names: names.split(',').map(s => s.trim()).filter(Boolean), consent, sources: hostSources }).then(revision => {
        if (alive && current === generation.current && revision !== graph.config_revision) { setGraph(undefined); setError('Approved configuration changed. Reread topology.'); }
      }).catch(() => { if (alive && current === generation.current) { setGraph(undefined); setError('Configuration freshness unavailable. Reread topology.'); } }).finally(() => { checking = false; });
    }, 15000);
    return () => { alive = false; clearInterval(timer); };
  }, [graph, consent, busy, view, hostId, files, roots, names, key]);
  async function load(e: React.FormEvent) {
    e.preventDefault(); const current = ++generation.current; setBusy(true); setGraph(undefined); setError('');
    try {
      const g = await native.topology({ view_id: view, generation: current, host_id: hostId, files: files.split('\n').filter(Boolean), roots: roots.split('\n').filter(Boolean), endpoint_names: names.split(',').map(s => s.trim()).filter(Boolean), consent, sources: hostSources });
      if (current === generation.current) { setGraph(g); setConfigure(false); }
    } catch (e) { if (current === generation.current) setError(String(e)); }
    finally { if (current === generation.current) setBusy(false); }
  }
  const visible = graph?.nodes ?? [];
  const host = status?.connections.find(c => c.host.id === hostId);
  return <section className="tool-page" aria-label="Configured service topology">
    <div className="tool-heading"><div><h2>Service topology</h2><p className="subtle">Explore configured service relationships on the selected host.</p></div>{host && <span className={`badge ${host.host.environment}`}>{host.host.target} · {host.host.environment}</span>}</div>
    {!hostId && <div className="tool-empty"><h3>Choose a host first</h3><p>Select a connection to configure its service topology.</p></div>}
    {hostId && !hostSources.length && <p className="notice">Start a source on this host before reading its topology.</p>}
    {hostId && (!graph || configure) && <section className="tool-card tool-form"><h3>Approved configuration</h3><p className="hint">Separate opt-in for exact Compose files on this host. Static block YAML/JSON subset; includes, interpolation and runtime environment remain unresolved. Discovery consent does not enable topology.</p>
      <form onSubmit={load}><fieldset disabled={busy} className="tool-fieldset">
      <label>Exact Compose files · one per line<textarea value={files} onChange={e => { setFiles(e.target.value); setConsent(false); }} maxLength={32768} placeholder="/srv/app/compose.yaml" required/></label>
      <label>Approved roots · one per line<textarea value={roots} onChange={e => { setRoots(e.target.value); setConsent(false); }} maxLength={32768} placeholder="/srv/app" required/></label>
      <details className="tool-help"><summary>Optional endpoint hints</summary><label>Allowed literal endpoint names in Compose environment · comma separated<input value={names} onChange={e => { setNames(e.target.value); setConsent(false); }} maxLength={2048}/></label></details>
      <label className="settings-check consent-row"><input type="checkbox" checked={consent} onChange={e => setConsent(e.target.checked)}/><span>Allow bounded reads and metadata freshness checks of these exact configuration files. No .env, runtime environment or execution.</span></label>
      <Button type="submit" variant="primary" disabled={!view || !hostId || !hostSources.length || !consent || busy || !files.trim()}>{busy ? 'Reading topology…' : 'Read topology'}</Button>
      </fieldset>{busy && <Button type="button" onClick={() => { generation.current++; void native.cancelPage(view, generation.current).catch(() => {}); setBusy(false); }}>Cancel</Button>}
      </form></section>}
    {error && <p role="alert" className="notice">{error}</p>}
    {graph && <section className="tool-results" aria-label="Topology snapshot"><div className="section-heading"><h3>Configured relationships</h3><Button onClick={() => setConfigure(true)}>Edit configuration</Button></div><p className="hint">{graph.coverage} · Snapshot counts; approved config metadata checked every 15 seconds. Changes require a new read.</p>{!graph.complete && <p className="notice">Partial graph: {graph.reasons.join(' · ')}</p>}
      {!visible.length && <div className="tool-empty"><h3>No services found in this snapshot</h3><p>Review the exact approved file paths and supported static Compose configuration.</p></div>}
      <div className="topology-graph" role="list" aria-label="Service topology">{visible.map(node => <article role="listitem" className="topology-node" key={node.id}><Button disabled={!node.sources.length} onClick={() => selectSources(node.sources)} title={node.sources.length ? 'Show these mapped sources in Logs' : 'No mapped sources'}>{node.id}</Button><p>{node.observed_errors === null ? 'Error count unknown · no mapped source in selected scope' : `${node.observed_errors} retained observed errors · rate unknown`}</p><small>{node.provenance}</small>{graph.edges.filter(e => e.from === node.id).map((edge, i) => <p key={i} className={edge.kind === 'inferred' ? 'inferred-edge' : ''}>→ {edge.to} · {edge.kind}<br/><small>{edge.provenance}</small></p>)}</article>)}</div>
      <p className="hint">Relationships are configured or inferred hints, not observed traffic. Counts require explicit source/service mappings. Select a mapped service to view its logs.</p>
    </section>}
  </section>;
}
