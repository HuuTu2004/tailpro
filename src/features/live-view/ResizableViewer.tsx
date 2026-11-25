import { useEffect, useRef, useState, type ReactNode } from 'react';

type Props = { children: ReactNode; panel?: ReactNode; side: 'left' | 'right' };
export function ResizableViewer({ children, panel, side }: Props) {
  const container = useRef<HTMLDivElement>(null);
  const drag = useRef<{ pointerId: number; startX: number; startWidth: number } | null>(null);
  const [width, setWidth] = useState(400);
  const [resizing, setResizing] = useState(false);
  const [limits, setLimits] = useState({ min: 260, max: 800 });
  const hasPanel = Boolean(panel);
  useEffect(() => {
    if (!hasPanel) { drag.current = null; setResizing(false); }
  }, [hasPanel]);

  useEffect(() => {
    const node = container.current;
    if (!node) return;
    const observer = new ResizeObserver(() => {
      const available = Math.max(0, node.clientWidth - 8);
      const min = Math.min(260, Math.floor(available * .45));
      const max = Math.max(min, available - Math.min(340, Math.floor(available * .5)));
      setLimits({ min, max });
    });
    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  const clamp = (next: number) => Math.min(limits.max, Math.max(limits.min, next));
  const visibleWidth = clamp(width);
  return <div ref={container} className={'viewer-body viewer-split' + (panel ? ' has-details details-' + side : '') + (resizing ? ' is-resizing' : '')}>
    <div className="viewer-log-pane">{children}</div>
    {panel && <>
      <div className="viewer-splitter" role="separator" tabIndex={0} aria-label="Resize log and event details" aria-orientation="vertical"
        aria-valuemin={limits.min} aria-valuemax={limits.max} aria-valuenow={Math.round(visibleWidth)}
        aria-valuetext={Math.round(visibleWidth) + ' pixels for event details'}
        title="Drag to resize · Arrow keys to adjust · Double-click to reset"
        onDoubleClick={() => setWidth(400)}
        onKeyDown={event => {
          if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
          event.preventDefault();
          const direction = (event.key === 'ArrowRight' ? 1 : -1) * (side === 'left' ? 1 : -1);
          setWidth(current => event.key === 'Home' ? limits.min : event.key === 'End' ? limits.max : clamp(clamp(current) + direction * (event.shiftKey ? 50 : 10)));
        }}
        onPointerDown={event => {
          if (event.button !== 0) return;
          event.preventDefault(); event.currentTarget.focus(); event.currentTarget.setPointerCapture(event.pointerId);
          drag.current = { pointerId: event.pointerId, startX: event.clientX, startWidth: visibleWidth }; setResizing(true);
        }}
        onPointerMove={event => {
          const started = drag.current;
          if (!started || started.pointerId !== event.pointerId) return;
          const delta = (event.clientX - started.startX) * (side === 'left' ? 1 : -1);
          setWidth(clamp(started.startWidth + delta));
        }}
        onPointerUp={event => {
          if (drag.current?.pointerId !== event.pointerId) return;
          drag.current = null; setResizing(false);
          if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
        }}
        onPointerCancel={() => { drag.current = null; setResizing(false); }}
        onLostPointerCapture={() => { drag.current = null; setResizing(false); }}
      ><span/></div>
      <div className="viewer-detail-pane" style={{ flexBasis: visibleWidth }}>{panel}</div>
    </>}
  </div>;
}
