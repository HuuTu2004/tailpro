import { useEffect, useRef, useState } from 'react';
import type { AppStatus, Candidate, DirectoryPage, DiscoveryPreferences, DiscoveryResult, HostCapabilities, OpenSourceResult, SourceSpec } from '../../shared/contracts/generated';
import { native } from '../../shared/client/native';
import { Button, TextField } from '../../shared/ui/Controls';
import { sourceKey, sourceLabel, sourcePath } from './source-labels';
import './add-log-path.css';

type Mode = 'found' | 'browse' | 'path';
type Row = { spec: SourceSpec; label: string; path: string; access: string; activity: string; size?: string | null; archive: boolean; provenance: string };
const directoryOf = (path: string) => path.slice(0, path.lastIndexOf('/')) || '/';
const compressed = (path: string) => /\.(gz|bz2|xz|zip|zst|lz4)$/i.test(path);
const readable = (row: Row) => !['denied', 'unsupported', 'missing'].includes(row.access);
const sizeLabel = (value?: string | null) => !value ? '' : Number(value) < 1024 ? `${value} B` : Number(value) < 1024 * 1024 ? `${(Number(value) / 1024).toFixed(1)} KiB` : `${(Number(value) / 1024 / 1024).toFixed(1)} MiB`;
function candidateRow(candidate: Candidate): Row {
  const spec: SourceSpec = candidate.container_id ? { kind: 'docker', container_id: candidate.container_id, aliases: [candidate.path, ...candidate.tags].slice(0, 16) } : candidate.provider === 'systemd_unit' ? { kind: 'journal', unit: candidate.path } : { kind: 'file', path: candidate.path };
  return { spec, label: candidate.container_id ? candidate.path : sourceLabel(spec), path: candidate.path, access: candidate.access, activity: candidate.activity, size: candidate.size, archive: candidate.tags.includes('archive_hint'), provenance: candidate.provenance };
}
function rememberedRow(spec: SourceSpec): Row {
  return { spec, label: sourceLabel(spec), path: sourcePath(spec), access: spec.kind === 'file' && compressed(spec.path) ? 'unsupported' : 'unknown', activity: 'Remembered · Rechecked on Open', archive: false, provenance: 'Previously opened on this host identity' };
}

