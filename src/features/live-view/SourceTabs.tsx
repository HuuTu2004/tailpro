import { useState } from 'react';
import type { AppStatus } from '../../shared/contracts/generated';
import { sourceLabel, sourcePath } from '../sources/source-labels';
import { WorkspaceDialog } from './WorkspaceDialog';
import './source-tabs.css';

type Props = {
  status?: AppStatus;
  scope: string[];
  combined: string[];
  closing: Set<string>;
  select: (id: string) => void;
  close: (id: string) => void;
  combine: (ids: string[]) => void;
};
export function SourceTabs({ status, scope, combined, closing, select, close, combine }: Props) {
  const [composer, setComposer] = useState(false);
  const [draft, setDraft] = useState<string[]>([]);
  const [search, setSearch] = useState('');
  const sources = status?.sources ?? [];
  const readableIds = new Set(sources.map(source => source.id));
  const group = combined.filter(id => readableIds.has(id));
  const chosen = draft.filter(id => readableIds.has(id));
  const hostName = (hostId: string) => status?.connections.find(connection => connection.host.id === hostId)?.host.target ?? hostId;
  return <div className="source-tabs-bar">
    <div className="source-editor-tabs" role="tablist" aria-label="Open log readers" onKeyDown={event => {
      const tab = (event.target as HTMLElement).closest('[role=tab]');
      if (!tab || !['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
      event.preventDefault();
      const tabs = [...event.currentTarget.querySelectorAll<HTMLButtonElement>('[role=tab]')];
      const index = tabs.indexOf(tab as HTMLButtonElement);
      const next = event.key === 'Home' ? 0 : event.key === 'End' ? tabs.length - 1 : (index + (event.key === 'ArrowRight' ? 1 : -1) + tabs.length) % tabs.length;
      tabs[next]?.focus(); tabs[next]?.click();
    }}>
      {group.length > 1 && <button role="tab" className={'combined-log-tab' + (scope.length > 1 ? ' active' : '')} aria-selected={scope.length > 1} tabIndex={scope.length > 1 ? 0 : -1} onClick={() => combine(group)}>Combined <span>{group.length}</span></button>}
      {sources.map(source => <div className={'source-editor-tab' + (scope.length === 1 && scope[0] === source.id ? ' active' : '')} key={source.id} role="presentation">
        <button role="tab" aria-selected={scope.length === 1 && scope[0] === source.id} tabIndex={scope.length === 1 && scope[0] === source.id ? 0 : -1} title={`${hostName(source.host_id)} · ${sourcePath(source.spec)} · ${source.state.replaceAll('_', ' ')}`} onClick={() => select(source.id)}><span>{sourceLabel(source.spec)}</span><small>{hostName(source.host_id)}</small></button>
        <button className="source-tab-close" disabled={closing.has(source.id)} title="Close tab and stop reading this log" aria-label={`Stop reading ${sourceLabel(source.spec)} on ${hostName(source.host_id)}`} onClick={() => close(source.id)}>{closing.has(source.id) ? '…' : '×'}</button>
      </div>)}
      {!sources.length && <span className="source-tabs-empty">Double-click a log in HOSTS to open it.</span>}
    </div>
    <button className={'combine-open-logs' + (scope.length > 1 ? ' active' : '')} disabled={sources.length < 2} title={sources.length < 2 ? 'Open at least two logs to combine them' : 'Choose which open logs to read together'} aria-haspopup="dialog" onClick={() => { setDraft(group.length > 1 ? group : scope); setSearch(''); setComposer(true); }}>Combine sources</button>
    {composer && <WorkspaceDialog title="Combine logs" close={() => setComposer(false)} className="combine-log-dialog">
      <p>Choose the open logs you want to view together.</p>
      {sources.length > 5 && <label>Find a log<input autoFocus value={search} onChange={event => setSearch(event.target.value)} placeholder="Host or log name…"/></label>}
      <div className="combine-log-list">{sources.filter(source => `${hostName(source.host_id)} ${sourceLabel(source.spec)} ${sourcePath(source.spec)}`.toLowerCase().includes(search.toLowerCase())).map(source => <label key={source.id}><input type="checkbox" checked={chosen.includes(source.id)} onChange={event => setDraft(previous => event.target.checked ? [...previous, source.id] : previous.filter(id => id !== source.id))}/><span><strong>{sourceLabel(source.spec)}</strong><small>{hostName(source.host_id)} · {sourcePath(source.spec)}</small></span></label>)}</div>
      <div className="combine-log-actions"><span>{chosen.length} selected · select at least 2</span><button className="primary" disabled={chosen.length < 2} onClick={() => { combine(chosen); setComposer(false); }}>View together</button></div>
      <p className="hint">Switching tabs keeps readers running. The × on a log tab stops that reader.</p>
    </WorkspaceDialog>}
  </div>;
}
