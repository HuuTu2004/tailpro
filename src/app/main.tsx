import { Button } from '../shared/ui/Controls';
import React, { useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { Connections } from '../features/connections/Connections';
import { AddLogPath } from '../features/sources/AddLogPath';
import { HostLogTree } from '../features/sources/HostLogTree';
import { sourceKey, type LogSource } from '../features/sources/source-labels';
import { SourceTabs } from '../features/live-view/SourceTabs';
import { LiveView } from '../features/live-view/LiveView';
import { Settings } from '../features/settings/Settings';
import { native } from '../shared/client/native';
import { listen } from '@tauri-apps/api/event';
import type { AppStatus, ConnectionInfo, HostProfile, SavedWorkspace, SourcesOpened } from '../shared/contracts/generated';
import './styles.css';

type Page = 'workspace' | 'connections' | 'settings' | 'new-connection';

const route = new URLSearchParams(location.search);
const managementWindow = route.has('manager');
const connectionEditor = route.get('manager') === 'new-connection';
const initialPage: Page = connectionEditor ? 'new-connection' : managementWindow ? route.get('manager') === 'settings' ? 'settings' : 'connections' : 'workspace';

function App() {
  const [page, setPage] = useState<Page>(initialPage);
  const [status, setStatus] = useState<AppStatus>();
  const [savedHosts, setSavedHosts] = useState<HostProfile[]>([]);
  const [selected, setSelected] = useState('');
  const [error, setError] = useState('');
  const [readingStatus, setReadingStatus] = useState('Ready');
  const [selectedSources, setSelectedSources] = useState<string[]>([]);
  const [combinedSources, setCombinedSources] = useState<string[]>([]);
  const [opening, setOpening] = useState<Set<string>>(new Set());
  const [closing, setClosing] = useState<Set<string>>(new Set());
  const openingRef = useRef(new Set<string>());
  const closingRef = useRef(new Set<string>());
  const selectionRevision = useRef(0);
  const statusRef = useRef<AppStatus | undefined>(undefined); statusRef.current = status;
  const workspace = useRef<SavedWorkspace | null>(null);
  const workspaceDirty = useRef(false);
  const readerOrder = useRef<string[]>([]);
  const sources = [...(status?.sources ?? [])].sort((a, b) => readerOrder.current.indexOf(a.id) - readerOrder.current.indexOf(b.id));
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
    selectionRevision.current++; workspaceDirty.current = true;
    const readable = [...new Set(ids)].filter(id => statusRef.current?.sources.some(source => source.id === id));
    setSelectedSources(readable);
    if (readable.length > 1) setCombinedSources(readable);
  }
  function selectSourceTab(id: string) {
    const source = statusRef.current?.sources.find(item => item.id === id);
    if (!source) return;
    setScope([id]); setSelected(source.host_id); setPage('workspace');
  }
  async function openTreeSource(entry: LogSource) {
    const key = sourceKey(entry);
    if (openingRef.current.has(key)) return;
    const existing = statusRef.current?.sources.find(source => sourceKey({ hostId: source.host_id, spec: source.spec }) === key);
    if (existing) { selectSourceTab(existing.id); return; }
    const revision = ++selectionRevision.current;
    openingRef.current.add(key); setOpening(new Set(openingRef.current)); setError('');
    try {
      const [opened] = await native.openSources(entry.hostId, [entry.spec]);
      if (!opened?.source_id || opened.error) throw new Error(opened?.error ?? 'Reader could not be opened');
      const id = opened.source_id;
      readerOrder.current = [...new Set([...readerOrder.current, id])];
      await refreshStatus();
      if (alive.current && revision === selectionRevision.current) selectSourceTab(id);
    } catch (reason) { if (alive.current) setError(String(reason)); }
    finally { openingRef.current.delete(key); if (alive.current) setOpening(new Set(openingRef.current)); }
  }
  async function closeLog(id: string) {
    if (closingRef.current.has(id)) return;
    selectionRevision.current++; closingRef.current.add(id); setClosing(new Set(closingRef.current)); setError('');
    try {
      await native.stop(id); workspaceDirty.current = true;
      if (!alive.current) return;
      const currentReaders = statusRef.current?.sources ?? [];
      const index = currentReaders.findIndex(source => source.id === id);
      const neighbor = [...currentReaders.slice(index + 1), ...currentReaders.slice(0, Math.max(0, index)).reverse()].find(source => source.id !== id && !closingRef.current.has(source.id));
      setSelectedSources(previous => { const remaining = previous.filter(sourceId => sourceId !== id); return remaining.length ? remaining : neighbor ? [neighbor.id] : []; });
      setStatus(previous => previous ? { ...previous, sources: previous.sources.filter(source => source.id !== id) } : previous);
      await refreshStatus();
    } catch (reason) { if (alive.current) setError(String(reason)); }
    finally { closingRef.current.delete(id); if (alive.current) setClosing(new Set(closingRef.current)); }
  }
  const queryInput = useRef<HTMLInputElement>(null);
  const viewport = useRef<HTMLDivElement>(null);
  const alive = useRef(false);
  const statusGeneration = useRef(0);

  async function refreshStatus() {
    const generation = ++statusGeneration.current;
    const next = await native.status();
    if (!alive.current || generation !== statusGeneration.current) return;
    readerOrder.current = [...new Set([...readerOrder.current, ...next.sources.map(row => row.id)])].filter(id => next.sources.some(row => row.id === id));
    next.sources.sort((a, b) => readerOrder.current.indexOf(a.id) - readerOrder.current.indexOf(b.id));
    statusRef.current = next; setStatus(next);
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
    if (managementWindow) return;
    let mounted = true;
    const stops: (() => void)[] = [];
    async function loadWorkspace() { const saved = await native.savedWorkspace(); if (mounted) { workspace.current = saved; if (saved.selected_host) setSelected(saved.selected_host); } }
    async function opened(host: string, ids: string[]) {
      readerOrder.current = [...new Set([...readerOrder.current, ...ids])];
      const next = await refreshStatus(); if (!mounted || !next || !ids.length) return;
      setSelected(host); setScope(ids); setPage('workspace');
    }
    const catchError = (reason: unknown) => { if (mounted) setError(String(reason)); };
    void loadWorkspace().catch(catchError);
    void listen<SourcesOpened>('sources-opened', event => void opened(event.payload.host_id, event.payload.source_ids).catch(catchError)).then(stop => mounted ? stops.push(stop) : stop());
    void listen<string>('connection-ready', event => void (async () => {
      const host = event.payload;
      await refreshStatus(); if (!mounted) return; setSelected(host);
      const saved = await native.savedWorkspace(); workspace.current = saved;
      const restored = await native.restoreHostSources(host);
      const ids = restored.flatMap(row => row.source_id ? [row.source_id] : []);
      if (ids.length) {
        await opened(host, ids);
        const byKey = new Map(saved.sources.map(row => [sourceKey({ hostId: row.host_id, spec: row.spec }), row.key]));
        const state = statusRef.current;
        const resolve = (keys: string[]) => (state?.sources ?? []).filter(row => keys.includes(byKey.get(sourceKey({ hostId: row.host_id, spec: row.spec })) ?? '')).map(row => row.id);
        const selection = resolve(saved.selected_sources); const combined = resolve(saved.combined_sources);
        if (selection.length) setSelectedSources(selection); if (combined.length > 1) setCombinedSources(combined);
        const failure = restored.find(row => row.error); if (failure) setError(failure.error ?? 'Some readers need attention');
      } else if (!(await native.rememberedSources(host)).length) await native.openLogPathEditor(host);
    })().catch(catchError)).then(stop => mounted ? stops.push(stop) : stop());
    void listen('workspace-preferences-changed', () => void loadWorkspace().catch(catchError)).then(stop => mounted ? stops.push(stop) : stop());
    return () => { mounted = false; stops.forEach(stop => stop()); };
  }, []);

  useEffect(() => {
    if (managementWindow || !workspaceDirty.current || !workspace.current || !status) return;
    const timer = setTimeout(() => {
      const current = statusRef.current;
      if (!current || current.sources.some(row => !current.connections.some(host => host.host.id === row.host_id && host.state === 'connected'))) return;
      const ids = [...new Set([...readerOrder.current, ...current.sources.map(row => row.id)])].filter(id => current.sources.some(row => row.id === id));
      void native.saveWorkspace({ source_ids: ids, selected_sources: selectedSources, combined_sources: combinedSources, selected_host: selected || null, restore_previous_readers: workspace.current?.restore_previous_readers ?? false })
        .then(() => { workspaceDirty.current = false; }).catch(reason => setError(String(reason)));
    }, 300);
    return () => clearTimeout(timer);
  }, [sourcesKey, selectedSources, combinedSources, selected]);

  const activeProfilesKey = JSON.stringify(status?.connections.map(connection => connection.host) ?? []);
  useEffect(() => {
    let mounted = true;
    let revision = 0;
    let unlisten: (() => void) | undefined;
    async function loadSavedHosts() {
      const request = ++revision;
      try { const hosts = await native.savedHosts(); if (mounted && request === revision) setSavedHosts(hosts); }
      catch (reason) { if (mounted && request === revision) setError(String(reason)); }
    }
    void loadSavedHosts();
    window.addEventListener('focus', loadSavedHosts);
    void listen('connection-profiles-changed', () => void loadSavedHosts()).then(stop => { if (mounted) unlisten = stop; else stop(); }).catch(reason => { if (mounted) setError(String(reason)); });
    return () => { mounted = false; unlisten?.(); window.removeEventListener('focus', loadSavedHosts); };
  }, [activeProfilesKey]);

  const connections: ConnectionInfo[] = [...(status?.connections ?? [])];
  for (const host of savedHosts) if (!connections.some(connection => connection.host.id === host.id)) connections.push({ host, state: 'disconnected', detail: null });
  const connectionsKey = connections.map(connection => connection.host.id).join('|');
  useEffect(() => {
    setSelected(current => connections.some(connection => connection.host.id === current) ? current : connections[0]?.host.id ?? '');
  }, [connectionsKey]);

  useEffect(() => {
    function shortcut(event: KeyboardEvent) {
      if (managementWindow || document.querySelector('dialog[open]') || event.altKey || event.repeat) return;
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'f') {
        event.preventDefault(); setPage('workspace');
        requestAnimationFrame(() => { queryInput.current?.focus(); queryInput.current?.select(); });
      }
    }
    window.addEventListener('keydown', shortcut);
    return () => window.removeEventListener('keydown', shortcut);
  }, []);

  async function run(action: () => Promise<unknown>) {
    setError('');
    const revision = selectionRevision.current;
    try { const result = await action(); const next = await refreshStatus(); if (revision === selectionRevision.current && typeof result === 'string' && next?.sources.some(source => source.id === result)) selectSourceTab(result); }
    catch (reason) { if (alive.current) setError(String(reason)); }
  }
  function changePage(next: Page) {
    if (!managementWindow) {
      const opening = next === 'settings' ? native.openWorkspaceManager('settings') : native.openConnectionEditor();
      void opening.catch(reason => setError(String(reason)));
      return;
    }
    setPage(next);
  }

  if (route.get('manager') === 'add-log') return <AddLogPath hostId={route.get('host') ?? ''} status={status}/>;

  if (connectionEditor) return <div className="connection-editor-shell"><Connections standaloneEditor connections={connections} selected={selected} select={setSelected} run={run}/></div>;

  return <div className={'app-shell' + (managementWindow ? ' management-shell' : ' log-tree-shell')}>
    {!managementWindow && <aside className="app-sidebar">
      <HostLogTree connections={connections} sources={sources} selectedHost={selected} scope={scope} opening={opening} closing={closing} selectHost={setSelected} open={openTreeSource} connect={host => run(() => native.openConnectionEditor(host.id, true))} manageConnections={() => changePage('connections')}/>
      <div className="navigator-tools"><Button variant="ghost" onClick={() => changePage('connections')}>Connections</Button><Button variant="ghost" onClick={() => changePage('settings')}>Settings</Button></div>
    </aside>}
    {managementWindow && <header className="app-header"><div className="page-title"><h1>{page === 'connections' ? 'Connections' : 'Settings'}</h1></div></header>}
    {error && <div role="alert" className="error app-error"><span>{error}</span><Button aria-label="Dismiss error" onClick={() => setError('')}>×</Button></div>}
    <main className="app-main">
      {!managementWindow && <section className="workspace-page" id="page-workspace" role="region" aria-label="Live logs" hidden={page !== 'workspace' || managementWindow}>
        <div className="workspace-body">
          <div className="log-workspace">{!managementWindow && <LiveView onReadingStatus={setReadingStatus} onGetStarted={connections.length ? undefined : () => changePage('connections')} status={status} scope={scope} reading={scope.length > 0} sourceTabs={<SourceTabs status={status} scope={scope} combined={combinedSources} closing={closing} select={selectSourceTab} close={closeLog} combine={setScope}/>} queryInput={queryInput} viewport={viewport} active={page === 'workspace'}/>}</div>
        </div>
      </section>}
      {managementWindow && page === 'connections' && <section className="standalone-page" id="page-connections" role="region" aria-label="Connections"><Connections connections={connections} selected={selected} select={setSelected} run={run}/></section>}
      {managementWindow && page === 'settings' && <section className="standalone-page" id="page-settings" role="region" aria-label="Settings"><Settings status={status}/></section>}
    </main>
    <footer className="app-footer" aria-label="Workspace status"><span>{status ? `${connections.filter(c => c.state === 'connected').length}/${connections.length} hosts connected · ${sources.length} reading` : 'Connecting to local runtime…'}{connections.some(c => c.state === 'needs_action' || c.state === 'reconnecting') && ' · Host needs attention'}</span><span title={readingStatus}>{managementWindow ? 'Local settings' : readingStatus}</span></footer>
  </div>;
}

createRoot(document.getElementById('root')!).render(<React.StrictMode><App/></React.StrictMode>);
