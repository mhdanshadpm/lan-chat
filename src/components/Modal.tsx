import { useEffect, type ReactNode } from 'react';
import { IconX } from './Icons';
import { cls } from '../utils';

interface Props {
  title: string;
  subtitle?: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  wide?: boolean;
  bodyClassName?: string;
}

export function Modal({ title, subtitle, onClose, children, footer, wide, bodyClassName }: Props) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onClose]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-end justify-center bg-slate-950/55 p-0 backdrop-blur-sm sm:items-center sm:p-6"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={title}
        className={cls(
          'card flex max-h-[92vh] w-full animate-rise flex-col overflow-hidden rounded-b-none sm:rounded-2xl',
          wide ? 'sm:max-w-2xl' : 'sm:max-w-lg',
        )}
      >
        <header className="flex items-start justify-between gap-4 border-b px-5 py-4 divider">
          <div>
            <h2 className="text-base font-semibold">{title}</h2>
            {subtitle && (
              <p className="mt-0.5 text-xs" style={{ color: 'var(--muted)' }}>
                {subtitle}
              </p>
            )}
          </div>
          <button className="icon-btn" onClick={onClose} aria-label="Close dialog">
            <IconX size={18} />
          </button>
        </header>
        <div className={cls('scroll-thin min-h-0 flex-1 overflow-y-auto px-5 py-4', bodyClassName)}>
          {children}
        </div>
        {footer && (
          <footer className="flex items-center justify-end gap-2 border-t px-5 py-3.5 divider">
            {footer}
          </footer>
        )}
      </div>
    </div>
  );
}
