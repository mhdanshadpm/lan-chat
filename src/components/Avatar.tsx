import { cls, colorHex, initials } from '../utils';
import { IconGlobe, IconUsers } from './Icons';

interface Props {
  name: string;
  color: number;
  size?: number;
  online?: boolean | null;
  kind?: 'direct' | 'group' | 'everyone';
  className?: string;
}

export function Avatar({ name, color, size = 40, online, kind = 'direct', className }: Props) {
  const iconSize = Math.round(size * 0.44);
  return (
    <span
      className={cls(
        'relative inline-flex shrink-0 select-none items-center justify-center rounded-full font-semibold text-white ring-1 ring-black/5',
        className,
      )}
      style={{
        width: size,
        height: size,
        background: `radial-gradient(125% 125% at 22% 14%, ${colorHex(color)} 0%, ${colorHex(color)} 48%, rgba(0,0,0,0.32) 150%)`,
      }}
      aria-hidden="true"
    >
      {kind === 'everyone' ? (
        <IconGlobe size={iconSize} />
      ) : kind === 'group' ? (
        <IconUsers size={iconSize} />
      ) : (
        <span style={{ fontSize: Math.max(10, size * 0.36) }}>{initials(name)}</span>
      )}
      {online != null && (
        <span
          className="absolute -bottom-0.5 -right-0.5 rounded-full border-2"
          style={{
            width: Math.max(9, size * 0.3),
            height: Math.max(9, size * 0.3),
            background: online ? '#22c55e' : '#9aa4b2',
            borderColor: 'var(--bg-elevated)',
          }}
        />
      )}
    </span>
  );
}
