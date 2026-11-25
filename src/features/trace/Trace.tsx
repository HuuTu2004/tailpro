import { Button, Select } from '../../shared/ui/Controls';
import { useEffect, useRef, useState } from 'react';
import type { AppStatus, Evidence, ReadResponse } from '../../shared/contracts/generated';
import { native } from '../../shared/client/native';
import '../settings/tools.css';
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
  const evidencePanel = useRef<HTMLElement>(null);
  const evidenceTrigger = useRef<HTMLButtonElement | null>(null);
  useEffect(() => { if (selected) evidencePanel.current?.focus(); }, [selected]);
  function closeEvidence() { setSelected(undefined); evidenceTrigger.current?.focus(); }
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
  return <section className="tool-page" aria-label="Request trace">
    <div className="tool-heading"><div><h2>Request trace</h2><p className="subtle">Follow an exact trace or order ID across the selected sources.</p></div><span className="badge">{scope.length} sources</span></div>
    <section className="tool-card tool-form"><form onSubmit={read}>
      <div className="fields"><label>ID type<Select value={kind} disabled={busy} onChange={e => setKind(e.target.value as typeof kind)}><option value="trace_id">trace_id</option><option value="order_id">order_id</option></Select></label><label>Exact ID<input value={value} disabled={busy} onChange={e => setValue(e.target.value)} maxLength={256} placeholder="req-42" required/></label></div>
      <details className="tool-help"><summary>Limit the observed-time window</summary><div className="fields"><label>From local observed ms · optional<input value={from} disabled={busy} onChange={e => setFrom(e.target.value)} pattern="[0-9]+" maxLength={20}/></label><label>To local observed ms · optional<input value={to} disabled={busy} onChange={e => setTo(e.target.value)} pattern="[0-9]+" maxLength={20}/></label></div></details>
      <div className="actions"><Button type="submit" variant="primary" disabled={!view || !scope.length || busy || !value.trim()}>{busy ? 'Tracing…' : 'Trace request'}</Button>{busy && <Button type="button" onClick={() => { generation.current++; void native.cancelPage(view, generation.current).catch(() => {}); setBusy(false); }}>Cancel</Button>}</div>
    </form><p className="hint">Uses the log viewer source selection. Local observed-time correlation; clock offsets, causal links and missing parent spans are unknown.</p></section>
    {error && <p role="alert" className="notice">{error}</p>}
    {!scope.length && <div className="tool-empty"><h3>Start a source to trace requests</h3><p>Connect a host and start reading a file, container or journal first.</p></div>}
    {!result && !busy && scope.length > 0 && <div className="tool-empty"><h3>Find the events behind a request</h3><p>Enter an exact ID above. Duration bars appear only when the logs include an explicit duration.</p></div>}
    {result && <section className="tool-results" aria-label="Trace results"><div className="section-heading"><h3>Retained evidence</h3><span className="hint">{result.time_basis} · {result.coverage}</span></div>{(!result.complete || result.truncated || result.ambiguous) && <p className="notice">{!result.complete ? 'Partial retained evidence. ' : ''}{result.truncated ? 'Result reached its bound. ' : ''}{result.ambiguous ? 'Business ID may refer to multiple requests. ' : ''}{result.gaps.join(' · ')}</p>}
      {!result.events.length ? <div className="tool-empty"><h3>No retained match</h3><p>Try another ID, source scope or time window. This does not prove the request was absent.</p></div> : <div className="tool-result-layout"><div className="tool-result-main trace-lanes">{result.sources.map(s => <section key={s.id}><h3>{s.server} · {s.id} <span className={`badge ${s.environment}`}>{s.environment}</span> · {s.health}</h3>{result.timeline.filter(p => p.source === s.id).map(p => <Button className="trace-event" key={p.evidence_ref} onClick={e => { evidenceTrigger.current = e.currentTarget; setSelected(result.events.find(event => event.evidence_ref === p.evidence_ref)); }}>
        <span>{p.event_time ?? p.observed_at_ms} · {p.semantics}</span>{p.duration_ms === null ? <span className="trace-marker">◆ Event · no explicit duration</span> : <span className="trace-duration" style={{ width: `${Math.max(2, p.duration_ms / maxDuration * 100)}%` }}>{p.duration_ms} ms</span>}
      </Button>)}</section>)}</div>
      {selected && <aside className="tool-card trace-evidence" ref={evidencePanel} tabIndex={-1} aria-label="Trace evidence" onKeyDown={e => { if (e.key === 'Escape') { e.stopPropagation(); closeEvidence(); } }}><div className="section-heading"><h3>Trace evidence</h3><Button onClick={closeEvidence} aria-label="Close trace evidence">Close</Button></div><p className="hint">{selected.source} · {selected.level} · {selected.quality}</p><pre className="raw-text">{selected.text}</pre><p className="hint">Redacted retained evidence. Bars compare explicit durations; their horizontal position does not represent causal start time.</p></aside>}
      </div>}
      <p className="hint">Timestamps across hosts do not prove latency. Duration bars compare explicit duration_ms values; event-only records stay markers.</p>
    </section>}
  </section>;
}
