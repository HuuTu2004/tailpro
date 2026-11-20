import { useEffect, useRef, useState } from 'react';
import type { AppStatus, Evidence, ReadResponse } from '../../shared/contracts/generated';
import { native } from '../../shared/client/native';
export function Trace({ status, sources }: { status?: AppStatus; sources: string[] }) {
  const [view, setView] = useState('');
  const [kind, setKind] = useState<'trace_id' | 'order_id'>('trace_id');
  const [value, setValue] = useState('');
  const [from, setFrom] = useState('');
  const [to, setTo] = useState('');
  const [result, setResult] = useState<ReadResponse>();
  const [selected, setSelected] = useState<Evidence>();
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const generation = useRef(0);
  const scope = sources.length ? sources : status?.sources.map(s => s.id) ?? [];
  const scopeKey = scope.join('|');
  const sourceGenerations = status?.sources.map(s => `${s.id}:${s.generation}`).join('|');
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
  useEffect(() => { generation.current++; setResult(undefined); setSelected(undefined); setBusy(false); if (view) void native.cancelPage(view, generation.current).catch(() => {}); }, [scopeKey, sourceGenerations, kind, value, from, to, view, status?.health.policy_revision]);
  async function read(e: React.FormEvent) {
    e.preventDefault(); const current = ++generation.current; setError(''); setBusy(true); setSelected(undefined); setResult(undefined);
    try {
      const r = await native.debugRead(view, current, 'trace_id', { sources: scope, query: '', correlation: { kind, value }, from_ms: from || null, to_ms: to || null, service: null, limit: 200, max_bytes: 128 * 1024, cursor: null, evidence_ref: null });
      if (current === generation.current) setResult(r);
    } catch (e) { if (current === generation.current) setError(String(e)); }
    finally { if (current === generation.current) setBusy(false); }
  }
  const maxDuration = Math.max(1, ...result?.timeline.map(p => p.duration_ms ?? 0) ?? []);
  return <details className="output-panel"><summary>Request trace & waterfall</summary>
    <p className="hint">Uses the live viewer source selection ({scope.length} sources). Exact ID correlation uses a local observed-time window. Clock offsets, causal links and missing parent spans are unknown. Bars compare explicit durations; their horizontal position does not represent causal start time.</p>
    <form onSubmit={read}><div className="fields"><label>ID type<select value={kind} onChange={e => setKind(e.target.value as typeof kind)}><option value="trace_id">trace_id</option><option value="order_id">order_id</option></select></label><label>Exact ID<input value={value} onChange={e => setValue(e.target.value)} maxLength={256} required/></label></div>
      <div className="fields"><label>From local observed ms (optional)<input value={from} onChange={e => setFrom(e.target.value)} pattern="[0-9]+" maxLength={20}/></label><label>To local observed ms (optional)<input value={to} onChange={e => setTo(e.target.value)} pattern="[0-9]+" maxLength={20}/></label></div>
      <button disabled={!view || !scope.length || busy}>Trace request</button>{busy && <button type="button" onClick={() => { generation.current++; void native.cancelPage(view, generation.current).catch(() => {}); setBusy(false); }}>Cancel</button>}
    </form>
    {error && <p role="alert">{error}</p>}
    {result && <><p className="hint">{result.time_basis} · {result.coverage}</p>{(!result.complete || result.truncated || result.ambiguous) && <p className="notice">{!result.complete ? 'Partial retained evidence. ' : ''}{result.ambiguous ? 'Business ID may refer to multiple requests. ' : ''}{result.gaps.join(' · ')}</p>}
      {!result.events.length && <p>No retained match in this scope/window. This does not prove the request was absent.</p>}
      <div className="trace-lanes">{result.sources.map(s => <section key={s.id}><h3>{s.server} · {s.id} <span className={`badge ${s.environment}`}>{s.environment}</span> · {s.health}</h3>{result.timeline.filter(p => p.source === s.id).map(p => <button className="trace-event" key={p.evidence_ref} onClick={() => setSelected(result.events.find(e => e.evidence_ref === p.evidence_ref))}>
        <span>{p.event_time ?? p.observed_at_ms} · {p.semantics}</span>{p.duration_ms === null ? <span className="trace-marker">◆ event</span> : <span className="trace-duration" style={{ width: `${Math.max(2, p.duration_ms / maxDuration * 100)}%` }}>{p.duration_ms} ms</span>}
      </button>)}</section>)}</div></>}
    {selected && <aside className="inspector"><div className="section-heading"><h3>Trace evidence</h3><button onClick={() => setSelected(undefined)}>Close</button></div><p>{selected.source} · {selected.level} · {selected.quality}</p><pre className="raw-text">{selected.text}</pre><p className="hint">Redacted retained evidence. Duration bars require explicit duration_ms; timestamps across hosts do not prove latency.</p></aside>}
  </details>;
}
