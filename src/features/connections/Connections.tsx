import { useEffect, useState } from 'react';
import type { ConfigImport, ConnectionInfo, HostProfile } from '../../shared/contracts/generated';
import { native } from '../../shared/client/native';
type Props = { connections: ConnectionInfo[]; selected: string; select: (id: string) => void; run: (action: () => Promise<unknown>) => void };
export function Connections({ connections, selected, select, run }: Props) {
  const [consent, setConsent] = useState(false);
  const [target, setTarget] = useState('');
  const [port, setPort] = useState('');
  const [user, setUser] = useState('');
  const [agent, setAgent] = useState('');
  const [environment, setEnvironment] = useState<HostProfile['environment']>('development');
  const [config, setConfig] = useState<ConfigImport>();
  const [saved, setSaved] = useState<HostProfile[]>([]);
  useEffect(() => { native.savedHosts().then(setSaved).catch(() => {}); }, []);
  function connect(profile?: HostProfile) {
    run(async () => {
      const host: HostProfile = profile ?? { id: crypto.randomUUID(), target, port: port ? Number(port) : null, user: user || null, identity_agent: agent || null, environment };
      await native.connect(host, consent); select(host.id);
    });
  }
  return <aside className="connections">
    <div className="section-heading"><h2>Connections</h2><span>SSH</span></div>
    <div className="host-list">{connections.map(c => <div key={c.host.id} className={`host ${c.host.environment} ${selected === c.host.id ? 'selected' : ''}`}>
      <button className="host-select" aria-label={`${c.host.target}, ${c.host.environment}`} onClick={() => select(c.host.id)}>
        <span className={`dot ${c.state}`} /><strong>{c.host.target}</strong><small>{c.host.environment === 'production' ? 'PRODUCTION' : c.host.environment}</small>
        <span className="host-state">{c.state.replaceAll('_', ' ')}</span>
      </button>
      <select aria-label={`Environment for ${c.host.target}`} value={c.host.environment} onChange={e => run(() => native.setEnvironment(c.host.id, e.target.value as HostProfile['environment']))}><option value="development">Development</option><option value="staging">Staging</option><option value="production">Production</option></select>
      <button className="text-button" onClick={() => run(() => native.disconnect(c.host.id))}>Disconnect</button>
      {c.detail && <p className="hint">{c.detail}</p>}
    </div>)}</div>
    <h3>Add connection</h3>
    <form onSubmit={e => { e.preventDefault(); connect(); }}>
      <label>SSH alias or hostname<input required value={target} onChange={e => setTarget(e.target.value)} placeholder="app-server" list="ssh-aliases" /></label>
      <datalist id="ssh-aliases">{config?.aliases.map(a => <option key={a} value={a}/>)}</datalist>
      <div className="fields"><label>User<input value={user} onChange={e => setUser(e.target.value)} placeholder="SSH config"/></label><label>Port<input type="number" min="1" max="65535" placeholder="SSH config" value={port} onChange={e => setPort(e.target.value)}/></label></div>
      <label>Environment<select value={environment} onChange={e => setEnvironment(e.target.value as HostProfile['environment'])}><option value="development">Development</option><option value="staging">Staging</option><option value="production">Production</option></select></label>
      <label>IdentityAgent socket <small>optional</small><input value={agent} onChange={e => setAgent(e.target.value)} placeholder="Use configured SSH agent"/></label>
      <label className="check-row"><input type="checkbox" checked={consent} onChange={e => setConsent(e.target.checked)}/> Allow native SSH dynamic config for this session (may run local commands)</label>
      <button className="primary" type="submit">Connect host <span>→</span></button>
    </form>
    <button className="text-button" onClick={() => run(async () => setConfig(await native.importConfig()))}>Import static SSH aliases</button>
    {config?.disclosures.map(d => <p className="hint" key={d}>{d}</p>)}
    {saved.length > 0 && <><h3>Saved profiles</h3>{saved.map(h => <button className="saved" key={h.id} onClick={() => connect(h)}>{h.target} <small>{h.environment}</small></button>)}</>}
    <p className="hint safety">Host keys must already be trusted in OpenSSH. TailPro uses your existing agent and never accepts a new or changed host key automatically.</p>
  </aside>;
}
