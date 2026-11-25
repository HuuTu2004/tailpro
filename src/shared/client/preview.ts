// Development-only UI preview. Data commands never reach the real runtime.
import type { AppStatus, HostProfile, ViewRow, ReadResponse, ExplainProvider, SourceSpec } from '../contracts/generated';

const started = Date.now();
const initialProfiles: HostProfile[] = [
  { id: 'demo-api-host', target: 'api-prod-01', environment: 'production', port: 22, user: 'deploy', identity_agent: null },
  { id: 'demo-stage-host', target: 'staging-01', environment: 'staging', port: 22, user: 'deploy', identity_agent: null },
];
const status: AppStatus = {
  health: { schema_version: 1, policy_revision: '1', status: 'ok', read_only: true },
  connections: initialProfiles.map(host => ({ host, state: 'connected', detail: 'Sample connection · no SSH session' })),
  sources: [
    { id: 'demo-api', host_id: 'demo-api-host', spec: { kind: 'file', path: '/var/log/app/api.log' }, state: 'streaming', detail: 'Sample logs', generation: '1' },
    { id: 'demo-worker', host_id: 'demo-api-host', spec: { kind: 'docker', container_id: 'worker', aliases: ['worker'] }, state: 'streaming', detail: 'Sample logs', generation: '1' },
    { id: 'demo-nginx', host_id: 'demo-stage-host', spec: { kind: 'file', path: '/var/log/nginx/access.log' }, state: 'streaming', detail: 'Sample logs', generation: '1' },
  ],
};
const ids = status.sources.map(source => source.id);
const samples = [
  'GET /api/orders 200 · request completed',
  'Job completed · email.order-confirmed',
  'POST /api/checkout 201 · order created',
  'Cache hit · catalog:featured',
  'GET /health 200 · all checks passed',
  'Payment authorized · order #1042',
  'Connection timeout · upstream inventory-api',
  'Retry scheduled · attempt 2 of 3',
  'GET /api/products 200 · 24 items',
  'POST /api/checkout 201 · trace_id=req-42 · ' + JSON.stringify({ customer: 'demo-customer', region: 'ap-southeast-1', payment: 'authorized', items: [{ product: 'Wireless keyboard', quantity: 1, price: 89 }, { product: 'USB-C hub', quantity: 2, price: 49 }, { product: 'Monitor stand', quantity: 1, price: 129 }], shipping: { status: 'queued', method: 'standard', destination: 'Demo address' }, metadata: { service: 'checkout-api', revision: 'demo', duration_ms: 184, retries: 0 } }),
];
function read<T>(key: string, fallback: T): T {
  try { return JSON.parse(localStorage.getItem('tailpro.preview.' + key) ?? 'null') as T ?? fallback; }
  catch { return fallback; }
}
function write(key: string, value: unknown) { localStorage.setItem('tailpro.preview.' + key, JSON.stringify(value)); }
const count = () => 240 + Math.min(1500, Math.floor((Date.now() - started) / 2000));
function row(index: number): ViewRow {
  const source = ids[index % ids.length];
  const sourceInfo = status.sources.find(item => item.id === source);
  const host = status.connections.find(connection => connection.host.id === sourceInfo?.host_id)?.host ?? initialProfiles[0];
  return {
    id: 'demo-event-' + index, source, server: host.target, environment: host.environment,
    level: index % 13 === 0 ? 'error' : index % 9 === 0 ? 'warn' : 'info',
    message: samples[index % samples.length], time: String(started - 90000 + index * 300),
    time_basis: 'local_observed_at', trace_id: index % 5 === 0 ? 'req-42' : 'req-' + (80 + index % 12),
    duration_ms: index % 13 === 0 ? 1200 : 18 + index % 95, quality: 'Demo data',
  };
}
function matches(event: ViewRow, query: string) {
  return query.split(/\s+AND\s+/i).every(part => {
    part = part.trim();
    if (!part) return true;
    if (/\b(OR|NOT)\b/.test(part)) throw new Error('OR and NOT are not supported.');
    const duration = part.match(/^duration(?:_ms)?\s*(>=|<=|>|<|=)\s*(\d+)(ms|s)?$/);
    if (duration) {
      const value = Number(duration[2]) * (duration[3] === 's' ? 1000 : 1);
      const actual = event.duration_ms ?? 0;
      return duration[1] === '>' ? actual > value : duration[1] === '<' ? actual < value : duration[1] === '>=' ? actual >= value : duration[1] === '<=' ? actual <= value : actual === value;
    }
    const colon = part.indexOf(':');
    if (colon > 0) {
      const key = part.slice(0, colon);
      const value = part.slice(colon + 1).replace(/^"|"$/g, '');
      const field = event[key as keyof ViewRow];
      return key === 'message' ? String(field).toLowerCase().includes(value.toLowerCase()) : String(field).toLowerCase() === value.toLowerCase();
    }
    return event.message.toLowerCase().includes(part.replace(/^"|"$/g, '').toLowerCase());
  });
}

