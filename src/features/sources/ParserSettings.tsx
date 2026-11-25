import { Button } from '../../shared/ui/Controls';
import { useState } from 'react';
import { native } from '../../shared/client/native';
export function ParserSettings({ source }: { source: string }) {
  const [config, setConfig] = useState(`version = 1\n\n[parse.profile]\n# time_unit = "milliseconds"\n# level_namespace = "pino"\n# timezone_offset_minutes = 420\n`);
  const [sample, setSample] = useState('');
  const [preview, setPreview] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const [validated, setValidated] = useState('');
  async function action(apply: boolean) {
    setBusy(true); setError('');
    try {
      if (apply) { const revision = await native.applyParser(source, config); setPreview(`Applied parser revision ${revision}. Settings last for this source session.`); setValidated(''); }
      else { setPreview(JSON.stringify(JSON.parse(await native.previewParser(source, config, sample)), null, 2)); setValidated(config); }
    } catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  }
  return <details className="parser-settings"><summary>Parser profile & rules</summary>
    <p className="hint">Source ID: <code>{source}</code>. Import a local .tailpro.toml, preview, then apply. Samples stay local. Invalid changes keep the current parser.</p>
    <label>Import local configuration<input type="file" accept=".toml" disabled={busy} onChange={async event => { const file = event.target.files?.[0]; if (!file) return; if (file.size > 65536) { setError('Configuration exceeds 64 KiB.'); return; } setConfig(await file.text()); setValidated(''); }}/></label>
    <label>Configuration<textarea aria-label={`Parser config ${source}`} value={config} disabled={busy} maxLength={65536} onChange={event => { setConfig(event.target.value); setValidated(''); }}/></label>
    <p className="hint">A rule needs [[parse.rules]], id, source = "{source}", and pattern with named time/level/msg groups. Maximum 4 rules per source / 16 per workspace. Lookaround and backreferences are unsupported.</p>
    <label>Sample record<textarea value={sample} disabled={busy} maxLength={65536} onChange={event => setSample(event.target.value)}/></label>
    <div className="actions"><Button disabled={busy} onClick={() => void action(false)}>Preview</Button><Button disabled={busy || validated !== config} onClick={() => void action(true)}>Apply previewed configuration</Button><Button disabled={busy} onClick={() => { setConfig('version = 1\n[parse.profile]\nraw_only = true\n'); setValidated(''); }}>Prepare raw-only rollback</Button></div>
    {error && <p role="alert" className="notice">{error}</p>}{preview && <pre className="raw-text">{preview}</pre>}
  </details>;
}
