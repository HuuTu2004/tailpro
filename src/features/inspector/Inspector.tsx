import { ExplainError } from '../explain/ExplainError';
import { IncidentExport } from '../export/IncidentExport';
import { useEffect, useRef, useState } from 'react';
import { native } from '../../shared/client/native';
import type { EventDetail, InspectorNode } from '../../shared/contracts/generated';
type Branch = { path: number[]; nodes: InspectorNode[]; more: boolean };
type Props = { sources: string[]; viewId: string; eventId: string; nextGeneration: () => number; close: () => void; select: (id: string) => void; active?: boolean };
const MAX_BYTES = 1024 * 1024;
export function Inspector({ sources, viewId, eventId, nextGeneration, close, select, active: visible = true }: Props) {
  const [detail, setDetail] = useState<EventDetail>();
  const [branches, setBranches] = useState<Record<string, Branch>>({});
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [copied, setCopied] = useState('');
  const [tab, setTab] = useState<'fields' | 'raw' | 'stack' | 'evidence' | 'bytes'>('fields');
  const active = useRef(0);
  const panel = useRef<HTMLElement>(null);
  const budget = useRef({ bytes: 0, nodes: 0 });
  async function load(path: number[], offset = 0, root = false) {
    const generation = nextGeneration(); active.current = generation; setBusy(true); setError('');
    try {
      const result = await native.detail({ view_id: viewId, generation, event_id: eventId, path, offset, include_raw: root });
      if (active.current !== generation) return;
      const bytes = JSON.stringify(result).length * 2 + result.nodes.length * 128;
      const count = result.nodes.length;
      if (budget.current.bytes + bytes > MAX_BYTES || budget.current.nodes + count > 4096) { setError('Inspector budget reached (1 MiB / 4096 nodes). Close and reopen to release detail.'); return; }
      budget.current.bytes += bytes; budget.current.nodes += count;
      if (root) setDetail({ ...result, nodes: [] });
      if (result.json_error && !root) setError(result.json_error);
      setBranches(prev => ({ ...prev, [path.join('.')]: { path, nodes: offset ? [...(prev[path.join('.')]?.nodes ?? []), ...result.nodes] : result.nodes, more: result.more } }));
    } catch (e) { if (active.current === generation) setError(String(e)); }
    finally { if (active.current === generation) setBusy(false); }
  }
  useEffect(() => {
    setDetail(undefined); setBranches({}); setCopied(''); budget.current = { bytes: 0, nodes: 0 };
    void load([], 0, true); if (visible) panel.current?.querySelector<HTMLButtonElement>('[data-close-inspector]')?.focus();
    return () => { active.current = -1; void native.cancelDetail(viewId, nextGeneration()).catch(() => {}); };
  }, [viewId, eventId]);
  async function copy(text: string, label: string) {
    try { await navigator.clipboard.writeText(text); setCopied(label); } catch { setError('Clipboard unavailable. Select and copy the displayed text.'); }
  }
  function renderBranch(path: number[], depth = 0): React.ReactNode {
    const branch = branches[path.join('.')]; if (!branch) return null;
    return <ul className="json-tree">{branch.nodes.map(node => {
      const child = [...path, node.index]; const key = child.join('.');
      const container = node.kind === 'object' || node.kind === 'array';
      return <li key={node.index}><div className="json-node">
        {container && <button disabled={busy || depth >= 63} aria-label={`Expand ${node.key}`} onClick={() => { if (branches[key]) setBranches(prev => { const next = { ...prev }; for (const p of Object.keys(next)) if (p === key || p.startsWith(`${key}.`)) delete next[p]; return next; }); else void load(child); }}>{branches[key] ? '−' : '+'}</button>}
        <code>{node.key}: {container ? node.kind : node.value}{node.truncated ? ' … (preview)' : ''}</code>
        <button disabled={node.truncated} onClick={() => void copy(node.key, 'Field copied')}>Copy field</button><button disabled={container || node.truncated} onClick={() => { const value: unknown = JSON.parse(node.value); void copy(typeof value === 'string' ? value : node.value, 'Value copied'); }}>Copy value</button>
      </div>{branches[key] && renderBranch(child, depth + 1)}</li>;
    })}{branch.more && <li><button disabled={busy} onClick={() => void load(path, branch.nodes.length)}>Load next children (up to 200)</button></li>}</ul>;
  }
  return <aside className="inspector" id="event-details-panel" ref={panel} tabIndex={-1} aria-label="Event Inspector" onKeyDown={e => { if (visible && e.key === 'Escape' && !document.querySelector('dialog[open]')) { e.stopPropagation(); close(); } }}>
    <div className="section-heading"><h2>Event details</h2><div className="inspector-header-actions"><button data-close-inspector onClick={close} aria-label="Close event details" title="Close details · Esc">×</button></div></div>
    <p className="hint">{eventId} · Local copy only</p>{error && <p role="alert" className="notice">{error}</p>}{busy && <p role="status">Loading bounded detail…</p>}
    {detail && <><p className="hint">{detail.quality}</p><div className="inspector-tabs" aria-label="Event detail sections">{(['fields', 'raw', 'stack', 'evidence', 'bytes'] as const).map(value => <button key={value} aria-pressed={tab === value} onClick={() => setTab(value)}>{value === 'fields' ? 'Fields' : value === 'raw' ? 'Raw' : value === 'stack' ? 'Stack' : value === 'evidence' ? 'Why these fields?' : 'Original bytes'}</button>)}</div>
      <div className="inspector-content">{tab === 'fields' && <>{detail.json_error ? <p className="hint">{detail.json_error}</p> : renderBranch([])}</>}
      {tab === 'evidence' && <pre className="raw-text">{detail.evidence_json ? JSON.stringify(JSON.parse(detail.evidence_json), null, 2) : 'No provenance in this preview.'}</pre>}
      {tab === 'bytes' && <><p className="hint">{detail.raw_bytes} original bytes including the known delimiter. Hex preview capped at 16 KiB; text copy can be lossy. Fragment neighbors may have expired.</p><pre className="raw-text">{detail.raw_hex ?? 'Not requested'}</pre></>}
      {tab === 'raw'  && <><div className="section-heading"><h3>Raw text</h3><button disabled={detail.raw === null} onClick={() => void copy(detail.raw ?? '', 'Raw text copied')}>Copy raw</button></div><pre className="raw-text">{detail.raw}</pre></>}
      {tab === 'stack' && (detail.group_id ? <><h3>Stack group · retained parts</h3><p className="hint">Some parts may have expired from retention. {detail.group_parts_truncated ? 'Part list capped at 200.' : ''}</p><div className="stack-parts">{detail.group_parts.map((id, i) => <button key={id} disabled={id === eventId} onClick={() => select(id)}>Part {i + 1}</button>)}</div></> : <p className="hint">This event has no retained stack group.</p>)}</div>
      <div className="inspector-actions"><IncidentExport sources={sources} eventIds={[eventId]}/><ExplainError eventId={eventId} sources={sources}/></div>
    </>}{copied && <p role="status" className="hint">{copied}</p>}
  </aside>;
}
