import React, { useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { Connections } from '../features/connections/Connections';
import { Sources } from '../features/sources/Sources';
import { native } from '../shared/client/native';
import type { AppStatus } from '../shared/contracts/generated';
import { LiveView } from '../features/live-view/LiveView';
import { CommandPalette } from '../shared/ui/CommandPalette';
import './styles.css';
import { Trace } from '../features/trace/Trace';
import { Topology } from '../features/topology/Topology';
import { ServiceMappings } from '../features/settings/ServiceMappings';
import { AiSettings } from '../features/settings/AiSettings';
function App() {
  const [status, setStatus] = useState<AppStatus>();
  const [selected, setSelected] = useState('');
  const [error, setError] = useState('');
  const [scope, setScope] = useState<string[]>([]);
  const [palette, setPalette] = useState(false);
  const [requestedPath, setRequestedPath] = useState<{ value: string; revision: number }>();
  const queryInput = useRef<HTMLInputElement>(null);
  const viewport = useRef<HTMLDivElement>(null);
  useEffect(() => {
    function shortcut(event: KeyboardEvent) {
      if (document.querySelector('dialog[open]') || event.altKey || event.repeat) return;
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') { event.preventDefault(); setPalette(true); }
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'f') { event.preventDefault(); queryInput.current?.focus(); queryInput.current?.select(); }
    }
    window.addEventListener('keydown', shortcut); return () => window.removeEventListener('keydown', shortcut);
  }, []);
  useEffect(() => {
    let alive = true;
    const refresh = () => native.status().then(s => { if (alive) setStatus(s); }).catch(e => { if (alive) setError(String(e)); });
    refresh(); const timer = setInterval(refresh, 1000);
    return () => { alive = false; clearInterval(timer); };
  }, []);
  async function run(action: () => Promise<unknown>) {
    setError(''); try { await action(); setStatus(await native.status()); } catch(e) { setError(String(e)); }
  }
  const host = status?.connections.find(c => c.host.id === selected);
  return <><header><a className="brand" href="#">▥ <span>TailPro</span><small>LOCAL WORKSPACE</small></a><span className="read-only"><span className="dot connected"/>Read-only over SSH</span></header>
    {error && <div role="alert" className="error">{error}<button aria-label="Dismiss error" onClick={() => setError('')}>×</button></div>}
    <main><Connections connections={status?.connections ?? []} selected={selected} select={setSelected} run={run}/><div className="workspace">
      <AiSettings status={status}/><div className="workspace-tools"><button onClick={() => setPalette(true)}>Switch host / source <kbd>Ctrl K</kbd></button><details><summary>Keyboard help</summary><p className="hint">Ctrl K: command palette · Ctrl F: query · Space: pause/resume when the log table has focus · Arrow keys: select a row · Enter: Inspector · Escape: close panel. Text inputs keep their normal Space behavior.</p></details></div>
      <details className="source-workspace" open={!status?.sources.length || Boolean(requestedPath)}><summary>Connections & source discovery · {host?.host.target ?? 'Choose a host'}</summary><Sources host={host} sources={status?.sources.filter(s => s.host_id === selected) ?? []} requestedPath={requestedPath} run={run}/></details>
      <ServiceMappings status={status}/><Trace status={status} sources={scope}/><Topology status={status} hostId={selected} selectSources={setScope}/>
      <LiveView status={status} scope={scope} setScope={setScope} queryInput={queryInput} viewport={viewport}/>
    </div></main>{palette && <CommandPalette status={status} hostId={selected} selectHost={setSelected} selectSource={id => setScope([id])} choosePath={value => setRequestedPath({ value, revision: Date.now() })} close={() => setPalette(false)} run={run}/>}
    <footer><span>Local-first. No remote agent installed.</span><span>Log viewer · Phase 4</span></footer></>;

}
createRoot(document.getElementById('root')!).render(<React.StrictMode><App/></React.StrictMode>);
