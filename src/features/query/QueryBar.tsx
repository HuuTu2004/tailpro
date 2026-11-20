import { useState, type RefObject } from 'react';

type Props = { value: string; change: (value: string) => void; input: RefObject<HTMLInputElement | null> };
export function QueryBar({ value, change, input }: Props) {
  const [help, setHelp] = useState(false);
  return <div className="query-bar">
    <div className="query-toolbar">
      <label className="query-label"><span className="query-label-text">Search retained logs</span><input ref={input} aria-label="Log query" value={value} maxLength={4096} onChange={event => change(event.target.value)} placeholder="Search text, an ID, or level:error…"/><kbd aria-hidden="true">Ctrl F</kbd></label>
      <button aria-expanded={help} aria-controls="query-syntax-help" aria-label="Query syntax help" title="Query syntax help" className="query-help-trigger" onClick={() => setHelp(!help)}>?</button>
    </div>
    {help && <p id="query-syntax-help" className="hint query-help">Use literal text and AND, or field:value for level, source (ID), server, message, trace_id, order_id and user_id. IDs match exactly; message matches a literal substring. Examples: <code>level:error</code>, <code>trace_id:req-42 AND duration&gt;200ms</code>. Quote text containing spaces. OR, NOT and regex are unsupported.</p>}
  </div>;
}
