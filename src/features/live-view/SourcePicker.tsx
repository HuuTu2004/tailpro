import { useRef, useState, type FormEvent } from 'react';
import type { AppStatus, SourceInfo } from '../../shared/contracts/generated';

type Props = {
  status?: AppStatus;
  scope: string[];
  scopeHost: string | null;
  setScope: (ids: string[]) => void;
  selectHost: (hostId: string) => void;
  addPath: (hostId: string, path: string, combine: boolean) => Promise<void>;
  manageSources: (hostId: string) => void;
};
const sourcePath = (source: SourceInfo) => source.spec.kind === 'file' ? source.spec.path : source.spec.kind === 'journal' ? source.spec.unit : source.spec.kind === 'kubernetes' ? `${source.spec.namespace}/${source.spec.pod}/${source.spec.container}` : source.spec.container_id;

export function SourcePicker({ status, scope, scopeHost, setScope, selectHost, addPath, manageSources }: Props) {
  const [combine, setCombine] = useState(false);
  const [editingHost, setEditingHost] = useState('');
  const [path, setPath] = useState('');
  const [error, setError] = useState('');
  const [pending, setPending] = useState(false);
  const pathInput = useRef<HTMLInputElement>(null);
  const allHosts = !scopeHost && scope.length === 0;
  const hosts = status?.connections ?? [];
  const sources = status?.sources ?? [];

  function chooseSource(id: string) {
    if (!combine || allHosts) { setScope([id]); return; }
    // Keep the final source selected. Only All hosts clears the source filter.
    const selected = scope.filter(sourceId => sources.some(source => source.id === sourceId));
    setScope(selected.includes(id) ? selected.length > 1 ? selected.filter(sourceId => sourceId !== id) : selected : [...selected, id]);
  }
  function openPath(hostId: string) {
    setEditingHost(hostId); setPath(''); setError('');
    requestAnimationFrame(() => pathInput.current?.focus());
  }
  async function submitPath(event: FormEvent) {
    event.preventDefault();
    if (pending) return;
    setPending(true); setError('');
    try { await addPath(editingHost, path.trim(), combine); setEditingHost(''); setPath(''); }
    catch (reason) { setError(String(reason)); }
    finally { setPending(false); }
  }

  return <section className="source-picker" aria-label="Choose log sources by host">
    <div className="source-picker-heading">
      <div className="source-picker-summary"><button className="all-hosts-button" aria-pressed={allHosts} onClick={() => setScope([])}>All hosts</button><span role="status">{scopeHost ? `${hosts.find(host => host.host.id === scopeHost)?.host.target ?? 'Host'} · all logs` : allHosts ? `${sources.length} sources` : `${scope.length} selected`}</span></div>
      <button className="combine-sources-button" aria-pressed={combine} title="Select several sources to view their logs together" onClick={() => setCombine(value => !value)}><span aria-hidden="true">{combine ? '☑' : '☐'}</span>Combine sources</button>
    </div>
    <div className="host-source-rows">
      {hosts.map(connection => {
        const host = connection.host;
        const hostSources = sources.filter(source => source.host_id === host.id);
        const hostAll = scopeHost === host.id;
        const canAdd = connection.state === 'connected' && sources.length < 8;
        return <div className="host-source-group" key={host.id}>
          <div className="host-source-row">
            <div className="source-host-name" title={`${host.target} · ${host.environment} · ${connection.state.replaceAll('_', ' ')}`}><span className={`dot ${connection.state}`}/><strong>{host.target}</strong>{host.environment === 'production' && <span className="prod-dot" aria-label="Production">●</span>}</div>
            <div className="source-tiles" role="group" aria-label={`${host.target} log sources`}>
              <button className="source-tile source-tile-all" aria-pressed={hostAll} disabled={!hostSources.length} onClick={() => selectHost(host.id)} title={`View all logs from ${host.target}, including newly added sources`}>All <span>{hostSources.length}</span></button>
              {hostSources.map(source => {
                const fullPath = sourcePath(source);
                return <button key={source.id} className="source-tile" aria-pressed={scope.includes(source.id)} onClick={() => chooseSource(source.id)} title={`${fullPath}\n${source.spec.kind} · ${source.state.replaceAll('_', ' ')}${source.detail ? '\n' + source.detail : ''}`}><span className={`source-state-dot ${/error|fail|needs_action|reconnect/i.test(source.state) ? 'attention' : source.state === 'streaming' ? 'reading' : ''}`} aria-hidden="true"/><span className="source-tile-name">{fullPath.split('/').filter(Boolean).at(-1) ?? fullPath}</span><small>{source.spec.kind === 'journal' ? 'Journal' : source.spec.kind === 'docker' ? 'Docker' : source.spec.kind === 'kubernetes' ? 'Kubernetes' : 'File'}</small></button>;
              })}
              {!hostSources.length && <span className="source-row-empty">No log sources yet</span>}
            </div>
            <button className="add-log-path" disabled={!canAdd || pending} aria-expanded={editingHost === host.id} title={sources.length >= 8 ? 'Source limit reached (8). Manage sources to stop an unused reader.' : connection.state !== 'connected' ? 'Connect this host to add a log path' : `Add a log file on ${host.target}`} onClick={() => editingHost === host.id ? setEditingHost('') : openPath(host.id)}>+ Add path</button>
          </div>
          {editingHost === host.id && <form className="inline-log-path" onSubmit={submitPath}>
            <label className="inline-log-path-field">Log file on {host.target}<input ref={pathInput} value={path} onChange={event => setPath(event.target.value)} placeholder="/var/log/app/error.log" aria-label={`Absolute log path on ${host.target}`} required disabled={pending} maxLength={4096}/></label>
            <div className="inline-log-path-actions"><button type="button" disabled={pending} onClick={() => manageSources(host.id)}>Discover sources</button><button type="button" disabled={pending} onClick={() => setEditingHost('')}>Cancel</button><button className="primary" disabled={!canAdd || pending || !path.trim().startsWith('/')}>{pending ? 'Starting…' : 'Start & view'}</button></div>
            {error && <p className="notice" role="alert">{error}</p>}
          </form>}
        </div>;
      })}
      {!hosts.length && <p className="source-picker-empty">Connect a host to choose or add logs.</p>}
    </div>
  </section>;
}
