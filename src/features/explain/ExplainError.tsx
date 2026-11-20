import { useEffect, useRef, useState } from 'react';
import { native } from '../../shared/client/native';
import type { Explanation, ExplainProvider, PreparedPreview } from '../../shared/contracts/generated';
import { WorkspaceDialog } from '../live-view/WorkspaceDialog';
export function ExplainError({ eventId, sources }: { eventId: string; sources: string[] }) {
  const [open, setOpen] = useState(false);
  const [preview, setPreview] = useState<PreparedPreview>(); const [provider, setProvider] = useState<ExplainProvider | null>(null);
  const [consent, setConsent] = useState(false); const [answer, setAnswer] = useState<Explanation>();
  const [busy, setBusy] = useState(false); const [message, setMessage] = useState(''); const active = useRef(0);
  const ownsPrepared = useRef(false);
  const scopeKey = JSON.stringify(sources);
  useEffect(() => {
    active.current++; setPreview(undefined); setAnswer(undefined); setConsent(false); setOpen(false); setBusy(false);
    return () => { active.current++; if (ownsPrepared.current) { ownsPrepared.current = false; void native.cancelPrepared().catch(() => {}); } };
  }, [eventId, scopeKey]);
  useEffect(() => {
    if (!preview) return;
    const timer = setTimeout(() => { setPreview(undefined); setConsent(false); setMessage('Preview expired. Prepare again.'); }, Math.max(0, Number(preview.expires_at_ms) - Date.now()));
    return () => clearTimeout(timer);
  }, [preview]);
  function close() {
    active.current++; setBusy(false); setPreview(undefined); setConsent(false); setAnswer(undefined); setOpen(false);
    if (ownsPrepared.current) { ownsPrepared.current = false; void native.cancelPrepared().catch(() => {}); }
  }
  async function prepare() {
    const revision = ++active.current; setBusy(true); setMessage(''); setConsent(false); setAnswer(undefined); setPreview(undefined); ownsPrepared.current = true;
    try {
      const p = await native.explainProvider(); if (!p) throw new Error('Enable Ollama or BYOK in Settings → AI & MCP before explaining an error.');
      if (active.current !== revision) return;
      const result = await native.prepareExplanation({ sources, event_ids: [eventId], query: '', from_ms: null, to_ms: null });
      if (active.current === revision) { setProvider(p); setPreview(result); }
    } catch (e) { if (active.current === revision) setMessage(String(e)); }
    finally { if (active.current === revision) setBusy(false); }
  }
  async function explain() {
    if (!preview || !consent) return;
    const revision = ++active.current; setBusy(true); setMessage('');
    try { const result = await native.explain(preview.id, preview.digest, consent); if (active.current === revision) { ownsPrepared.current = false; setAnswer(result); setPreview(undefined); setConsent(false); } }
    catch (e) { if (active.current === revision) { setMessage(String(e)); setPreview(undefined); setConsent(false); } }
    finally { if (active.current === revision) setBusy(false); }
  }
  return <><button disabled={!sources.length} onClick={() => { setOpen(true); void prepare(); }}>Explain error</button>
    {open && <WorkspaceDialog title="Explain error" close={close}>
      <p className="hint">Optional explanation of the selected event. Review the exact redacted payload and provider before sending.</p>
      {busy && <p role="status">Working…</p>}
      {preview && <><p>{provider?.kind === 'ollama' ? 'Local Ollama inference' : 'BYOK cloud'} · {provider?.model}</p><p className="hint">{provider?.endpoint}</p><pre className="output-preview">{preview.payload}</pre><label className="consent-row"><input disabled={busy} type="checkbox" checked={consent} onChange={e => setConsent(e.target.checked)}/> Send these exact redacted bytes to this provider and model for explanation.</label><button className="primary" disabled={!consent || busy} onClick={() => void explain()}>Send preview for explanation</button></>}
      {answer && <><h3>Selected evidence</h3><p>{answer.evidence.join(', ') || 'No retained evidence'}</p><h3>Model explanation / hypotheses · untrusted</h3><pre className="raw-text explanation-result">{answer.hypothesis}</pre><p className="hint">Model output is a hypothesis. No suggested action is executed.</p></>}
      {message && <p role="status" className="notice">{message}</p>}
      {!preview && !busy && !answer && <button onClick={() => void prepare()}>Prepare preview again</button>}
    </WorkspaceDialog>}
  </>;
}
