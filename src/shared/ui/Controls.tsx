import { forwardRef, useId, useState, type ButtonHTMLAttributes, type InputHTMLAttributes, type ReactNode, type SelectHTMLAttributes } from 'react';
import { createPortal } from 'react-dom';

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & { variant?: 'primary' | 'secondary' | 'ghost'; busy?: boolean };
export function Button({ variant = 'secondary', busy = false, className = '', disabled, type = 'button', title, onFocus, onBlur, onPointerEnter, onPointerLeave, onKeyDown, ...props }: ButtonProps) {
  const id = useId();
  const [tooltip, setTooltip] = useState<{ left: number; top: number }>();
  const show = (button: HTMLButtonElement) => {
    if (!title) return;
    const box = button.getBoundingClientRect();
    setTooltip({ left: Math.max(8, Math.min(box.left, innerWidth - Math.min(360, innerWidth - 16) - 8)), top: box.bottom + 64 < innerHeight ? box.bottom + 4 : Math.max(8, box.top - 64) });
  };
  return <><button {...props} type={type} className={`ui-button ${className}`} data-variant={variant} disabled={disabled || busy} aria-busy={busy || undefined}
    aria-describedby={[props['aria-describedby'], tooltip && title ? id : ''].filter(Boolean).join(' ') || undefined}
    onFocus={event => { show(event.currentTarget); onFocus?.(event); }} onBlur={event => { setTooltip(undefined); onBlur?.(event); }}
    onPointerEnter={event => { show(event.currentTarget); onPointerEnter?.(event); }} onPointerLeave={event => { setTooltip(undefined); onPointerLeave?.(event); }}
    onKeyDown={event => { if (event.key === 'Escape') setTooltip(undefined); onKeyDown?.(event); }}/>
    {tooltip && title && createPortal(<span id={id} role="tooltip" className="ui-tooltip" style={tooltip}>{title}</span>, document.body)}</>;
}
type TextFieldProps = InputHTMLAttributes<HTMLInputElement> & { label: string; hint?: ReactNode };
export const TextField = forwardRef<HTMLInputElement, TextFieldProps>(function TextField({ label, hint, className = '', ...props }, ref) {
  return <label className={`ui-field ${className}`}><span>{label}</span><input {...props} ref={ref}/>{hint && <small>{hint}</small>}</label>;
});
export function Select({ className = '', ...props }: SelectHTMLAttributes<HTMLSelectElement>) {
  return <span className={`ui-select ${className}`}><select {...props}/></span>;
}
