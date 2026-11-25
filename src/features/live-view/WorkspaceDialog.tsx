import { Button } from '../../shared/ui/Controls';
import { useLayoutEffect, useRef, type ReactNode } from 'react';
import { createPortal } from 'react-dom';

type Props = { title: string; close: () => void; children: ReactNode; className?: string; size?: 'small' | 'medium' | 'large' };

export function WorkspaceDialog({ title, close, children, className = '', size = 'medium' }: Props) {
  const dialog = useRef<HTMLDialogElement>(null);
  useLayoutEffect(() => {
    const opener = document.activeElement as HTMLElement | null;
    const node = dialog.current;
    node?.showModal();
    return () => {
      node?.close();
      queueMicrotask(() => { if (opener?.isConnected) opener.focus(); });
    };
  }, []);
  return createPortal(<dialog ref={dialog} className={`workspace-dialog ${className}`} data-size={size} aria-label={title}
    onCancel={event => { event.preventDefault(); close(); }}
    onKeyDown={event => { if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); close(); } }}>
    <div className="dialog-heading"><h2>{title}</h2><Button onClick={close} aria-label={`Close ${title}`}>Close · Esc</Button></div>
    {children}
  </dialog>, document.body);
}
