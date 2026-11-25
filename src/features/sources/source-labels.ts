import type { SourceSpec } from '../../shared/contracts/generated';

export type LogSource = { hostId: string; spec: SourceSpec };
export function sourceKey(source: LogSource) {
  const spec = source.spec;
  if (spec.kind === 'file') return JSON.stringify([source.hostId, 'file', spec.path]);
  if (spec.kind === 'journal') return JSON.stringify([source.hostId, 'journal', spec.unit]);
  if (spec.kind === 'docker') return JSON.stringify([source.hostId, 'docker', spec.container_id]);
  return JSON.stringify([source.hostId, 'kubernetes', spec.kubeconfig, spec.context, spec.namespace, spec.pod_uid, spec.container, spec.container_id, spec.restart_count, spec.previous]);
}
export function sourcePath(spec: SourceSpec) {
  return spec.kind === 'file' ? spec.path : spec.kind === 'journal' ? spec.unit : spec.kind === 'docker' ? spec.container_id : `${spec.namespace}/${spec.pod}/${spec.container}`;
}
export function sourceLabel(spec: SourceSpec) {
  if (spec.kind === 'file') return spec.path.split('/').filter(Boolean).at(-1) ?? spec.path;
  if (spec.kind === 'docker') return spec.aliases[0] || spec.container_id;
  if (spec.kind === 'kubernetes') return `${spec.pod} / ${spec.container}`;
  return spec.unit;
}
