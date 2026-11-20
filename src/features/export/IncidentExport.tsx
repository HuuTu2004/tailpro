import { useEffect, useRef, useState } from 'react';
import { native } from '../../shared/client/native';
import type { ExportFormat, PreparedPreview } from '../../shared/contracts/generated';
type Props = { sources: string[]; eventIds?: string[]; query?: string };
export function IncidentExport({ sources, eventIds = [], query = '' }: Props) {
  const [open, setOpen] = useState(false);
  const [format, setFormat] = useState<ExportFormat>('ndjson');
  const [from, setFrom] = useState(''); const [to, setTo] = useState('');
  const [preview, setPreview] = useState<PreparedPreview>();
  const [destination, setDestination] = useState(''); const [overwrite, setOverwrite] = useState(false);
  const [busy, setBusy] = useState(false); const [message, setMessage] = useState('');
  const revision = useRef(0);
  const selectionKey = JSON.stringify({ sources, eventIds, query, format, from, to });
  useEffect(() => { revision.current++; setPreview(undefined); }, [selectionKey]);
  useEffect(() => {
    if (!preview) return;
    const timer = setTimeout(() => { setPreview(undefined); setMessage('Preview expired. Prepare again.'); }, Math.max(0, Number(preview.expires_at_ms) - Date.now()));
    return () => clearTimeout(timer);
  }, [preview]);
  async function prepare() {
    const current = ++revision.current; setBusy(true); setMessage(''); setPreview(undefined);
    try { const result = await native.prepareExport({ sources, event_ids: eventIds, query, from_ms: from || null, to_ms: to || null }, format); if (revision.current === current) setPreview(result); }
    catch (e) { setMessage(String(e)); } finally { setBusy(false); }
  }
  async function save() {
    if (!preview) return; setBusy(true); setMessage('');
    try { await native.commitExport(preview.id, preview.digest, destination, overwrite); setPreview(undefined); setMessage('Sanitized incident saved.'); }
    catch (e) { setPreview(undefined); setMessage(String(e)); } finally { setBusy(false); }
  }
  return <section className="output-panel"><button disabled={!sources.length} onClick={() => { if (open) { revision.current++; setPreview(undefined); void native.cancelPrepared().catch(e => setMessage(String(e))); } setOpen(!open); }}>Export incident</button>{open && <div>
    <p className="hint">{eventIds.length ? `${eventIds.length} selected event(s)` : `${sources.length} selected source(s)`} · Query: {query || '(all retained)'} · Redacted NDJSON / spreadsheet-safe CSV. Preview includes coverage and gaps.</p>
    <label>Format <select value={format} onChange={e => setFormat(e.target.value as ExportFormat)}><option value="ndjson">NDJSON</option><option value="csv">CSV</option></select></label>
    <label>Observed from (Unix ms) <input value={from} onChange={e => setFrom(e.target.value)} inputMode="numeric"/></label>
    <label>Observed to (Unix ms) <input value={to} onChange={e => setTo(e.target.value)} inputMode="numeric"/></label>
    <button disabled={busy} onClick={() => void prepare()}>Prepare exact preview</button>
    {preview && <><p className="hint">Expires {new Date(Number(preview.expires_at_ms)).toLocaleTimeString()} · {preview.redaction_incomplete ? 'Some content could not be safely projected and was masked.' : 'Shared redaction applied.'}</p><pre className="output-preview">{preview.payload}</pre>
      <label>Absolute destination path <input value={destination} onChange={e => setDestination(e.target.value)} placeholder="/home/user/incident.ndjson"/></label>
      <label><input type="checkbox" checked={overwrite} onChange={e => setOverwrite(e.target.checked)}/> Replace an existing file at this path</label>
      <button disabled={busy || !destination.startsWith('/')} onClick={() => void save()}>Save these exact bytes</button></>}
    <button onClick={() => { revision.current++; void native.cancelPrepared().catch(e => setMessage(String(e))); setPreview(undefined); setOpen(false); }}>Cancel / close</button>
    {message && <p role="status">{message}</p>}
  </div>}</section>;
}
