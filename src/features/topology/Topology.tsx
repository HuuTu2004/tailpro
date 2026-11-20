import { useEffect, useRef, useState } from 'react';
import type { AppStatus, TopologyGraph } from '../../shared/contracts/generated';
import { native } from '../../shared/client/native';
export function Topology({ status, hostId, selectSources }: { status?: AppStatus; hostId: string; selectSources: (sources: string[]) => void }) {
  const [view, setView] = useState('');
  const [files, setFiles] = useState('');
  const [roots, setRoots] = useState('');
  const [names, setNames] = useState('');
  const [consent, setConsent] = useState(false);
  const [graph, setGraph] = useState<TopologyGraph>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
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
      if (current === generation.current) setGraph(g);
    } catch (e) { if (current === generation.current) setError(String(e)); }
    finally { if (current === generation.current) setBusy(false); }
  }
  const visible = graph?.nodes ?? [];
  return <details className="output-panel"><summary>Configured service topology</summary>
    <form onSubmit={load}><p className="hint">Separate opt-in for exact Compose files on the selected host. Static block YAML/JSON subset; includes, interpolation and runtime environment remain unresolved. Existing discovery consent does not enable topology. Counts require an explicit source/service mapping.</p>
      <div className="fields"><label>Exact Compose files, one per line<textarea value={files} onChange={e => setFiles(e.target.value)} maxLength={32768} required/></label><label>Approved roots, one per line<textarea value={roots} onChange={e => setRoots(e.target.value)} maxLength={32768} required/></label></div>
      <label>Allowed literal endpoint names in Compose environment (optional, comma separated)<input value={names} onChange={e => setNames(e.target.value)} maxLength={2048}/></label>
      <label className="check-row"><input type="checkbox" checked={consent} onChange={e => setConsent(e.target.checked)}/> Allow this bounded topology config read and metadata freshness checks; no .env/runtime env or execution</label>
      <button disabled={!view || !hostId || !hostSources.length || !consent || busy}>Read topology</button>{busy && <button type="button" onClick={() => { generation.current++; void native.cancelPage(view, generation.current).catch(() => {}); setBusy(false); }}>Cancel</button>}
    </form>
    {error && <p role="alert">{error}</p>}
    {graph && <><p className="hint">{graph.coverage} · Snapshot counts; approved config metadata checked every 15 seconds, changes require reread.</p>{!graph.complete && <p className="notice">Partial graph: {graph.reasons.join(' · ')}</p>}
      <div className="topology-graph" role="list" aria-label="Service topology">{visible.map(node => <article role="listitem" className="topology-node" key={node.id}><button disabled={!node.sources.length} onClick={() => selectSources(node.sources)}>{node.id}</button><p>{node.observed_errors === null ? 'Error count unknown · no mapped source in selected scope' : `${node.observed_errors} retained observed errors · rate unknown`}</p><small>{node.provenance}</small>{graph.edges.filter(e => e.from === node.id).map((e, i) => <p key={i} className={e.kind === 'inferred' ? 'inferred-edge' : ''}>→ {e.to} · {e.kind}<br/><small>{e.provenance}</small></p>)}</article>)}</div>
    </>}
  </details>;
}
