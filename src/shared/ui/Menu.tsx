import { useEffect, useId, useLayoutEffect, useRef, useState, type ReactNode } from 'react';

export function Menu({ label, children, className = '', ariaLabel }: { label: ReactNode; children: ReactNode; className?: string; ariaLabel?: string }) {
  const root = useRef<HTMLDetailsElement>(null);
  const popup = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const id = useId();
  function close(restore = false) {
    if (root.current) root.current.open = false;
    setOpen(false);
    if (restore) root.current?.querySelector('summary')?.focus();
  }
  useLayoutEffect(() => {
    if (!open) return;
    const place = () => {
      const trigger = root.current?.querySelector('summary')?.getBoundingClientRect();
      const content = popup.current;
      if (!trigger || !content) return;
      const box = content.getBoundingClientRect();
      const below = trigger.bottom + 4;
      content.style.left = `${Math.max(8, Math.min(trigger.left, innerWidth - box.width - 8))}px`;
      content.style.top = `${Math.max(8, Math.min(below + box.height <= innerHeight - 8 ? below : trigger.top - box.height - 4, innerHeight - box.height - 8))}px`;
    };
    place();
    window.addEventListener('resize', place);
    window.addEventListener('scroll', place, true);
    return () => { window.removeEventListener('resize', place); window.removeEventListener('scroll', place, true); };
  }, [open]);
  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => { if (!root.current?.contains(event.target as Node)) close(); };
    const escape = (event: KeyboardEvent) => { if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); close(true); } };
    document.addEventListener('pointerdown', outside);
    document.addEventListener('keydown', escape, true);
    return () => { document.removeEventListener('pointerdown', outside); document.removeEventListener('keydown', escape, true); };
  }, [open]);
  return <details className={`ui-menu ${className}`} ref={root} onToggle={event => setOpen(event.currentTarget.open)} onBlur={event => { if (event.relatedTarget && !event.currentTarget.contains(event.relatedTarget as Node)) close(); }}>
    <summary aria-label={ariaLabel} aria-expanded={open} aria-controls={open ? id : undefined} onKeyDown={event => {
      if (event.key !== 'ArrowDown') return;
      event.preventDefault(); if (root.current) root.current.open = true; setOpen(true);
      requestAnimationFrame(() => popup.current?.querySelector<HTMLButtonElement>('button:not(:disabled), input, select')?.focus());
    }}>{label}</summary>
    {open && <div id={id} className="ui-menu-content" ref={popup} onClick={event => { if ((event.target as HTMLElement).closest('button:not([data-menu-keep])')) close(true); }} onKeyDown={event => {
      if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key) || (event.target as HTMLElement).tagName === 'INPUT') return;
      const buttons = [...event.currentTarget.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')];
      if (!buttons.length) return;
      event.preventDefault(); const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
      buttons[event.key === 'Home' ? 0 : event.key === 'End' ? buttons.length - 1 : (index + (event.key === 'ArrowDown' ? 1 : -1) + buttons.length) % buttons.length]?.focus();
    }}>{children}</div>}
  </details>;
}
