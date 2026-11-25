import { useState } from 'react';
import { native } from '../../shared/client/native';
import type { KubernetesCapability, SourceSpec } from '../../shared/contracts/generated';
export function KubernetesSource({ hostId, disabled, start }: { hostId: string; disabled: boolean; start: (spec: SourceSpec, label: string) => void }) {
  const [scope, setScope] = useState({ kubeconfig: '', context: '', namespace: '', pod: '', container: '' });
  const [capability, setCapability] = useState<KubernetesCapability>();
  const [trusted, setTrusted] = useState(false);
  const [previous, setPrevious] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  async function inspect() {
    setBusy(true); setError(''); setCapability(undefined); setTrusted(false);
    try { setCapability(await native.inspectKubernetes(hostId, scope.kubeconfig, scope.context, scope.namespace, scope.pod, scope.container)); }
    catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  }
  return <details><summary>Kubernetes · selected container (runtime validation pending)</summary>
    <p className="hint">Read one named Pod on this host using its installed kubectl. Inspection reads the selected kubeconfig and certificate files, then GETs the selected Pod. Credential plugins, token files and insecure TLS are unsupported.</p>
    <fieldset disabled={disabled || busy}><div className="fields">{(Object.keys(scope) as (keyof typeof scope)[]).map(key => <label key={key}>{key}<input value={scope[key]} maxLength={key === 'kubeconfig' ? 4096 : 253} placeholder={key === 'kubeconfig' ? '/home/user/.kube/config' : key} onChange={event => { setScope({ ...scope, [key]: event.target.value }); setCapability(undefined); setTrusted(false); }}/></label>)}</div>
    <button disabled={Object.values(scope).some(value => !value.trim())} onClick={() => void inspect()}>Inspect selected scope</button>
    {capability && <><p className="hint">Pod UID: {capability.pod_uid} · container: {capability.container_id} · restart: {capability.restart_count}<br/>{capability.coverage}</p><label><input type="checkbox" checked={trusted} onChange={event => setTrusted(event.target.checked)}/>Trust this context and these credentials for reading this container</label><label><input type="checkbox" checked={previous} disabled={!capability.previous_available} onChange={event => setPrevious(event.target.checked)}/>Read previous instance once (up to 200 recent lines)</label><button disabled={!trusted} onClick={() => start({ kind: 'kubernetes', ...scope, ...capability, trusted, previous }, 'kubernetes')}>Start selected container</button></>}
    </fieldset>{error && <p className="notice" role="alert">{error}</p>}
  </details>;
}
