import React, { useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { Connections } from '../features/connections/Connections';
import { Sources } from '../features/sources/Sources';
import { HostLogTree } from '../features/sources/HostLogTree';
import { sourceKey, type LogSource } from '../features/sources/source-labels';
import { SourceTabs } from '../features/live-view/SourceTabs';
import { LiveView } from '../features/live-view/LiveView';
import { Trace } from '../features/trace/Trace';
import { Topology } from '../features/topology/Topology';
import { Settings } from '../features/settings/Settings';
import { CommandPalette } from '../shared/ui/CommandPalette';
import { native } from '../shared/client/native';
import type { AppStatus } from '../shared/contracts/generated';
import './styles.css';

type Page = 'workspace' | 'connections' | 'settings' | 'new-connection';
type Tool = 'sources' | 'trace' | 'topology';
const pages: { id: Page; label: string }[] = [
  { id: 'connections', label: 'Connections' },
  { id: 'settings', label: 'Settings' },
];
const tools: { id: Tool; label: string; description: string }[] = [
  { id: 'sources', label: 'Sources', description: 'Add and manage log sources' },
  { id: 'trace', label: 'Trace', description: 'Find a request across sources' },
  { id: 'topology', label: 'Topology', description: 'Inspect configured service relationships' },
];

function Icon({ name }: { name: Page | Tool | 'search' | 'close' }) {
  const paths: Record<typeof name, string> = {
    workspace: 'M3 4h18v16H3z M3 9h18 M8 9v11',
    connections: 'M4 4h16v6H4z M4 14h16v6H4z M7 7h.01 M7 17h.01',
    'new-connection': 'M4 4h16v6H4z M4 14h16v6H4z',
    settings: 'M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8z M12 3v2 M12 19v2 M3 12h2 M19 12h2 M5.6 5.6 7 7 M17 17l1.4 1.4 M5.6 18.4 7 17 M17 7l1.4-1.4',
    sources: 'M4 3h10l6 6v12H4z M14 3v6h6 M8 13h8 M8 17h6',
    trace: 'M4 5h16 M4 12h11 M4 19h7 M8 3v4 M15 10v4 M10 17v4',
    topology: 'M9 3h6v6H9z M3 15h6v6H3z M15 15h6v6h-6z M12 9v3 M6 15v-3h12v3',
    search: 'M10.5 3a7.5 7.5 0 1 0 0 15 7.5 7.5 0 0 0 0-15z M16 16l5 5',
    close: 'm6 6 12 12 M18 6 6 18',
  };
  return <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true"><path d={paths[name]}/></svg>;
}

const route = new URLSearchParams(location.search);
const managementWindow = route.has('manager');
const connectionEditor = route.get('manager') === 'new-connection';
const initialPage: Page = connectionEditor ? 'new-connection' : managementWindow ? route.get('manager') === 'settings' ? 'settings' : 'connections' : 'workspace';
const navigationPages = managementWindow ? pages : pages.filter(item => item.id === 'connections');

function App() {
  const [page, setPage] = useState<Page>(initialPage);
  const [tool, setTool] = useState<Tool | null>(null);
  const [status, setStatus] = useState<AppStatus>();
  const [selected, setSelected] = useState('');
  const [error, setError] = useState('');
  const [selectedSources, setSelectedSources] = useState<string[]>([]);
  const [combinedSources, setCombinedSources] = useState<string[]>([]);
  const [opening, setOpening] = useState<Set<string>>(new Set());
  const [closing, setClosing] = useState<Set<string>>(new Set());
  const openingRef = useRef(new Set<string>());
  const closingRef = useRef(new Set<string>());
  const selectionRevision = useRef(0);
  const statusRef = useRef<AppStatus | undefined>(undefined); statusRef.current = status;
  const sources = status?.sources ?? [];
  const selectedScope = selectedSources.filter(id => sources.some(source => source.id === id));
  const scope = selectedScope.length ? selectedScope : sources[0] ? [sources[0].id] : [];
  const sourcesKey = sources.map(source => source.id).join('|');
  useEffect(() => {
    setSelectedSources(previous => {
      const remaining = previous.filter(id => sources.some(source => source.id === id));
      if (remaining.length) return remaining.length === previous.length ? previous : remaining;
      return sources[0] ? [sources[0].id] : [];
    });
    setCombinedSources(previous => {
      const remaining = previous.filter(id => sources.some(source => source.id === id));
      return remaining.length === previous.length ? previous : remaining;
    });
  }, [sourcesKey]);
  function setScope(ids: string[]) {
    selectionRevision.current++;
    const readable = [...new Set(ids)].filter(id => statusRef.current?.sources.some(source => source.id === id));
    setSelectedSources(readable);
    if (readable.length > 1) setCombinedSources(readable);
  }
  function selectSourceTab(id: string) {
    const source = statusRef.current?.sources.find(item => item.id === id);
    if (!source) return;
    setScope([id]); setSelected(source.host_id); setPage('workspace'); setTool(null);
  }
  function manageHostSources(hostId: string) {
    setSelected(hostId); setPage('workspace'); setTool('sources');
    setRequestedPath({ value: '', revision: Date.now(), hostId, mode: 'discover' });
  }
  async function openTreeSource(entry: LogSource) {
    const key = sourceKey(entry);
    if (openingRef.current.has(key)) return;
    const existing = statusRef.current?.sources.find(source => sourceKey({ hostId: source.host_id, spec: source.spec }) === key);
    if (existing) { selectSourceTab(existing.id); return; }
    const revision = ++selectionRevision.current;
    openingRef.current.add(key); setOpening(new Set(openingRef.current)); setError('');
    try {
      const id = await native.start(entry.hostId, entry.spec);
      await refreshStatus();
      if (alive.current && revision === selectionRevision.current) selectSourceTab(id);
    } catch (reason) { if (alive.current) setError(String(reason)); }
    finally { openingRef.current.delete(key); if (alive.current) setOpening(new Set(openingRef.current)); }
  }
  async function closeLog(id: string) {
    if (closingRef.current.has(id)) return;
    selectionRevision.current++; closingRef.current.add(id); setClosing(new Set(closingRef.current)); setError('');
    try {
      await native.stop(id);
      if (!alive.current) return;
      setStatus(previous => previous ? { ...previous, sources: previous.sources.filter(source => source.id !== id) } : previous);
      await refreshStatus();
    } catch (reason) { if (alive.current) setError(String(reason)); }
    finally { closingRef.current.delete(id); if (alive.current) setClosing(new Set(closingRef.current)); }
  }
  const [palette, setPalette] = useState(false);
  const [requestedPath, setRequestedPath] = useState<{ value: string; revision: number; hostId: string; mode?: 'file' | 'discover' }>();
  const queryInput = useRef<HTMLInputElement>(null);
  const viewport = useRef<HTMLDivElement>(null);
  const panel = useRef<HTMLElement>(null);
  const toolTrigger = useRef<HTMLElement | null>(null);
  const alive = useRef(false);
  const statusGeneration = useRef(0);

  async function refreshStatus() {
    const generation = ++statusGeneration.current;
    const next = await native.status();
    if (!alive.current || generation !== statusGeneration.current) return;
    statusRef.current = next; setStatus(next);
    setSelected(current => next.connections.some(connection => connection.host.id === current) ? current : next.connections[0]?.host.id ?? '');
    return next;
  }

  useEffect(() => {
    alive.current = true;
    let polling = true;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      try { await refreshStatus(); }
      catch (reason) { if (polling) setError(String(reason)); }
      finally { if (polling) timer = setTimeout(poll, 1000); }
    }
    void poll();
    return () => { polling = false; alive.current = false; statusGeneration.current++; clearTimeout(timer); };
  }, []);

  useEffect(() => {
    function shortcut(event: KeyboardEvent) {
      if (managementWindow || document.querySelector('dialog[open]') || event.altKey || event.repeat) return;
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault(); setPalette(true);
      }
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'f') {
        event.preventDefault(); setPage('workspace'); setTool(null);
        requestAnimationFrame(() => { queryInput.current?.focus(); queryInput.current?.select(); });
      }
    }
    window.addEventListener('keydown', shortcut);
    return () => window.removeEventListener('keydown', shortcut);
  }, []);

  useEffect(() => { if (tool && page === 'workspace') panel.current?.focus(); }, [tool, page]);

  async function run(action: () => Promise<unknown>) {
    setError('');
    try { const result = await action(); const next = await refreshStatus(); if (typeof result === 'string' && next?.sources.some(source => source.id === result)) selectSourceTab(result); }
    catch (reason) { if (alive.current) setError(String(reason)); }
  }
  function changePage(next: Page) {
    if (!managementWindow) { void native.openWorkspaceManager(next === 'settings' ? 'settings' : 'connections').catch(reason => setError(String(reason))); return; }
    setPage(next); setTool(null);
  }
  function toggleTool(next: Tool, trigger?: HTMLElement) {
    toolTrigger.current = trigger ?? document.activeElement as HTMLElement;
    setPage('workspace'); setTool(current => current === next ? null : next);
  }
  function closeTool() {
    setTool(null);
    requestAnimationFrame(() => { if (toolTrigger.current?.isConnected) toolTrigger.current.focus(); });
  }
  function choosePath(value: string) {
    setPage('workspace'); setTool('sources');
    setRequestedPath({ value, revision: Date.now(), hostId: selected });
  }
  const host = status?.connections.find(connection => connection.host.id === selected);
  const currentTool = tools.find(item => item.id === tool);
  const connections = status?.connections ?? [];

  if (connectionEditor) return <div className="connection-editor-shell"><Connections standaloneEditor connections={connections} selected={selected} select={setSelected} run={run}/></div>;

  return <div className={'app-shell' + (managementWindow ? ' management-shell' : ' log-tree-shell')}>
    {!managementWindow && <aside className="app-sidebar">
      <button className="brand" onClick={() => { if (managementWindow) setPage('connections'); else viewport.current?.focus(); }} aria-label="TailPro workspace"><span className="brand-symbol"><Icon name="workspace"/></span><span>TailPro</span></button>
      <nav className="workspace-tabs" role={managementWindow ? 'tablist' : undefined} aria-label="Workspace management" aria-orientation={managementWindow ? 'vertical' : undefined} onKeyDown={event => {
        if (!managementWindow || !['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
        event.preventDefault();
        const index = pages.findIndex(item => item.id === page);
        const next = event.key === 'Home' ? 0 : event.key === 'End' ? pages.length - 1 : (index + (event.key === 'ArrowRight' || event.key === 'ArrowDown' ? 1 : -1) + pages.length) % pages.length;
        changePage(pages[next].id); document.getElementById('tab-' + pages[next].id)?.focus();
      }}>{navigationPages.map(item => <button key={item.id} id={'tab-' + item.id} role={managementWindow ? 'tab' : undefined} className={page === item.id ? 'is-selected' : ''} aria-haspopup={managementWindow ? undefined : 'dialog'} aria-label={item.label} title={item.label} tabIndex={!managementWindow || page === item.id ? 0 : -1} aria-selected={managementWindow ? page === item.id : undefined} aria-controls={managementWindow ? 'page-' + item.id : undefined} onClick={() => changePage(item.id)}><Icon name={item.id}/><span>{item.label}</span>{item.id === 'workspace' && <span className="sidebar-count">{status?.sources.length ?? 0}</span>}</button>)}</nav>
      <HostLogTree connections={connections} sources={sources} selectedHost={selected} scope={scope} opening={opening} selectHost={setSelected} open={openTreeSource} discover={manageHostSources} manageConnections={() => changePage('connections')}/>
      <div className="sidebar-session"><span className="local-session-mark"><Icon name="connections"/></span><div><strong>SSH sessions</strong><span>Read-only over SSH</span></div><span className="dot connected"/></div>
    </aside>}
    <header className="app-header">
      <div className="page-title"><h1>{page === 'workspace' ? 'Logs' : page === 'connections' ? 'Connections' : 'Settings'}</h1></div>
      <div className="app-header-actions">
        {!managementWindow && page === 'workspace' && <div className="workspace-tool-buttons"><button className={tool === 'sources' ? 'active' : ''} aria-expanded={tool === 'sources'} aria-controls="workspace-tool-panel" onClick={event => toggleTool('sources', event.currentTarget)}><Icon name="sources"/>Sources<span className="tool-count">{status?.sources.length ?? 0}</span></button>
        <details className="investigate-menu"><summary><Icon name="trace"/>Investigate<svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" aria-hidden="true"><path d="m3 4.5 3 3 3-3"/></svg></summary><div className="investigate-options">{tools.filter(item => item.id !== 'sources').map(item => <button key={item.id} onClick={event => { const menu = event.currentTarget.closest('details'); const opener = menu?.querySelector('summary'); menu?.removeAttribute('open'); toggleTool(item.id, opener ?? event.currentTarget); }}><Icon name={item.id}/><span><strong>{item.label}</strong><small>{item.description}</small></span></button>)}</div></details></div>}
        {!managementWindow && <><button className="icon-button" aria-label="Settings" title="Settings" onClick={() => void native.openWorkspaceManager('settings').catch(reason => setError(String(reason)))}><Icon name="settings"/></button><button className="quick-switch" onClick={() => setPalette(true)} title="Switch host or source · Ctrl K" aria-label="Switch host or source"><Icon name="search"/><kbd>Ctrl K</kbd></button></>}

      </div>
    </header>
    {error && <div role="alert" className="error app-error"><span>{error}</span><button aria-label="Dismiss error" onClick={() => setError('')}>×</button></div>}
    <main className="app-main">
      {!managementWindow && <section className="workspace-page" id="page-workspace" role="region" aria-label="Live logs" hidden={page !== 'workspace' || managementWindow}>
        <div className={'workspace-body' + (tool ? ' has-tool' : '')}>
          <div className="log-workspace">{!managementWindow && <LiveView onGetStarted={() => connections.length ? toggleTool('sources') : changePage('connections')} status={status} scope={scope} reading={scope.length > 0} sourceTabs={<SourceTabs status={status} scope={scope} combined={combinedSources} closing={closing} select={selectSourceTab} close={closeLog} combine={ids => { setScope(ids); setTool(null); }}/>} queryInput={queryInput} viewport={viewport} active={page === 'workspace'} inspectorVisible={!tool} onInspect={() => setTool(null)}/>}</div>
          {page === 'workspace' && currentTool && <aside className="workspace-tool-panel" id="workspace-tool-panel" ref={panel} tabIndex={-1} aria-label={currentTool.label} onKeyDown={event => { if (event.key === 'Escape' && !event.defaultPrevented && !document.querySelector('dialog[open]')) { event.stopPropagation(); closeTool(); } }}>
            <div className="tool-panel-header"><div><Icon name={currentTool.id}/><strong>{currentTool.label}</strong></div><button className="icon-button" onClick={closeTool} title="Close tool · Escape" aria-label={'Close ' + currentTool.label}><Icon name="close"/></button></div>
            <div className="tool-panel-content">
              {tool === 'sources' && <Sources host={host} sources={status?.sources.filter(source => source.host_id === selected) ?? []} requestedPath={requestedPath?.hostId === selected ? requestedPath : undefined} run={run}/>}
              {tool === 'trace' && <Trace status={status} sources={scope}/>}
              {tool === 'topology' && <Topology status={status} hostId={selected} selectSources={setScope}/>}
            </div>
          </aside>}
        </div>
      </section>}
      {managementWindow && page === 'connections' && <section className="standalone-page" id="page-connections" role="tabpanel" aria-labelledby="tab-connections"><Connections connections={connections} selected={selected} select={setSelected} run={run}/></section>}
      {managementWindow && page === 'settings' && <section className="standalone-page" id="page-settings" role="tabpanel" aria-labelledby="tab-settings"><Settings status={status}/></section>}
    </main>
    {palette && <CommandPalette status={status} hostId={selected} selectHost={setSelected} selectSource={selectSourceTab} choosePath={choosePath} close={() => setPalette(false)} run={run}/>}
    <footer className="app-footer"><span><span className="dot connected"/>Local-first<span className="footer-divider">·</span>No remote agent installed</span><span>{connections.length} host{connections.length === 1 ? '' : 's'} · {status?.sources.length ?? 0} sources</span></footer>
  </div>;
}

createRoot(document.getElementById('root')!).render(<React.StrictMode><App/></React.StrictMode>);
