import { IncidentExport } from '../export/IncidentExport';
import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode, type RefObject } from 'react';
import { native } from '../../shared/client/native';
import type { AppStatus, ViewNotice, ViewPage, ViewRow } from '../../shared/contracts/generated';
import { QueryBar } from '../query/QueryBar';
import { Inspector } from '../inspector/Inspector';
import { ResizableViewer } from './ResizableViewer';
import { LOG_ROW_HEIGHT as HEIGHT, LOG_HEADER_HEIGHT } from './geometry';
import { HighlightedMessage, messageNeedles, Timestamp } from './LogText';
import { Button } from '../../shared/ui/Controls';
import { Menu } from '../../shared/ui/Menu';
import { TimeFilter, timeQuery, type TimeRange } from './TimeFilter';
import './workspace.css';
type Props = { onGetStarted?: () => void; status?: AppStatus; scope: string[]; sourceTabs: ReactNode; reading: boolean; queryInput: RefObject<HTMLInputElement | null>; viewport: RefObject<HTMLDivElement | null>; active?: boolean; onReadingStatus?: (status: string) => void; onInspect?: () => void; inspectorVisible?: boolean };
const columns = ['time', 'server', 'source', 'level', 'message', 'trace_id', 'duration_ms'] as const;
type Column = typeof columns[number];
const columnLabels: Record<Column, string> = { time: 'Time', server: 'Host', source: 'Source', level: 'Level', message: 'Message', trace_id: 'Trace ID', duration_ms: 'Duration (ms)' };
const pageSizes = new WeakMap<ViewPage, number>();
const pageMessageWidths = new WeakMap<ViewPage, number>();
function messageWidth(page: ViewPage) {
  const cached = pageMessageWidths.get(page);
  if (cached !== undefined) return cached;
  let width = 300;
  for (const event of page.events) width = Math.max(width, event.message.length * (/[^\x00-\x7f]/.test(event.message) ? 14 : 8) + 24);
  pageMessageWidths.set(page, width);
  return width;
}
function bytes(page: ViewPage) { const cached = pageSizes.get(page); if (cached !== undefined) return cached; const size = JSON.stringify(page).length * 2 + page.events.length * 512; pageSizes.set(page, size); return size; }
export function LiveView({ onGetStarted, status, scope, sourceTabs, reading, queryInput, viewport, active = true, onReadingStatus, onInspect, inspectorVisible = true }: Props) {
  const [viewId, setViewId] = useState('');
  const [session, setSession] = useState(0);
  const [query, setQuery] = useState('');
  const [timeRange, setTimeRange] = useState<TimeRange>({ from: '', to: '' });
  const [level, setLevel] = useState('');
  const effectiveQuery = useMemo(() => [query.trim(), ...timeQuery(timeRange), level ? `level:${level}` : ''].filter(Boolean).join(' AND '), [query, timeRange, level]);
  const [renderedQuery, setRenderedQuery] = useState('');
  const needles = useMemo(() => messageNeedles(renderedQuery), [renderedQuery]);
  const [pages, setPages] = useState<ViewPage[]>([]);
  const [notice, setNotice] = useState<ViewNotice>();
  const [error, setError] = useState('');
  const [paused, setPaused] = useState(false);
  const [arrivals, setArrivals] = useState('0');
  const [expired, setExpired] = useState(false);
  const [busy, setBusy] = useState(false);
  const [selection, setSelection] = useState('');
  const [activeRow, setActiveRow] = useState('');
  const [viewportWidth, setViewportWidth] = useState(0);
  const [horizontalScroll, setHorizontalScroll] = useState(0);
  const [scroll, setScroll] = useState(0);
  const [visibleHeight, setVisibleHeight] = useState(432);
  const [shown, setShown] = useState<Column[]>(['time', 'level', 'source', 'message']);
  const generation = useRef(0);
  const detailGeneration = useRef(0);
  const epoch = useRef(0);
  const pausedRef = useRef(false);
  const anchor = useRef<string | null>(null);
  const readingAnchor = useRef<string | null>(null);
  const needsVerify = useRef(false);
  const debounceUntil = useRef(0);
  const baseline = useRef(0n);
  const inFlight = useRef(false);
  const pageRevision = useRef('');
  const noticeRef = useRef<ViewNotice | undefined>(undefined);
  const pagesRef = useRef<ViewPage[]>([]);
  const current = useRef({ query: effectiveQuery, scope, viewId }); current.current = { query: effectiveQuery, scope, viewId };
  const fetchRef = useRef<(kind?: 'reset' | 'older' | 'verify') => Promise<void>>(async () => {});
  const configKey = JSON.stringify(scope);
  useEffect(() => { setSelection(''); setActiveRow(''); pausedRef.current = false; setPaused(false); setArrivals('0'); anchor.current = null; readingAnchor.current = null; needsVerify.current = false; }, [configKey]);
  const nextDetail = useCallback(() => ++detailGeneration.current, []);
  const rowName = (row: ViewRow) => {
    const source = status?.sources.find(s => s.id === row.source);
    return source ? source.spec.kind === 'file' ? source.spec.path : source.spec.kind === 'journal' ? source.spec.unit : source.spec.kind === 'kubernetes' ? `${source.spec.namespace}/${source.spec.pod}/${source.spec.container}` : source.spec.container_id : row.source;
  };
  const environmentFor = (row: ViewRow) => { const source = status?.sources.find(s => s.id === row.source); return status?.connections.find(c => c.host.id === source?.host_id)?.host.environment ?? row.environment; };
  function rememberReadingAnchor(scrollTop: number) {
    let index = Math.max(0, Math.floor((scrollTop - LOG_HEADER_HEIGHT) / HEIGHT));
    for (const page of pagesRef.current) {
      if (index < page.events.length) {
        const id = page.events[index].id;
        if (readingAnchor.current !== id) needsVerify.current = true;
        readingAnchor.current = id; return;
      }
      index -= page.events.length;
    }
  }
  function pause() {
    if (pausedRef.current) return;
    pausedRef.current = true; setPaused(true); setArrivals('0');
    baseline.current = BigInt(noticeRef.current?.accepted ?? '0');
    anchor.current = pagesRef.current.at(-1)?.events.at(-1)?.id ?? null;
    rememberReadingAnchor(viewport.current?.scrollTop ?? 0);
    epoch.current++; const cancelled = ++generation.current; inFlight.current = false; setBusy(false);
    if (current.current.viewId) void native.cancelPage(current.current.viewId, cancelled).catch(() => {});
  }
  function resume() {
    pausedRef.current = false; setPaused(false); setArrivals('0'); setExpired(false); anchor.current = null;
    readingAnchor.current = null; needsVerify.current = false; epoch.current++; void fetchRef.current('reset');
  }
  fetchRef.current = async (kind = 'reset') => {
    const config = current.current;
    if (!config.viewId || !reading) return;
    const before = kind === 'older' ? pagesRef.current[0]?.older_cursor ?? null : null;
    if (kind === 'older' && !before) return;
    const requestEpoch = epoch.current;
    const next = ++generation.current;
    inFlight.current = true; setBusy(true);
    try {
      const page = await native.viewPage({ view_id: config.viewId, generation: next, query: config.query, sources: config.scope, before, anchor: kind === 'verify' ? readingAnchor.current : pausedRef.current ? anchor.current : null });
      if (next !== generation.current || requestEpoch !== epoch.current) return;
      pageRevision.current = page.revision;
      if (page.anchor_expired) { setExpired(true); setError('Reading anchor or page cursor expired from retention. Jump to live or reload retained logs.'); return; }
      if (kind === 'verify') return;
      setError(''); setExpired(false);
      let nextPages = kind === 'older' ? [page, ...pagesRef.current] : [page];
      while (nextPages.length > 10 || nextPages.reduce((n, p) => n + p.events.length, 0) > 2000 || nextPages.reduce((n, p) => n + bytes(p), 0) > 24 * 1024 * 1024) nextPages.pop();
      const oldCount = pagesRef.current.reduce((n, p) => n + p.events.length, 0);
      const newRows = page.events.length;
      pagesRef.current = nextPages; setPages(nextPages); setRenderedQuery(config.query);
      if (pausedRef.current && !anchor.current) anchor.current = page.events.at(-1)?.id ?? null;
      requestAnimationFrame(() => {
        if (next !== generation.current || !viewport.current) return;
        if (kind === 'older') viewport.current.scrollTop = newRows * HEIGHT;
        else if (!pausedRef.current || oldCount === 0) viewport.current.scrollTop = viewport.current.scrollHeight;
        if (pausedRef.current) rememberReadingAnchor(viewport.current.scrollTop);
      });
    } catch (e) {
      if (next === generation.current && requestEpoch === epoch.current) { setError(String(e)); pageRevision.current = noticeRef.current?.revision ?? ''; }
    } finally { if (next === generation.current) { inFlight.current = false; setBusy(false); } }
  };
  useEffect(() => {
    setViewId(''); setPages([]); pagesRef.current = []; pageRevision.current = ''; noticeRef.current = undefined; setNotice(undefined); setSelection(''); setError('');
    if (!reading) { generation.current++; inFlight.current = false; setBusy(false); setExpired(false); return; }
    let alive = true; let id = ''; let timer: ReturnType<typeof setTimeout>;
    let ack: string | null = null;
    async function poll() {
      try {
        const update = await native.viewNotice(id, ack);
        if (!alive) return;
        ack = update.revision;
        const previous = noticeRef.current;
        noticeRef.current = update;
        if (!previous || previous.revision !== update.revision) setNotice(prev => ({ ...update,
          complete: (prev?.complete ?? true) && update.complete,
          gap_sources: [...new Set([...(prev?.gap_sources ?? []), ...update.gap_sources])].slice(0, 32),
        }));
        if (pausedRef.current) {
          setArrivals((BigInt(update.accepted) - baseline.current).toString());
          if (previous && previous.evicted !== update.evicted) needsVerify.current = true;
          if (needsVerify.current && !inFlight.current && readingAnchor.current) { needsVerify.current = false; void fetchRef.current('verify'); }
        } else if (performance.now() >= debounceUntil.current && pageRevision.current !== update.revision && !inFlight.current) void fetchRef.current();
      } catch (e) { if (alive) setError(String(e)); }
      finally { if (alive) timer = setTimeout(poll, 100); }
    }
    native.openView().then(opened => { id = opened; if (!alive) { void native.closeView(id).catch(() => {}); return; } setViewId(id); void poll(); }).catch(e => { if (alive) setError(String(e)); });
    return () => { alive = false; clearTimeout(timer); generation.current++; if (id) void native.closeView(id).catch(() => {}); };
  }, [session, reading]);
  useEffect(() => {
    epoch.current++; generation.current++; inFlight.current = true; debounceUntil.current = performance.now() + 150;
    setSelection(''); setActiveRow(''); pagesRef.current = []; setPages([]); pageRevision.current = '';
    anchor.current = null; readingAnchor.current = null; needsVerify.current = false;
    const timer = setTimeout(() => { void fetchRef.current(); }, 150);
    return () => clearTimeout(timer);
  }, [effectiveQuery, configKey, viewId]);
  useEffect(() => {
    if (!viewport.current) return;
    const observer = new ResizeObserver(entries => { setVisibleHeight(entries[0].contentRect.height); const node = viewport.current; if (node) setViewportWidth(node.clientWidth); }); observer.observe(viewport.current);
    return () => observer.disconnect();
  }, []);
  const rows = useMemo(() => pages.flatMap(p => p.events), [pages]);
  const start = Math.min(Math.max(0, rows.length - 1), Math.max(0, Math.floor((scroll - LOG_HEADER_HEIGHT) / HEIGHT) - 8));
  const visibleColumns = columns.filter(c => shown.includes(c));
  const end = Math.min(rows.length, start + Math.ceil(visibleHeight / HEIGHT) + 16);
  const longestMessage = pages.reduce((width, page) => Math.max(width, messageWidth(page)), 300);
  const columnWidths: Record<Column, string> = { time: `${Math.min(360, Math.max(148, ...rows.map(row => row.time_basis === 'local_observed_at' ? 240 : row.time.length * 8 + 24)))}px`, server: '130px', source: '160px', level: '80px', message: `minmax(${longestMessage}px, 1fr)`, trace_id: '170px', duration_ms: '105px' };
  const grid = visibleColumns.map(c => columnWidths[c]).join(' ');
  const tableMinWidth = visibleColumns.reduce((width, column) => width + (column === 'message' ? longestMessage : parseInt(columnWidths[column])), 0);
  const horizontalMax = Math.max(0, tableMinWidth - viewportWidth);
  const scrollThumbWidth = `${Math.min(100, viewportWidth / Math.max(tableMinWidth, 1) * 100)}%`;
  const closeInspector = useCallback(() => { setSelection(''); viewport.current?.focus(); }, [viewport]);
  useEffect(() => {
    if (!selection || !active || !inspectorVisible) return;
    const escape = (event: KeyboardEvent) => { if (event.key === 'Escape' && !document.querySelector('dialog[open]')) { event.preventDefault(); closeInspector(); } };
    window.addEventListener('keydown', escape); return () => window.removeEventListener('keydown', escape);
  }, [selection, closeInspector, active, inspectorVisible]);
  const incomplete = scope.some(id => !status?.sources.some(s => s.id === id)) || Boolean(notice && (notice.discarded !== '0' || notice.evicted !== '0' || !notice.complete || notice.gap_sources.length)) || pages.some(p => p.gap_sources.length || !p.complete || p.truncated);
  const unhealthy = status?.sources.filter(source => scope.includes(source.id) && /reconnect|error|fail|needs_action/i.test(source.state)).length ?? 0;
  useEffect(() => {
    const filters = [query ? `Search: ${query}` : '', timeRange.from || timeRange.to ? 'Time range' : '', level ? `Status: ${level.toUpperCase()}` : ''].filter(Boolean).join(' · ') || 'All retained';
    onReadingStatus?.(`${scope.length} source${scope.length === 1 ? '' : 's'} in scope · ${filters} · ${!reading ? 'Ready' : paused ? `Paused · ${arrivals} new` : 'Following'}${incomplete ? ' · Partial log coverage' : ''}${unhealthy ? ` · ${unhealthy} source(s) need attention` : ''}`);
  }, [scope.length, query, timeRange, level, reading, paused, arrivals, incomplete, unhealthy, onReadingStatus]);
  return <section className="live-view" aria-label="Multi-source live logs" style={{ '--log-row-height': `${HEIGHT}px`, '--log-header-height': `${LOG_HEADER_HEIGHT}px` } as CSSProperties}>
    {sourceTabs}
    <div className="viewer-toolbar"><div className="section-heading log-toolbar"><h2>Live logs <span className={`badge ${paused ? 'paused' : 'following'}`}>{!reading ? 'Ready' : paused ? 'Paused' : 'Live'}</span></h2><div className="actions"><Button disabled={!viewId} title={paused ? 'Resume live' : 'Pause viewport · Space'} onClick={() => paused ? resume() : pause()}>{paused ? `Resume · ${arrivals} new` : 'Pause'}</Button>{paused && <Button disabled={!viewId} onClick={resume}>Jump to live</Button>}<Button disabled={!activeRow} aria-expanded={Boolean(selection && inspectorVisible)} aria-controls="event-details-panel" onClick={() => { if (selection && inspectorVisible) closeInspector(); else { pause(); onInspect?.(); setSelection(activeRow); } }}>{selection && inspectorVisible ? 'Hide details' : 'Details'}</Button><IncidentExport sources={scope.length ? scope : status?.sources.map(s => s.id) ?? []} query={effectiveQuery}/></div></div>
    <QueryBar value={query} change={value => { if (value !== query) { epoch.current++; generation.current++; setQuery(value); } }} input={queryInput}/></div>
    <div className="view-context-row" aria-label="Log display and filters">
      <Menu className="view-options" ariaLabel="Choose visible columns" label={`Columns · ${visibleColumns.length}`}><div className="view-option-content"><h3>Visible columns</h3><div className="column-options">{columns.map(column => <label key={column}><input type="checkbox" checked={shown.includes(column)} disabled={shown.length === 1 && shown.includes(column)} onChange={event => setShown(previous => event.target.checked ? [...previous, column] : previous.filter(value => value !== column))}/>{columnLabels[column]}</label>)}</div></div></Menu>
      <Menu className="time-filter-menu" ariaLabel="Filter by time" label={`Time · ${timeRange.from || timeRange.to ? 'Custom' : 'All'}`}><TimeFilter value={timeRange} change={setTimeRange}/></Menu>
      <Menu className="status-filter-menu" ariaLabel="Filter by status" label={`Status · ${level ? level.toUpperCase() : 'All'}`}><div className="status-filter-options">{['', 'trace', 'debug', 'info', 'warn', 'error', 'unknown'].map(value => <Button key={value} aria-pressed={level === value} onClick={() => setLevel(value)}>{value ? value.toUpperCase() : 'All statuses'}</Button>)}</div></Menu>
    </div>
    {error && <p role="alert" className="notice">{error}{error.includes('View expired') && <Button onClick={() => { pausedRef.current = false; setPaused(false); anchor.current = null; setSession(n => n + 1); }}>Reopen viewer</Button>}{expired && <Button onClick={() => { anchor.current = null; readingAnchor.current = null; needsVerify.current = false; setExpired(false); void fetchRef.current(); }}>Reload retained logs</Button>}</p>}
    <ResizableViewer side="right" panel={selection && viewId && inspectorVisible ? <Inspector active={active} sources={scope.length ? scope : status?.sources.map(s => s.id) ?? []} viewId={viewId} eventId={selection} nextGeneration={nextDetail} close={closeInspector} select={setSelection}/> : null}><div className="table-area"><div className="paging"><Button disabled={busy || !pages[0]?.older_cursor || expired} onClick={() => { pause(); void fetchRef.current('older'); }}>Load older page</Button><span role="status">{busy ? 'Reading…' : `${rows.length} cached rows / 2000`}{paused && ` · ${arrivals} new events across sources`}</span></div>
      <div className="log-viewport" ref={viewport} tabIndex={0} role="grid" aria-label="Log events. Space pauses, arrows select, Enter inspects" aria-rowcount={rows.length + 1} aria-colcount={visibleColumns.length} aria-activedescendant={activeRow && rows.slice(start, end).some(r => r.id === activeRow) ? `event-${activeRow}` : undefined}
        onWheel={e => { if (e.deltaY < 0) pause(); }} onScroll={e => { const node = e.currentTarget; setHorizontalScroll(node.scrollLeft); setScroll(node.scrollTop); if (pausedRef.current) rememberReadingAnchor(node.scrollTop); if (!inFlight.current && node.scrollTop + node.clientHeight < node.scrollHeight - HEIGHT * 2) pause(); }}
        onKeyDown={e => {
          if (e.target !== e.currentTarget) return;
          if (e.code === 'Space' && !e.ctrlKey && !e.metaKey && !e.altKey && !e.repeat) { e.preventDefault(); if (pausedRef.current) resume(); else pause(); }
          if (['ArrowUp', 'ArrowDown', 'PageUp', 'PageDown', 'Home'].includes(e.key)) pause();
          if (e.key === 'ArrowUp' || e.key === 'ArrowDown') { e.preventDefault(); const index = rows.findIndex(r => r.id === activeRow); const next = Math.max(0, Math.min(rows.length - 1, index < 0 ? rows.length - 1 : index + (e.key === 'ArrowUp' ? -1 : 1))); const row = rows[next]; if (row) { setActiveRow(row.id); e.currentTarget.scrollTop = Math.max(0, LOG_HEADER_HEIGHT + next * HEIGHT - visibleHeight / 2); } }
          if (e.key === 'Enter' && activeRow) { e.preventDefault(); onInspect?.(); setSelection(activeRow); }
          if (e.key === 'Escape' && selection) { e.preventDefault(); closeInspector(); }
        }}>
        <div role="row" className="log-head" style={{ gridTemplateColumns: grid, minWidth: tableMinWidth }}>{visibleColumns.map(c => <div role="columnheader" key={c}>{columnLabels[c]}</div>)}</div>
        <div className="virtual-rows" style={{ height: rows.length * HEIGHT, minWidth: tableMinWidth }}>
          {rows.slice(start, end).map((row, offset) => <div key={row.id} id={`event-${row.id}`} role="row" aria-label={environmentFor(row) === 'production' ? `Production · ${row.server}` : undefined} aria-rowindex={start + offset + 2} aria-selected={activeRow === row.id} data-level={row.level.toLowerCase()} className={`log-row ${selection === row.id && inspectorVisible ? 'inspected' : ''} ${activeRow === row.id ? 'keyboard-active' : ''} ${environmentFor(row)}`} style={{ top: (start + offset) * HEIGHT, gridTemplateColumns: grid }} onClick={() => { viewport.current?.focus({ preventScroll: true }); pause(); setActiveRow(row.id); if (selection && inspectorVisible) setSelection(row.id); }} onDoubleClick={() => { pause(); onInspect?.(); setActiveRow(row.id); setSelection(row.id); }}>
            {visibleColumns.map(c => <div role="gridcell" key={c} title={c === 'message' ? row.quality : c === 'time' ? row.time_basis : undefined}>
              {c === 'source' ? <span className="source-badge" title={`${rowName(row)} · ${row.source}`}>{rowName(row).split('/').at(-1)}</span> : c === 'server' ? <span aria-label={`${row.server}, ${environmentFor(row)}`}>{row.server}{environmentFor(row) === 'production' && <b className="prod-dot" title="Production" aria-label="Production">●</b>}</span> : c === 'level' ? <span className={`level ${row.level.toLowerCase()}`}>{row.level}</span> : c === 'time' ? <Timestamp value={row.time} basis={row.time_basis}/> : c === 'trace_id' ? row.trace_id ?? '—' : c === 'duration_ms' ? row.duration_ms ?? '—' : <HighlightedMessage text={row.message} needles={needles}/>}
            </div>)}
          </div>)}
        </div>{rows.length === 0 && !busy && <div className="empty log-empty"><h3>{reading ? 'No matching retained logs' : 'Open a log to start'}</h3><p>{reading ? 'Try a different query or source selection.' : 'Expand a host on the left, then double-click a log. Add a path with the + next to its host.'}</p>{!status?.sources.length && onGetStarted && <Button variant="primary" onClick={onGetStarted}>Add connection</Button>}</div>}
      </div><div className="log-horizontal-scroll"><input className="log-horizontal-slider" type="range" min={0} max={horizontalMax} step={1} value={Math.min(horizontalScroll, horizontalMax)} disabled={!horizontalMax} aria-label="Scroll log messages horizontally" style={{ '--scroll-thumb-width': scrollThumbWidth } as CSSProperties} onChange={event => { const offset = Number(event.target.value); setHorizontalScroll(offset); if (viewport.current) viewport.current.scrollLeft = offset; }}/></div>
    </div></ResizableViewer>
  </section>;
}
