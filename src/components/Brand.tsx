import { IconChat } from './Icons';

export function BrandMark({ size = 34, className }: { size?: number; className?: string }) {
  return (
    <span
      className={`inline-flex shrink-0 items-center justify-center text-white ${className ?? ''}`}
      style={{
        width: size,
        height: size,
        borderRadius: size * 0.32,
        background: 'linear-gradient(140deg, #4f5dff 0%, #7a5cff 50%, #10b3a2 120%)',
        boxShadow: '0 10px 24px -14px rgba(79, 93, 255, 0.85)',
      }}
      aria-hidden="true"
    >
      <IconChat size={size * 0.58} strokeWidth={2} />
    </span>
  );
}
