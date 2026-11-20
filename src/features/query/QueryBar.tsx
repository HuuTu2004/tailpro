import { useEffect, useState, type RefObject } from 'react';
import { native } from '../../shared/client/native';
import type { SavedView } from '../../shared/contracts/generated';
type Props = { value: string; change: (value: string) => void; sources: string[]; restore: (view: SavedView) => void; input: RefObject<HTMLInputElement | null> };
export function QueryBar({ value, change, sources, restore, input }: Props) {
  const [views, setViews] = useState<SavedView[]>([]);
  const [name, setName] = useState('');
  const [error, setError] = useState('');
  const [selected, setSelected] = useState('');
  useEffect(() => { let alive = true; native.savedViews().then(v => { if (alive) setViews(v); }).catch(e => { if (alive) setError(String(e)); }); return () => { alive = false; }; }, []);
  async function save() {
    try { await native.saveView({ name: name.trim(), query: value, sources }); setViews(await native.savedViews()); setName(''); setError(''); }
    catch (e) { setError(String(e)); }
  }
  return <div className="query-bar">
    <label className="query-label">Search retained logs <kbd>Ctrl F</kbd><input ref={input} aria-label="Log query" value={value} maxLength={4096} onChange={e => change(e.target.value)} placeholder='trace_id:req-42 AND level:error · duration>200ms'/></label>
    <details><summary>Query help & Saved Views</summary>
      <p className="hint">Literals and AND; field:value for level, source (ID), server, message, trace_id, order_id, user_id. IDs match exactly; message matches a literal substring. duration&gt;200ms or duration_ms&gt;=1s. Quote text containing spaces or syntax. OR/NOT/regex are unsupported. Order follows local ingestion.</p>
      <div className="saved-controls"><label>Saved View<select value={selected} onChange={e => { setSelected(e.target.value); const view = views.find(v => v.name === e.target.value); if (view) restore(view); }}><option value="">Choose a saved filter</option>{views.map(v => <option key={v.name}>{v.name}</option>)}</select></label>
        <button disabled={!selected} onClick={async () => { try { await native.deleteView(selected); setViews(await native.savedViews()); setSelected(''); } catch (e) { setError(String(e)); } }}>Delete</button>
        <form onSubmit={e => { e.preventDefault(); void save(); }}><label>Save current filter<input aria-label="Saved View name" maxLength={80} value={name} onChange={e => setName(e.target.value)} placeholder="View name" required/></label><button disabled={!name.trim()}>Save</button></form>
      </div><p className="hint">Restoring a view only restores its query and source IDs. Start sources explicitly; IDs from an earlier session may be unavailable.</p>
    </details>{error && <p role="alert" className="notice">{error}</p>}
  </div>;
}
