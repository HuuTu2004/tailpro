import { useState, type MouseEvent } from 'react';
import { Button } from '../../shared/ui/Controls';

export type TimeRange = { from: string; to: string };

function localDate(value: string): Date | undefined {
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}(:\d{2})?$/.test(value)) return;
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) return;
  const pad = (n: number) => String(n).padStart(2, '0');
  const roundTrip = `${String(date.getFullYear()).padStart(4, '0')}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
  // Reject calendar rollovers and times skipped by a local daylight-saving change.
  if (roundTrip !== (value.length === 16 ? `${value}:00` : value)) return;
  return date;
}

export function timeQuery(range: TimeRange): string[] {
  const clauses: string[] = [];
  if (range.from) {
    const date = localDate(range.from);
    if (!date) throw new Error('Enter a valid start date and time.');
    clauses.push(`time>=${date.toISOString()}`);
  }
  if (range.to) {
    const date = localDate(range.to);
    if (!date) throw new Error('Enter a valid end date and time.');
    if (range.from && localDate(range.from)!.getTime() > date.getTime()) throw new Error('The end time must be at or after the start time.');
    // Inputs have second precision; include the entire selected final second.
    clauses.push(`time<${new Date(date.getTime() + 1000).toISOString()}`);
  }
  return clauses;
}

export function TimeFilter({ value, change }: { value: TimeRange; change: (range: TimeRange) => void }) {
  const [from, setFrom] = useState(value.from);
  const [to, setTo] = useState(value.to);
  const [error, setError] = useState('');
  const zone = Intl.DateTimeFormat().resolvedOptions().timeZone;
  function apply(event: MouseEvent<HTMLButtonElement>) {
    const next = { from, to };
    try { timeQuery(next); }
    catch (reason) { event.stopPropagation(); setError((reason as Error).message); return; }
    change(next);
  }
  return <div className="time-filter-content" onKeyDown={event => {
    if (event.key === 'Enter' && (event.target as HTMLElement).tagName === 'INPUT') {
      event.preventDefault(); event.currentTarget.querySelector<HTMLButtonElement>('[data-apply-time]')?.click();
    }
  }}>
    <h3>Time range</h3>
    <p className="hint">{zone} · Filters retained logs by the Time column.</p>
    <label>From<input type="datetime-local" step="1" aria-label="Time from" value={from} onChange={event => { setFrom(event.target.value); setError(''); }}/></label>
    <label>To · including this second<input type="datetime-local" step="1" aria-label="Time to" value={to} onChange={event => { setTo(event.target.value); setError(''); }}/></label>
    {error && <p className="notice" role="alert">{error}</p>}
    <div className="time-filter-actions"><Button onClick={() => change({ from: '', to: '' })}>Clear</Button><Button variant="primary" data-apply-time onClick={apply}>Apply</Button></div>
  </div>;
}