export function AddLogPath({ hostId, status }: { hostId: string; status?: AppStatus }) {
  const host = status?.connections.find(item => item.host.id === hostId);
  const [capabilities, setCapabilities] = useState<HostCapabilities>();
  const [preferences, setPreferences] = useState<DiscoveryPreferences>({ initialized: false, directories: [], docker: false, journal: false });
  const [remembered, setRemembered] = useState<SourceSpec[]>([]);
  const [result, setResult] = useState<DiscoveryResult>();
  const [mode, setMode] = useState<Mode>('found');
  const [search, setSearch] = useState('');
  const [selected, setSelected] = useState<Map<string, Row>>(new Map());
  const [extraRows, setExtraRows] = useState<Map<string, Row>>(new Map());
  const [directory, setDirectory] = useState<DirectoryPage>();
  const [path, setPath] = useState('');
  const [suggestions, setSuggestions] = useState<Candidate[]>([]);
  const [busy, setBusy] = useState(false);
  const [browsing, setBrowsing] = useState(false);
  const [opening, setOpening] = useState(false);
  const [opened, setOpened] = useState<OpenSourceResult[]>([]);
  const [error, setError] = useState('');
  const [restore, setRestore] = useState(false);
  const alive = useRef(false);
  const scanRevision = useRef(0);
  const browseRevision = useRef(0);
  const completionRevision = useRef(0);
  const modeRevision = useRef(0);
  const openingRef = useRef(false);
  const key = (spec: SourceSpec) => sourceKey({ hostId, spec });
  const currentPreferences = useRef(preferences); currentPreferences.current = preferences;

  async function scan(scope: DiscoveryPreferences, refresh = false) {
    const revision = ++scanRevision.current;
    setBusy(true); setError('');
    try {
      if (refresh) {
        const cap = await native.inspectHost(hostId, true);
        if (!alive.current || revision !== scanRevision.current) return;
        setCapabilities(cap);
        if (!cap.helper_ready) return;
      }
      const found = await native.discover({ host_id: hostId, directories: scope.directories, project_roots: [], docker: scope.docker, journal: scope.journal, var_log: false, pm2_root: null, selected_pids: [], selected_units: [], config_files: [], config_roots: [], config_consent: false });
      if (alive.current && revision === scanRevision.current) setResult(found);
    } catch (reason) { if (alive.current && revision === scanRevision.current) setError(String(reason)); }
    finally { if (alive.current && revision === scanRevision.current) setBusy(false); }
  }
  useEffect(() => {
    alive.current = true;
    if (host?.state !== 'connected') return () => { alive.current = false; };
    let mounted = true;
    setBusy(true);
    void (async () => {
      try {
        const [cap, saved, history, workspace] = await Promise.all([native.inspectHost(hostId), native.discoveryPreferences(hostId), native.rememberedSources(hostId), native.savedWorkspace()]);
        if (!mounted) return;
        setCapabilities(cap); setRemembered(history); setRestore(workspace.restore_previous_readers);
        const defaults = [...new Set([...history.filter((s): s is Extract<SourceSpec, { kind: 'file' }> => s.kind === 'file').map(s => directoryOf(s.path)), ...cap.locations.map(location => location.resolved_path)])].slice(0, 16);
        const scope = saved.initialized ? saved : { initialized: true, directories: defaults, docker: cap.providers.some(p => p.kind === 'docker' && p.state === 'ready'), journal: cap.providers.some(p => p.kind === 'journal' && p.state === 'ready') };
        setPreferences(scope);
        if (cap.helper_ready) await scan(scope);
        else setBusy(false);
      } catch (reason) { if (mounted) { setError(String(reason)); setBusy(false); } }
    })();
    return () => { mounted = false; alive.current = false; scanRevision.current++; browseRevision.current++; completionRevision.current++; void native.cancelDiscovery(hostId).catch(() => {}); };
  }, [hostId, host?.state]);

  async function saveScope(next: DiscoveryPreferences, rescan = true) {
    next = { ...next, initialized: true };
    try {
      await native.saveDiscoveryPreferences(hostId, next);
      if (!alive.current) return;
      setPreferences(next);
      if (rescan) await scan(next);
    } catch (reason) { if (alive.current) setError(String(reason)); }
  }
  async function browse(path: string, cursor: string | null = null) {
    const revision = ++browseRevision.current; setBrowsing(true); setError('');
    try {
      const page = await native.browseDirectory(hostId, path, cursor);
      if (!alive.current || revision !== browseRevision.current) return;
      setDirectory(previous => cursor && previous?.path === page.path ? { ...page, entries: [...previous.entries, ...page.entries] } : page);
    } catch (reason) { if (alive.current && revision === browseRevision.current) setError(String(reason)); }
    finally { if (alive.current && revision === browseRevision.current) setBrowsing(false); }
  }
  async function switchMode(next: Mode) {
    const revision = ++modeRevision.current; scanRevision.current++; browseRevision.current++; completionRevision.current++;
    try { await native.cancelDiscovery(hostId); }
    catch (reason) { setError(String(reason)); }
    if (!alive.current || revision !== modeRevision.current) return;
    setBusy(false); setBrowsing(false); setMode(next);
    if (next === 'browse' && !directory && capabilities?.home) await browse(capabilities.home);
  }
  useEffect(() => {
    const revision = ++completionRevision.current;
    if (mode !== 'path' || !path.startsWith('/')) { setSuggestions([]); return; }
    const timer = setTimeout(() => {
      void native.complete(hostId, directoryOf(path), path.slice(path.lastIndexOf('/') + 1)).then(found => { if (alive.current && revision === completionRevision.current) setSuggestions(found.candidates); }).catch(() => { if (alive.current && revision === completionRevision.current) setSuggestions([]); });
    }, 200);
    return () => clearTimeout(timer);
  }, [path, mode, hostId]);

  const rows = new Map<string, Row>();
  for (const spec of remembered) rows.set(key(spec), rememberedRow(spec));
  for (const candidate of result?.candidates ?? []) { const row = candidateRow(candidate); rows.set(key(row.spec), row); }
  for (const [id, row] of extraRows) if (!rows.has(id)) rows.set(id, row);
  const needle = search.toLocaleLowerCase();
  const visible = [...rows.values()].filter(row => `${row.label} ${row.path}`.toLocaleLowerCase().includes(needle));
  const archives = visible.filter(row => row.archive);
  const current = visible.filter(row => !row.archive);
  const activeKeys = new Set(status?.sources.filter(source => source.host_id === hostId).map(source => key(source.spec)));
  const slots = Math.max(0, (capabilities?.max_sources ?? 0) - (status?.sources.length ?? 0));
  const newCount = [...selected.keys()].filter(id => !activeKeys.has(id)).length;
  function toggle(row: Row) {
    setSelected(previous => { const next = new Map(previous); const id = key(row.spec); if (next.has(id)) next.delete(id); else next.set(id, row); return next; });
  }
  function selectPath() {
    const value = path.trim();
    if (!value.startsWith('/') || /[\r\n\0]/.test(value)) { setError('Enter an absolute file path.'); return; }
    const exact = suggestions.find(candidate => candidate.path === value);
    const row = exact ? candidateRow(exact) : rememberedRow({ kind: 'file', path: value });
    if (!readable(row)) { setError('This file is unavailable or its compressed format is not supported.'); return; }
    setExtraRows(previous => new Map(previous).set(key(row.spec), row));
    setSelected(previous => new Map(previous).set(key(row.spec), row)); setError('');
  }
  async function openSelected() {
    if (openingRef.current || !selected.size) return;
    openingRef.current = true; setOpening(true); setError(''); setOpened([]);
    try {
      await native.saveDiscoveryPreferences(hostId, currentPreferences.current);
      const results = await native.openSources(hostId, [...selected.values()].map(row => row.spec));
      if (!alive.current) return;
      setOpened(results);
      if (results.length && results.every(result => result.state === 'streaming' && !result.error)) await native.closeLogPathEditor();
    } catch (reason) { if (alive.current) setError(String(reason)); }
    finally { openingRef.current = false; if (alive.current) setOpening(false); }
  }
  async function close() { await native.cancelSourceOpen(hostId).catch(() => {}); await native.cancelDiscovery(hostId).catch(() => {}); await native.closeLogPathEditor(); }
  const renderRow = (row: Row) => <label className={'picker-source' + (!readable(row) ? ' unavailable' : '')} key={key(row.spec)}>
    <input type="checkbox" aria-label={`Select ${row.path}`} checked={selected.has(key(row.spec))} disabled={opening || !readable(row)} onChange={() => toggle(row)}/>
    <span className="picker-source-name"><strong>{row.label}</strong><small title={row.provenance}>{row.path}</small></span>
    <span className="picker-source-type">{row.spec.kind === 'journal' ? 'Journal' : row.spec.kind === 'docker' ? 'Docker' : 'File'}{sizeLabel(row.size) && <small>{sizeLabel(row.size)}</small>}</span>
    <span className="picker-source-activity">{activeKeys.has(key(row.spec)) ? 'Open' : row.activity}</span>
    <span className="picker-source-access">{row.access === 'ready' ? 'Readable' : row.access === 'unsupported' ? 'Unsupported archive' : row.access === 'unknown' ? 'Recheck on Open' : row.access}</span>
  </label>;
  return <main className="source-picker" aria-label="Open logs" onKeyDown={event => { if (event.key === 'Escape') { event.preventDefault(); void close().catch(reason => setError(String(reason))); } }}>
    <header className="source-picker-header"><div><h1>Open logs</h1><p>{host?.host.target ?? 'Selected host'}{host?.host.environment === 'production' && <span className="badge production">Production</span>}<span className="hint">{host?.state === 'connected' ? 'SSH connected' : host?.state ?? 'Checking connection…'}{capabilities && ` · ${capabilities.os}`}</span></p></div><details className="picker-capabilities"><summary>Host details</summary><div>{capabilities?.providers.map(provider => <p key={provider.kind}><strong>{provider.kind}</strong> · {provider.state}<small>{provider.detail}</small></p>)}{capabilities?.issues.map(issue => <p key={issue}>{issue}</p>)}</div></details></header>
    <nav className="source-picker-tabs" aria-label="Find log sources">{([{ id: 'found', label: 'Found sources' }, { id: 'browse', label: 'Browse folders' }, { id: 'path', label: 'Enter path' }] as const).map(tab => <Button key={tab.id} aria-pressed={mode === tab.id} disabled={opening || !capabilities?.helper_ready} onClick={() => void switchMode(tab.id)}>{tab.label}</Button>)}</nav>
    {error && <p role="alert" className="notice">{error}</p>}
    {capabilities && !capabilities.helper_ready && <p className="notice">{capabilities.issues.join(' ')}<Button onClick={() => void scan(preferences, true)}>Check again</Button></p>}
    <div className="source-picker-body">
      {mode === 'found' && <><div className="picker-search"><input aria-label="Search found sources" placeholder="Search found sources…" value={search} onChange={event => setSearch(event.target.value)}/><Button disabled={busy || opening || !capabilities?.helper_ready} onClick={() => void scan(preferences, true)}>Refresh</Button>{busy && <Button onClick={() => { scanRevision.current++; setBusy(false); void native.cancelDiscovery(hostId).catch(reason => setError(String(reason))); }}>Cancel scan</Button>}</div>
        <div className="picker-locations"><span>Locations</span>{preferences.directories.map(location => <span className="picker-location" key={location} title={location}>{location}<Button disabled={opening} aria-label={`Remove location ${location}`} onClick={() => void saveScope({ ...preferences, directories: preferences.directories.filter(path => path !== location) })}>×</Button></span>)}<Button disabled={opening || !capabilities?.helper_ready} onClick={() => void switchMode('browse')}>+ Add location</Button></div>
        <details className="picker-more"><summary>More locations &amp; sources</summary><div className="picker-more-content">{capabilities?.providers.filter(provider => provider.kind !== 'file' && provider.state === 'ready').map(provider => <label key={provider.kind}><input type="checkbox" checked={provider.kind === 'docker' ? preferences.docker : preferences.journal} disabled={opening} onChange={event => void saveScope({ ...preferences, [provider.kind === 'docker' ? 'docker' : 'journal']: event.target.checked })}/>{provider.kind === 'docker' ? 'Running Docker containers' : 'Running services / Journal'}</label>)}<label><input type="checkbox" checked={restore} disabled={opening} onChange={event => { const enabled = event.target.checked; void native.setRestoreReaders(enabled).then(() => setRestore(enabled)).catch(reason => setError(String(reason))); }}/>Restore previous readers after I reconnect</label></div></details>
        <div className="picker-list-heading"><span>Source</span><span>Type</span><span>Activity</span><span>Access</span></div>
        <div className="picker-source-list">{current.map(renderRow)}{archives.length > 0 && <details className="picker-archives"><summary>Older files ({archives.length}) · name/path hints</summary>{archives.map(renderRow)}</details>}{!visible.length && <div className="picker-empty"><h3>{busy ? 'Finding sources…' : 'No sources found in these locations'}</h3><p>Browse folders or enter a file path to add a source.</p></div>}</div>
        {result && <details className={'picker-coverage' + (!result.complete ? ' partial' : '')}><summary>{result.complete ? 'Scanned selected locations' : 'Some locations or providers were not fully scanned'} · metadata only</summary>{result.coverage.map((coverage, i) => <p key={i}>{coverage.location} · {coverage.state}{coverage.detail && ` · ${coverage.detail}`}</p>)}{result.reasons.map(reason => <p key={reason}>{reason}</p>)}</details>}
      </>}
      {mode === 'browse' && <div className="picker-browser"><div className="picker-breadcrumb"><Button disabled={!directory?.parent || browsing} onClick={() => directory?.parent && void browse(directory.parent)}>↑ Up</Button><span title={directory?.path}>{directory?.path ?? capabilities?.home ?? 'Choose a directory'}</span><Button disabled={browsing || !directory} onClick={() => directory && void browse(directory.path)}>Refresh</Button></div>
        {directory?.resolved_path && <p className="notice">This location resolves to {directory.resolved_path}.<Button onClick={() => void browse(directory.resolved_path!)}>Use resolved location</Button></p>}
        {directory && !directory.resolved_path && <Button disabled={opening || preferences.directories.includes(directory.path) || preferences.directories.length >= 16} onClick={() => { const next = { ...preferences, directories: [...preferences.directories, directory.path] }; setMode('found'); void saveScope(next); }}>Add this folder to locations</Button>}
        {browsing && <p role="status" className="hint">Reading directory metadata…</p>}
        <div className="picker-directory-list">{directory?.entries.map(entry => entry.kind === 'directory' ? <Button className="picker-directory" key={entry.path} disabled={browsing || opening} onClick={() => void browse(entry.path)}><span>▸</span>{entry.name}<small>Folder</small></Button> : entry.kind === 'symlink' ? <Button className="picker-directory" key={entry.path} disabled={browsing || opening} onClick={() => void browse(entry.path)}><span>↗</span>{entry.name}<small>Symbolic link</small></Button> : entry.kind === 'file' ? renderRow({ ...rememberedRow({ kind: 'file', path: entry.path }), access: compressed(entry.path) ? 'unsupported' : entry.access, activity: 'Metadata only', size: entry.size }) : <p className="hint" key={entry.path}>{entry.name} · Unsupported file type</p>)}</div>
        {directory?.next_cursor && <Button disabled={browsing} onClick={() => void browse(directory.path, directory.next_cursor)}>Load more entries</Button>}
        {directory?.reasons.map(reason => <p className="hint" key={reason}>{reason}</p>)}
      </div>}
      {mode === 'path' && <form className="picker-path" onSubmit={event => { event.preventDefault(); selectPath(); }}><TextField label="Absolute log file path" autoFocus value={path} onChange={event => setPath(event.target.value)} disabled={opening} list="source-path-suggestions" autoComplete="off" maxLength={4096}/><datalist id="source-path-suggestions">{suggestions.map(candidate => <option key={candidate.id} value={candidate.path}/>)}</datalist><p className="hint">Choose a file, then Open selected. The reader checks its type, identity and permissions.</p><Button type="submit" disabled={opening || !path.trim().startsWith('/')}>Select file</Button>{[...selected.values()].map(renderRow)}</form>}
      {opened.length > 0 && <div className="picker-open-results" role="status">{opened.map(result => <p key={key(result.spec)} className={result.state === 'streaming' ? 'success' : 'failure'}>{sourceLabel(result.spec)} · {result.state === 'streaming' ? 'Opened successfully' : result.state}{result.error && ` · ${result.error}`}</p>)}</div>}
    </div>
    <footer className="source-picker-footer"><span role="status">{opening ? 'Opening selected sources…' : `${selected.size} selected · ${slots} new reader slots available`}{newCount > slots && ' · Close a reader or select fewer sources'}</span><div><Button onClick={() => void close().catch(reason => setError(String(reason)))}>{opened.some(result => result.state === 'streaming') ? 'Continue with opened logs' : 'Cancel'}</Button><Button variant="primary" disabled={opening || !selected.size || newCount > slots || host?.state !== 'connected'} busy={opening} onClick={() => void openSelected()}>{opening ? 'Opening…' : opened.some(result => result.state !== 'streaming') ? `Retry selected (${selected.size})` : `Open selected (${selected.size})`}</Button></div></footer>
  </main>;
}
