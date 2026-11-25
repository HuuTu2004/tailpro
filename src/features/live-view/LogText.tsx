import type { ReactNode } from 'react';

export function Timestamp({ value, basis }: { value: string; basis: string }) {
  const display = basis === 'local_observed_at'
    ? new Date(Number(value)).toLocaleString([], { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false, fractionalSecondDigits: 3 })
    : value;
  // Preserve the source's spelling, fractional precision and zone. Unknown formats stay intact.
  const match = /^(.*?)(\d{2}:\d{2}:\d{2})([.,]\d+)?(.*)$/.exec(display);
  return <time className="log-time" title={`${display} · ${basis}`}>{match ? <><span className="time-date">{match[1]}</span><span>{match[2]}</span><span className="time-fraction">{match[3]}{match[4]}</span></> : display}</time>;
}

// Display decoration only; Rust owns query validation and matching. Never infer matches
// for IDs/fields or cross-fragment raw hits that are absent from the message preview.
export function messageNeedles(query: string): string[] {
  const tokens: string[] = [];
  let token = '', quoted = false, escaped = false;
  for (const char of query) {
    if (escaped) { token += char; escaped = false; }
    else if (quoted && char === '\\') escaped = true;
    else if (char === '"') { quoted = !quoted; token += char; }
    else if (/\s/.test(char) && !quoted) { if (token) tokens.push(token); token = ''; }
    else token += char;
  }
  if (quoted || escaped) return [];
  if (token) tokens.push(token);
  const needles = tokens.flatMap(value => {
    if (value === 'AND') return [];
    if (value.startsWith('message:') || value.startsWith('message=')) value = value.slice(8);
    else if (!value.startsWith('"') && /[:=<>]/.test(value)) return [];
    if (value.startsWith('"') && value.endsWith('"')) value = value.slice(1, -1);
    return value ? [value] : [];
  });
  return [...new Set(needles)].slice(0, 32);
}

export function HighlightedMessage({ text, needles }: { text: string; needles: string[] }) {
  if (!needles.length) return text;
  const parts: ReactNode[] = [];
  let offset = 0;
  // Bound markup even for a one-character query in a maximum-size preview.
  while (offset < text.length && parts.length < 128) {
    let start = -1, length = 0;
    for (const needle of needles) {
      const at = text.indexOf(needle, offset);
      if (at !== -1 && (start === -1 || at < start || (at === start && needle.length > length))) { start = at; length = needle.length; }
    }
    if (start === -1) break;
    if (start > offset) parts.push(text.slice(offset, start));
    parts.push(<mark key={start}>{text.slice(start, start + length)}</mark>);
    offset = start + length;
  }
  parts.push(text.slice(offset));
  return <>{parts}</>;
}
