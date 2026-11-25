import { Button, Select } from '../../shared/ui/Controls';
import { useEffect, useRef, useState } from 'react';
import { native } from '../../shared/client/native';
import type { ExportFormat, PreparedPreview } from '../../shared/contracts/generated';
import { WorkspaceDialog } from '../live-view/WorkspaceDialog';
type Props = { sources: string[]; eventIds?: string[]; query?: string };
export function IncidentExport({ sources, eventIds = [], query = '' }: Props) {
  const [open, setOpen] = useState(false);
  const [format, setFormat] = useState<ExportFormat>('ndjson');
  const [from, setFrom] = useState(''); const [to, setTo] = useState('');
  const [preview, setPreview] = useState<PreparedPreview>();
  const [destination, setDestination] = useState(''); const [overwrite, setOverwrite] = useState(false);
  const [busy, setBusy] = useState(false); const [message, setMessage] = useState('');
  const revision = useRef(0);
  const ownsPrepared = useRef(false);
  const selectionKey = JSON.stringify({ sources, eventIds, query, format, from, to });
  useEffect(() => { revision.current++; setPreview(undefined); setBusy(false); setOverwrite(false); }, [selectionKey]);
  useEffect(() => () => {
    revision.current++;
    if (ownsPrepared.current) void native.cancelPrepared().catch(() => {});
  }, []);
  useEffect(() => {
    if (!preview) return;
    const timer = setTimeout(() => { setPreview(undefined); setOverwrite(false); setMessage('Preview expired. Prepare again.'); }, Math.max(0, Number(preview.expires_at_ms) - Date.now()));
    return () => clearTimeout(timer);
  }, [preview]);
  function close() {
    revision.current++; setPreview(undefined); setBusy(false); setOverwrite(false); setOpen(false);
    if (ownsPrepared.current) { ownsPrepared.current = false; void native.cancelPrepared().catch(() => {}); }
  }
  async function prepare() {
    const current = ++revision.current; setBusy(true); setMessage(''); setPreview(undefined); setOverwrite(false); ownsPrepared.current = true;
    try { const result = await native.prepareExport({ sources, event_ids: eventIds, query, from_ms: from || null, to_ms: to || null }, format); if (revision.current === current) setPreview(result); }
    catch (e) { if (revision.current === current) setMessage(String(e)); }
    finally { if (revision.current === current) setBusy(false); }
  }
  async function save() {
    if (!preview) return;
    const current = ++revision.current; setBusy(true); setMessage('');
    try {
      await native.commitExport(preview.id, preview.digest, destination, overwrite);
      if (revision.current === current) { ownsPrepared.current = false; setPreview(undefined); setMessage('Sanitized incident saved.'); }
    } catch (e) { if (revision.current === current) { setPreview(undefined); setMessage(String(e)); } }
    finally { if (revision.current === current) setBusy(false); }
  }
  return <><Button className="incident-export-trigger" disabled={!sources.length} onClick={() => { setMessage(''); setOpen(true); }}>{eventIds.length ? 'Export event' : 'Export'}</Button>{open && <WorkspaceDialog title="Export incident" close={close}>
    <p className="hint">{eventIds.length ? `${eventIds.length} selected event(s)` : `${sources.length} selected source(s)`} · Query: {query || '(all retained)'}. Redacted NDJSON or spreadsheet-safe CSV, including coverage and gaps.</p>
    <label>Format<Select disabled={busy} value={format} onChange={e => setFormat(e.target.value as ExportFormat)}><option value="ndjson">NDJSON</option><option value="csv">CSV</option></Select></label>
    <div className="fields"><label>Observed from · Unix ms<input disabled={busy} value={from} onChange={e => setFrom(e.target.value)} inputMode="numeric" placeholder="Optional"/></label><label>Observed to · Unix ms<input disabled={busy} value={to} onChange={e => setTo(e.target.value)} inputMode="numeric" placeholder="Optional"/></label></div>
    <Button disabled={busy} onClick={() => void prepare()}>{busy ? 'Working…' : 'Prepare exact preview'}</Button>
    {preview && <><h3>Exact sanitized output</h3><p className="hint">Expires {new Date(Number(preview.expires_at_ms)).toLocaleTimeString()} · {preview.redaction_incomplete ? 'Some content could not be safely projected and was masked.' : 'Shared redaction applied.'}</p><pre className="output-preview">{preview.payload}</pre>
      <label>Absolute destination path<input disabled={busy} value={destination} onChange={e => setDestination(e.target.value)} placeholder="/home/user/incident.ndjson"/></label>
      <label className="consent-row"><input disabled={busy} type="checkbox" checked={overwrite} onChange={e => setOverwrite(e.target.checked)}/> Replace an existing file at this path</label>
      <Button variant="primary" disabled={busy || !destination.startsWith('/')} onClick={() => void save()}>Save these exact bytes</Button></>}
    {message && <p role="status" className="notice">{message}</p>}
  </WorkspaceDialog>}</>;
}
