import { useEffect, useRef, useState, type FormEvent } from 'react';
import { listen } from '@tauri-apps/api/event';
import { native } from '../../shared/client/native';
import type { AddLogPathResult, AppStatus } from '../../shared/contracts/generated';
import { Button, TextField } from '../../shared/ui/Controls';
import './add-log-path.css';

export function AddLogPath({ hostId, status }: { hostId: string; status?: AppStatus }) {
  const [path, setPath] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  const mounted = useRef(false);
  const pending = useRef<(() => void) | null>(null);
  const host = status?.connections.find(item => item.host.id === hostId);
  useEffect(() => {
    mounted.current = true;
    input.current?.focus();
    return () => { mounted.current = false; pending.current?.(); };
  }, []);

  async function close() {
    if (busy) return;
    try { await native.closeLogPathEditor(); }
    catch (reason) { if (mounted.current) setError(String(reason)); }
  }
  async function add(event: FormEvent) {
    event.preventDefault();
    if (busy || pending.current) return;
    const value = path.trim();
    if (!value.startsWith('/') || /[\r\n\0]/.test(value)) { setError('Enter an absolute log file path.'); return; }
    setBusy(true); setError('');
    const requestId = crypto.randomUUID();
    let unlisten: (() => void) | undefined;
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
      let accept!: () => void;
      let reject!: (reason: Error) => void;
      const result = new Promise<void>((resolve, fail) => { accept = resolve; reject = fail; });
      // Keep a rejection handler installed while the event listener is registering.
      void result.catch(() => {});
      pending.current = () => reject(new Error('Add log cancelled.'));
      unlisten = await listen<AddLogPathResult>('log-path-result', event => {
        if (event.payload.request_id !== requestId) return;
        if (event.payload.error) reject(new Error(event.payload.error)); else accept();
      });
      if (!mounted.current) return;
      timer = setTimeout(() => reject(new Error('The main window did not respond. Try again.')), 5000);
      await native.submitLogPath(requestId, value);
      await result;
      if (mounted.current) await native.closeLogPathEditor();
    } catch (reason) { if (mounted.current) setError(String(reason)); }
    finally {
      clearTimeout(timer); unlisten?.(); pending.current = null;
      if (mounted.current) setBusy(false);
    }
  }

  return <main className="add-log-window" aria-label="Add log path" onKeyDown={event => {
    if (event.key === 'Escape') { event.preventDefault(); void close(); }
  }}>
    <form onSubmit={event => void add(event)}>
      <p className="add-log-host">{host?.host.target ?? 'Selected host'}{host?.host.environment === 'production' && <span className="badge production">Production</span>}</p>
      <TextField label="Log file path" ref={input} value={path} disabled={busy} onChange={event => { setPath(event.target.value); setError(''); }} placeholder="/var/log/app.log" maxLength={4096} required autoComplete="off" aria-label="Log file path"/>
      <p className="hint">Double-click the added log in HOSTS to start reading.</p>
      {error && <p role="alert" className="notice">{error}</p>}
      <div className="add-log-actions"><Button disabled={busy} onClick={() => void close()}>Cancel</Button><Button variant="primary" type="submit" busy={busy} disabled={!host || !path.trim().startsWith('/')}>Add</Button></div>
    </form>
  </main>;
}