export async function previewInvoke(command: string, args: Record<string, unknown> = {}): Promise<unknown> {
  const request = (args.request ?? {}) as Record<string, unknown>;
  switch (command) {
    case 'status': return structuredClone(status);
    case 'saved_hosts': return read('profiles', initialProfiles);
    case 'save_host_profile': {
      const profile = args.profile as HostProfile;
      write('profiles', [...read<HostProfile[]>('profiles', initialProfiles).filter(host => host.id !== profile.id), profile].slice(-100));
      return;
    }
    case 'test_connection': await new Promise(resolve => setTimeout(resolve, 500)); return;
    case 'cancel_connection_test': case 'cancel_page': case 'cancel_detail': case 'close_view': case 'cancel_discovery': case 'cancel_prepared': case 'store_provider_key': return;
    case 'connect': {
      const profile = args.profile as HostProfile;
      if (!status.connections.some(connection => connection.host.id === profile.id)) status.connections.push({ host: profile, state: 'connected', detail: 'Demo connection' });
      return;
    }
    case 'disconnect': status.connections = status.connections.filter(connection => connection.host.id !== args.hostId); return;
    case 'set_environment': {
      const host = status.connections.find(connection => connection.host.id === args.hostId);
      if (host) host.host.environment = args.environment as HostProfile['environment'];
      return;
    }
    case 'open_view': return 'demo-view-' + crypto.randomUUID();
    case 'view_notice': return { schema_version: 1, revision: String(count()), accepted: String(count()), retained_rows: String(count()), resident_bytes: '1572864', discarded: '0', evicted: '0', complete: true, gap_sources: [] };
    case 'view_page': {
      const last = Math.min(count(), Number(String(request.before ?? request.anchor ?? '').replace('demo-event-', '')) || count());
      const scope = request.sources as string[];
      const events = Array.from({ length: Math.min(last, 1900) }, (_, index) => row(index + 1)).filter(event => (!scope.length || scope.includes(event.source)) && matches(event, String(request.query ?? ''))).slice(-200);
      return { schema_version: 1, generation: request.generation, revision: String(count()), events, older_cursor: events[0]?.id ?? null, complete: true, truncated: false, anchor_expired: false, scanned: last, elapsed_us: '2400' };
    }
    case 'preview_parser': case 'apply_parser': case 'ingestion_diagnostics': case 'inspect_kubernetes': throw new Error('This operation requires the native runtime; demo data is not an ingestion validation.');
    case 'event_detail': {
      const event = row(Number(String(request.event_id).replace('demo-event-', '')) || 1);
      const data = { message: event.message, level: event.level, trace_id: event.trace_id, duration_ms: event.duration_ms };
      return { schema_version: 1, generation: request.generation, event_id: event.id, quality: 'Demo data', raw_hex: null, raw_bytes: '0', evidence_json: '{}', raw: request.include_raw ? JSON.stringify(data, null, 2) : null, nodes: Object.entries(data).map(([key, value], index) => ({ index, key, value: JSON.stringify(value), kind: typeof value === 'number' ? 'number' : 'string', truncated: false })), group_id: null, group_parts: [], group_parts_truncated: false, more: false, json_error: null };
    }
    case 'saved_views': return read('views', [{ name: 'Errors only', query: 'level:error', sources: [] }, { name: 'Slow requests', query: 'duration>200ms', sources: [] }]);
    case 'save_view': write('views', [...read<Record<string, unknown>[]>('views', []).filter(view => view.name !== (args.view as Record<string, unknown>).name), args.view].slice(-32)); return;
    case 'delete_view': write('views', read<Record<string, unknown>[]>('views', []).filter(view => view.name !== args.name)); return;
    case 'pins': return ['/var/log/app/api.log'];
    case 'pin': return;
    case 'import_config': return { aliases: ['demo-api', 'demo-staging'], disclosures: ['Demo aliases only.'] };
    case 'complete': case 'discover': return { complete: true, reasons: [], candidates: [{ id: 'demo-candidate', path: '/var/log/app/api.log', provider: 'var_log', access: 'ready', activity: 'growing', provenance: 'Demo metadata', tags: ['api'], container_id: null, file_type: 'regular', confidence: 'declared', process_hint: null, identity: 'demo', host_id: args.hostId ?? request.host_id, size: '24576', mtime: String(started), observed_at_ms: String(started) }] };
    case 'start_source': {
      const host = status.connections.find(connection => connection.host.id === args.hostId);
      if (host?.state !== 'connected') throw new Error('Connect this host first.');
      const spec = args.spec as SourceSpec;
      const existing = status.sources.find(source => source.host_id === args.hostId && JSON.stringify(source.spec) === JSON.stringify(spec));
      if (existing) return existing.id;
      if (status.sources.length >= 8) throw new Error('Source limit reached (8).');
      const id = 'demo-added-' + crypto.randomUUID();
      status.sources.push({ id, host_id: host.host.id, spec, state: 'streaming', detail: 'Sample logs · no remote file is read', generation: '1' });
      ids.push(id);
      return id;
    }
    case 'stop_source': { const source = status.sources.find(source => source.id === args.sourceId); if (source) source.state = 'stopped'; return; }
    case 'service_mappings': return read('mappings', []);
    case 'save_service_mapping': write('mappings', [args.mapping]); return;
    case 'delete_service_mapping': write('mappings', []); return;
    case 'grants': return [];
    case 'enroll': throw new Error('Demo preview does not grant access to external clients.');
    case 'revoke': return;
    case 'explain_provider': return read<ExplainProvider | null>('provider', null);
    case 'configure_explainer': write('provider', args.provider); return;
    case 'prepare_export': case 'prepare_explanation': return { id: 'demo-preview', digest: 'demo', payload: JSON.stringify({ demo: true, source: 'api.log', message: 'Connection timeout', trace_id: 'req-42' }, null, 2), expires_at_ms: String(Date.now() + 60000), redaction_incomplete: false };
    case 'commit_export': throw new Error('Demo preview: no file is written.');
    case 'explain': return { evidence: ['demo-event-130'], hypothesis: 'Demo hypothesis: the inventory service may have exceeded its timeout. Inspect the request trace to compare retained events.', untrusted: true };
    case 'topology': return { schema_version: 2, nodes: [{ id: 'api', sources: ['demo-api'], observed_errors: 6, provenance: 'Demo configured service' }, { id: 'worker', sources: ['demo-worker'], observed_errors: 2, provenance: 'Demo configured service' }], edges: [{ from: 'api', to: 'worker', kind: 'configured', provenance: 'Demo depends_on' }], complete: true, reasons: [], coverage: 'Demo snapshot · rate unknown', config_digest: 'demo', config_revision: 'demo' };
    case 'topology_revision': return 'demo';
    case 'debug_read': {
      const events = [row(130), row(131), row(132)];
      const result: ReadResponse = { query_scanned: "3", query_matched_lower_bound: "3", query_returned: 3, schema_version: 2, sources: status.sources.map(source => { const host = status.connections.find(host => host.host.id === source.host_id)!.host; return { id: source.id, server: host.target, environment: host.environment, health: source.state }; }), events: events.map(event => ({ evidence_ref: event.id, source: event.source, server: event.server, environment: event.environment, level: event.level, text: event.message, observed_at_ms: event.time, event_time: null, transport_time: null, provenance: "Demo evidence", redaction_incomplete: false, quality: 'Demo evidence' })), time_basis: 'local_observed_at', coverage: 'Demo retained evidence', complete: true, gaps: [], truncated: false, redaction_incomplete: false, next_cursor: null, observation_cursor: null, trace_ref: 'demo-trace', capabilities: [], candidates: [], ambiguous: false, clusters: [], timeline: events.map(event => ({ source: event.source, evidence_ref: event.id, observed_at_ms: event.time, event_time: null, duration_ms: event.duration_ms, semantics: 'Explicit demo duration' })), code_hints: [] };
      return result;
    }
    default: throw new Error('This action is unavailable in demo preview.');
  }
}
