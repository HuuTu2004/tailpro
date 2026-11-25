import { Button } from './Controls';
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { native } from '../client/native';
import type { AppStatus, Candidate } from '../contracts/generated';
type Props = { status?: AppStatus; hostId: string; selectHost: (id: string) => void; selectSource: (id: string) => void; choosePath: (path: string) => void; close: () => void; run: (action: () => Promise<unknown>) => void };
export function CommandPalette({ status, hostId, selectHost, selectSource, choosePath, close, run }: Props) {
  const dialog = useRef<HTMLDialogElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const [text, setText] = useState('');
  const [candidates, setCandidates] = useState<Candidate[]>([]);
  const [error, setError] = useState('');
  const [chosen, setChosen] = useState<Candidate>();
  const [active, setActive] = useState(0);
  const version = useRef(0);
  const returnFocus = useRef(document.activeElement as HTMLElement | null);
  const restore = useRef(true);
  useLayoutEffect(() => {
    const node = dialog.current;
    node?.showModal(); input.current?.focus();
    return () => { node?.close(); queueMicrotask(() => { if (!node?.isConnected && restore.current && returnFocus.current?.isConnected) returnFocus.current.focus(); }); };
  }, []);
  useEffect(() => {
    const current = ++version.current; setCandidates([]); setChosen(undefined); setActive(0); setError('');
    if (!text.startsWith('/') || !hostId) return;
    const index = text.lastIndexOf('/');
    const timer = setTimeout(() => {
      native.complete(hostId, text.slice(0, index) || '/', text.slice(index + 1)).then(result => { if (current === version.current) { setCandidates(result.candidates.slice(0, 20)); setError(result.complete ? '' : result.reasons.join(' · ')); } }).catch(e => { if (current === version.current) setError(String(e)); });
    }, 200);
    return () => { clearTimeout(timer); version.current++; void native.cancelDiscovery(hostId).catch(() => {}); };
  }, [text, hostId]);
  useEffect(() => { document.getElementById(`palette-${active}`)?.scrollIntoView({ block: 'nearest' }); }, [active]);
  const needle = text.toLowerCase();
  const actions = [
    ...(status?.connections ?? []).filter(c => c.host.target.toLowerCase().includes(needle)).map(c => ({ key: c.host.id, title: `Host · ${c.host.target}${c.host.environment === 'production' ? ' · Production' : ''}`, action: () => { selectHost(c.host.id); close(); } })),
    ...(status?.sources ?? []).filter(s => `${s.id} ${s.spec.kind === 'file' ? s.spec.path : s.spec.kind === 'journal' ? s.spec.unit : s.spec.container_id}`.toLowerCase().includes(needle)).map(s => ({ key: s.id, title: `Reading source · ${s.spec.kind === 'file' ? s.spec.path : s.spec.kind === 'journal' ? s.spec.unit : s.spec.container_id}`, action: () => { selectHost(s.host_id); selectSource(s.id); close(); } })),
    ...candidates.map(c => ({ key: c.id, title: `${c.path} · ${c.access}`, action: () => setChosen(c) })),
  ].slice(0, 32);
  return <dialog ref={dialog} className="command-palette" aria-label="Command palette" onCancel={e => { e.preventDefault(); close(); }} onClick={e => { if (e.target === e.currentTarget) close(); }}>
    <h2>Switch host or source</h2><label>Host, source, or absolute log path<input ref={input} aria-label="Search host, source, or absolute path" role="combobox" aria-expanded="true" aria-controls="palette-results" aria-activedescendant={actions[active] ? `palette-${active}` : undefined} value={text} maxLength={4096} placeholder="Host, source, or /absolute/log/path" onChange={e => setText(e.target.value)} onKeyDown={e => {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') { e.preventDefault(); setActive(i => actions.length ? (i + (e.key === 'ArrowDown' ? 1 : actions.length - 1)) % actions.length : 0); }
      if (e.key === 'Enter' && actions[active]) { e.preventDefault(); actions[active].action(); }
    }}/>
    </label><div id="palette-results" role="listbox" aria-label="Host and source candidates">{actions.map((action, i) => <Button id={`palette-${i}`} key={action.key} role="option" aria-selected={i === active} className={i === active ? 'active' : ''} onClick={action.action}>{action.title}</Button>)}</div>
    {chosen && <div className="notice"><p>Selected: {chosen.path}. Reading begins only when you press Start.</p><div className="actions"><Button onClick={() => { restore.current = false; choosePath(chosen.path); close(); }}>Use path in source form</Button><Button variant="primary" disabled={chosen.access !== 'ready'} onClick={() => { run(() => native.start(hostId, chosen.provider === 'systemd_unit' ? { kind: 'journal', unit: chosen.path } : chosen.container_id ? { kind: 'docker', container_id: chosen.container_id, aliases: [chosen.path, ...chosen.tags].slice(0,16).map(s => s.slice(0,128)) } : { kind: 'file', path: chosen.path })); close(); }}>Start selected source</Button></div></div>}
    {error && <p role="alert" className="notice">{error}</p>}<p className="hint">Paths use the current host and bounded metadata autocomplete. Selecting a host never connects it. Selecting a file never starts it.</p><Button onClick={close}>Close · Esc</Button>
  </dialog>;
}
