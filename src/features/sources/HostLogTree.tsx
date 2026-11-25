import { Icon } from '../../shared/ui/Icon';
import { Button } from '../../shared/ui/Controls';
import { useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import type { AddLogPathRequest, ConnectionInfo, HostProfile, SourceSpec, SourceInfo } from '../../shared/contracts/generated';
import { native } from '../../shared/client/native';
import { sourceKey, sourceLabel, sourcePath, type LogSource } from './source-labels';
import './host-log-tree.css';

type Props = {
  connections: ConnectionInfo[];
  sources: SourceInfo[];
  selectedHost: string;
  scope: string[];
  opening: Set<string>;
  closing: Set<string>;
  selectHost: (id: string) => void;
  open: (source: LogSource) => void;
  connect: (host: HostProfile) => Promise<void>;
  manageConnections: () => void;
};
const HISTORY_LIMIT = 128;
export function HostLogTree({ connections, sources, selectedHost, scope, opening, closing, selectHost, open, connect, manageConnections }: Props) {
  const [known, setKnown] = useState<Map<string, LogSource>>(new Map());
  const [pins, setPins] = useState<Record<string, SourceSpec[]>>({});
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [focused, setFocused] = useState('');
  const [filter, setFilter] = useState('');
  const [editorReady, setEditorReady] = useState(false);
  const [pathError, setPathError] = useState('');
  const connectingRef = useRef(new Set<string>());
  const addRequestedPath = useRef<(hostId: string, path: string) => string | null>(() => 'The host navigator is not ready.');
  const rememberAfterRead = useRef(new Set<string>());
  const hostsKey = connections.map(host => `${host.host.id}:${host.state}`).join('|');
  const sourcesKey = JSON.stringify(sources.map(source => [source.host_id, source.spec]));
  const active = new Map(sources.map(source => [sourceKey({ hostId: source.host_id, spec: source.spec }), source]));
  useEffect(() => {
    setKnown(previous => {
      const next = new Map(previous);
      let changed = false;
      for (const source of sources) {
        const entry = { hostId: source.host_id, spec: source.spec };
        const key = sourceKey(entry);
        if (!next.has(key) || JSON.stringify(next.get(key)?.spec) !== JSON.stringify(entry.spec)) { next.set(key, entry); changed = true; }
      }
      for (const [key] of next) {
        if (next.size <= HISTORY_LIMIT) break;
        if (!active.has(key)) { next.delete(key); changed = true; }
      }
      return changed ? next : previous;
    });
  }, [sourcesKey]);
  useEffect(() => {
    let alive = true;
    const connected = connections.filter(host => host.state === 'connected');
    // Serial metadata reads keep many expanded hosts from starting a request burst.
    void (async () => {
      const paths: Record<string, SourceSpec[]> = {};
      for (const host of connected) {
        if (!alive) return;
        try { paths[host.host.id] = (await native.rememberedSources(host.host.id)).slice(0, 100); }
        catch { /* Already known logs remain usable if saved paths are unavailable. */ }
      }
      if (alive) setPins(paths);
    })();
    return () => { alive = false; };
  }, [hostsKey]);
  useEffect(() => { if (selectedHost) setExpanded(previous => new Set([...previous, selectedHost])); }, [selectedHost]);

  const entries = new Map(known);
  for (const [hostId, paths] of Object.entries(pins)) for (const value of paths) {
    const entry: LogSource = { hostId, spec: value };
    entries.set(sourceKey(entry), entry);
  }
  for (const source of sources) {
    const entry = { hostId: source.host_id, spec: source.spec };
    entries.set(sourceKey(entry), entry);
  }
  const readerStates = sources.map(source => `${source.id}:${source.state}`).join('|');
  useEffect(() => {
    let alive = true;
    for (const source of sources) {
      if (source.state !== 'streaming' || source.spec.kind !== 'file') continue;
      const key = sourceKey({ hostId: source.host_id, spec: source.spec });
      if (!rememberAfterRead.current.delete(key)) continue;
      const value = source.spec.path;
      // Existing native policy pins only a file that has actually started successfully.
      void native.pin(source.host_id, value).then(() => {
        if (alive) setPins(previous => ({ ...previous, [source.host_id]: [...new Set([...(previous[source.host_id] ?? []), { kind: 'file' as const, path: value }])].slice(-100) }));
      }).catch(() => { /* Keep the session entry even if it cannot be remembered on disk. */ });
    }
    return () => { alive = false; };
  }, [readerStates]);
  const needle = filter.trim().toLowerCase();
  async function reconnect(host: HostProfile) {
    if (connectingRef.current.has(host.id)) return;
    connectingRef.current.add(host.id); selectHost(host.id);
    try { await connect(host); }
    finally { connectingRef.current.delete(host.id); }
  }
  async function startAdding(hostId: string) {
    selectHost(hostId); setExpanded(previous => new Set([...previous, hostId])); setPathError('');
    try { await native.openLogPathEditor(hostId); }
    catch (reason) { setPathError(String(reason)); }
  }
  addRequestedPath.current = (hostId, value) => {
    if (!connections.some(connection => connection.host.id === hostId)) return 'This host is no longer available. Reconnect it before adding a log.';
    if (value.length > 4096 || !value.startsWith('/') || /[\r\n\0]/.test(value)) return 'Enter an absolute log file path.';
    const entry: LogSource = { hostId, spec: { kind: 'file', path: value } };
    const key = sourceKey(entry);
    if (!entries.has(key) && known.size >= HISTORY_LIMIT) return 'Too many remembered logs in this session. Reopen a known log or restart the app to clear session history.';
    rememberAfterRead.current.add(key); setKnown(previous => new Map(previous).set(key, entry)); setFocused(key); setFilter('');
    selectHost(hostId); setExpanded(previous => new Set([...previous, hostId]));
    // Adding a tree entry grants no read permission. Double-click/Enter starts it.
    return null;
  };
  useEffect(() => {
    let alive = true;
    let unlisten: (() => void) | undefined;
    void listen<AddLogPathRequest>('log-path-submit', event => {
      if (!alive) return;
      const { request_id, host_id, path } = event.payload;
      const error = addRequestedPath.current(host_id, path);
      void native.finishLogPath(request_id, error).catch(reason => { if (alive) setPathError(String(reason)); });
    }).then(stop => { if (alive) { unlisten = stop; setEditorReady(true); } else stop(); }).catch(reason => { if (alive) setPathError(String(reason)); });
    return () => { alive = false; unlisten?.(); };
  }, []);
  return <section className="log-navigator" aria-label="Host log navigator">
    <div className="log-navigator-heading"><h2>HOSTS</h2><Button className="sidebar-add" title="Manage connections" aria-label="Manage connections" onClick={manageConnections}>+</Button></div>
    {(connections.length > 3 || entries.size > 6 || filter) && <div className="log-tree-search"><label>Filter hosts or logs<input value={filter} onChange={event => setFilter(event.target.value)} placeholder="Filter hosts or logs…" aria-label="Filter hosts or log paths"/></label>{filter && <Button aria-label="Clear navigator filter" onClick={() => setFilter('')}>×</Button>}</div>}
    <div className="log-tree-hosts">
      {connections.filter(connection => !needle || connection.host.target.toLowerCase().includes(needle) || [...entries.values()].some(entry => entry.hostId === connection.host.id && `${sourcePath(entry.spec)} ${sourceLabel(entry.spec)}`.toLowerCase().includes(needle))).map(connection => {
        const host = connection.host;
        const hostEntries = [...entries.entries()].filter(([, entry]) => entry.hostId === host.id).sort(([, first], [, second]) => sourceLabel(first.spec).localeCompare(sourceLabel(second.spec)));
        const hostMatches = host.target.toLowerCase().includes(needle);
        const visible = hostEntries.filter(([, entry]) => !needle || hostMatches || `${sourcePath(entry.spec)} ${sourceLabel(entry.spec)}`.toLowerCase().includes(needle));
        const isExpanded = expanded.has(host.id) || Boolean(needle);
        return <div className="log-tree-host" key={host.id}>
          <div className={'log-tree-host-heading' + (selectedHost === host.id ? ' selected' : '')}>
            <Button className="log-tree-host-toggle" aria-label={`${isExpanded ? 'Collapse' : 'Expand'} ${host.target} logs`} aria-expanded={isExpanded} title={`${host.target} · ${host.environment} · ${hostEntries.length} known logs`} onClick={() => { selectHost(host.id); if (connection.state === 'disconnected' || connection.state === 'needs_action') void reconnect(host); setExpanded(previous => { const next = new Set(previous); if (isExpanded) next.delete(host.id); else next.add(host.id); return next; }); }}><span className="tree-chevron" aria-hidden="true"><Icon name={isExpanded ? 'chevron-down' : 'chevron-right'}/></span><span className={'dot ' + connection.state}/><strong>{host.target}</strong>{host.environment === 'production' && <span className="tree-prod" aria-label="Production" title="Production">●</span>}<span className="tree-log-count">{hostEntries.length}</span></Button>
            <Button className="log-tree-add" aria-label={`Open logs on ${host.target}`} title={connection.state === 'connected' ? 'Open logs' : 'Connect this host to add a log'} disabled={!editorReady || connection.state !== 'connected'} onClick={() => startAdding(host.id)}>+</Button>
          </div>
          {connection.state !== 'connected' && <div className="log-tree-connection"><span title={connection.detail ?? `${host.user ? `${host.user}@` : ''}${host.target}:${host.port ?? 22}`}>{connection.state === 'disconnected' ? 'Saved · Disconnected' : connection.state.replaceAll('_', ' ')}</span></div>}
          {isExpanded && <div className="log-tree-children" role="group" aria-label={`${host.target} logs`}>
            {visible.map(([key, entry]) => {
              const reading = active.get(key);
              const isOpening = opening.has(key);
              const isClosing = reading ? closing.has(reading.id) : false;
              return <Button className={'log-tree-source' + (reading && scope.includes(reading.id) ? ' active' : '') + (focused === key ? ' focused' : '')} key={key} title={`${sourcePath(entry.spec)}\n${isClosing ? 'Stopping…' : isOpening ? 'Starting…' : reading ? reading.state.replaceAll('_', ' ') : 'Double-click or press Enter to start reading'}`} aria-label={`${sourceLabel(entry.spec)} on ${host.target}${host.environment === 'production' ? ', Production' : ''}${reading ? ', reading' : ', closed'}`} disabled={isOpening || isClosing || connection.state !== 'connected'} onClick={() => { setFocused(key); selectHost(host.id); }} onDoubleClick={() => open(entry)} onKeyDown={event => { if (event.key === 'Enter') { event.preventDefault(); open(entry); } }}><span className={'tree-source-state' + (reading ? ' reading' : '')} aria-hidden="true">{isOpening || isClosing ? '…' : <Icon name={entry.spec.kind === 'file' ? 'sources' : entry.spec.kind}/>}</span><span>{sourceLabel(entry.spec)}</span>{reading && <i className={'tree-reading-mark' + (/error|fail|needs_action/i.test(reading.state) ? ' attention' : '')} aria-label={reading.state}/>}</Button>;
            })}
            {!visible.length && <p className="log-tree-empty">{needle ? 'No matching logs' : connection.state === 'connected' ? 'No logs added yet' : 'Connect to view remembered logs.'}</p>}
          </div>}
        </div>;
      })}
      {!connections.length && <p className="log-tree-empty">Add a connection to browse its logs.</p>}
      {connections.length > 0 && needle && !connections.some(host => host.host.target.toLowerCase().includes(needle) || [...entries.values()].some(entry => entry.hostId === host.host.id && `${sourcePath(entry.spec)} ${sourceLabel(entry.spec)}`.toLowerCase().includes(needle))) && <p className="log-tree-empty">No matching hosts or logs.</p>}
    </div>
    {pathError && <p role="alert" className="notice">{pathError}</p>}
    <p className="log-tree-help">Double-click a log to open it.</p>
  </section>;
}
