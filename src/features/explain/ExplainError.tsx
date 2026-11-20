import { useEffect, useRef, useState } from 'react';
import { native } from '../../shared/client/native';
import type { Explanation, ExplainProvider, PreparedPreview } from '../../shared/contracts/generated';
export function ExplainError({ eventId, sources }: { eventId: string; sources: string[] }) {
  const [preview, setPreview] = useState<PreparedPreview>(); const [provider, setProvider] = useState<ExplainProvider | null>(null);
  const [consent, setConsent] = useState(false); const [answer, setAnswer] = useState<Explanation>();
  const [busy, setBusy] = useState(false); const [message, setMessage] = useState(''); const active = useRef(0);
  useEffect(() => { active.current++; setPreview(undefined); setAnswer(undefined); setConsent(false); return () => { active.current++; void native.cancelPrepared().catch(() => {}); }; }, [eventId]);
  useEffect(() => { if (!preview) return; const timer = setTimeout(() => { setPreview(undefined); setConsent(false); setMessage('Preview expired. Prepare again.'); }, Math.max(0, Number(preview.expires_at_ms) - Date.now())); return () => clearTimeout(timer); }, [preview]);
  async function prepare() {
    const revision = ++active.current; setBusy(true); setMessage(''); setConsent(false); setAnswer(undefined);
    try {
      const p = await native.explainProvider(); if (!p) throw new Error('Enable Ollama or BYOK in AI & MCP settings.');
      const result = await native.prepareExplanation({ sources, event_ids: [eventId], query: '', from_ms: null, to_ms: null });
      if (active.current === revision) { setProvider(p); setPreview(result); }
    } catch (e) { if (active.current === revision) setMessage(String(e)); } finally { if (active.current === revision) setBusy(false); }
  }
  async function explain() {
    if (!preview || !consent) return; const revision = ++active.current; setBusy(true); setMessage('');
    try { const result = await native.explain(preview.id, preview.digest, consent); if (active.current === revision) { setAnswer(result); setPreview(undefined); } }
    catch (e) { if (active.current === revision) { setMessage(String(e)); setPreview(undefined); } } finally { if (active.current === revision) setBusy(false); }
  }
  return <section className="output-panel"><button disabled={busy || !sources.length} onClick={() => void prepare()}>Explain Error · prepare preview</button>
    {preview && <><p>{provider?.kind === 'ollama' ? 'Local Ollama inference' : 'BYOK cloud'} · {provider?.model} · {provider?.endpoint}</p><pre className="output-preview">{preview.payload}</pre><label><input type="checkbox" checked={consent} onChange={e => setConsent(e.target.checked)}/> Send these exact redacted bytes to this provider/model for explanation.</label><button disabled={!consent || busy} onClick={() => void explain()}>Send preview for explanation</button></>}
    {(preview || busy) && <button onClick={() => { active.current++; setBusy(false); setPreview(undefined); setConsent(false); void native.cancelPrepared().catch(e => setMessage(String(e))); }}>Cancel / clear</button>}
    {answer && <><h3>Selected evidence</h3><p>{answer.evidence.join(', ') || 'No retained evidence'}</p><h3>Model explanation / hypotheses · untrusted</h3><pre className="raw-text">{answer.hypothesis}</pre></>}
    {message && <p role="status">{message}</p>}
  </section>;
}
